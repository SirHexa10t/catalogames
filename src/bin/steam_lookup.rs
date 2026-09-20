//! Resolves bundle game titles to their Steam pages and prints
//! `src/inventory/steam/regular.rs`.
//!
//! **Not part of the library.** This is a maintenance tool: it is built only under the `tools`
//! feature, runs rarely, and costs roughly four requests per game. Nothing here is reachable
//! from `catalogames` as a dependency, which is deliberate — a library should not carry the
//! machinery that generated its data.
//!
//! Two ways in. Given no argument it resolves the games Humble is currently bundling, by
//! searching Steam for each title — the fragile half, and the reason this is not in the library.
//! Given a listing this crate printed, it takes the app-ids already in it and looks each up
//! directly, with no searching at all:
//!
//! ```text
//! cargo run --release --features tools --bin steam_lookup > src/inventory/steam/regular.rs
//! cargo run --release --features tools --bin steam_lookup -- listing.txt > entries.rs
//! ```
//!
//! Several sources may be named at once, and should be: the table is one table — `regular`
//! means games on sale today, whichever store is selling them — so emitting one source alone
//! would drop every entry the others found.
//!
//! ```text
//! cargo run --release --features tools --bin steam_lookup -- humble fanatical.txt \
//!     > src/inventory/steam/regular.rs
//! ```
//!
//! It prints to stdout and never writes in place. The generated table holds *curated* entries:
//! reading the diff is the only thing that catches a title resolved to the wrong app-id, so the
//! workflow deliberately forces a human past it.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use catalogames::inventory::steam::{Tag, comparable, source};
use catalogames::steam::store::{self, AppDetails, AppKind, GameDetails};
use serde::Deserialize;

/// Identifies the tool rather than the library, so Steam's logs say which thing called.
const USER_AGENT: &str = concat!("catalogames-steam-lookup/", env!("CARGO_PKG_VERSION"));

/// Pause between requests.
///
/// Steam publishes no rate limit and none is probed for here — establishing one means tripping
/// it. One request per second is slower than a person browsing and is the defensible floor.
const REQUEST_SPACING: Duration = Duration::from_secs(1);

/// How many search hits are worth confirming with `appdetails`.
///
/// Steam orders results by relevance; past a handful they are other games in the franchise.
const CANDIDATES_EXAMINED: usize = 5;

// ---------------------------------------------------------------------------------------------
// The lookup
// ---------------------------------------------------------------------------------------------

/// What a title resolved to.
enum Outcome {
    /// One base game, confidently.
    Found(Box<Resolved>),
    /// A base game, but the generator could not be sure it is the right one.
    Uncertain(Box<Resolved>, String),
    /// Nothing on Steam is this title — a coupon, a course, a DLC, or a bundle-only name.
    Absent(String),
}

/// One resolved game, ready to print: what Steam says about it, plus what only this generator
/// knows.
struct Resolved {
    details: GameDetails,
    /// The bundle's own wording for an edition, when it differs from Steam's — see `find_game`.
    aliases: Vec<String>,
    /// The tags as enum variants. The library hands them back as Steam's strings so a tag Valve
    /// adds cannot break a runtime fetch; *here*, writing a `const` table, an unknown tag is a
    /// failure — a silently shorter tag list looks exactly like a game nobody tagged that way.
    tags: Vec<Tag>,
}

/// Maps Steam's tag names onto the enum, refusing any the enum does not know.
fn tag_variants(names: &[String]) -> Result<Vec<Tag>, String> {
    names
        .iter()
        .map(|name| {
            Tag::from_name(name).ok_or_else(|| {
                format!("unknown tag {name:?} — regenerate tags.rs with `steam_tags`")
            })
        })
        .collect()
}

/// Finds the Steam game a bundle calls `title`.
///
/// The search is deliberately loose and the *confirmation* strict: Steam's search is fragile in
/// ways that have nothing to do with the game (see [`searchable`]), while `appdetails` states
/// plainly whether an app-id is a base game. So the shape is search wide, then verify — never
/// guess from the title which results to discard.
///
/// Ambiguity is reported, not resolved. `type == "game"` rules out DLC, demos and soundtracks,
/// but not a Game of the Year edition, a regional re-release, or another entry in the same
/// franchise — all of which pass every automatic check and produce a plausible row pointing at
/// the wrong game. Those come back as [`Outcome::Uncertain`] for a human to settle.
fn find_game(net: &mut Net, title: &str) -> Result<Outcome, String> {
    let mut searched_as = title.to_owned();
    let mut hits = search(net, &searchable(title))?;

    // Nothing under the store's own wording: try it without its edition phrase.
    if hits.is_empty() {
        for variant in edition_variants(title) {
            let attempt = search(net, &searchable(&variant))?;
            if !attempt.is_empty() {
                searched_as = variant;
                hits = attempt;
                break;
            }
        }
    }

    let mut games = Vec::new();
    for hit in hits.iter().take(CANDIDATES_EXAMINED) {
        let body = net.get(&store::app_details_url(hit.id), false)?;
        // A search hit Steam no longer has details for — delisted, region-locked — is skipped,
        // not fatal: it is one candidate among several, not the answer.
        let Some(details) = store::parse_app_details(&body, hit.id).map_err(|e| e.to_string())?
        else {
            continue;
        };
        if details.kind == AppKind::Game {
            games.push(details);
        }
    }

    let exact: Vec<&AppDetails> = games
        .iter()
        .filter(|game| comparable(&game.name) == comparable(&searched_as))
        .collect();

    let (chosen, note) = match (exact.as_slice(), games.as_slice()) {
        ([one], _) => (*one, None),
        ([first, rest @ ..], _) => (
            *first,
            Some(format!(
                "{} titles match exactly; took {}. Others: {}",
                rest.len() + 1,
                first.app_id,
                rest.iter()
                    .map(|g| g.app_id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        ),
        ([], [only]) => (
            only,
            Some(format!(
                "Steam calls this {:?}, the bundle calls it {title:?}",
                only.name
            )),
        ),
        ([], [first, rest @ ..]) => (
            first,
            Some(format!(
                "no exact match for {title:?}; took {} ({:?}). Others: {}",
                first.app_id,
                first.name,
                rest.iter()
                    .map(|g| format!("{} ({:?})", g.app_id, g.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        ),
        ([], []) => {
            return Ok(Outcome::Absent(if hits.is_empty() {
                "no search results".to_owned()
            } else {
                format!("{} result(s), none a base game", hits.len())
            }));
        }
    };

    let html = net.get(&store::page_url(chosen.app_id), true)?;
    let page = store::parse_store_page(&html, chosen.app_id).map_err(|e| e.to_string())?;

    // The store's wording differed from whatever actually matched. Record it as an alias only
    // if this product really is sold under it — otherwise the shortened search found something
    // else, and saying so is more useful than a confident wrong alias.
    let mut note = note;
    let mut aliases = Vec::new();
    if comparable(&searched_as) != comparable(title) {
        if sells_edition(chosen, title) || names_product(&html, title) {
            aliases.push(title.to_owned());
        } else {
            note = Some(format!(
                "searched {searched_as:?} after dropping an edition phrase from {title:?}, \
                 but app {} does not offer that edition",
                chosen.app_id
            ));
        }
    }

    let app_id = chosen.app_id;
    let all_time = store::parse_reviews(&net.get(&store::reviews_url(app_id), false)?, app_id)
        .map_err(|e| e.to_string())?;
    let deck = store::parse_deck(&net.get(&store::deck_url(app_id), false)?, app_id)
        .map_err(|e| e.to_string())?;
    let details = store::assemble(chosen.clone(), page, all_time, deck);
    let tags = tag_variants(&details.tags)?;
    let game = Resolved {
        details,
        aliases,
        tags,
    };

    Ok(match note {
        Some(note) => Outcome::Uncertain(Box::new(game), note),
        None => Outcome::Found(Box::new(game)),
    })
}

/// Words that end an edition phrase, in the wording stores actually use.
const EDITION_WORDS: &[&str] = &["edition", "bundle", "collection", "cut", "pack"];

/// Progressively shorter forms of an edition-suffixed title, longest first.
///
/// A store sells "Frostpunk: Game of the Year edition"; Steam has no such app — it sells that
/// edition as a *bundle* over the base game's app-id, so the only findable product is
/// "Frostpunk". Since the edition phrase is a variable number of words ("Deluxe Edition" is
/// one, "Game of the Year Edition" is four), the phrase boundary is not guessed: each cut point
/// before the edition word is tried in turn and the first that finds anything wins.
///
/// Guessing wrongly is cheap because the result is confirmed afterwards — see [`names_product`].
fn edition_variants(title: &str) -> Vec<String> {
    let words: Vec<&str> = title.split_whitespace().collect();
    let Some(last) = words
        .iter()
        .rposition(|w| EDITION_WORDS.contains(&trim_word(w).to_lowercase().as_str()))
    else {
        return Vec::new();
    };

    // Cut before the edition word, then progressively earlier — but never down to nothing, and
    // never more than a handful of attempts, each of which costs a request.
    (last.saturating_sub(4).max(1)..=last)
        .rev()
        .map(|cut| trim_separators(&words[..cut].join(" ")))
        .filter(|variant| !variant.is_empty())
        .collect()
}

fn trim_word(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_alphanumeric())
}

/// Drops a separator left dangling by a cut, e.g. `"Frostpunk:"` -> `"Frostpunk"`.
fn trim_separators(text: &str) -> String {
    text.trim()
        .trim_end_matches([':', '-', '\u{2013}', '\u{2014}', ','])
        .trim()
        .to_owned()
}

/// Whether a store page names `title` anywhere on it.
///
/// This is what makes the edition fallback safe. Searching the shortened "Frostpunk" returns
/// "Frostpunk 2" as its *first* result, so position proves nothing — but the base game's page
/// carries the words "Frostpunk: Game of the Year Edition" (Steam sells it there as a bundle)
/// and Frostpunk 2's page does not. Requiring the page to name the edition turns a guess into a
/// confirmation.
///
/// Both sides are normalised, so the page's markup and entities cannot hide a match.
///
/// The search is anchored to the parts of the page that name *this* product for sale — the
/// purchase area and the bundle constructors in it. A free-text search of the whole page would
/// also read recommendation carousels, franchise blurbs and other products' bundles, any of
/// which can mention an edition belonging to a different game.
fn names_product(html: &str, title: &str) -> bool {
    let wanted = edition_key(title);
    ANCHORS.iter().any(|anchor| {
        html.match_indices(anchor)
            .any(|(at, _)| edition_key(window(html, at, ANCHOR_WINDOW)).contains(&wanted))
    })
}

/// A title reduced for comparing two spellings of the same edition.
///
/// Beyond [`comparable`], this drops trailing edition words, because stores and Steam disagree
/// about which one to use for the same product: a store's "Endzone 2 Deluxe **Edition**" is
/// Steam's "Endzone 2 Deluxe **Bundle**", and "TerraTech: Prospector **Edition**" is offered as
/// "TerraTech - Prospector". Comparing the words that identify the edition, and not the noun
/// naming how it is packaged, matches those; exact containment rejects about a third of real
/// cases.
fn edition_key(title: &str) -> String {
    let mut words: Vec<&str> = title.split_whitespace().collect();
    while words
        .last()
        .is_some_and(|w| EDITION_WORDS.contains(&trim_word(w).to_lowercase().as_str()))
    {
        words.pop();
    }
    comparable(&words.join(" "))
}

/// Whether this product is sold under `title` as one of its purchase options.
///
/// Checked before the page because it costs nothing — the options arrive with `appdetails`.
/// Only packages appear here; editions Steam sells as a store *bundle* do not, which is the
/// more common shape, so a miss here is not an answer.
fn sells_edition(details: &AppDetails, title: &str) -> bool {
    let wanted = edition_key(title);
    details
        .package_options
        .iter()
        .any(|option| edition_key(option) == wanted)
}

/// Markers that introduce a purchasable product's own name.
const ANCHORS: &[&str] = &[
    "game_area_purchase",
    "AddFreeBundle(",
    "bundle_purchase_label",
];

/// How much text after an anchor can still be that product's name.
const ANCHOR_WINDOW: usize = 4_096;

/// `len` bytes of `text` from `at`, trimmed back to a character boundary.
fn window(text: &str, at: usize, len: usize) -> &str {
    let mut end = (at + len).min(text.len());
    while end > at && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[at..end]
}

/// Rewrites a title into something Steam's search can actually match.
///
/// A dash surrounded by spaces returns **zero** results, whatever sits either side of it, and
/// Steam's own DLC names are overwhelmingly `Game - Something` — which is why searching for a
/// bundle's exact wording so often finds nothing. Measured, holding one variable at a time:
///
/// ```text
/// "Warhammer 40,000: Rogue Trader - Season Pass"   0 results
/// "Warhammer 40,000: Rogue Trader Season Pass"     2 results   (only " - " removed)
/// "Rogue Trader: Season Pass"                      2 results   (a colon is harmless)
/// "Rogue Trader – Season Pass"                     0 results   (an en-dash is equally fatal)
/// "Season Pass"                                   10 results   (the words are not the problem)
/// "Half-Life 2"                                   10 results   (a hyphen inside a word is fine)
/// ```
///
/// So the separator is what gets normalised, and the words never are. Dropping titles
/// containing "DLC" or "Pack" — the obvious reading of the symptom — would both miss the cause
/// and throw away games that are perfectly findable.
fn searchable(title: &str) -> String {
    title
        .replace(" - ", " ")
        .replace(" \u{2013} ", " ")
        .replace(" \u{2014} ", " ")
}

// ---------------------------------------------------------------------------------------------
// Steam endpoints
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct Hit {
    id: u32,
    #[allow(dead_code)]
    name: String,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    items: Vec<Hit>,
}

fn search(net: &mut Net, term: &str) -> Result<Vec<Hit>, String> {
    let url = format!(
        "https://store.steampowered.com/api/storesearch/?cc=US&l=en&term={}",
        urlencode(term)
    );
    let body = net.get(&url, false)?;
    let parsed: SearchResponse =
        serde_json::from_str(&body).map_err(|e| format!("search response: {e}"))?;
    Ok(parsed.items)
}

fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Network, with a disk cache
// ---------------------------------------------------------------------------------------------

/// HTTP with an on-disk cache.
///
/// The cache is what makes a ~560-request run survivable: a throttle at request 400 would
/// otherwise cost the first 399, and a parse bug found next month can be fixed against saved
/// responses instead of fetching the world again. Steam itself sends
/// `Cache-Control: public,max-age=3600` on `appdetails`, so keeping copies is what it asks for.
///
/// The talking to Steam itself — headers, the age gate, throttle handling — is not repeated
/// here. That lives once, in the library's [`store::Client`]; this only decides whether to ask
/// it, and paces the asks.
struct Net {
    client: store::Client,
    cache: PathBuf,
    fetched: usize,
    served_from_cache: usize,
}

impl Net {
    fn new(cache: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&cache).map_err(|e| format!("cache dir {}: {e}", cache.display()))?;
        // Zero spacing inside the client: pacing belongs here, where a cache hit costs nothing
        // and should not wait for anything.
        let client = store::Client::configure(USER_AGENT, Duration::ZERO, true)
            .map_err(|e| format!("http client: {e}"))?;
        Ok(Self {
            client,
            cache,
            fetched: 0,
            served_from_cache: 0,
        })
    }

    fn get(&mut self, url: &str, age_gated: bool) -> Result<String, String> {
        let path = self.cache.join(format!("{:016x}", fnv1a(url)));
        if let Ok(cached) = fs::read_to_string(&path) {
            self.served_from_cache += 1;
            return Ok(cached);
        }

        if self.fetched > 0 {
            std::thread::sleep(REQUEST_SPACING);
        }
        self.fetched += 1;

        let body = self.client.get(url, age_gated).map_err(|e| e.to_string())?;
        let _ = fs::write(&path, &body);
        Ok(body)
    }
}

/// FNV-1a, for naming cache files after their URL. Not a security hash — it only has to avoid
/// collisions between a few hundred URLs and produce a valid filename.
fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

// ---------------------------------------------------------------------------------------------
// Emitting the table
// ---------------------------------------------------------------------------------------------

/// The whole generated module: header, capture date, and the table of entries.
fn emit(games: &[(Resolved, Option<String>)], captured: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "//! Games on sale in the bundles catalogames tracks, resolved to their Steam pages.\n\
         //!\n\
         //! GENERATED — do not hand-edit; regenerate and review the diff:\n\
         //!\n\
         //! ```text\n\
         //! cargo run --release --features tools --bin steam_lookup > \
         src/inventory/steam/regular.rs\n\
         //! ```\n\
         //!\n\
         //! Hand edits are lost on the next run. A wrong entry is fixed by fixing the \
         generator, or\n\
         //! by moving the entry to a hand-maintained module. Lines marked `REVIEW:` are ones \
         the\n\
         //! generator could not settle on its own — a human decides those.\n\n",
    );
    out.push_str("use super::{Deck, Os, Reviews, SteamGame, Tag, Vr};\n\n");
    let _ = writeln!(
        out,
        "/// When these entries were captured. See the staleness house rule in [`super`].\n\
         pub const CAPTURED: &str = {};\n",
        source::literal(captured)
    );
    // `rustfmt::skip` keeps this table in the shape the generator prints it. Without it
    // `cargo fmt` puts every tag on its own line — 880 lines become 3,557, the house style's
    // compact one-entry-per-line table is lost, and worse, regenerating no longer reproduces
    // the committed file, so every diff is swamped by reformatting.
    out.push_str(
        "/// Every base game the tracked bundles currently offer, sorted by app-id.\n\
         #[rustfmt::skip]\n\
         pub const REGULAR: &[SteamGame] = &[\n",
    );

    for (game, note) in games {
        if let Some(note) = note {
            let _ = writeln!(out, "    // REVIEW: {note}");
        }
        out.push_str(&source::entry(&game.details, &game.aliases, &game.tags));
    }
    out.push_str("];\n");
    out
}

// ---------------------------------------------------------------------------------------------

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("steam_lookup: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// One app-id to resolve, and the name the source listing gave it.
struct Requested {
    app_id: u32,
    /// What the store called it — kept to cross-check against what Steam calls it.
    name: String,
}

/// Reads app-ids out of a listing this crate printed.
///
/// The listing's own format is the input: every game line ends with its store link, and a link
/// to `/app/<id>` is an app-id the store vouched for. Lines ending in a search link carry no id
/// — a multi-game pack that is not one Steam product — and are passed over.
fn requested_from_listing(text: &str) -> Vec<Requested> {
    text.lines()
        .filter_map(|line| {
            let (name, link) = line.trim_start().strip_prefix("- ")?.rsplit_once("  ")?;
            let app_id = catalogames::steam::app_id_from_url(link.trim())?;
            Some(Requested {
                app_id,
                // The middle columns (review verdicts) are dropped: only the first is the name.
                name: name.split("  ").next()?.trim().to_owned(),
            })
        })
        .collect()
}

/// Resolves one app-id straight to an entry.
///
/// No searching, no title matching, none of the machinery [`find_game`] needs — the id is given.
/// What remains is a check the id is worth making: a store can put something that is *not* an
/// app-id in an app-id field, and four of the ids in the listing this was written for turned out
/// to be Steam **package** ids that no app answers to.
fn resolve_app_id(net: &mut Net, requested: &Requested) -> Result<Outcome, String> {
    let app_id = requested.app_id;
    let body = net.get(&store::app_details_url(app_id), false)?;
    let Some(details) = store::parse_app_details(&body, app_id).map_err(|e| e.to_string())? else {
        return Ok(Outcome::Absent(format!(
            "Steam has no app {app_id} — the listing's id is not an app-id (a package id, most \
             likely)"
        )));
    };

    let page = store::parse_store_page(&net.get(&store::page_url(app_id), true)?, app_id)
        .map_err(|e| e.to_string())?;
    let all_time = store::parse_reviews(&net.get(&store::reviews_url(app_id), false)?, app_id)
        .map_err(|e| e.to_string())?;
    let deck = store::parse_deck(&net.get(&store::deck_url(app_id), false)?, app_id)
        .map_err(|e| e.to_string())?;
    let resolved_details = store::assemble(details, page, all_time, deck);
    let tags = tag_variants(&resolved_details.tags)?;

    // The store named this app; Steam names it something else. Usually an edition wording the
    // store uses and Steam does not, in which case the store's wording is worth keeping as an
    // alias. But it is also exactly what a wrong id looks like, so it is flagged either way and
    // a human reads the diff.
    let differs = edition_key(&resolved_details.name) != edition_key(&requested.name);
    let note = differs.then(|| {
        format!(
            "listing calls this {:?}, Steam calls it {:?}",
            requested.name, resolved_details.name
        )
    });
    let game = Resolved {
        details: resolved_details,
        aliases: if differs {
            vec![requested.name.clone()]
        } else {
            Vec::new()
        },
        tags,
    };

    Ok(match note {
        Some(note) => Outcome::Uncertain(Box::new(game), note),
        None => Outcome::Found(Box::new(game)),
    })
}

/// Names the Humble resolution rather than a file.
const HUMBLE_SOURCE: &str = "humble";

/// Collapses entries two sources both found, keeping what each of them knew.
///
/// Plain deduplication would keep whichever came first and silently drop the other's alias —
/// and an alias is exactly what a second store contributes, since it is the wording *that*
/// store uses. Requires `found` to be sorted by app-id.
fn merge_duplicates(found: &mut Vec<(Resolved, Option<String>)>) {
    let mut merged: Vec<(Resolved, Option<String>)> = Vec::with_capacity(found.len());
    for (game, note) in found.drain(..) {
        match merged.last_mut() {
            Some((kept, kept_note)) if kept.details.app_id == game.details.app_id => {
                for alias in game.aliases {
                    if !kept.aliases.contains(&alias)
                        && !alias.eq_ignore_ascii_case(&kept.details.name)
                    {
                        kept.aliases.push(alias);
                    }
                }
                // A doubt either source had is a doubt about the entry.
                if kept_note.is_none() {
                    *kept_note = note;
                }
            }
            _ => merged.push((game, note)),
        }
    }
    *found = merged;
}

/// What one pass produced: entries, titles that are not on Steam, and outright failures.
type Harvest = (
    Vec<(Resolved, Option<String>)>,
    Vec<(String, String)>,
    Vec<(String, String)>,
);

/// Resolves the app-ids a previously-printed listing names.
fn from_listing(net: &mut Net, path: &str) -> Result<Harvest, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut requested = requested_from_listing(&text);
    requested.sort_by_key(|r| r.app_id);
    requested.dedup_by_key(|r| r.app_id);
    eprintln!("{} distinct app-ids in {path}\n", requested.len());

    let (mut found, mut skipped, mut failed) = (Vec::new(), Vec::new(), Vec::new());
    for (index, request) in requested.iter().enumerate() {
        eprint!(
            "[{:>3}/{}] {} ({}) … ",
            index + 1,
            requested.len(),
            request.name,
            request.app_id
        );
        match resolve_app_id(net, request) {
            Ok(Outcome::Found(game)) => {
                eprintln!("{} tag(s)", game.tags.len());
                found.push((*game, None));
            }
            Ok(Outcome::Uncertain(game, note)) => {
                eprintln!("REVIEW: {note}");
                found.push((*game, Some(note)));
            }
            Ok(Outcome::Absent(why)) => {
                eprintln!("skipped — {why}");
                skipped.push((request.name.clone(), why));
            }
            Err(error) => {
                eprintln!("FAILED — {error}");
                failed.push((request.name.clone(), error));
            }
        }
    }
    Ok((found, skipped, failed))
}

/// Resolves the games Humble is currently bundling, by name.
fn from_humble(net: &mut Net) -> Result<Harvest, String> {
    eprintln!("listing bundles from Humble…");
    let listing = catalogames::commands::sales::humble::Client::new()
        .map_err(|e| e.to_string())?
        .list_bundles()
        .map_err(|e| e.to_string())?;
    for problem in &listing.problems {
        eprintln!("  humble: {problem}");
    }

    // One lookup per distinct title: bundles repeat games, and one bundle deliberately sells two
    // copies of each of its games.
    let titles: BTreeMap<String, ()> = listing
        .bundles
        .iter()
        .flat_map(|bundle| bundle.games.iter())
        .map(|game| (game.title.clone(), ()))
        .collect();
    eprintln!("{} distinct titles to resolve\n", titles.len());

    let (mut found, mut skipped, mut failed) = (Vec::new(), Vec::new(), Vec::new());
    for (index, title) in titles.keys().enumerate() {
        eprint!("[{:>3}/{}] {title} … ", index + 1, titles.len());
        match find_game(net, title) {
            Ok(Outcome::Found(game)) => {
                eprintln!("{} ({})", game.details.app_id, game.tags.len());
                found.push((*game, None));
            }
            Ok(Outcome::Uncertain(game, note)) => {
                eprintln!("{} REVIEW: {note}", game.details.app_id);
                found.push((*game, Some(note)));
            }
            Ok(Outcome::Absent(why)) => {
                eprintln!("skipped — {why}");
                skipped.push((title.clone(), why));
            }
            Err(error) => {
                eprintln!("FAILED — {error}");
                failed.push((title.clone(), error));
            }
        }
    }
    Ok((found, skipped, failed))
}

fn run() -> Result<(), String> {
    let cache = std::env::var("CATALOGAMES_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("catalogames-steam-cache"));
    eprintln!("cache: {}", cache.display());
    let mut net = Net::new(cache)?;

    // Sources differ in the only part that is ever wrong: where the app-id came from. A
    // listing's ids are already known and are taken at their word (checked, but not searched
    // for); `humble` resolves them from titles by search, which is the fragile half this tool
    // exists to keep out of the library.
    //
    // Several may be named, because the table is one table: `regular` means games on sale
    // today, whichever store happens to be selling them. Emitting one source alone would drop
    // every entry the others found.
    let mut sources: Vec<String> = std::env::args().skip(1).collect();
    if sources.is_empty() {
        sources.push(HUMBLE_SOURCE.to_owned());
    }

    let (mut found, mut skipped, mut failed) = (Vec::new(), Vec::new(), Vec::new());
    for source in &sources {
        let (mut f, mut s, mut e) = if source == HUMBLE_SOURCE {
            from_humble(&mut net)?
        } else {
            from_listing(&mut net, source)?
        };
        found.append(&mut f);
        skipped.append(&mut s);
        failed.append(&mut e);
    }

    // Sorted by app-id so that regenerating produces a diff a human can read, rather than a
    // reshuffled file, and so that the same game found through two stores collapses to one row.
    found.sort_by_key(|(game, _)| game.details.app_id);
    merge_duplicates(&mut found);

    let captured =
        std::env::var("CATALOGAMES_CAPTURED_ON").unwrap_or_else(|_| "unknown".to_owned());
    print!("{}", emit(&found, &captured));

    eprintln!(
        "\n{} entries, {} skipped, {} failed",
        found.len(),
        skipped.len(),
        failed.len()
    );
    eprintln!(
        "{} requests made, {} served from cache",
        net.fetched, net.served_from_cache
    );
    for (title, why) in &skipped {
        eprintln!("  skipped: {title} — {why}");
    }
    for (title, why) in &failed {
        eprintln!("  FAILED:  {title} — {why}");
    }
    let review_count = found.iter().filter(|(_, note)| note.is_some()).count();
    if review_count > 0 {
        eprintln!("\n{review_count} entr(ies) marked REVIEW: — read those in the diff.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use catalogames::inventory::steam::{Deck, Os, Reviews, Vr};

    fn resolved(details: GameDetails, aliases: &[&str], tags: Vec<Tag>) -> Resolved {
        Resolved {
            details,
            aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
            tags,
        }
    }

    /// The search rewrite, pinned against the measurements in [`searchable`]'s documentation.
    #[test]
    fn a_spaced_dash_is_removed_because_steam_returns_nothing_for_one() {
        assert_eq!(
            searchable("Warhammer 40,000: Rogue Trader - Season Pass"),
            "Warhammer 40,000: Rogue Trader Season Pass"
        );
        assert_eq!(
            searchable("Rogue Trader \u{2013} Season Pass"),
            "Rogue Trader Season Pass"
        );
        assert_eq!(
            searchable("Rogue Trader \u{2014} Season Pass"),
            "Rogue Trader Season Pass"
        );
    }

    #[test]
    fn a_hyphen_inside_a_word_is_left_alone() {
        // "Half-Life 2" searches perfectly well; breaking it would lose the game.
        assert_eq!(searchable("Half-Life 2"), "Half-Life 2");
        assert_eq!(searchable("Spider-Man"), "Spider-Man");
    }

    #[test]
    fn the_words_dlc_and_pack_are_never_touched() {
        // They are not the cause — "Season Pass" alone returns ten results. Filtering on them
        // would discard findable games.
        assert_eq!(
            searchable("Mega Man Legacy Collection"),
            "Mega Man Legacy Collection"
        );
        assert_eq!(searchable("Deluxe Pack"), "Deluxe Pack");
        assert_eq!(searchable("DLC Pack"), "DLC Pack");
    }

    #[test]
    fn titles_compare_across_punctuation_spacing_and_case() {
        // A real pair from the data: the bundle writes "Vol.2", Steam writes "Vol. 2".
        assert_eq!(
            comparable("Mega Man Battle Network Legacy Collection Vol.2"),
            comparable("Mega Man Battle Network Legacy Collection Vol. 2"),
        );
        assert_eq!(
            comparable("Warhammer 40,000: Rogue Trader"),
            comparable("warhammer 40000 rogue trader"),
        );
    }

    #[test]
    fn different_games_stay_different() {
        // The comparison decides which app-id an entry gets, so it must not collapse
        // neighbouring titles in a series.
        assert_ne!(comparable("Torchlight II"), comparable("Torchlight III"));
        assert_ne!(
            comparable("Salt and Sanctuary"),
            comparable("Salt and Sacrifice")
        );
        assert_ne!(comparable("Mega Man 11"), comparable("Mega Man 1"));
    }

    #[test]
    fn urls_escape_everything_outside_the_unreserved_set() {
        assert_eq!(urlencode("Salt and Sanctuary"), "Salt%20and%20Sanctuary");
        assert_eq!(urlencode("Warhammer 40,000"), "Warhammer%2040%2C000");
        assert_eq!(urlencode("A-Z_a.z~0"), "A-Z_a.z~0");
    }

    #[test]
    fn cache_keys_differ_between_urls_and_repeat_for_the_same_one() {
        assert_eq!(
            fnv1a("https://example.test/a"),
            fnv1a("https://example.test/a")
        );
        assert_ne!(
            fnv1a("https://example.test/a"),
            fnv1a("https://example.test/b")
        );
    }

    #[test]
    fn the_emitted_table_is_valid_rust_shaped_output() {
        let details = GameDetails {
            app_id: 283640,
            name: "Salt and Sanctuary".to_owned(),
            kind: AppKind::Game,
            released: "May 17, 2016".to_owned(),
            recent: Some(Reviews {
                approval: 83,
                count: 73,
            }),
            all_time: Reviews {
                approval: 89,
                count: 21898,
            },
            tags: vec!["Souls-like".to_owned(), "Metroidvania".to_owned()],
            os: Os {
                windows: true,
                mac: true,
                linux: true,
            },
            vr: Vr::None,
            deck: Deck::Verified,
            features: vec!["Single-player".to_owned()],
            package_options: Vec::new(),
        };
        let game = resolved(details, &[], vec![Tag::SoulsLike, Tag::Metroidvania]);
        let out = emit(&[(game, None)], "2026-09-11");
        assert!(out.contains("pub const CAPTURED: &str = \"2026-09-11\";"));
        // Without this the formatter rewrites the table and regeneration stops matching the
        // committed file, which breaks the diff review the whole workflow depends on.
        assert!(out.contains("#[rustfmt::skip]\npub const REGULAR"));
        assert!(out.contains("app_id: 283640,"));
        assert!(out.contains("name: \"Salt and Sanctuary\","));
        assert!(out.contains("recent: Some(Reviews { approval: 83, count: 73 }),"));
        assert!(out.contains("all_time: Reviews { approval: 89, count: 21898 },"));
        assert!(out.contains("tags: &[Tag::SoulsLike, Tag::Metroidvania],"));
        assert!(out.contains("os: Os { windows: true, mac: true, linux: true },"));
        assert!(out.contains("vr: Vr::None,"));
        assert!(out.contains("deck: Deck::Verified,"));
        assert!(out.contains("features: &[\"Single-player\"],"));
        assert!(out.contains("aliases: &[],"));
    }

    #[test]
    fn an_uncertain_entry_carries_its_reason_into_the_output() {
        let details = GameDetails {
            app_id: 1,
            name: "A".to_owned(),
            kind: AppKind::Game,
            released: "x".to_owned(),
            recent: None,
            all_time: Reviews {
                approval: 55,
                count: 5,
            },
            tags: vec!["Action".to_owned()],
            os: Os {
                windows: true,
                mac: false,
                linux: false,
            },
            vr: Vr::Only,
            deck: Deck::Unsupported,
            features: Vec::new(),
            package_options: Vec::new(),
        };
        let game = resolved(details, &["An Alias"], vec![Tag::Action]);
        let out = emit(
            &[(game, Some("two exact matches".to_owned()))],
            "2026-01-01",
        );
        assert!(out.contains("// REVIEW: two exact matches"));
    }

    #[test]
    fn an_edition_phrase_is_peeled_back_one_cut_at_a_time() {
        // The phrase is a variable number of words, so the boundary is tried rather than
        // guessed: "Deluxe Edition" is one word of qualifier, "Game of the Year Edition" four.
        let variants = edition_variants("Frostpunk: Game of the Year edition");
        assert_eq!(
            variants.first().map(String::as_str),
            Some("Frostpunk: Game of the Year")
        );
        assert!(variants.contains(&"Frostpunk".to_owned()), "{variants:?}");

        let variants = edition_variants("Per Aspera Deluxe Edition");
        assert!(variants.contains(&"Per Aspera".to_owned()), "{variants:?}");
    }

    #[test]
    fn a_dangling_separator_is_trimmed_from_a_cut_title() {
        // Cutting "Frostpunk: Game of the Year edition" down to one word leaves "Frostpunk:",
        // which Steam's search does not match.
        assert!(
            edition_variants("Frostpunk: Game of the Year edition")
                .iter()
                .all(|v| !v.ends_with(':'))
        );
    }

    #[test]
    fn a_title_with_no_edition_phrase_produces_no_variants() {
        // Without this, every failed lookup would fan out into extra requests.
        assert!(edition_variants("Salt and Sanctuary").is_empty());
        assert!(edition_variants("HAAK").is_empty());
    }

    #[test]
    fn editions_match_across_the_noun_that_names_the_packaging() {
        // Real disagreements between a store's wording and Steam's, all the same product.
        assert_eq!(
            edition_key("Endzone 2 Deluxe Edition"),
            edition_key("Endzone 2 Deluxe Bundle")
        );
        assert_eq!(
            edition_key("TerraTech: Prospector Edition"),
            edition_key("TerraTech - Prospector")
        );
        assert_eq!(
            edition_key("SKALD: Against the Black Priory - Deluxe Bundle"),
            edition_key("SKALD: Against the Black Priory - Deluxe Edition"),
        );
    }

    #[test]
    fn different_editions_of_one_game_do_not_match_each_other() {
        // The tolerance must not go so far that it attaches the wrong edition.
        assert_ne!(
            edition_key("Frostpunk: Deluxe Edition"),
            edition_key("Frostpunk: Game of the Year Edition")
        );
        assert_ne!(
            edition_key("Steelrising - Bastille Edition"),
            edition_key("Steelrising")
        );
    }

    fn entry_for(app_id: u32, name: &str, aliases: &[&str]) -> (Resolved, Option<String>) {
        let details = GameDetails {
            app_id,
            name: name.to_owned(),
            kind: AppKind::Game,
            released: "x".to_owned(),
            recent: None,
            all_time: Reviews {
                approval: 90,
                count: 100,
            },
            tags: Vec::new(),
            os: Os {
                windows: true,
                mac: false,
                linux: false,
            },
            vr: Vr::None,
            deck: Deck::Unknown,
            features: Vec::new(),
            package_options: Vec::new(),
        };
        (resolved(details, aliases, Vec::new()), None)
    }

    #[test]
    fn a_game_two_stores_both_sell_becomes_one_entry_keeping_both_wordings() {
        // An alias is what a second store contributes — its own wording for the same game — so
        // plain deduplication would throw away the very thing the second source was for.
        let mut found = vec![
            entry_for(1, "A Game", &["A Game: Deluxe Edition"]),
            entry_for(1, "A Game", &["A Game Gold"]),
            entry_for(2, "Another", &[]),
        ];
        merge_duplicates(&mut found);

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0.details.app_id, 1);
        assert_eq!(
            found[0].0.aliases,
            ["A Game: Deluxe Edition", "A Game Gold"]
        );
        assert_eq!(found[1].0.details.app_id, 2);
    }

    #[test]
    fn a_doubt_either_source_had_survives_the_merge() {
        let mut found = vec![entry_for(1, "A Game", &[]), entry_for(1, "A Game", &[])];
        found[1].1 = Some("the id looked wrong".to_owned());
        merge_duplicates(&mut found);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1.as_deref(), Some("the id looked wrong"));
    }

    #[test]
    fn an_alias_that_only_repeats_the_name_is_not_kept() {
        let mut found = vec![
            entry_for(1, "A Game", &[]),
            entry_for(1, "A Game", &["a game"]),
        ];
        merge_duplicates(&mut found);
        assert!(found[0].0.aliases.is_empty(), "{:?}", found[0].0.aliases);
    }

    #[test]
    fn app_ids_are_read_out_of_a_listing_this_crate_printed() {
        let listing = "\
Build your own Bundle
  - Starship Troopers: Terran Command    https://store.steampowered.com/app/1202130
  - art of rally    Very Positive 4275 | Very Positive 40    https://store.steampowered.com/app/550320
";
        let requested = requested_from_listing(listing);
        assert_eq!(requested.len(), 2);
        assert_eq!(requested[0].app_id, 1202130);
        assert_eq!(requested[0].name, "Starship Troopers: Terran Command");
        // The review columns sit between the name and the link and are not part of either.
        assert_eq!(requested[1].app_id, 550320);
        assert_eq!(requested[1].name, "art of rally");
    }

    #[test]
    fn a_line_with_no_app_id_is_passed_over_rather_than_guessed_at() {
        // A search link means the store sells something that is not one Steam product.
        let listing = "\
  - House Builder Bundle    https://store.steampowered.com/search/?term=House%20Builder
Build your own Bundle
";
        assert!(requested_from_listing(listing).is_empty());
    }

    #[test]
    fn a_tag_the_enum_does_not_know_fails_the_entry_rather_than_shortening_its_list() {
        // The library returns Steam's strings so a new tag cannot break a runtime fetch; the
        // table is where strictness belongs, because a silently shorter tag list looks exactly
        // like a game nobody tagged that way.
        assert_eq!(
            tag_variants(&["Souls-like".to_owned()]).as_deref(),
            Ok(&[Tag::SoulsLike][..])
        );
        let failure = tag_variants(&["Souls-like".to_owned(), "Brand New Tag".to_owned()]);
        assert!(failure.is_err(), "{failure:?}");
        assert!(failure.unwrap_err().contains("steam_tags"));
    }
}
