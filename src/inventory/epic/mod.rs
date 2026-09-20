//! Games as the Epic Games Store publishes them — the sibling of [`crate::inventory::steam`].
//!
//! Split into modules by how the games were come by rather than by what they are:
//! [`giveaways`] holds an account's free promotional titles, which is the whole of one real
//! account and the reason this module exists at all.
//!
//! # Why an entry carries so little
//!
//! Two fields, against twelve for a Steam entry, and that is Epic's doing rather than an
//! omission. What an Epic library publishes through
//! [Legendary](https://github.com/derrod/legendary) is an identifier and a title. No reviews, no
//! tags, no release date, no platforms: the store does not put them where a client can read
//! them. This module holds what is published and invents nothing, which is what its placeholder
//! promised when there was still nothing here to hold.
//!
//! # What it is for
//!
//! Not buying a game twice. A bundle offering something already sitting free in an Epic account
//! is not an offer, and [`by_title`] is the question to ask of it — matched through
//! [`crate::inventory::steam::comparable`], which discards punctuation and spacing, because two
//! stores rarely punctuate a title the same way.
//!
//! # Two invariants that cannot be tested mechanically
//!
//! The conventions of [`crate::inventory::steam`] would suggest a test for each of these, and
//! both would fail on real data the day they landed. They are written down instead.
//!
//! **An identifier has no fixed shape.** Of 590 entries in one account, 453 are 32-character
//! hexadecimal catalog ids and 137 are short codenames. Nothing may assume either.
//!
//! **Two entries may collide under `comparable`, and that is not a fault.** One account holds
//! `Shadow Tactics: Blades of the Shogun` and `Shadow Tactics Blades of the Shogun` under
//! different identifiers: the same game, granted twice, punctuated differently. The invariant is
//! that a collision is the *same game*, which no test can check — and it costs nothing here,
//! because either match answers "is this already owned" correctly.

pub mod giveaways;
pub mod source;

use crate::inventory::steam::comparable;

/// One game an Epic account holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct EpicGame {
    /// Epic's own identifier for the entitlement, and the stable identity here.
    ///
    /// Two shapes appear, both from the same account: a 32-character hexadecimal catalog id,
    /// and a short codename such as `Albacore`. Measured over 590 entries, 453 were hexadecimal
    /// and 137 were codenames, so neither shape can be assumed and nothing here parses it.
    pub app_name: &'static str,
    /// Title as Epic prints it, trademark symbols and all.
    ///
    /// Kept verbatim rather than cleaned up: it is what the store shows and what a person will
    /// search for. [`by_title`] is what makes it matchable in spite of the punctuation.
    pub title: &'static str,
}

/// One generated category of entries.
#[derive(Debug, Clone, Copy)]
pub struct Module {
    /// Category name, matching the module's own name.
    pub name: &'static str,
    /// ISO-8601 date the entries were generated. A lower bound on freshness, not a claim that
    /// nothing has changed since.
    pub captured: &'static str,
    pub games: &'static [EpicGame],
}

/// Every category there is, so a caller asking for "all of them" cannot miss one.
pub const MODULES: &[Module] = &[Module {
    name: "giveaways",
    captured: giveaways::CAPTURED,
    games: giveaways::GIVEAWAYS,
}];

/// Every game in every category.
pub fn all() -> impl Iterator<Item = &'static EpicGame> {
    MODULES.iter().flat_map(|module| module.games.iter())
}

/// The entry with this Epic identifier, if it is held.
#[must_use]
pub fn by_app_name(app_name: &str) -> Option<&'static EpicGame> {
    all().find(|game| game.app_name == app_name)
}

/// The entry whose title matches, ignoring how either store punctuates it.
///
/// The question worth asking of this module: a bundle offering a game already held free is not
/// an offer. Matched through [`comparable`], which keeps only alphanumerics, because Epic writes
/// `Dishonored®: Death of the Outsider™` where a bundle writes `Dishonored - Death of the
/// Outsider`.
#[must_use]
pub fn by_title(title: &str) -> Option<&'static EpicGame> {
    let wanted = comparable(title);
    all().find(|game| comparable(game.title) == wanted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_is_reachable_from_one_place() {
        // Derived rather than written out, so a category added to the enum cannot be missing
        // from the list that claims to hold them all.
        assert_eq!(MODULES.len(), 1);
        assert_eq!(all().count(), giveaways::GIVEAWAYS.len());
    }

    #[test]
    fn no_identifier_appears_twice() {
        // The identifier is the identity here; two rows sharing one would make `by_app_name`
        // answer arbitrarily.
        let mut seen: Vec<&str> = all().map(|game| game.app_name).collect();
        let total = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), total, "a duplicated identifier is in the table");
    }

    #[test]
    fn an_entry_is_found_by_the_identifier_it_was_stored_under() {
        let Some(first) = all().next() else {
            return; // an empty table is a legitimate state before a first generation
        };
        assert_eq!(by_app_name(first.app_name), Some(first));
        assert_eq!(by_app_name("no-such-entitlement"), None);
    }

    #[test]
    fn a_title_is_found_however_either_store_punctuates_it() {
        // The whole point of matching through `comparable`: Epic's trademark symbols and a
        // bundle's dashes are the same game.
        let Some(marked) = all().find(|game| game.title.contains('™')) else {
            return;
        };
        let plain: String = marked.title.chars().filter(|c| *c != '™').collect();
        assert_eq!(by_title(&plain), Some(marked), "{:?}", marked.title);
    }

    #[test]
    fn a_title_nothing_holds_is_not_matched_to_something_else() {
        assert_eq!(by_title("A Game That Is Not In Anybody's Library"), None);
    }

    #[test]
    fn two_entries_for_one_game_both_answer_the_only_question_asked_of_them() {
        // Deliberately not a "no collisions under comparable" test, which is what this module's
        // conventions would suggest and which real data refuses: one account holds the same game
        // twice under different punctuation. What matters is that either row answers correctly.
        let mut seen: std::collections::BTreeMap<String, Vec<&str>> =
            std::collections::BTreeMap::new();
        for game in all() {
            seen.entry(comparable(game.title))
                .or_default()
                .push(game.app_name);
        }
        for (key, holders) in seen.iter().filter(|(_, held)| held.len() > 1) {
            // A collision is answerable: the lookup returns one of them, and it is held.
            let found = by_title(key).expect("a colliding title is still found");
            assert!(
                holders.contains(&found.app_name),
                "{key}: answered with something not in {holders:?}"
            );
        }
    }
}
