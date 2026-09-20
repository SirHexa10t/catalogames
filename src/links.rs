//! Turning a set of chosen games into a shell script that opens their store pages.
//!
//! A script rather than opening the pages directly, and that is the point rather than a
//! shortcut. How a page should open is not this program's business to decide — a browser, a
//! particular profile, a terminal browser, nothing at all on a headless box — so what it
//! produces is a file whose **one function** does the opening. Change that function and every
//! link changes with it.

use crate::Listing;
use crate::clock::Timestamp;

/// What the generated script is called when no other name is given.
pub const FILE_NAME: &str = "catalogames-open.sh";

/// First line of every script this writes, so a re-run can tell its own file from someone else's.
///
/// Checked before overwriting anything. A generated file living in a working directory is one
/// `git add -A` away from being committed and one careless run away from replacing something a
/// person wrote; recognising its own output is what lets it refuse the second.
pub const MARKER: &str = "# catalogames: generated link opener";

/// Seconds the script waits between one page and the next.
///
/// Browsers order tabs by when they were asked, not by when each page finishes loading, so
/// asking for several at once races and the tabs come out shuffled — which is the confusion the
/// ordering exists to prevent. Two-fifths of a second settles the order and costs eight seconds
/// over twenty links.
///
/// A starting value, not a rule, which is why it is a variable at the top of the script rather
/// than a number inside the function: a browser already running takes a page over IPC in
/// milliseconds, while a cold start can take seconds and race its own launch. It is the one
/// thing a reader will want to change, so it is the first thing they meet.
const DELAY: &str = "0.4";

/// Seconds before the *first* page, which may have to start the browser.
const FIRST_DELAY: &str = "2";

/// One page to open, and what it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Bundle this came from, written into the script so the grouping survives into the file.
    pub bundle: String,
    /// What the page is, for a human reading the script.
    pub title: String,
    pub url: String,
}

/// Every page the chosen games point at, in the order they were chosen.
///
/// **Not deduplicated.** A game ticked in two bundles is two links and opens twice: it was
/// picked twice, and silently collapsing that would decide something the person choosing already
/// decided. A pack contributes the pages of the games it delivers, because a pack has no page of
/// its own.
#[must_use]
pub fn of_listing(listing: &Listing) -> Vec<Link> {
    let mut links = Vec::new();
    for bundle in &listing.bundles {
        for game in &bundle.games {
            let held: &[crate::Game] = if game.is_pack() {
                &game.contains
            } else {
                std::slice::from_ref(game)
            };
            links.extend(held.iter().map(|game| Link {
                bundle: bundle.title.clone(),
                title: game.title.clone(),
                url: crate::render::page_of(game),
            }));
        }
    }
    links
}

/// The script itself, as text.
///
/// Separate from writing it, so a caller can put it somewhere this crate knows nothing about —
/// a pipe, a clipboard, a different name — without the file handling coming along.
#[must_use]
pub fn script(links: &[Link]) -> String {
    let mut out = String::new();
    out.push_str("#!/bin/sh\n");
    out.push_str(MARKER);
    out.push_str(&format!("\n# Written {}.\n#\n", Timestamp::now()));
    out.push_str(
        "# The pages picked in catalogames, in the order they were picked — so that two tabs\n\
         # side by side are two games side by side in the same bundle.\n\
         #\n\
         # Every page goes through open_link. Change that one function — a different browser, a\n\
         # particular profile, a private window, a line printed instead of a window — and every\n\
         # link below changes with it.\n\
         #\n\
         # A page listed twice was picked twice, in two different bundles. That is deliberate\n\
         # and not a mistake to tidy up: the bundles are alternatives, and seeing the same game\n\
         # in both is the comparison.\n\n",
    );
    out.push_str(&format!(
        "# Seconds between pages. Browsers order tabs by when they were asked, so opening them\n\
         # too fast shuffles them. Raise it if your tabs still come out of order; lower it to\n\
         # zero if you do not care. A fractional value needs a `sleep` that accepts one — use\n\
         # whole seconds if yours does not.\n\
         DELAY={DELAY}\n\
         # Longer for the first, which may be starting the browser.\n\
         FIRST_DELAY={FIRST_DELAY}\n\n"
    ));
    out.push_str(
        "open_link() {\n\
        \x20   xdg-open \"$1\" >/dev/null 2>&1 || printf 'could not open %s\\n' \"$1\" >&2\n\
        \x20   sleep \"$2\"\n\
         }\n",
    );

    let mut current = None;
    let mut current_is_first = true;
    for link in links {
        if current != Some(&link.bundle) {
            out.push_str(&format!("\n# {}\n", comment(&link.bundle)));
            current = Some(&link.bundle);
        }
        out.push_str(&format!(
            "open_link {} \"${}\"  # {}\n",
            quote(&link.url),
            if current_is_first {
                "FIRST_DELAY"
            } else {
                "DELAY"
            },
            comment(&link.title)
        ));
        current_is_first = false;
    }
    out
}

/// A value as one shell word, safe whatever is in it.
///
/// Single quotes, with the one character they cannot contain spliced in the usual way. The URLs
/// this writes are percent-encoded already — a search term by this crate, an app id by its type —
/// so nothing quotable survives into them today. That is the reason to quote properly now: the
/// safety is a property of how the URLs happen to be built, not a promise the stores made, and
/// a vendor field reaching a shell word unquoted is how a product name becomes a command.
///
/// Titles go only into `#` comments, where every metacharacter is inert. That is deliberate and
/// worth keeping: twelve of the product names in one capture carry shell metacharacters, one of
/// them an apostrophe in "Beorn's Lodge Pack". Anything that moves a title out of a comment must
/// send it through here.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// A title as one comment line: no newlines, nothing that ends the line early.
fn comment(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bundle, Game};

    fn game(title: &str, app_id: Option<u32>) -> Game {
        Game {
            title: title.to_owned(),
            machine_name: title.to_lowercase(),
            steam_app_id: app_id,
            contains: Vec::new(),
        }
    }

    fn listing(bundles: Vec<Bundle>) -> Listing {
        Listing {
            bundles,
            problems: Vec::new(),
        }
    }

    fn bundle(title: &str, games: Vec<Game>) -> Bundle {
        Bundle {
            title: title.to_owned(),
            url: "https://example.test".to_owned(),
            price: None,
            ends_at: None,
            games,
        }
    }

    #[test]
    fn a_link_is_produced_for_every_game_in_the_order_it_was_picked() {
        // Order is the whole point: the person picking should not have to wonder which bundle
        // an adjacent tab came from.
        let links = of_listing(&listing(vec![
            bundle("First", vec![game("Alpha", Some(1)), game("Beta", Some(2))]),
            bundle("Second", vec![game("Gamma", Some(3))]),
        ]));
        let order: Vec<(&str, &str)> = links
            .iter()
            .map(|link| (link.bundle.as_str(), link.title.as_str()))
            .collect();
        assert_eq!(
            order,
            [("First", "Alpha"), ("First", "Beta"), ("Second", "Gamma")]
        );
    }

    #[test]
    fn a_game_picked_in_two_bundles_opens_twice() {
        // Picked twice means opened twice. Collapsing them would decide something the person
        // choosing has already decided.
        let links = of_listing(&listing(vec![
            bundle("First", vec![game("Alpha", Some(1))]),
            bundle("Second", vec![game("Alpha", Some(1))]),
        ]));
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].url, links[1].url);
    }

    #[test]
    fn a_pack_opens_the_pages_of_the_games_it_delivers() {
        // A pack has no page of its own, so opening "the pack" means opening what it holds.
        let mut pack = game("A Pack", None);
        pack.contains = vec![game("Inner One", Some(7)), game("Inner Two", Some(8))];
        let links = of_listing(&listing(vec![bundle("First", vec![pack])]));

        let titles: Vec<&str> = links.iter().map(|link| link.title.as_str()).collect();
        assert_eq!(titles, ["Inner One", "Inner Two"]);
        assert!(links.iter().all(|link| link.url.contains("/app/")));
    }

    #[test]
    fn a_game_with_no_app_id_still_gets_something_to_open() {
        let links = of_listing(&listing(vec![bundle("First", vec![game("Unknown", None)])]));
        assert!(links[0].url.contains("/search/"), "{:?}", links[0].url);
    }

    #[test]
    fn every_link_goes_through_the_one_function_that_opens_them() {
        // The reason this is a script rather than a program that opens pages itself: one
        // function to change, and every link changes with it.
        let links = of_listing(&listing(vec![bundle(
            "First",
            vec![game("Alpha", Some(1)), game("Beta", Some(2))],
        )]));
        let text = script(&links);

        assert_eq!(text.matches("open_link ").count(), 2);
        assert_eq!(text.matches("open_link() {").count(), 1);
        assert!(text.starts_with("#!/bin/sh\n"), "{text}");
        assert!(
            text.contains("sleep \"$2\""),
            "the waiting happens in that one function, with how long passed in"
        );
    }

    #[test]
    fn the_bundle_each_link_came_from_is_written_beside_it() {
        let text = script(&of_listing(&listing(vec![
            bundle("First", vec![game("Alpha", Some(1))]),
            bundle("Second", vec![game("Beta", Some(2))]),
        ])));
        let first = text.find("\n# First\n").expect("a section per bundle");
        let alpha = text.find("# Alpha").expect("its game");
        let second = text.find("\n# Second\n").expect("the next section");
        assert!(first < alpha && alpha < second, "{text}");
    }

    #[test]
    fn a_url_is_one_shell_word_whatever_it_contains() {
        assert_eq!(quote("https://x.test/a b"), "'https://x.test/a b'");
        // The one character single quotes cannot hold, spliced the usual way.
        assert_eq!(quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn a_title_cannot_end_its_comment_line_early() {
        // A store's own wording ends up in a comment; a newline in it would turn the rest of
        // the title into a command.
        assert_eq!(comment("Half\nLife"), "Half Life");
        assert_eq!(comment("Half\r\nLife"), "Half  Life");
    }

    #[test]
    fn the_script_names_itself_so_a_rerun_knows_its_own_file() {
        // A generated file in a working directory is one careless run away from replacing
        // something a person wrote. Recognising its own output is what lets it refuse.
        let text = script(&of_listing(&listing(vec![bundle(
            "First",
            vec![game("Alpha", Some(1))],
        )])));
        assert_eq!(text.lines().nth(1), Some(MARKER), "{text}");
    }

    #[test]
    fn the_delay_is_a_variable_at_the_top_rather_than_a_number_inside_the_function() {
        // It is the one thing a reader will want to change — a running browser takes a page in
        // milliseconds, a cold start can race its own launch — so it is the first thing they
        // meet, and the first page waits longer because it may be starting the browser.
        let text = script(&of_listing(&listing(vec![bundle(
            "First",
            vec![game("Alpha", Some(1)), game("Beta", Some(2))],
        )])));
        let delay = text.find("\nDELAY=").expect("a delay variable");
        let function = text.find("open_link() {").expect("the function");
        assert!(delay < function, "the delay should come first: {text}");
        assert!(
            text.contains(r#"open_link 'https://store.steampowered.com/app/1' "$FIRST_DELAY""#),
            "{text}"
        );
        assert!(
            text.contains(r#"open_link 'https://store.steampowered.com/app/2' "$DELAY""#),
            "{text}"
        );
    }

    #[test]
    fn the_script_says_why_a_page_may_appear_twice() {
        // The person who opens the file and sees one URL twice is the one most likely to
        // "fix" it, so the reason lives where they will read it.
        let text = script(&of_listing(&listing(vec![
            bundle("First", vec![game("Alpha", Some(1))]),
            bundle("Second", vec![game("Alpha", Some(1))]),
        ])));
        assert!(text.contains("picked twice"), "{text}");
    }

    #[test]
    fn a_vendor_apostrophe_cannot_end_a_shell_word() {
        // Live product names carry apostrophes — "Beorn's Lodge Pack" is one — and titles are
        // safe only because they go into comments. Anything quoted has to survive one.
        assert_eq!(quote("Beorn's"), r"'Beorn'\''s'");
        assert!(!comment("Beorn's Lodge Pack").contains('\n'));
    }
}
