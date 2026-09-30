//! What the operator already owns, and what they already want.
//!
//! Read from the snapshot files [`crate::commands::gamelib`] writes, so that a listing can say which of the
//! games on sale are ones you have or ones you have been waiting for. Nothing is fetched here: the
//! files are the answer, and taking them is a separate, deliberate command.
//!
//! # A fact, never advice
//!
//! The same rule [`crate::user_games::crossover`] states at length. "Owned on Steam" says a copy is in the
//! account; it does not say the bundle is not worth buying. A second copy is a gift, an old
//! purchase may predate a remaster, and what to do about it is the reader's call.
//!
//! # Two kinds of answer, and they are not equally certain
//!
//! Steam publishes an app-id for every game it owns or wishlists. So a Steam claim is
//! **identity** — the same number on both sides, and nothing to get wrong.
//!
//! Getting a number for the other side is the part that varies. Fanatical publishes Steam ids for
//! what it sells; **Humble publishes none at all**, so for a Humble game the id comes from
//! [`steam_inventory::of_game`] instead — the same resolution that already decided which reviews,
//! which tags and which store link that row carries. A mark built on it is exactly as certain as
//! the rest of the line it sits on, which is why it is not hedged.
//!
//! Epic publishes no Steam id at all, only its own `app_name` and a title, so an Epic claim can
//! only ever be **title equality** under [`steam_inventory::comparable`] — the same rule, and the
//! same single failure mode, that `crossover` documents: two genuinely different games sharing a
//! name once punctuation is gone. The titles are matched against the inventory's own canonical
//! name and its aliases wherever an app-id names an entry, because a vendor's marketing wording
//! is the messier of the two strings.
//!
//! The difference is visible in the output rather than buried here: a store matched by title is
//! marked as a guess, and it has to be marked PER STORE because the claims stack — one bracket
//! can carry an identity and a guess at once.
//!
//! # The failure this is built to not have
//!
//! A directory with no snapshots in it and a directory misspelled produce the same answer —
//! nothing owned — and a listing showing no marks looks perfectly healthy either way. So
//! [`Holdings::summary`] names the directory it looked in and every file it did and did not find,
//! for a caller to print above the listing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::Game;
use crate::commands::gamelib::{self, Kind};
use crate::inventory::steam as steam_inventory;

/// A store whose snapshots are read, and how it is named to a reader.
struct Source {
    /// What the snapshot's own `store` field says, and the tail of its file name.
    id: &'static str,
    /// What a reader is shown.
    name: &'static str,
    /// What this store publishes, and therefore what [`crate::commands::gamelib`] can write for it.
    ///
    /// Per store, because they do not publish the same things: Steam has a wishlist and Epic has
    /// none that the tool reading it can see. Naming a file that could never exist among the ones
    /// that are missing would send a reader looking for a fault there is no way to fix.
    kinds: &'static [Kind],
}

/// Every store whose files are read, in the order a reader sees them named.
///
/// This table's order, not the alphabet's: "owned on Steam, Epic" puts the store that answers by
/// app-id before the one that can only be matched by title, so the certain claim comes first.
const SOURCES: &[Source] = &[
    Source {
        id: gamelib::steam::STORE,
        name: "Steam",
        kinds: &Kind::ALL,
    },
    Source {
        id: gamelib::epic::STORE,
        name: "Epic",
        // Library only: `legendary` reads entitlements, and Epic publishes no wishlist through it.
        kinds: &[Kind::Library],
    },
];

/// One game as a snapshot records it — and nothing else it records.
///
/// The fields are named individually rather than reusing [`gamelib::Entry`], and `provenance` is
/// **not declared at all**, which is the point: a struct holding the account handle is one `Debug`
/// derive or one error message away from printing it. What is never declared cannot leak. Same
/// guarantee `bin/epic_lookup.rs` gets, the same way.
#[derive(Debug, Deserialize)]
struct Recorded {
    /// The Steam app-id, where the store publishes one.
    #[serde(default)]
    app_id: Option<u32>,
    /// What the store calls it, where the store publishes titles.
    #[serde(default)]
    name: Option<String>,
}

/// A snapshot, of which only these three parts are read.
#[derive(Debug, Deserialize)]
struct Recording {
    store: String,
    kind: String,
    /// `2026-09-18 01:27:20Z`, of which only the date is shown.
    captured: String,
    games: Vec<Recorded>,
}

/// One store's claim on a game, and how it was arrived at.
///
/// Facts only: the wording and the colouring are [`crate::render`]'s, which is where every other
/// decision about how a game is presented already lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claim {
    /// The store, named as a reader sees it.
    pub store: &'static str,
    /// Whether the match was an identity rather than a guess — the same app-id on both sides,
    /// as against the same title. See the module doc: only Steam can answer this way.
    pub by_id: bool,
}

/// What the operator's own files say about one game.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Standings {
    /// Stores holding a copy, in [`SOURCES`] order.
    pub owned: Vec<Claim>,
    /// Stores with it on a wishlist.
    pub wanted: Vec<Claim>,
}

impl Standings {
    /// Whether any file said anything about this game.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.owned.is_empty() && self.wanted.is_empty()
    }
}

/// Which sources reported something. Indexes into [`SOURCES`], so iteration is the table's order.
type Reporters = BTreeSet<usize>;

/// One standing's index, keyed every way an entry in it can be reached.
///
/// Three maps rather than two, and the split is the whole matching rule. An entry that published
/// an app-id is normally found by that id and never by its title, because a title must not be
/// allowed to contradict an identity: a library holding Prey (2017) must not answer for a bundle
/// selling Prey (2006). But when the game on sale has no id of its own and none can be resolved
/// for it, a title is all there is, and refusing to look would be refusing the only question that
/// can be asked.
#[derive(Debug, Default)]
struct Index {
    /// Entries that published an app-id, by that id.
    by_app_id: BTreeMap<u32, Reporters>,
    /// Entries that published NO app-id — Epic's whole library. Title equality is the only handle
    /// on these, whatever is known about the game on sale.
    idless: BTreeMap<String, Reporters>,
    /// Titles of the entries in `by_app_id`, for the one case that needs them: a game on sale
    /// whose own app-id is unknown, so there is no identity to contradict.
    named: BTreeMap<String, Reporters>,
}

/// One snapshot file that was read.
#[derive(Debug)]
struct Found {
    store: &'static str,
    kind: Kind,
    /// The capture date, so a claim can be read as of when it was true.
    captured: String,
    games: usize,
}

/// Everything the operator's own snapshot files say.
#[derive(Debug, Default)]
pub struct Holdings {
    owned: Index,
    wanted: Index,
    /// Files read, in the order they were looked for.
    found: Vec<Found>,
    /// Where it looked, so a listing with no marks on it can say so.
    directory: PathBuf,
    /// Files that were there and could not be used, worded for a reader.
    ///
    /// Not an error: a listing is still worth printing without these marks on it. But not silence
    /// either, because a file that is present and unreadable is a thing to go and fix.
    pub problems: Vec<String>,
}

impl Holdings {
    /// Reads whichever snapshots are in `directory`.
    ///
    /// A missing file is an ordinary setup — an operator may snapshot one store and not another —
    /// so it is not a problem and not an error. A file that is there and unusable is recorded in
    /// [`Holdings::problems`].
    #[must_use]
    pub fn load(directory: &Path) -> Self {
        let mut holdings = Self {
            directory: directory.to_path_buf(),
            ..Self::default()
        };
        for (at, source) in SOURCES.iter().enumerate() {
            for kind in source.kinds.iter().copied() {
                let path = directory.join(gamelib::file_name(kind, source.id));
                match read(&path, source.id, kind) {
                    Ok(None) => {}
                    Ok(Some(recording)) => holdings.absorb(at, source, kind, recording),
                    Err(problem) => holdings.problems.push(problem),
                }
            }
        }
        holdings
    }

    /// Holdings that say nothing, for a caller that has no files to read.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether a snapshot was read for `store` — its `store` field, as [`SOURCES`] names it.
    ///
    /// For a caller deciding whether a weaker source is still worth consulting: a guess about
    /// somebody's account is worth nothing once that account's own file is in hand.
    #[must_use]
    pub fn read_for(&self, store: &str) -> bool {
        self.found.iter().any(|found| found.store == store)
    }

    /// What the files say about a game on sale.
    #[must_use]
    pub fn of(&self, game: &Game) -> Standings {
        Standings {
            owned: self.claims_in(&self.owned, game),
            wanted: self.claims_in(&self.wanted, game),
        }
    }

    /// Where it looked and what it found, as one line to print above a listing.
    ///
    /// Names every file it did NOT find as well, because that is the difference between "you own
    /// none of these" and "the directory is wrong" — and with no marks on the listing those two
    /// look identical. Carries each capture date for the same reason the review columns are read
    /// as of their capture: a claim is about the world as it was when the snapshot was taken.
    #[must_use]
    pub fn summary(&self) -> String {
        let where_it_looked = self.directory.display();
        if self.found.is_empty() {
            return format!(
                "no library or wishlist files in {where_it_looked}, so nothing is marked as \
                 owned or wanted — `gamelib` writes them, and `--account-files` says where"
            );
        }
        let each: Vec<String> = self
            .found
            .iter()
            .map(|found| {
                format!(
                    "{} {} ({}, {})",
                    store_name(found.store),
                    found.kind,
                    counted(found.games),
                    date_of(&found.captured)
                )
            })
            .collect();
        let mut line = format!("your own files in {where_it_looked}: {}", each.join(", "));
        let missing = self.missing();
        if !missing.is_empty() {
            let _ = std::fmt::Write::write_fmt(
                &mut line,
                format_args!("; no {}", missing.join(", no ")),
            );
        }
        line
    }

    /// The snapshots a store publishes that this directory did not have, named for a reader.
    fn missing(&self) -> Vec<String> {
        let mut missing = Vec::new();
        for source in SOURCES {
            for kind in source.kinds.iter().copied() {
                let held = self
                    .found
                    .iter()
                    .any(|found| found.store == source.id && found.kind == kind);
                if !held {
                    missing.push(format!("{} {kind}", source.name));
                }
            }
        }
        missing
    }

    /// Folds one file's entries into the index for its kind.
    fn absorb(&mut self, at: usize, source: &Source, kind: Kind, recording: Recording) {
        self.found.push(Found {
            store: source.id,
            kind,
            captured: recording.captured,
            games: recording.games.len(),
        });
        let index = match kind {
            Kind::Library => &mut self.owned,
            Kind::Wishlist => &mut self.wanted,
        };
        for game in &recording.games {
            // An entry whose title reduces to nothing — punctuation only — would key every
            // untitled game to itself, so it is indexed by whatever else it has.
            let key = game
                .name
                .as_deref()
                .map(steam_inventory::comparable)
                .filter(|key| !key.is_empty());
            match (game.app_id, key) {
                (Some(app_id), key) => {
                    index.by_app_id.entry(app_id).or_default().insert(at);
                    if let Some(key) = key {
                        index.named.entry(key).or_default().insert(at);
                    }
                }
                (None, Some(key)) => {
                    index.idless.entry(key).or_default().insert(at);
                }
                (None, None) => {}
            }
        }
    }

    /// Which stores claim this game in one index, and how each of them arrived at it.
    fn claims_in(&self, index: &Index, game: &Game) -> Vec<Claim> {
        // One slot per source, so the answer comes out in the table's order by construction, and
        // a store that answers both ways is one claim rather than two.
        let mut how: Vec<Option<bool>> = vec![None; SOURCES.len()];
        let mut mark = |reporters: Option<&Reporters>, by_id: bool| {
            for at in reporters.into_iter().flatten() {
                // An identity already found beats a title match, never the other way round.
                how[*at] = Some(by_id || how[*at] == Some(true));
            }
        };
        let known = identified(game);
        if let Some(app_id) = known {
            mark(index.by_app_id.get(&app_id), true);
        }
        for title in titles(game) {
            let key = steam_inventory::comparable(title);
            // Always: nothing else can reach an entry that carries no id.
            mark(index.idless.get(&key), false);
            // Only when there is no identity for a title to contradict.
            if known.is_none() {
                mark(index.named.get(&key), false);
            }
        }
        how.iter()
            .enumerate()
            .filter_map(|(at, how)| {
                how.map(|by_id| Claim {
                    store: SOURCES[at].name,
                    by_id,
                })
            })
            .collect()
    }
}

/// The Steam app-id this game is known by, whether or not the store selling it published one.
///
/// **This is what a Humble listing turns on.** Humble publishes no Steam ids at all, so before
/// this existed every Humble row fell through to a title comparison and a library entry indexed
/// by its id was unreachable — a game sitting in the Steam library would be reported as owned on
/// Epic and nowhere else. The inventory is the join that supplies the missing number, and it is
/// the same join the row's reviews, tags and store link already came through.
fn identified(game: &Game) -> Option<u32> {
    steam_inventory::app_id_of(game)
}

/// The names a game on sale might be recorded under, best first.
///
/// An inventory entry's name came from Steam's own store page, which is a better thing to match a
/// title against than a vendor's marketing wording — and the aliases carry the year-disambiguated
/// spellings another store prints bare. The vendor's own wording is what is left when the game
/// resolves to no entry at all.
fn titles(game: &Game) -> Vec<&str> {
    match steam_inventory::of_game(game) {
        Some(entry) => steam_inventory::names(entry).collect(),
        None => vec![game.title.as_str()],
    }
}

/// `1 game`, `2 games`.
///
/// Spelled out rather than the `count(s)` this program writes elsewhere, because this one sits
/// inside parentheses already and `(1 game(s), 2026-09-18)` is a worse thing to read than the
/// four lines it takes to avoid.
fn counted(games: usize) -> String {
    match games {
        1 => "1 game".to_owned(),
        many => format!("{many} games"),
    }
}

/// `SOURCES`' display name for a store id, or the id itself if it somehow names no source.
fn store_name(id: &str) -> &str {
    SOURCES
        .iter()
        .find(|source| source.id == id)
        .map_or(id, |source| source.name)
}

/// The date out of a snapshot's `YYYY-MM-DD HH:MM:SSZ`, or the whole thing if it is shaped
/// otherwise — a capture stamp worth showing is better shown oddly than dropped.
fn date_of(captured: &str) -> &str {
    captured.split_whitespace().next().unwrap_or(captured)
}

/// The snapshot at `path`, or `None` when there is no file there.
///
/// The file is checked against what its name promised. A Steam snapshot read as an Epic one would
/// mark the wrong store on every row, and a renamed or hand-moved file is the way that arrives.
fn read(path: &Path, store: &str, kind: Kind) -> Result<Option<Recording>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{} could not be read: {error}", path.display())),
    };
    let recording: Recording = serde_json::from_str(&text).map_err(|error| {
        format!(
            "{} is not a snapshot this version can read: {error}",
            path.display()
        )
    })?;
    if recording.store != store || recording.kind != kind.to_string() {
        return Err(format!(
            "{} says it holds a {} {} snapshot, not a {store} {kind} one — it is not being read",
            path.display(),
            recording.store,
            recording.kind
        ));
    }
    Ok(Some(recording))
}
