//! Writing an Epic entry back out as Rust source.
//!
//! The sibling of [`crate::inventory::steam::source`], and separate from the generator for the
//! same reason: what a generated table looks like is decided in one place, and the shape can be
//! tested against the committed file without running anything.

use std::fmt::Write as _;

use crate::inventory::steam::source::literal;

/// One entry, indented to sit inside the table, ending with its own comma and newline.
///
/// One line per entry, unlike a Steam entry's block, because two fields fit on one line and a
/// table of six hundred single lines is readable in a way six hundred blocks are not.
/// Takes the two values rather than an [`EpicGame`](crate::inventory::epic::EpicGame), because
/// that type is `#[non_exhaustive]` and a generator lives in its own crate: what it writes is
/// source text, and it never needs to build one.
#[must_use]
pub fn entry(app_name: &str, title: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "    EpicGame {{ app_name: {}, title: {} }},",
        literal(app_name),
        literal(title)
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed table, so a rendered entry can be compared against the real thing.
    const TABLE: &str = include_str!("giveaways.rs");

    #[test]
    fn what_it_writes_is_byte_for_byte_what_the_generated_table_holds() {
        // Checked against every entry rather than a chosen one, so a title with a quote or a
        // trademark symbol in it cannot be the one that differs.
        let missing: Vec<&str> = crate::inventory::epic::all()
            .filter(|game| !TABLE.contains(&entry(game.app_name, game.title)))
            .map(|game| game.app_name)
            .collect();
        assert!(
            missing.is_empty(),
            "rendered differently from the committed table: {missing:?}"
        );
    }

    #[test]
    fn a_title_carrying_a_quote_is_escaped_rather_than_breaking_the_table() {
        // A real one: `3 out of 10, EP 1: "Welcome To Shovelworks"`.
        let written = entry("Flounder", r#"3 out of 10, EP 1: "Welcome To Shovelworks""#);
        assert!(
            written.contains(r#"\"Welcome To Shovelworks\""#),
            "{written}"
        );
    }

    #[test]
    fn a_trademark_symbol_survives_as_itself() {
        // Twenty-one of one account's titles carry one, and a table full of unicode escapes
        // would be unreadable for no gain.
        let written = entry("x", "HOT WHEELS UNLEASHED™");
        assert!(written.contains("HOT WHEELS UNLEASHED™"), "{written}");
    }

    #[test]
    fn an_entry_is_one_line_ending_in_its_own_comma() {
        assert_eq!(
            entry("Albacore", "Assassins Creed Syndicate"),
            "    EpicGame { app_name: \"Albacore\", title: \"Assassins Creed Syndicate\" },\n"
        );
    }
}
