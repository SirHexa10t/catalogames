//! Marking a listing with what the reader already owns or already wants.
//!
//! The failure most of these exist to protect against is the quiet one: a directory with no
//! snapshots in it and a directory misspelled both produce a listing with no marks on it, and a
//! listing with no marks on it looks perfectly healthy. So the summary line is tested as
//! carefully as the matching.

use std::path::{Path, PathBuf};

use catalogames::render::{self, Palette};
use catalogames::user_games::holdings::Holdings;
use catalogames::{Bundle, Game, Listing};

/// A directory of this test's own, removed when the guard drops.
///
/// Named per test, like `tests/gamelib.rs` does and for the same reason: the suite's default
/// parallelism must not have two tests writing the same snapshot file.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "catalogames-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a writable temporary directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Writes a file into it, verbatim, so a malformed one can be written too.
    fn write(&self, name: &str, body: &str) -> &Self {
        std::fs::write(self.0.join(name), body).expect("a writable file");
        self
    }

    /// A Steam snapshot of the given app-ids.
    fn steam(&self, kind: &str, app_ids: &[u32]) -> &Self {
        let games: Vec<String> = app_ids
            .iter()
            .map(|id| format!(r#"{{"id":"{id}","app_id":{id}}}"#))
            .collect();
        self.write(
            &format!("game_{}_steam", part(kind)),
            &snapshot("steam", kind, &games.join(",")),
        )
    }

    /// An Epic snapshot, which carries titles and no Steam app-ids at all.
    fn epic(&self, titles: &[&str]) -> &Self {
        let games: Vec<String> = titles
            .iter()
            .map(|title| format!(r#"{{"id":"Slug{title}","name":"{title}"}}"#))
            .collect();
        self.write(
            "game_lib_epic",
            &snapshot("epic", "library", &games.join(",")),
        )
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn part(kind: &str) -> &str {
    match kind {
        "library" => "lib",
        other => other,
    }
}

fn snapshot(store: &str, kind: &str, games: &str) -> String {
    format!(
        r#"{{"store":"{store}","kind":"{kind}","captured":"2026-09-18 01:27:20Z",
            "captured_unix":1789617600,
            "provenance":{{"source_url":"https://example.test","account":"SECRET-HANDLE"}},
            "games":[{games}]}}"#
    )
}

fn game(title: &str, app_id: Option<u32>) -> Game {
    Game {
        title: title.to_owned(),
        machine_name: title.to_lowercase().replace(' ', "-"),
        steam_app_id: app_id,
        contains: Vec::new(),
    }
}

/// One bundle's listing, rendered plain, as the marks reach a reader.
fn shown(games: Vec<Game>, held: &Holdings) -> String {
    let bundle = Bundle {
        title: "A Bundle".to_owned(),
        url: "https://example.test/b".to_owned(),
        price: None,
        ends_at: None,
        games,
    };
    render::listing(
        &Listing {
            bundles: vec![bundle],
            problems: Vec::new(),
        },
        Palette::Plain,
        held,
    )
}

mod what_the_files_say {
    use super::*;

    #[test]
    fn a_game_the_library_names_is_marked_owned() {
        let scratch = Scratch::new("owned");
        scratch.steam("library", &[620]);
        let held = Holdings::load(scratch.path());

        let shown = shown(vec![game("Portal 2", Some(620))], &held);
        assert!(shown.contains("[owned on Steam]"), "{shown}");
        assert!(!shown.contains("wishlisted"), "{shown}");
    }

    #[test]
    fn a_game_the_wishlist_names_is_marked_wishlisted() {
        let scratch = Scratch::new("wanted");
        scratch.steam("wishlist", &[2_369_390]);
        let held = Holdings::load(scratch.path());

        let shown = shown(vec![game("Something Wanted", Some(2_369_390))], &held);
        assert!(shown.contains("[wishlisted on Steam]"), "{shown}");
        assert!(!shown.contains("owned"), "{shown}");
    }

    #[test]
    fn a_game_in_neither_file_carries_no_bracket_at_all() {
        let scratch = Scratch::new("neither");
        scratch.steam("library", &[620]);
        let held = Holdings::load(scratch.path());

        let shown = shown(vec![game("Not Yours", Some(999_999))], &held);
        assert!(!shown.contains('['), "{shown}");
    }

    #[test]
    fn owning_it_on_two_stores_stacks_into_one_bracket() {
        // Which is the whole reason the certainty marker sits on the STORE: this bracket carries
        // an app-id identity and a title guess at the same time.
        let scratch = Scratch::new("stacked");
        scratch.steam("library", &[620]).epic(&["Portal 2"]);
        let held = Holdings::load(scratch.path());

        let shown = shown(vec![game("Portal 2", Some(620))], &held);
        assert!(shown.contains("[owned on Steam, Epic?]"), "{shown}");
    }

    #[test]
    fn owning_it_on_one_store_and_wanting_it_on_another_reads_as_two_facts() {
        let scratch = Scratch::new("both-standings");
        scratch.steam("wishlist", &[620]).epic(&["Portal 2"]);
        let held = Holdings::load(scratch.path());

        let shown = shown(vec![game("Portal 2", Some(620))], &held);
        let owned = shown.find("[owned on Epic?]").expect("an owned bracket");
        let wanted = shown
            .find("[wishlisted on Steam]")
            .expect("a wishlisted bracket");
        assert!(owned < wanted, "having it settles what wanting it asks");
    }

    #[test]
    fn a_store_that_publishes_no_app_id_is_matched_by_title_and_marked_as_a_guess() {
        // Epic records its own `app_name` and a title, never a Steam id, so title equality is
        // the only rule available — and the mark has to say so.
        let scratch = Scratch::new("guessed");
        scratch.epic(&["Portal 2"]);
        let held = Holdings::load(scratch.path());

        let shown = shown(vec![game("Portal 2", Some(620))], &held);
        assert!(shown.contains("[owned on Epic?]"), "{shown}");
    }

    #[test]
    fn a_title_match_never_overrides_the_certainty_of_an_app_id_match() {
        // One store answering both ways is one claim, and the identity is the one to report.
        let scratch = Scratch::new("identity-wins");
        scratch.steam("library", &[620]);
        let held = Holdings::load(scratch.path());

        let shown = shown(vec![game("Portal 2", Some(620))], &held);
        assert!(shown.contains("[owned on Steam]"), "{shown}");
        assert!(
            !shown.contains("Steam?"),
            "an identity is not a guess: {shown}"
        );
    }

    #[test]
    fn punctuation_and_case_do_not_stop_a_title_matching() {
        // The same normalisation the rest of the crate compares titles under: what a vendor
        // prints and what a store records differ in exactly these ways and mean one game.
        let scratch = Scratch::new("normalised");
        scratch.epic(&["HELLO, WORLD!"]);
        let held = Holdings::load(scratch.path());

        let shown = shown(vec![game("hello world", None)], &held);
        assert!(shown.contains("[owned on Epic?]"), "{shown}");
    }
}

mod where_it_looked {
    use super::*;

    #[test]
    fn the_summary_names_the_directory_and_every_file_it_found() {
        let scratch = Scratch::new("summary");
        scratch.steam("library", &[620, 400]);
        let held = Holdings::load(scratch.path());

        let summary = held.summary();
        assert!(
            summary.contains(&scratch.path().display().to_string()),
            "{summary}"
        );
        assert!(
            summary.contains("Steam library (2 games, 2026-09-18)"),
            "{summary}"
        );
    }

    #[test]
    fn the_summary_names_the_files_it_did_not_find() {
        // The difference between "you own none of these" and "that directory is wrong", which
        // a listing with no marks on it cannot otherwise show.
        let scratch = Scratch::new("missing");
        scratch.steam("library", &[620]);
        let summary = Holdings::load(scratch.path()).summary();

        assert!(summary.contains("no Steam wishlist"), "{summary}");
        assert!(summary.contains("no Epic library"), "{summary}");
    }

    #[test]
    fn a_file_no_store_could_publish_is_never_named_as_missing() {
        // Epic publishes no wishlist that the tool reading it can see, so `gamelib epic` writes
        // only a library. Listing `game_wishlist_epic` among the missing files would send a
        // reader looking for a fault there is no way to fix.
        let scratch = Scratch::new("unpublishable");
        scratch.steam("library", &[620]);
        let summary = Holdings::load(scratch.path()).summary();

        assert!(summary.contains("no Epic library"), "{summary}");
        assert!(!summary.contains("Epic wishlist"), "{summary}");
    }

    #[test]
    fn an_empty_directory_says_so_and_says_what_writes_the_files() {
        let scratch = Scratch::new("empty");
        let held = Holdings::load(scratch.path());

        let summary = held.summary();
        assert!(
            summary.contains("no library or wishlist files"),
            "{summary}"
        );
        assert!(summary.contains("gamelib"), "{summary}");
        assert!(summary.contains("--account-files"), "{summary}");
        assert!(held.problems.is_empty(), "{:?}", held.problems);
    }

    #[test]
    fn a_directory_that_does_not_exist_is_not_a_crash() {
        let held = Holdings::load(Path::new("/nonexistent/catalogames-test"));
        assert!(held.problems.is_empty(), "a missing file is not a problem");
        assert!(held.summary().contains("no library or wishlist files"));
    }
}

mod a_file_that_cannot_be_used {
    use super::*;

    #[test]
    fn a_missing_file_is_an_ordinary_setup_rather_than_a_problem() {
        // Snapshotting one store and not another is normal, and must not be reported as a fault.
        let scratch = Scratch::new("partial");
        scratch.steam("library", &[620]);
        assert!(Holdings::load(scratch.path()).problems.is_empty());
    }

    #[test]
    fn a_snapshot_of_the_wrong_store_is_refused_and_named() {
        // How a hand-moved or renamed file arrives. Read anyway, it would mark the wrong store
        // on every row it matched.
        let scratch = Scratch::new("wrong-store");
        scratch.write("game_lib_steam", &snapshot("epic", "library", ""));
        let held = Holdings::load(scratch.path());

        assert_eq!(held.problems.len(), 1, "{:?}", held.problems);
        let said = &held.problems[0];
        assert!(said.contains("game_lib_steam"), "{said}");
        assert!(said.contains("epic"), "{said}");
        assert!(said.contains("not being read"), "{said}");
    }

    #[test]
    fn a_snapshot_of_the_wrong_kind_is_refused_too() {
        let scratch = Scratch::new("wrong-kind");
        scratch.write("game_lib_steam", &snapshot("steam", "wishlist", ""));
        let held = Holdings::load(scratch.path());
        assert_eq!(held.problems.len(), 1, "{:?}", held.problems);
    }

    #[test]
    fn a_file_that_is_not_json_is_reported_rather_than_ignored() {
        let scratch = Scratch::new("garbage");
        scratch.write("game_lib_steam", "{ not json");
        let held = Holdings::load(scratch.path());

        assert_eq!(held.problems.len(), 1, "{:?}", held.problems);
        assert!(held.problems[0].contains("game_lib_steam"));
    }

    #[test]
    fn one_unusable_file_does_not_stop_the_others_being_read() {
        let scratch = Scratch::new("one-bad");
        scratch.write("game_lib_steam", "{ not json");
        scratch.steam("wishlist", &[620]);
        let held = Holdings::load(scratch.path());

        assert_eq!(held.problems.len(), 1);
        let shown = shown(vec![game("Portal 2", Some(620))], &held);
        assert!(shown.contains("[wishlisted on Steam]"), "{shown}");
    }
}

mod what_never_reaches_a_reader {
    use super::*;

    #[test]
    fn the_account_a_snapshot_names_cannot_appear_anywhere() {
        // `provenance` is not declared on the type that reads these files, so there is nothing
        // holding the handle to print. Checked through every string this module produces.
        let scratch = Scratch::new("account");
        scratch.steam("library", &[620]);
        let held = Holdings::load(scratch.path());

        let everything = format!(
            "{} {:?} {} {:?}",
            held.summary(),
            held.problems,
            shown(vec![game("Portal 2", Some(620))], &held),
            held.of(&game("Portal 2", Some(620)))
        );
        assert!(!everything.contains("SECRET-HANDLE"), "{everything}");
    }
}

mod without_any_files {
    use super::*;

    #[test]
    fn holdings_that_say_nothing_mark_nothing() {
        let shown = shown(vec![game("Portal 2", Some(620))], &Holdings::none());
        assert!(!shown.contains("owned"), "{shown}");
        assert!(!shown.contains("wishlisted"), "{shown}");
    }
}

mod how_a_marked_line_looks {
    use super::*;

    /// The 24-bit sequence `paint` emits for a hex colour, so a test names a colour rather than
    /// an escape sequence and a changed shade fails one readable assertion.
    fn ansi(hex: &str) -> String {
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).expect("two hex digits");
        format!("\x1b[38;2;{};{};{}m", channel(1), channel(3), channel(5))
    }

    /// Owned turns the line red — the same red an urgent deadline uses, so there is one red.
    const RED: &str = "#F53030";
    /// Wanted turns it green.
    const GREEN: &str = "#4CA64C";
    /// The tag column is grey, quieter than the line it annotates.
    const GREY: &str = "#808080";

    fn coloured(games: Vec<Game>, held: &Holdings) -> String {
        let bundle = Bundle {
            title: "A Bundle".to_owned(),
            url: "https://example.test/b".to_owned(),
            price: None,
            ends_at: None,
            games,
        };
        render::listing(
            &Listing {
                bundles: vec![bundle],
                problems: Vec::new(),
            },
            Palette::Ansi,
            held,
        )
    }

    #[test]
    fn an_owned_game_turns_the_whole_line_red_and_a_wanted_one_green() {
        // The colour is on the line, not on a badge: there is no reason to buy an owned game,
        // and noticing that should not require reading anything. The bullet keeps out of it —
        // it marks where a row sits in the list, which is not a fact about the game.
        let scratch = Scratch::new("line-colour");
        scratch.steam("library", &[620]).steam("wishlist", &[400]);
        let held = Holdings::load(scratch.path());

        let shown = coloured(
            vec![game("Portal 2", Some(620)), game("Portal", Some(400))],
            &held,
        );
        assert!(
            shown.contains(&format!("{}Portal 2", ansi(RED))),
            "{shown:?}"
        );
        assert!(
            shown.contains(&format!("{}Portal", ansi(GREEN))),
            "{shown:?}"
        );
    }

    #[test]
    fn the_link_is_coloured_with_the_line_it_belongs_to() {
        let scratch = Scratch::new("link-colour");
        scratch.steam("library", &[620]);
        let held = Holdings::load(scratch.path());

        let shown = coloured(vec![game("Portal 2", Some(620))], &held);
        assert!(
            shown.contains(&format!(
                "{}https://store.steampowered.com/app/620",
                ansi(RED)
            )),
            "{shown:?}"
        );
    }

    #[test]
    fn the_tags_are_grey_rather_than_the_colour_of_the_line() {
        let scratch = Scratch::new("tag-colour");
        scratch.steam("library", &[620]);
        let held = Holdings::load(scratch.path());

        let shown = coloured(vec![game("Portal 2", Some(620))], &held);
        assert!(
            shown.contains(&format!("{}[owned on Steam]", ansi(GREY))),
            "{shown:?}"
        );
    }

    #[test]
    fn owning_it_beats_wanting_it() {
        // A game on both a library and a wishlist is one you HAVE. Red says so, and green would
        // say the opposite of what the reader needs to know.
        let scratch = Scratch::new("red-beats-green");
        scratch.steam("library", &[620]).steam("wishlist", &[620]);
        let held = Holdings::load(scratch.path());

        let tinted = coloured(vec![game("Portal 2", Some(620))], &held);
        assert!(
            tinted.contains(&format!("{}Portal 2", ansi(RED))),
            "{tinted:?}"
        );
        assert!(
            !tinted.contains(&format!("{}Portal 2", ansi(GREEN))),
            "{tinted:?}"
        );
        // Both facts are still stated, in the column that carries the detail.
        let plain = shown(vec![game("Portal 2", Some(620))], &held);
        assert!(
            plain.contains("[owned on Steam] [wishlisted on Steam]"),
            "{plain}"
        );
    }

    #[test]
    fn a_plain_palette_carries_the_words_and_none_of_the_colour() {
        let scratch = Scratch::new("plain-colours");
        scratch.steam("library", &[620]);
        let held = Holdings::load(scratch.path());

        let shown = shown(vec![game("Portal 2", Some(620))], &held);
        assert!(shown.contains("[owned on Steam]"), "{shown}");
        assert!(!shown.contains('\x1b'), "{shown:?}");
    }
}

mod where_the_tags_sit {
    use super::*;

    #[test]
    fn the_tags_come_after_the_link_in_a_column_of_their_own() {
        let scratch = Scratch::new("column");
        scratch.steam("library", &[620]);
        let held = Holdings::load(scratch.path());

        let shown = shown(vec![game("Portal 2", Some(620))], &held);
        let row = shown
            .lines()
            .find(|line| line.contains("Portal 2"))
            .expect("the game's row");
        let link = row.find("https://").expect("a link");
        let tag = row.find("[owned").expect("a tag");
        assert!(link < tag, "{row:?}");
        // A column, not an afterthought: the table separates cells by two or more spaces.
        assert!(row[link..tag].ends_with("  "), "{row:?}");
    }

    #[test]
    fn an_unmarked_line_still_ends_at_its_link() {
        // The column is dropped rather than left blank when nothing is owned, so a listing with
        // no marks is byte-identical to one rendered before the column existed.
        let held = Holdings::none();
        let shown = shown(vec![game("Portal 2", Some(620))], &held);
        let row = shown
            .lines()
            .find(|line| line.contains("Portal 2"))
            .expect("the game's row");
        assert!(row.trim_end().ends_with("/app/620"), "{row:?}");
    }

    #[test]
    fn every_link_is_still_reachable_by_matching_the_field_that_starts_with_https() {
        // The supported parse. It used to be "the last field of the line"; a marked line now
        // carries its tags after the link, so the documented rule is matching the field rather
        // than counting to it. Pinned here because the README hands this command to readers.
        let scratch = Scratch::new("parse");
        scratch.steam("library", &[620]);
        let held = Holdings::load(scratch.path());

        let shown = shown(
            vec![game("Portal 2", Some(620)), game("Unowned", Some(400))],
            &held,
        );
        let links: Vec<&str> = shown
            .split_whitespace()
            .filter(|field| field.starts_with("https://"))
            .collect();
        assert_eq!(
            links,
            [
                "https://store.steampowered.com/app/620",
                "https://store.steampowered.com/app/400"
            ]
        );
    }
}
