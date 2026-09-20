//! Games one store is selling that another store has already given away.
//!
//! A game sitting free in an Epic account is worth knowing about while deciding what to buy.
//! This is the one place that asks, because a mapping between two inventories belongs in
//! neither of them: [`crate::inventory::steam`] has no business knowing Epic exists, and the
//! reverse.
//!
//! # A fact, never advice
//!
//! What this reports is that the same title is held elsewhere. It does **not** say the bundle is
//! not worth buying, and must not be worded as though it did: an Epic copy is not a Steam copy.
//! No Steam key, so no achievements, no trading cards, no cloud saves, no Workshop, and no Deck
//! verdict — which is precisely why [`crate::inventory::steam::SteamGame`] carries `deck`, `os`,
//! `vr` and `features` at all. Somebody may want the Steam copy of a game they already own, and
//! that is their call to make.
//!
//! Wording it as a fact also costs less when it is wrong. A mislabelled fact is something to
//! check against your own library in a few seconds; a mislabelled instruction is something
//! acted on.
//!
//! # Nothing is fetched and nothing is stored
//!
//! Both inventories are already committed, so the answer is computed from them. No third file to
//! keep in step with the first two, no requests, and above all no searching Steam's catalogue by
//! name — the fragile half this crate deliberately keeps inside its generators.
//!
//! # The matching rule, and the one way it can be wrong
//!
//! Exact equality under [`comparable`], which keeps only alphanumerics. That is deliberately
//! strict, and it is **not** the match that once resolved "Ashen" to "Ashen Empires": that was a
//! prefix picked up by a search, and `ashen` does not equal `ashenempires`. Full equality cannot
//! do that.
//!
//! What it does instead is fail in the safe direction. An edition suffix one store carries and
//! the other drops means no match, so the answer is "not known to be owned" rather than a wrong
//! claim — and the whole table is a hint, not an inventory of your purchases.
//!
//! The one unsafe case left is two genuinely different games sharing a name once punctuation is
//! gone, and it is **already half-loaded**: one Epic account holds `PREY`, and the Steam table
//! holds no Prey at all. The pair does not exist today because one side is missing, not because
//! the rule is safe — the day a bundle carries Prey, name equality alone cannot tell Arkane's
//! 2017 game from Human Head's 2006 one.
//!
//! The inventory's own convention is the answer to that: three entries already carry a `(YYYY)`
//! suffix with the bare title as an alias. Which names the exact way a false positive will
//! arrive — two year-disambiguated entries each keeping the bare title as an alias, and one Epic
//! title matching both. [`ambiguous`] finds that, and its test asserts there is none today, so
//! the check cannot cry wolf on the day it is added.

use crate::Game;
use crate::inventory::epic as epic_inventory;
use crate::inventory::epic::EpicGame;
use crate::inventory::steam as steam_inventory;

/// The Epic entry for a game a bundle is offering, if one is held under the same name.
///
/// Asked of the **inventory's** name first, when the bundle publishes an app-id. That name came
/// from Steam's own store page, which is a better thing to match against than a bundle's
/// marketing wording — and where there is no app-id, the bundle's wording is all there is.
#[must_use]
pub fn owned_on_epic(game: &Game) -> Option<&'static EpicGame> {
    match game.steam_app_id.and_then(steam_inventory::by_app_id) {
        Some(entry) => held(entry),
        None => by_title(&game.title),
    }
}

/// The Epic entry for an inventory entry, under any of the names it goes by.
///
/// Aliases as well as the name, for the same reason [`steam_inventory::by_name`] consults them:
/// an entry disambiguated as `Broken Sword 3 - the Sleeping Dragon (2003)` keeps the bare title
/// as an alias, and the bare title is what another store will print. That convention exists to
/// tell same-named games apart, so it is the one thing certain to grow.
#[must_use]
pub fn held(entry: &steam_inventory::SteamGame) -> Option<&'static EpicGame> {
    steam_inventory::names(entry).find_map(by_title)
}

/// The Epic entry held under this title, if there is one.
#[must_use]
pub fn by_title(title: &str) -> Option<&'static EpicGame> {
    epic_inventory::by_title(title)
}

/// Every game in the Steam inventory that an Epic account already holds.
///
/// For a caller reporting on the whole catalogue rather than on one bundle. Paired both ways
/// because the two stores name the same game differently, and seeing both spellings is most of
/// what makes a match checkable by eye.
pub fn all() -> impl Iterator<Item = (&'static steam_inventory::SteamGame, &'static EpicGame)> {
    steam_inventory::all().filter_map(|steam| Some((steam, held(steam)?)))
}

/// Epic titles that match more than one Steam entry, which no match can resolve.
///
/// Empty today and asserted to be, which is when a detector is worth adding: it cannot cry wolf
/// on arrival, and the day it speaks it is describing something real. See the module doc for how
/// this will arrive — two year-disambiguated entries both keeping the bare title as an alias.
pub fn ambiguous() -> Vec<(&'static str, Vec<u32>)> {
    let mut found: Vec<(&'static str, Vec<u32>)> = Vec::new();
    for (steam, epic) in all() {
        match found.iter_mut().find(|(title, _)| *title == epic.title) {
            Some((_, ids)) => ids.push(steam.app_id),
            None => found.push((epic.title, vec![steam.app_id])),
        }
    }
    found.retain(|(_, ids)| ids.len() > 1);
    found
}

/// What a listing says beside a game already held elsewhere.
///
/// Names the store and stops. Not "already owned", not "skip this" — see the module doc: an Epic
/// copy is not a Steam copy, and what to do about it is the reader's call.
pub const NOTE: &str = "(on Epic)";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::steam::comparable;

    fn bundle_game(title: &str, steam_app_id: Option<u32>) -> Game {
        Game {
            title: title.to_owned(),
            machine_name: title.to_lowercase(),
            steam_app_id,
            contains: Vec::new(),
        }
    }

    #[test]
    fn the_two_committed_inventories_do_overlap() {
        // If this were zero the feature would be reporting nothing, and the matching rule would
        // be untested against anything real.
        let found: Vec<_> = all().collect();
        assert!(
            found.len() >= 20,
            "only {} crossovers; the matching rule has stopped working",
            found.len()
        );
    }

    #[test]
    fn punctuation_neither_store_agrees_on_does_not_prevent_a_match() {
        // Real pairs from the committed tables: a trademark symbol, a case difference, and
        // brackets against a dash.
        for (steam, epic) in all() {
            if steam.name != epic.title {
                assert_eq!(
                    comparable(steam.name),
                    comparable(epic.title),
                    "matched two titles that do not normalise to the same thing"
                );
                return;
            }
        }
        panic!("no differently-punctuated pair in the tables to check the rule against");
    }

    #[test]
    fn a_prefix_is_not_a_match_which_is_the_failure_this_rule_does_not_have() {
        // "Ashen" once resolved to "Ashen Empires" through a search that matched a prefix.
        // Full equality after normalising cannot do that, and this pins it.
        let Some((steam, _)) = all().next() else {
            return;
        };
        let longer = format!("{} Empires", steam.name);
        assert_eq!(by_title(&longer), None, "{longer:?} matched something");
        let shorter: String = steam.name.chars().take(3).collect();
        assert_eq!(by_title(&shorter), None, "{shorter:?} matched something");
    }

    #[test]
    fn a_bundle_publishing_an_app_id_is_matched_through_the_inventorys_own_name() {
        // The inventory's name came from Steam's store page; a bundle's wording is marketing.
        // Where an app-id says which game it is, that is the better name to match on.
        let Some((steam, epic)) = all().next() else {
            return;
        };
        let oddly_worded = bundle_game(
            "Whatever The Bundle Felt Like Calling It",
            Some(steam.app_id),
        );
        assert_eq!(owned_on_epic(&oddly_worded), Some(epic));
    }

    #[test]
    fn a_bundle_with_no_app_id_is_matched_on_its_own_wording() {
        let Some((_, epic)) = all().next() else {
            return;
        };
        assert_eq!(owned_on_epic(&bundle_game(epic.title, None)), Some(epic));
    }

    #[test]
    fn no_epic_title_matches_two_steam_entries_today() {
        // Asserted while it is TRUE, which is the only time a detector can be trusted not to
        // cry wolf. The day two year-disambiguated entries both keep the bare title as an
        // alias and an Epic title matches both, this says so instead of picking one.
        assert_eq!(ambiguous(), Vec::new(), "a crossover no match can resolve");
    }

    #[test]
    fn an_entry_disambiguated_by_year_is_still_matched_through_its_bare_title() {
        // The convention that exists to tell same-named games apart is the one certain to grow,
        // and a name-only match would miss every entry using it.
        let disambiguated = steam_inventory::all()
            .find(|entry| entry.name.contains("(20") && !entry.aliases.is_empty());
        let Some(entry) = disambiguated else {
            return;
        };
        // Whatever it resolves to, it must consider the aliases as well as the name.
        let by_alias_only: Vec<&str> = entry.aliases.to_vec();
        assert!(!by_alias_only.is_empty());
        assert_eq!(
            held(entry),
            by_alias_only
                .iter()
                .find_map(|a| by_title(a))
                .or(by_title(entry.name))
        );
    }

    #[test]
    fn a_game_nobody_holds_is_reported_as_nothing_rather_than_as_something_close() {
        assert_eq!(
            owned_on_epic(&bundle_game("A Game Nobody Owns", None)),
            None
        );
        assert_eq!(owned_on_epic(&bundle_game("", None)), None);
    }

    #[test]
    fn an_app_id_the_inventory_does_not_know_falls_back_to_the_wording() {
        // An id is authoritative when it is known; when it is not, the title is all there is,
        // and discarding it would lose a match for no reason.
        let Some((_, epic)) = all().next() else {
            return;
        };
        let unknown_id = bundle_game(epic.title, Some(u32::MAX));
        assert_eq!(owned_on_epic(&unknown_id), Some(epic));
    }
}
