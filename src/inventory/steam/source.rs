//! Writing an inventory entry back out as Rust source.
//!
//! One place decides what a `SteamGame { … }` literal looks like, because two places would
//! drift: the generator rewrites the whole table, and `add_project_entry` produces a single row
//! meant to be pasted into that same table. A row that did not match its neighbours would show
//! up as a spurious diff the next time the table was regenerated.
//!
//! The output is deliberately the *committed* shape, not rustfmt's. `regular.rs` carries
//! `#[rustfmt::skip]` so that the one-entry-per-line table survives `cargo fmt`; without it the
//! table triples in length and regenerating stops reproducing the committed file.

use std::fmt::Write as _;

use crate::inventory::steam::{Reviews, Tag};
use crate::steam::store::GameDetails;

/// One entry, indented to sit inside the table, ending with its own comma and newline.
///
/// `tags` is passed separately rather than read from `details.tags`, and the difference is the
/// point: `GameDetails` carries Steam's own tag *strings* so that a runtime lookup cannot fail
/// the day Valve adds a tag, while a table being written needs [`Tag`] variants and must fail
/// loudly on an unknown one. Mapping one to the other is the caller's decision, so the caller
/// makes it — see [`Tag::from_name`].
#[must_use]
pub fn entry(details: &GameDetails, aliases: &[String], tags: &[Tag]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "    SteamGame {{");
    let _ = writeln!(out, "        app_id: {},", details.app_id);
    let _ = writeln!(out, "        name: {},", literal(&details.name));
    let _ = writeln!(out, "        aliases: &[{}],", literals(aliases));
    let _ = writeln!(out, "        released: {},", literal(&details.released));
    match details.recent {
        Some(recent) => {
            let _ = writeln!(out, "        recent: Some({}),", reviews(recent));
        }
        None => {
            let _ = writeln!(out, "        recent: None,");
        }
    }
    let _ = writeln!(out, "        all_time: {},", reviews(details.all_time));
    let names: Vec<String> = tags.iter().map(|tag| format!("Tag::{tag:?}")).collect();
    let _ = writeln!(out, "        tags: &[{}],", names.join(", "));
    let _ = writeln!(
        out,
        "        os: Os {{ windows: {}, mac: {}, linux: {} }},",
        details.os.windows, details.os.mac, details.os.linux
    );
    let _ = writeln!(out, "        vr: Vr::{:?},", details.vr);
    let _ = writeln!(out, "        deck: Deck::{:?},", details.deck);
    let _ = writeln!(out, "        features: &[{}],", literals(&details.features));
    let _ = writeln!(out, "    }},");
    out
}

/// A Rust string literal, escaped the way the compiler would print it.
#[must_use]
pub fn literal(text: &str) -> String {
    format!("{text:?}")
}

/// A comma-separated run of string literals, for a `&[…]` slice.
#[must_use]
pub fn literals(values: &[String]) -> String {
    values
        .iter()
        .map(|value| literal(value))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A `Reviews { … }` literal.
#[must_use]
pub fn reviews(reviews: Reviews) -> String {
    format!(
        "Reviews {{ approval: {}, count: {} }}",
        reviews.approval, reviews.count
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::steam::SteamGame;
    use crate::steam::store::AppKind;

    /// The committed table, so a rendered entry can be compared against the real thing.
    const TABLE: &str = include_str!("regular.rs");

    /// Rebuilds the runtime shape from a committed entry, so the test drives the real function
    /// over real data rather than over a fixture written to match it.
    fn details_of(game: &SteamGame) -> GameDetails {
        GameDetails {
            app_id: game.app_id,
            name: game.name.to_owned(),
            kind: AppKind::Game,
            released: game.released.to_owned(),
            recent: game.recent,
            all_time: game.all_time,
            // Unused by `entry`; the typed tags are passed separately. See its doc.
            tags: Vec::new(),
            os: game.os,
            vr: game.vr,
            deck: game.deck,
            features: game.features.iter().map(|f| (*f).to_owned()).collect(),
            package_options: Vec::new(),
        }
    }

    fn rendered(game: &SteamGame) -> String {
        let aliases: Vec<String> = game.aliases.iter().map(|a| (*a).to_owned()).collect();
        entry(&details_of(game), &aliases, game.tags)
    }

    /// The guarantee this module exists for: what it writes is what is already committed.
    ///
    /// Checked against every entry rather than a chosen one, so an entry with an unusual shape —
    /// no recent row, an alias, a quote in its title — cannot be the one that differs.
    #[test]
    fn what_it_writes_is_byte_for_byte_what_the_generated_table_holds() {
        let mut missing = Vec::new();
        for game in crate::inventory::steam::all() {
            if !TABLE.contains(&rendered(game)) {
                missing.push(game.app_id);
            }
        }
        assert!(
            missing.is_empty(),
            "rendered differently from the committed table: {missing:?}\nfirst: {}",
            crate::inventory::steam::by_app_id(missing[0])
                .map(rendered)
                .unwrap_or_default()
        );
    }

    #[test]
    fn a_game_with_no_recent_row_writes_none_rather_than_an_empty_reviews() {
        // The difference between "nobody reviewed it lately" and "nobody reviewed it" is the
        // whole reason `recent` is an Option.
        let details = GameDetails {
            recent: None,
            ..details_of(
                crate::inventory::steam::all()
                    .next()
                    .expect("a table with entries"),
            )
        };
        let text = entry(&details, &[], &[]);
        assert!(text.contains("recent: None,"), "{text}");
        assert!(!text.contains("recent: Some"), "{text}");
    }

    #[test]
    fn a_title_carrying_a_quote_is_escaped_rather_than_breaking_the_table() {
        assert_eq!(literal(r#"Dead "Space""#), r#""Dead \"Space\"""#);
        assert_eq!(literal(r"C:\games"), r#""C:\\games""#);
    }

    #[test]
    fn an_entry_ends_with_its_own_comma_so_it_can_be_pasted_between_two_others() {
        let text = rendered(crate::inventory::steam::all().next().expect("an entry"));
        assert!(text.ends_with("    },\n"), "{text}");
        assert!(text.starts_with("    SteamGame {\n"), "{text}");
    }

    #[test]
    fn tags_are_written_as_enum_variants_not_as_strings() {
        let details = details_of(crate::inventory::steam::all().next().expect("an entry"));
        let text = entry(&details, &[], &[Tag::SoulsLike, Tag::CoOp]);
        assert!(
            text.contains("tags: &[Tag::SoulsLike, Tag::CoOp],"),
            "{text}"
        );
    }
}
