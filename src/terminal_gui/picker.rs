//! Choosing games out of a listing, in a terminal form.
//!
//! Lives in the binary rather than the library, for the reason clap does: it draws on a terminal
//! and reads keys, which is the command line's business. The library supplies the listing and
//! the lines; what this adds is the asking.
//!
//! The form is [`terminal_choice`], and it is driven the way the author's own programs drive it:
//! build the form, mutate its options for their starting state, run it, then read the answers
//! back off the same items.
//!
//! Written against whatever revision of that crate `Cargo.lock` pins. The dependency tracks a
//! branch rather than a tag, so a future update can move it; the shapes this module depends on
//! are the folding API and the sub-choice API (`Choice::sub`), which is the newer of the two.
//!
//! The form draws on **stderr** and the chosen listing goes to stdout, which is what keeps
//! `sales | less` working. Keys are read from the controlling terminal rather than from stdin,
//! so a piped stdin is no obstacle; having no terminal at all is, and says so.

use catalogames::links::Link;
use catalogames::render::{self, Palette};
use catalogames::steam::items::Classified;
use catalogames::user_games::holdings::Holdings;
use catalogames::{Bundle, Error, Game, Listing, Result};

use super::cli::Vendor;
use terminal_choice::{Choice, Form, Item, Outcome, run};

/// Puts the listing on the terminal and hands back what was ticked.
///
/// `None` when the form was cancelled. An empty selection is `Some` with no games in it: those
/// are different answers, and only one of them means "never mind".
pub fn pick(
    listings: Vec<(Vendor, Listing)>,
    palette: Palette,
    notes: &[String],
    held: &Holdings,
    classified: &Classified,
) -> Result<Option<Picked>> {
    let (mut form, offers) = build(&listings, palette, notes, held, classified);

    match run(&mut form) {
        Ok(Outcome::Submitted) => {}
        Ok(Outcome::Cancelled) => return Ok(None),
        // Not a failure of the listing, which is already in hand: the question simply cannot be
        // asked here. Said as a setup problem so the message tells the reader what to do.
        Err(source) => {
            return Err(Error::Setup {
                detail: format!(
                    "the picker needs a terminal to draw on ({source}).\n\
                     Pass --plain to print the listing instead."
                ),
            });
        }
    }
    Ok(Some(chosen(listings, &form, &offers)))
}

/// What one run of the form produced, keeping each store's games under their own store.
pub struct Picked {
    pub listings: Vec<(Vendor, Listing)>,
    /// Every page the picked games point at, in the order they were offered.
    pub links: Vec<Link>,
}

impl Picked {
    /// Whether anything at all was ticked.
    pub fn is_empty(&self) -> bool {
        self.listings
            .iter()
            .all(|(_, listing)| listing.bundles.is_empty())
    }
}

/// Builds the form: every game a checkbox, every bundle a heading that folds.
///
/// One form over every store, not one form per store. The person choosing is comparing bundles,
/// and bundles from two stores are still bundles; asking twice would make them compare across
/// two separate questions.
///
/// A pack — one product delivering several games — becomes a box with those games as
/// sub-choices under it. Ticking it takes the lot, which is how the pack is sold; ticking some
/// of them opens only those pages, which is the only thing this form actually does with a
/// choice. The crate keeps the two in step (see `Choice::sub`), so nothing here has to.
fn build(
    listings: &[(Vendor, Listing)],
    palette: Palette,
    notes: &[String],
    held: &Holdings,
    classified: &Classified,
) -> (Form, Vec<Option<String>>) {
    // The offers are kept, not just the lines drawn from them: each already carries the page its
    // box opens, and handing that to `chosen` is what stops the opener deriving it a second time.
    // Flat and in form order, which is the order `chosen` walks the games in.
    let mut offered_pages = Vec::new();
    let mut items = Vec::new();
    // Anything the reader should know before choosing goes above everything, as a comment: the
    // crate draws those dim and the cursor never lands on them.
    //
    // One comment per note rather than one carrying them all. A form is as wide as the terminal
    // and the crate clips every line to it, so two facts joined into one sentence lose whichever
    // of them came second.
    for note in notes {
        items.push(Item::Comment(format!("# {note}")));
    }

    let mut offered = 0;
    let mut bundles = 0;
    for (vendor, listing) in listings {
        let mut options = Vec::new();
        for bundle in &listing.bundles {
            // The bundle's own line comes first, as a comment row: its page and, for a
            // pick-and-mix, what each next game costs. Unselectable, and folded away with the
            // bundle, so it is read on opening the section and takes no room while shut. It
            // carries the heading too — that is the slot the fold opens and shuts, and giving
            // it to the note keeps every game's row a plain box. No page is kept for it: it
            // answers nothing, and `chosen` skips it exactly as the cursor does.
            options.push(
                Choice::comment(render::bundle_note(bundle))
                    .heading(render::bundle_heading(bundle, palette)),
            );
            for offer in render::choices(bundle, palette, held, classified) {
                offered_pages.push(offer.url);
                let mut option = Choice::named(offer.line);
                if offer.included {
                    option = option.sub();
                }
                options.push(option);
            }
            bundles += 1;
        }
        offered += options.len();

        // A blank line before each store's heading, so the sections read apart at a glance.
        //
        // The newline is INSIDE the comment rather than an empty comment of its own: the crate
        // draws a comment by walking `lines()`, so an empty one yields no lines at all and
        // draws nothing, while a leading newline yields an empty line and then the text.
        items.push(Item::Comment(format!("\n# {} game bundles", vendor.name())));
        // A group per store rather than one for everything, so each can be introduced by its
        // own comment. The label is empty on purpose: the crate's rule is that an anonymous
        // group draws no heading and sits flush inside the comments around it, which is what a
        // comment above it wants. Folding is unaffected — a fold is keyed by item and slot, so
        // several groups fold as readily as one.
        items.push(Item::Checkboxes {
            label: String::new(),
            options,
        });
    }

    // Says what is on offer and nothing about the keys: the form draws its own legend, and a
    // second copy here would be one more thing to keep in step with the crate.
    let title = format!("{bundles} bundle(s), {offered} thing(s) to buy");
    let mut form = Form::new().title(title).collapsible();
    form.items = items;

    // **After the items, and that ordering is load-bearing.** `all_folded` walks the items that
    // exist when it is called, so called first it folds nothing — silently. The crate's own test
    // pins that with the message "asked before the grid existed: nothing to fold", and a form
    // that opens fully expanded over several hundred games looks exactly like folding not
    // working rather than like a call in the wrong place.
    //
    // Folded to start with because that is the shape the crate names as the reason folding
    // exists: a reader opens the bundles they care about instead of scrolling past the rest.
    (form.all_folded(), offered_pages)
}

/// The listing again, holding only the games that were ticked.
///
/// A filtered listing rather than a list of names, so everything downstream keeps working: the
/// same renderer, the same prices and deadlines, the same link-is-the-last-field contract.
/// Bundles nothing was taken from drop out, because a heading over nothing is noise.
fn chosen(listings: Vec<(Vendor, Listing)>, form: &Form, pages: &[Option<String>]) -> Picked {
    // A box per thing the bundle sells, plus one per game a pack delivers — the shape `build`
    // offers, counted the same way so the assertion below can hold it to it.
    let offered: usize = listings
        .iter()
        .flat_map(|(_, listing)| &listing.bundles)
        .flat_map(|bundle| &bundle.games)
        .map(|game| 1 + game.contains.len())
        .sum();

    // Comment rows are skipped here as the cursor skips them: each occupies a slot in the group
    // and answers nothing, so reading its `checked` would attach every answer after it to the
    // game before. This is the fourth place in this crate where a positional read-back could
    // slip, and the first row type that is index-divergent by definition.
    let answers: Vec<bool> = form
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Checkboxes { options, .. } => Some(options),
            _ => None,
        })
        .flatten()
        .filter(|option| !option.comment)
        .map(|option| option.checked)
        .collect();
    // Answers are paired with games BY POSITION, never by the text on the box: the same game
    // appears in more than one bundle routinely, and two boxes reading the same would be
    // indistinguishable by label. If the counts ever disagree the pairing is meaningless, so
    // nothing is returned rather than the right games under the wrong bundle. The pages `build`
    // kept are in that same order and are held to the same count.
    assert_eq!(
        answers.len(),
        offered,
        "the form came back with a different number of answers than it was given options"
    );
    assert_eq!(
        pages.len(),
        offered,
        "the pages kept when the form was built do not line up with the boxes it offered"
    );

    // One box at a time, carrying both what the user did with it and the page it was offering.
    let mut boxes = answers.into_iter().zip(pages.iter());
    let mut links = Vec::new();
    let mut kept_listings = Vec::new();

    // Written as plain loops rather than nested closures because two things are now produced in
    // step — the filtered listing and the pages to open — and the order of the second is the
    // order of the form, which is exactly what these loops walk.
    for (store, listing) in listings {
        let Listing { bundles, problems } = listing;
        let mut kept_bundles = Vec::new();
        for bundle in &bundles {
            let mut games = Vec::new();
            for game in &bundle.games {
                // Every option is read, a disabled one included: `build` offers one per element
                // here and skips none, so the walk cannot slip out of step. Only the CURSOR skips
                // a locked row, and that is the crate's business rather than this pairing's.
                let (whole, page) = boxes.next().unwrap_or((false, &None));
                if !game.is_pack() {
                    if whole {
                        push(&mut links, bundle, game, page);
                        games.push(game.clone());
                    }
                    continue;
                }
                let mut held = Vec::new();
                for inner in &game.contains {
                    let (picked, page) = boxes.next().unwrap_or((false, &None));
                    if picked {
                        push(&mut links, bundle, inner, page);
                        held.push(inner.clone());
                    }
                }
                // Two numbers for one fact. The box over a pack is read but never consulted: what
                // is ticked UNDER it is the answer, and says more than the box can, since a pack
                // can be part-picked. The form keeps that box equal to "all of these are ticked"
                // all the same, so a disagreement means the propagation has stopped working — and
                // the links would then be neither what the form showed nor what it says.
                debug_assert_eq!(
                    whole,
                    held.len() == game.contains.len(),
                    "{:?}: its own box reads {whole} while {} of the {} games under it are ticked",
                    game.title,
                    held.len(),
                    game.contains.len(),
                );
                // A part-picked pack is still the pack: it is one product, and a game left out of
                // the choosing is a page not worth opening rather than a game not bought. Gone
                // entirely only when none of them was wanted.
                if !held.is_empty() {
                    games.push(Game {
                        contains: held,
                        ..game.clone()
                    });
                }
            }
            if !games.is_empty() {
                kept_bundles.push(Bundle {
                    games,
                    ..bundle.clone()
                });
            }
        }
        kept_listings.push((
            store,
            Listing {
                bundles: kept_bundles,
                // Carried through rather than dropped. A problem is a fact about the fetch, not
                // about the choice, and a filtered listing calling itself complete would make a
                // degraded run look clean to anything reading `is_complete`.
                problems,
            },
        ));
    }

    Picked {
        listings: kept_listings,
        links,
    }
}

/// Records the page one ticked box was offering.
///
/// The page comes from the box, not from the game: it was resolved once when the form was built
/// and is carried here, which is what keeps the link a reader saw and the link that opens the
/// same string rather than two derivations of it. A box offering no page — a pack, which has none
/// of its own — contributes nothing, and its contents speak for themselves directly below it.
fn push(links: &mut Vec<Link>, bundle: &Bundle, game: &Game, page: &Option<String>) {
    if let Some(url) = page {
        links.push(Link {
            bundle: bundle.title.clone(),
            title: game.title.clone(),
            url: url.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(title: &str, app_id: Option<u32>) -> Game {
        Game {
            title: title.to_owned(),
            machine_name: title.to_lowercase().replace(' ', "-"),
            steam_app_id: app_id,
            contains: Vec::new(),
        }
    }

    fn pack(title: &str, held: &[&str]) -> Game {
        Game {
            contains: held.iter().map(|name| game(name, Some(1))).collect(),
            ..game(title, None)
        }
    }

    fn bundle(title: &str, games: Vec<Game>) -> Bundle {
        Bundle {
            title: title.to_owned(),
            url: format!("https://example.test/{title}"),
            price: None,
            ends_at: None,
            games,
        }
    }

    fn offered() -> Vec<(Vendor, Listing)> {
        vec![(Vendor::Humblebundle, listing())]
    }

    /// The one store's filtered listing, which is all these tests offer.
    fn only(picked: &Picked) -> &Listing {
        &picked.listings.first().expect("one store").1
    }

    fn listing() -> Listing {
        Listing {
            bundles: vec![
                bundle("First", vec![game("Alpha", Some(1)), game("Beta", Some(2))]),
                bundle(
                    "Second",
                    vec![
                        game("Gamma", Some(3)),
                        pack("A Pack", &["Inner One", "Inner Two"]),
                    ],
                ),
            ],
            problems: Vec::new(),
        }
    }

    /// The one store's checkbox group, skipping the comments that introduce it.
    fn options(form: &Form) -> &[Choice] {
        form.items
            .iter()
            .find_map(|item| match item {
                Item::Checkboxes { options, .. } => Some(options.as_slice()),
                _ => None,
            })
            .expect("a checkbox group")
    }

    /// Ticks boxes by position, exactly as given.
    ///
    /// Raw on purpose: the form's own parent/child propagation is the dependency's contract and
    /// is tested there, so reproducing it here would be a second implementation to disagree
    /// with. What that means for these tests is that a state has to be written out the way the
    /// form would hand it back — a pack's box ticked together with every box under it — and
    /// `chosen`'s cross-check holds them to it.
    fn tick(form: &mut Form, at: &[usize]) {
        let options = form
            .items
            .iter_mut()
            .find_map(|item| match item {
                Item::Checkboxes { options, .. } => Some(options),
                _ => None,
            })
            .expect("a checkbox group");
        for index in at {
            options[*index].checked = true;
        }
    }

    #[test]
    fn every_bundle_opens_folded() {
        // `all_folded` walks the items that exist when it is called, so calling it before the
        // options are pushed folds nothing — silently, and the form then opens fully expanded
        // over hundreds of games. This pins the ordering rather than the wording.
        let (form, _pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        assert_eq!(form.collapsed.len(), 2, "one fold per bundle: {form:?}");
        assert!(form.collapsible);
    }

    #[test]
    fn each_store_is_introduced_by_a_comment_of_its_own() {
        // A comment is display-only in this crate — drawn dim, never focusable — so it says
        // whose bundles follow without becoming something to tick.
        let (form, _pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        let comments: Vec<&str> = form
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Comment(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(comments, ["\n# HumbleBundle game bundles"]);
    }

    #[test]
    fn a_blank_line_stands_above_each_stores_heading() {
        // The newline has to be INSIDE the comment: the crate draws a comment by walking
        // `lines()`, so an empty comment of its own yields no lines at all and draws nothing.
        let (form, _pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        let heading = form
            .items
            .iter()
            .find_map(|item| match item {
                Item::Comment(text) if text.contains("game bundles") => Some(text),
                _ => None,
            })
            .expect("a store heading");
        assert!(heading.starts_with('\n'), "{heading:?}");
        assert_eq!(heading.lines().count(), 2, "a blank line, then the heading");
    }

    #[test]
    fn a_stores_group_is_anonymous_so_its_comment_is_the_only_heading() {
        // The crate's own rule: an anonymous group draws no heading and sits flush inside the
        // comments around it. A label here would print a second heading under the first.
        let (form, _pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(form.items.iter().all(|item| match item {
            Item::Checkboxes { label, .. } => label.is_empty(),
            _ => true,
        }));
    }

    #[test]
    fn each_note_gets_a_line_of_its_own() {
        // A form is as wide as the terminal and every line is clipped to it, so two facts
        // joined into one sentence would lose whichever came second.
        let (form, _pages) = build(
            &offered(),
            Palette::Plain,
            &[
                "where it came from".to_owned(),
                "what went wrong".to_owned(),
            ],
            &Holdings::none(),
            &Classified::none(),
        );
        assert_eq!(
            form.items.iter().take(2).collect::<Vec<_>>(),
            [
                &Item::Comment("# where it came from".to_owned()),
                &Item::Comment("# what went wrong".to_owned()),
            ]
        );
    }

    #[test]
    fn a_note_is_shown_above_everything_when_there_is_one() {
        // How the form says it is showing something saved rather than something just fetched.
        let (form, _pages) = build(
            &offered(),
            Palette::Plain,
            &["saved earlier".to_owned()],
            &Holdings::none(),
            &Classified::none(),
        );
        assert_eq!(
            form.items.first(),
            Some(&Item::Comment("# saved earlier".to_owned()))
        );
    }

    #[test]
    fn comments_are_not_answers_and_do_not_shift_the_pairing() {
        // The read-back walks checkbox options only. A comment between groups must not be
        // counted, or every answer after it would attach to the wrong game.
        let (mut form, pages) = build(
            &offered(),
            Palette::Plain,
            &["a note".to_owned()],
            &Holdings::none(),
            &Classified::none(),
        );
        tick(&mut form, &[4]);

        let picked = chosen(offered(), &form, &pages);
        let titles: Vec<&str> = only(&picked)
            .bundles
            .iter()
            .flat_map(|bundle| &bundle.games)
            .map(|game| game.title.as_str())
            .collect();
        assert_eq!(titles, ["Gamma"]);
    }

    #[test]
    fn each_bundle_heads_its_own_section() {
        let (form, _pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        let headings: Vec<Option<&str>> = options(&form)
            .iter()
            .map(|option| option.heading.as_deref())
            .collect();
        assert_eq!(
            headings,
            [
                Some("First"),
                None,
                None,
                Some("Second"),
                None,
                None,
                None,
                None
            ],
            "a heading belongs to the note row that opens each bundle"
        );
    }

    #[test]
    fn a_pack_is_a_box_with_the_games_it_delivers_under_it() {
        // A pack is sold as one thing, so it gets a box of its own — but each game it delivers
        // has a page, and picking pages is all this form does, so each gets a box too. The
        // crate ties them together; what this pins is that the nesting was declared at all.
        let (form, _pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        let nested: Vec<bool> = options(&form).iter().map(|option| option.sub).collect();
        assert_eq!(
            nested,
            [false, false, false, false, false, false, true, true],
            "only the two games inside the pack are nested"
        );

        let pack = &options(&form)[5].name;
        assert!(pack.starts_with("A Pack"), "{pack}");
        assert!(
            !pack.contains("http"),
            "a pack has no page of its own to link to: {pack}"
        );
        assert!(
            !pack.contains("Inner One"),
            "the boxes under it name its games; naming them here too would say it twice: {pack}"
        );
    }

    #[test]
    fn a_box_is_offered_for_every_page_worth_opening_and_no_more() {
        // Four things on sale, one of which delivers two games: six boxes — and a note row per
        // bundle beside them, which is a row but not a box.
        let (form, pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        let (notes, boxes): (Vec<&Choice>, Vec<&Choice>) =
            options(&form).iter().partition(|option| option.comment);
        assert_eq!(boxes.len(), 6);
        assert_eq!(notes.len(), 2, "one note per bundle");
        assert_eq!(pages.len(), 6, "a page is kept per box and none for a note");
    }

    #[test]
    fn each_bundle_opens_on_a_note_row_that_names_its_page() {
        // The first thing inside an opened bundle is where it is. Unselectable, and folded
        // with the bundle, so it costs a shut section nothing.
        let (form, _pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        let first = &options(&form)[0];
        assert!(first.comment, "{first:?}");
        assert_eq!(first.name, "# https://example.test/First");
        assert_eq!(
            first.heading.as_deref(),
            Some("First"),
            "and it carries the fold"
        );
    }

    #[test]
    fn a_pick_and_mix_note_says_what_each_next_game_costs() {
        // The Platinum Collection's own page: 3 for $9.99, 5 for $14.99, 7 for $19.95 — read as
        // what the NEXT picks cost, which the page never says in so many words.
        let ladder = catalogames::Ladder::new(vec![
            catalogames::Tier {
                games: 3,
                total: catalogames::Money::new(999, "USD"),
            },
            catalogames::Tier {
                games: 5,
                total: catalogames::Money::new(1499, "USD"),
            },
            catalogames::Tier {
                games: 7,
                total: catalogames::Money::new(1995, "USD"),
            },
        ])
        .expect("a ladder");
        let mut priced = bundle("Platinum", vec![game("Alpha", Some(1))]);
        priced.price = Some(catalogames::Price::PerGame(ladder));
        let listing = Listing {
            bundles: vec![priced],
            problems: Vec::new(),
        };
        let (form, _pages) = build(
            &[(Vendor::Fanatical, listing)],
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        assert_eq!(
            options(&form)[0].name,
            "# https://example.test/Platinum  ;  #ofGames:Cost-Each : 3: $3.33, then 2: $2.50, then 2: $2.48, then any: $2.85"
        );
    }

    #[test]
    fn a_box_after_a_note_row_still_returns_its_own_game() {
        // The precise shape of every positional bug this crate has had: a row that occupies a
        // slot and is not an answer, with a real answer straight after it. Alpha sits behind
        // the first bundle's note; ticking it must return Alpha, not shift onto Beta.
        let (mut form, pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        tick(&mut form, &[1]);
        let picked = chosen(offered(), &form, &pages);
        let titles: Vec<&str> = only(&picked)
            .bundles
            .iter()
            .flat_map(|bundle| &bundle.games)
            .map(|game| game.title.as_str())
            .collect();
        assert_eq!(titles, ["Alpha"]);
    }

    #[test]
    fn answers_are_paired_with_games_by_position_not_by_their_wording() {
        // Two bundles can offer the same game, so two boxes can read the same. Ticking the
        // second bundle's copy must return the second bundle's copy.
        let (mut form, pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        tick(&mut form, &[2, 5, 6, 7]);

        let picked = chosen(offered(), &form, &pages);
        let shape: Vec<(&str, Vec<&str>)> = only(&picked)
            .bundles
            .iter()
            .map(|b| {
                (
                    b.title.as_str(),
                    b.games.iter().map(|g| g.title.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(shape, [("First", vec!["Beta"]), ("Second", vec!["A Pack"])]);
    }

    #[test]
    fn a_bundle_nothing_was_taken_from_drops_out() {
        let (mut form, pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        tick(&mut form, &[4]);

        let picked = chosen(offered(), &form, &pages);
        assert_eq!(
            only(&picked).bundles.len(),
            1,
            "a heading over nothing is noise"
        );
        assert_eq!(only(&picked).bundles[0].title, "Second");
    }

    #[test]
    fn ticking_nothing_gives_an_empty_listing_rather_than_the_whole_one() {
        let (form, pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        let picked = chosen(offered(), &form, &pages);
        assert!(picked.is_empty());
        assert!(picked.links.is_empty(), "nothing ticked opens nothing");
    }

    #[test]
    fn a_pack_that_was_picked_keeps_the_games_it_delivers() {
        // Ticking the pack's own box is what the form does to every box under it, so this is
        // the state it hands back — and the pages that come out are the pack's contents, in
        // the order the form showed them.
        let (mut form, pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        tick(&mut form, &[5, 6, 7]);

        let picked = chosen(offered(), &form, &pages);
        // And the pack contributes the pages of what it delivers, not one for itself.
        let opened: Vec<&str> = picked
            .links
            .iter()
            .map(|link| link.title.as_str())
            .collect();
        assert_eq!(opened, ["Inner One", "Inner Two"]);

        let held: Vec<&str> = only(&picked).bundles[0].games[0]
            .contains
            .iter()
            .map(|g| g.title.as_str())
            .collect();
        assert_eq!(held, ["Inner One", "Inner Two"], "the pack came back whole");
    }

    #[test]
    fn part_picking_a_pack_opens_only_the_pages_that_were_picked() {
        // The boxes under a pack are the answer, and they can say something its own box cannot:
        // one of the two games is already owned, or already played, and its page is not wanted.
        // The pack is still the pack — one product — so it comes back, holding what was picked.
        let (mut form, pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        tick(&mut form, &[6]);

        let picked = chosen(offered(), &form, &pages);
        let opened: Vec<&str> = picked
            .links
            .iter()
            .map(|link| link.title.as_str())
            .collect();
        assert_eq!(opened, ["Inner One"], "one box ticked, one page to open");

        let bundles = &only(&picked).bundles;
        assert_eq!(bundles.len(), 1);
        assert_eq!(bundles[0].games.len(), 1, "the pack, and nothing beside it");
        let held: Vec<&str> = bundles[0].games[0]
            .contains
            .iter()
            .map(|g| g.title.as_str())
            .collect();
        assert_eq!(held, ["Inner One"]);
    }

    #[test]
    fn a_pack_nobody_picked_from_drops_out_even_though_its_games_were_offered() {
        // The count of boxes and the count of games differ now, which is exactly the kind of
        // gap a positional walk falls into: the three boxes this bundle contributes have to be
        // consumed whether or not any of them was ticked, or the next bundle reads them.
        let (mut form, pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        tick(&mut form, &[1]);

        let picked = chosen(offered(), &form, &pages);
        let shape: Vec<(&str, Vec<&str>)> = only(&picked)
            .bundles
            .iter()
            .map(|b| {
                (
                    b.title.as_str(),
                    b.games.iter().map(|g| g.title.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(shape, [("First", vec!["Alpha"])]);
    }

    #[test]
    fn the_page_opened_is_the_page_the_form_offered() {
        // The opener takes the page off the BOX, not off the game a second time. This is the
        // regression that made it worth carrying: a store publishing no app-id — Humble publishes
        // none — has its page resolved through the inventory when the form is built, and an
        // opener that re-derived it from the game alone wrote a search link instead.
        let Some(entry) = catalogames::inventory::steam::all().next() else {
            return;
        };
        let unidentified = Game {
            title: entry.name.to_owned(),
            machine_name: String::new(),
            steam_app_id: None,
            contains: Vec::new(),
        };
        let listings = vec![(
            Vendor::Humblebundle,
            Listing {
                bundles: vec![bundle("Only", vec![unidentified])],
                problems: Vec::new(),
            },
        )];

        let (mut form, pages) = build(
            &listings,
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        tick(&mut form, &[1]);

        let picked = chosen(listings, &form, &pages);
        let opened: Vec<&str> = picked.links.iter().map(|link| link.url.as_str()).collect();
        assert_eq!(
            opened,
            [format!(
                "https://store.steampowered.com/app/{}",
                entry.app_id
            )],
            "the box showed the game's own page, so that is what opens"
        );
    }

    #[test]
    fn a_box_offering_no_page_opens_nothing_of_its_own() {
        // A pack has no page. Its box carries none, so ticking it contributes only the pages of
        // the games under it — which is what `a_pack_that_was_picked_keeps_the_games_it_delivers`
        // checks from the other end.
        let (_form, pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );
        assert_eq!(
            pages.iter().filter(|page| page.is_none()).count(),
            1,
            "exactly the one pack in the fixture: {pages:?}"
        );
    }

    #[test]
    fn what_went_wrong_during_the_fetch_survives_the_choosing() {
        // A problem is a fact about the run, not about the choice. Dropped, a degraded run
        // would report itself complete and the exit code would say so too.
        let mut before = listing();
        before.problems.push(catalogames::Problem {
            bundle: "First".to_owned(),
            kind: catalogames::ProblemKind::UnresolvedItems {
                machine_names: vec!["missing".to_owned()],
            },
        });
        // Built from a listing of the same shape, since a form only needs the games.
        let (form, pages) = build(
            &offered(),
            Palette::Plain,
            &[],
            &Holdings::none(),
            &Classified::none(),
        );

        let picked = chosen(vec![(Vendor::Humblebundle, before)], &form, &pages);
        assert_eq!(only(&picked).problems.len(), 1);
        assert!(!only(&picked).is_complete());
    }
}
