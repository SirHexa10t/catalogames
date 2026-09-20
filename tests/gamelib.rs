//! Snapshotting an account's own libraries, from captured and hand-written responses.
//!
//! The rule most of these exist to protect: a store that declines to answer must never be
//! recorded as an account that owns nothing. Every other failure here is recoverable by running
//! the command again; that one overwrites the last good snapshot with an empty file.

use std::path::{Path, PathBuf};

use catalogames::Error;
use catalogames::commands::gamelib::{Kind, Snapshot, epic, steam};

const OWNED: &str = include_str!("fixtures/gamelib/steam-owned-games.json");
const WISHLIST: &str = include_str!("fixtures/gamelib/steam-wishlist.json");
const WITHHELD: &str = include_str!("fixtures/gamelib/steam-withheld.json");
const LEGENDARY: &str = include_str!("fixtures/gamelib/legendary-list.json");

const ACCOUNT: &str = "76561197960287930";

/// A directory of this test's own, removed when the guard drops.
///
/// Each test names its own, so the suite's default parallelism cannot have two tests writing
/// the same snapshot file and reading each other's results.
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
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn written(snapshot: &Snapshot, scratch: &Scratch) -> serde_json::Value {
    let path = snapshot.write(scratch.path()).expect("writes");
    let text = std::fs::read_to_string(path).expect("reads back");
    serde_json::from_str(&text).expect("the file is JSON")
}

mod a_store_that_declines_to_answer {
    use super::*;

    /// The response Steam actually sends for a profile whose games are not public.
    ///
    /// Captured live: it is a 200 with `{"response":{}}` and no error of any kind, which is why
    /// nothing downstream can tell it from a real answer unless this is checked here.
    #[test]
    fn a_withheld_library_is_refused_rather_than_recorded_as_empty() {
        let failure = steam::parse_library(WITHHELD, ACCOUNT).unwrap_err();
        assert!(matches!(failure, Error::Setup { .. }), "{failure}");
        let message = failure.to_string();
        assert!(message.contains("no owned games"), "{message}");
        assert!(message.contains("privacy setting"), "{message}");
    }

    #[test]
    fn a_withheld_wishlist_is_refused_rather_than_recorded_as_empty() {
        let failure = steam::parse_wishlist(WITHHELD, ACCOUNT).unwrap_err();
        assert!(matches!(failure, Error::Setup { .. }), "{failure}");
        assert!(failure.to_string().contains("no wishlist"), "{failure}");
    }

    #[test]
    fn the_message_says_the_privacy_change_can_be_undone() {
        // Asking someone to make their library public is only reasonable if they are also told
        // they can put it back, and that the snapshot will not need it again.
        let message = steam::parse_library(WITHHELD, ACCOUNT)
            .unwrap_err()
            .to_string();
        assert!(message.contains("set it back afterwards"), "{message}");
    }

    #[test]
    fn a_genuinely_empty_wishlist_is_accepted_because_the_key_is_present() {
        // The distinction the two tests above depend on: absent `items` is a refusal,
        // `items: []` is an answer.
        let snapshot = steam::parse_wishlist(r#"{"response":{"items":[]}}"#, ACCOUNT)
            .expect("an empty list is an answer");
        assert!(snapshot.games.is_empty());
    }
}

mod reading_a_steam_library {
    use super::*;

    #[test]
    fn every_owned_game_is_carried_over_with_its_title() {
        let snapshot = steam::parse_library(OWNED, ACCOUNT).expect("parses");
        let titles: Vec<&str> = snapshot
            .games
            .iter()
            .filter_map(|game| game.name.as_deref())
            .collect();
        // In app-id order, which is the order the file is written in.
        assert_eq!(
            titles,
            [
                "Half-Life 2",
                "Dota 2",
                "The Witcher 3: Wild Hunt",
                "Starfield"
            ]
        );
    }

    #[test]
    fn games_are_ordered_by_app_id_so_two_snapshots_diff_into_what_changed() {
        let snapshot = steam::parse_library(OWNED, ACCOUNT).expect("parses");
        let ids: Vec<u32> = snapshot
            .games
            .iter()
            .filter_map(|game| game.app_id)
            .collect();
        assert_eq!(ids, [220, 570, 292_030, 1_202_130]);
    }

    #[test]
    fn played_unplayed_and_unreported_playtimes_stay_three_different_things() {
        let snapshot = steam::parse_library(OWNED, ACCOUNT).expect("parses");
        let playtime = |app_id: u32| {
            snapshot
                .games
                .iter()
                .find(|game| game.app_id == Some(app_id))
                .expect("the game is in the fixture")
                .playtime_minutes
        };
        assert_eq!(playtime(220), Some(1243), "a played game keeps its minutes");
        assert_eq!(playtime(1_202_130), Some(0), "unplayed is zero, not absent");
        assert_eq!(playtime(292_030), None, "unreported is absent, not zero");
    }

    #[test]
    fn steams_own_count_is_recorded_beside_the_number_of_games_written() {
        // Two numbers for one fact. While they agree the read was complete, and a disagreement
        // is the earliest sign that it was not.
        let snapshot = steam::parse_library(OWNED, ACCOUNT).expect("parses");
        assert_eq!(snapshot.provenance.reported_count, Some(4));
        assert_eq!(snapshot.games.len(), 4);
    }

    #[test]
    fn the_snapshot_names_the_account_it_belongs_to() {
        let snapshot = steam::parse_library(OWNED, ACCOUNT).expect("parses");
        assert_eq!(snapshot.provenance.account.as_deref(), Some(ACCOUNT));
    }
}

mod reading_a_steam_wishlist {
    use super::*;

    #[test]
    fn each_wishlisted_game_keeps_the_rank_it_was_given() {
        let snapshot = steam::parse_wishlist(WISHLIST, ACCOUNT).expect("parses");
        let ranked: Vec<(u32, Option<u32>)> = snapshot
            .games
            .iter()
            .map(|game| (game.app_id.expect("an app-id"), game.priority))
            .collect();
        assert_eq!(
            ranked,
            [(105_600, Some(0)), (367_520, Some(2)), (1_091_500, Some(1))]
        );
    }

    #[test]
    fn no_titles_are_invented_for_an_endpoint_that_publishes_none() {
        let snapshot = steam::parse_wishlist(WISHLIST, ACCOUNT).expect("parses");
        assert!(snapshot.games.iter().all(|game| game.name.is_none()));
    }
}

mod reading_an_epic_library {
    use super::*;

    #[test]
    fn every_owned_game_is_carried_over_with_its_title() {
        let entries = epic::parse_list(LEGENDARY).expect("parses");
        let named: Vec<(&str, Option<&str>)> = entries
            .iter()
            .map(|entry| (entry.id.as_str(), entry.name.as_deref()))
            .collect();
        assert_eq!(
            named,
            [("Sandworm", Some("Control")), ("Snapdragon", Some("Hades"))]
        );
    }

    #[test]
    fn downloadable_content_is_not_counted_as_a_game() {
        let entries = epic::parse_list(LEGENDARY).expect("parses");
        assert_eq!(entries.len(), 2, "the nested DLC reached the game list");
    }

    #[test]
    fn epic_identifiers_are_never_passed_off_as_steam_app_ids() {
        let entries = epic::parse_list(LEGENDARY).expect("parses");
        assert!(entries.iter().all(|entry| entry.app_id.is_none()));
    }
}

mod the_file_that_gets_written {
    use super::*;

    #[test]
    fn each_store_and_kind_writes_the_file_the_operator_was_promised() {
        let scratch = Scratch::new("names");
        let library = steam::parse_library(OWNED, ACCOUNT)
            .expect("parses")
            .write(scratch.path())
            .expect("writes");
        let wishlist = steam::parse_wishlist(WISHLIST, ACCOUNT)
            .expect("parses")
            .write(scratch.path())
            .expect("writes");

        assert_eq!(library.file_name().expect("named"), "game_lib_steam");
        assert_eq!(wishlist.file_name().expect("named"), "game_wishlist_steam");
    }

    #[test]
    fn it_is_json_carrying_the_games_and_where_they_came_from() {
        let scratch = Scratch::new("shape");
        let snapshot = steam::parse_library(OWNED, ACCOUNT).expect("parses");
        let file = written(&snapshot, &scratch);

        assert_eq!(file["store"], "steam");
        assert_eq!(file["kind"], "library");
        assert_eq!(file["games"].as_array().expect("a list").len(), 4);
        assert_eq!(file["provenance"]["reported_count"], 4);
        assert!(
            file["captured"]
                .as_str()
                .expect("a timestamp")
                .ends_with('Z'),
            "the capture time should say it is UTC"
        );
    }

    #[test]
    fn the_recorded_endpoint_carries_no_key_even_in_its_redacted_form() {
        // The snapshot is a file the operator may hand to someone else, so the URL it records
        // has to be one that is safe to pass on.
        let scratch = Scratch::new("redaction");
        let snapshot = steam::parse_library(OWNED, ACCOUNT).expect("parses");
        let file = written(&snapshot, &scratch);

        let source = file["provenance"]["source_url"]
            .as_str()
            .expect("a source url");
        assert!(source.contains("key=REDACTED"), "{source}");
        assert!(source.contains("GetOwnedGames"), "{source}");
    }

    #[test]
    fn nobody_but_the_owner_can_read_it() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("mode");
        let path = steam::parse_library(OWNED, ACCOUNT)
            .expect("parses")
            .write(scratch.path())
            .expect("writes");

        let mode = std::fs::metadata(&path)
            .expect("exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "mode is {mode:o}");
    }

    #[test]
    fn a_library_that_came_back_short_does_not_overwrite_the_last_good_one() {
        let scratch = Scratch::new("shrink");
        steam::parse_library(OWNED, ACCOUNT)
            .expect("parses")
            .write(scratch.path())
            .expect("writes");

        let short = r#"{"response":{"game_count":1,"games":[{"appid":220,"name":"Half-Life 2"}]}}"#;
        let failure = steam::parse_library(short, ACCOUNT)
            .expect("parses")
            .write(scratch.path())
            .unwrap_err();
        assert!(failure.to_string().contains("does not shrink"), "{failure}");

        let kept: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(scratch.path().join("game_lib_steam")).expect("still there"),
        )
        .expect("still JSON");
        assert_eq!(kept["games"].as_array().expect("a list").len(), 4);
    }

    #[test]
    fn a_wishlist_that_came_back_short_replaces_the_old_one() {
        // Buying a wishlisted game takes it off the list, so shrinking is this one's normal
        // behaviour and guarding against it would block every ordinary update.
        let scratch = Scratch::new("wishshrink");
        steam::parse_wishlist(WISHLIST, ACCOUNT)
            .expect("parses")
            .write(scratch.path())
            .expect("writes");

        let short = r#"{"response":{"items":[{"appid":105600,"priority":1}]}}"#;
        let path = steam::parse_wishlist(short, ACCOUNT)
            .expect("parses")
            .write(scratch.path())
            .expect("a shorter wishlist is ordinary");

        let file: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("reads")).expect("JSON");
        assert_eq!(file["games"].as_array().expect("a list").len(), 1);
    }

    #[test]
    fn the_two_steam_snapshots_do_not_write_over_each_other() {
        let scratch = Scratch::new("coexist");
        let library = steam::parse_library(OWNED, ACCOUNT).expect("parses");
        let wishlist = steam::parse_wishlist(WISHLIST, ACCOUNT).expect("parses");
        assert_ne!(library.file_name(), wishlist.file_name());
        assert_eq!(library.kind, Kind::Library);
        assert_eq!(wishlist.kind, Kind::Wishlist);

        library.write(scratch.path()).expect("writes");
        wishlist.write(scratch.path()).expect("writes");
        assert!(scratch.path().join("game_lib_steam").exists());
        assert!(scratch.path().join("game_wishlist_steam").exists());
    }
}

mod every_kind_there_is {
    use super::*;

    /// `Kind::ALL` is a written-out list, so something has to hold it to the enum.
    ///
    /// The match is what the compiler checks: a new variant stops this compiling until it is
    /// named here, and the round-trip then fails until it is added to `ALL` in the same position.
    /// That is the difference between "someone remembered" and "it does not build otherwise".
    #[test]
    fn all_lists_every_kind_in_its_own_order() {
        for (at, kind) in Kind::ALL.iter().enumerate() {
            let named = match kind {
                Kind::Library => 0,
                Kind::Wishlist => 1,
            };
            assert_eq!(at, named, "{kind} is in ALL at {at} and named {named}");
        }
    }

    /// And the file names it drives, which is what `holdings` looks for.
    #[test]
    fn each_kind_names_the_file_a_snapshot_of_it_is_written_to() {
        let names: Vec<String> = Kind::ALL
            .iter()
            .map(|kind| catalogames::commands::gamelib::file_name(*kind, steam::STORE))
            .collect();
        assert_eq!(names, ["game_lib_steam", "game_wishlist_steam"]);
    }
}
