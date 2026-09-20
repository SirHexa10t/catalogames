//! Generates `inventory/epic/giveaways.rs` from a snapshot of an Epic library.
//!
//! ```text
//! cargo run --release --features tools --bin epic_lookup -- game_lib_epic \
//!     > src/inventory/epic/giveaways.rs
//! ```
//!
//! The sibling of `steam_lookup`, and much the smaller of the two, because there is far less to
//! mine: an Epic library published through Legendary carries an identifier and a title per game
//! and nothing else. No searching, no matching, no resolving — so none of the machinery that
//! makes the Steam generator fragile, and none of its `// REVIEW:` lines either.
//!
//! **It reads a snapshot rather than the store.** `gamelib epic` already talks to Legendary,
//! checks the sign-in, cross-checks the count and writes the result down; asking again here
//! would be a second way to do one thing. Point this at that file.
//!
//! # What it deliberately does not carry across
//!
//! The snapshot's provenance names the **account** it came from. That stays out of the generated
//! module: every file in this tree has to read identically for anyone, and this crate is meant
//! to be published. What comes across is the capture date and the titles.
//!
//! Prints to stdout and has no write-in-place flag, for the same reason `steam_lookup` does not:
//! these are curated entries, and reading the diff is the only thing that catches a bad one.

use std::fmt::Write as _;

use catalogames::inventory::epic::source;

/// One game as a snapshot records it.
#[derive(Debug, serde::Deserialize)]
struct Recorded {
    id: String,
    /// Absent for a store that publishes no titles. Epic publishes them, so an entry without
    /// one is a shape nobody has seen and is reported rather than given an invented name.
    name: Option<String>,
}

/// A snapshot, of which only these three parts are read.
#[derive(Debug, serde::Deserialize)]
struct Snapshot {
    store: String,
    kind: String,
    /// `2026-09-18 01:27:20Z`, of which the date is what a generated module records.
    captured: String,
    games: Vec<Recorded>,
}

/// What the file must say it is, so a Steam snapshot cannot be read as an Epic one.
const STORE: &str = "epic";
const KIND: &str = "library";

fn main() -> std::process::ExitCode {
    let mut arguments = std::env::args().skip(1);
    let Some(path) = arguments.next() else {
        eprintln!(
            "usage: epic_lookup <snapshot>\n\n\
             The snapshot is the file `gamelib epic` writes, by default\n\
             ~/.local/share/catalogames/game_lib_epic ."
        );
        return std::process::ExitCode::FAILURE;
    };

    match run(&path) {
        Ok(module) => {
            print!("{module}");
            std::process::ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("epic_lookup: {problem}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(path: &str) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|source| format!("{path}: {source}"))?;
    let snapshot: Snapshot =
        serde_json::from_str(&text).map_err(|source| format!("{path}: {source}"))?;

    // Checked rather than assumed: the two snapshot kinds have the same shape, so a wishlist or
    // a Steam library would parse happily and generate a table of the wrong thing.
    if snapshot.store != STORE || snapshot.kind != KIND {
        return Err(format!(
            "{path} is a {} {}; this reads an {STORE} {KIND}",
            snapshot.store, snapshot.kind
        ));
    }
    let entries = entries(&snapshot.games)?;
    if entries.is_empty() {
        return Err(format!("{path} holds no games"));
    }
    Ok(emit(&entries, date_of(&snapshot.captured)))
}

/// The snapshot's games as inventory entries, sorted and deduplicated.
///
/// Sorted by identifier so that regenerating after a few more giveaways produces a diff of what
/// was added rather than a reshuffle. Deduplicated because two rows sharing an identifier would
/// make a lookup answer arbitrarily; an entitlement cannot be held twice, so a repeat is a fault
/// in the snapshot and is dropped rather than carried.
fn entries(games: &[Recorded]) -> Result<Vec<(String, String)>, String> {
    let nameless: Vec<&str> = games
        .iter()
        .filter(|game| game.name.as_deref().unwrap_or("").trim().is_empty())
        .map(|game| game.id.as_str())
        .collect();
    if !nameless.is_empty() {
        return Err(format!(
            "no title for {nameless:?} — Epic publishes one for every entitlement, so this is a \
             shape nobody has seen rather than something to name for it"
        ));
    }

    let mut entries: Vec<(String, String)> = games
        .iter()
        .map(|game| {
            (
                game.id.clone(),
                game.name.clone().unwrap_or_default().trim().to_owned(),
            )
        })
        .collect();
    entries.sort();
    entries.dedup_by(|a, b| a.0 == b.0);
    Ok(entries)
}

/// The date out of a capture timestamp, which is all a generated module records.
fn date_of(captured: &str) -> &str {
    captured.split_whitespace().next().unwrap_or(captured)
}

/// The whole generated module.
fn emit(entries: &[(String, String)], captured: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "//! Games an Epic account was given free in a promotion.\n\
         //!\n\
         //! GENERATED — do not hand-edit; regenerate and review the diff:\n\
         //!\n\
         //! ```text\n\
         //! cargo run --release --features tools --bin epic_lookup -- <snapshot> \\\n\
         //!     > src/inventory/epic/giveaways.rs\n\
         //! ```\n\
         //!\n\
         //! The snapshot is what `gamelib epic` writes. Hand edits are lost on the next run.\n\
         //!\n\
         //! Two fields per entry, because that is everything Epic publishes about a library\n\
         //! through Legendary. See [`super`] for why there is no more to have.\n\n",
    );
    out.push_str("use super::EpicGame;\n\n");
    let _ = writeln!(
        out,
        "/// When these entries were captured. A lower bound on freshness.\n\
         pub const CAPTURED: &str = {};\n",
        catalogames::inventory::steam::source::literal(captured)
    );

    // `rustfmt::skip` keeps one entry per line. Without it every entry becomes a four-line
    // block, six hundred rows become two and a half thousand, and regenerating no longer
    // reproduces the committed file — so every diff is swamped by reformatting.
    let _ = writeln!(
        out,
        "/// Every game the account holds, sorted by Epic's own identifier.\n\
         #[rustfmt::skip]\n\
         pub const GIVEAWAYS: &[EpicGame] = &["
    );
    for (app_name, title) in entries {
        out.push_str(&source::entry(app_name, title));
    }
    out.push_str("];\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorded(id: &str, name: Option<&str>) -> Recorded {
        Recorded {
            id: id.to_owned(),
            name: name.map(str::to_owned),
        }
    }

    #[test]
    fn entries_are_sorted_by_identifier_so_a_regeneration_diffs_into_what_was_added() {
        let sorted = entries(&[
            recorded("Zebra", Some("Last")),
            recorded("Albacore", Some("First")),
            recorded("Mango", Some("Middle")),
        ])
        .expect("all named");
        let order: Vec<&str> = sorted.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(order, ["Albacore", "Mango", "Zebra"]);
    }

    #[test]
    fn an_entitlement_listed_twice_becomes_one_entry() {
        // It cannot be held twice, so a repeat is a fault in the snapshot rather than news.
        let once = entries(&[
            recorded("Albacore", Some("A")),
            recorded("Albacore", Some("A")),
        ])
        .expect("named");
        assert_eq!(once.len(), 1);
    }

    #[test]
    fn a_game_with_no_title_stops_the_run_rather_than_being_named_for_it() {
        // Epic publishes a title for every entitlement, so an absent one is a shape nobody has
        // seen — and inventing one would put a guess in a curated table.
        for missing in [None, Some(""), Some("   ")] {
            let refused = entries(&[recorded("Albacore", Some("A")), recorded("Odd", missing)])
                .expect_err("should refuse");
            assert!(refused.contains("Odd"), "{refused}");
        }
    }

    #[test]
    fn only_the_date_of_a_capture_reaches_the_generated_module() {
        assert_eq!(date_of("2026-09-18 01:27:20Z"), "2026-09-18");
        assert_eq!(date_of("2026-09-18"), "2026-09-18");
    }

    #[test]
    fn the_account_a_snapshot_names_never_reaches_the_generated_module() {
        // A snapshot's provenance names the account it came from, and a generated file in this
        // tree has to read identically for anyone — this crate is meant to be published. Driven
        // through the real reader with a real provenance block, rather than asserting on the
        // emitter alone, because it is the READING that has to drop the field.
        let snapshot = r#"{"store":"epic","kind":"library","captured":"2026-09-18 01:27:20Z",
            "provenance":{"account":"SomebodysHandle","read_with":"legendary version 0.21.1"},
            "games":[{"id":"Albacore","name":"A Game"}]}"#;
        let path = std::env::temp_dir().join(format!("epic-account-{}.json", std::process::id()));
        std::fs::write(&path, snapshot).expect("writes");

        let module = run(path.to_str().expect("utf-8")).expect("generates");
        assert!(module.contains("A Game"), "the games did come across");
        for personal in ["SomebodysHandle", "provenance", "read_with"] {
            assert!(!module.contains(personal), "{personal:?} reached the table");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_generated_module_keeps_one_entry_per_line() {
        // Without the skip attribute `cargo fmt` turns each entry into a block, and a
        // regeneration then no longer reproduces the committed file.
        let module = emit(
            &[
                ("Albacore".to_owned(), "First".to_owned()),
                ("Mango".to_owned(), "Second".to_owned()),
            ],
            "2026-09-18",
        );
        assert!(module.contains("#[rustfmt::skip]"), "{module}");
        assert_eq!(module.matches("EpicGame {").count(), 2, "{module}");
    }

    #[test]
    fn a_snapshot_of_something_else_is_refused() {
        // A wishlist and a library have the same shape, so nothing but the declared kind can
        // tell them apart — and a Steam snapshot would generate a table of the wrong store.
        let wishlist = r#"{"store":"steam","kind":"wishlist","captured":"2026-09-18 01:00:00Z",
            "games":[{"id":"1","name":"A"}]}"#;
        let path = std::env::temp_dir().join(format!("epic-lookup-{}.json", std::process::id()));
        std::fs::write(&path, wishlist).expect("writes");

        let refused = run(path.to_str().expect("utf-8")).expect_err("should refuse");
        assert!(refused.contains("steam wishlist"), "{refused}");
        let _ = std::fs::remove_file(&path);
    }
}
