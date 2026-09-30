//! Fetches Steam's catalogue into the files [`catalogames::store_inventory::steam`] describes.
//!
//! **Not part of the library.** Built only under the `tools` feature, like `steam_lookup` and
//! `steam_tags`.
//!
//! ```text
//! cargo run --release --features tools --bin steam_catalogue -- <out-dir> [how-many|all] [cc,cc,…]
//! cargo run --release --features tools --bin steam_catalogue -- --apply-overrides <dir>
//! cargo run --release --features tools --bin steam_catalogue -- --doctor <dir>
//! ```
//!
//! The last two make no requests at all. `--apply-overrides` applies the facts in
//! `<dir>/steam-overrides.tsv` to the snapshot already in `<dir>`, which is how a fact found by
//! hand takes effect before the next sweep — see [`catalogames::store_inventory::steam::overrides`].
//! `--doctor` reports what is wrong with that snapshot and what it does not know, and exits
//! non-zero if anything is invalid — see [`catalogames::store_inventory::steam::doctor`].
//!
//! Writes files rather than printing, which is the one place this departs from its siblings: it
//! produces three of them, and a generator whose output is a directory cannot be a pipe. The diff
//! is still the review.
//!
//! # What it takes, and what it deliberately does not
//!
//! Only the fields an inventory entry is built from. **No review text**, no descriptions, no
//! images — the per-game review summary is four numbers and the reviews themselves are megabytes.
//!
//! Two numbers are Valve's own and are stored unmapped, because inventing a second numbering for
//! something the source already numbers is how two systems drift apart:
//! - the **review band** is `review_score`, already 0-9, where 0 means too few reviews to band;
//! - **tags** are `tagid`, Valve's own ids, resolvable through the dictionary file this writes.
//!
//! # The population this records, stated because it is not the one the program prefers
//!
//! `summary_filtered` is `language=all` + `purchase_type=steam`. It excludes key-activated
//! copies, which are exactly what a bundle catalogue is about, and Valve's bulk surfaces expose
//! no summary that includes them. Measured over the 415 committed entries, it agrees with the
//! all-purchase-type band 92% of the time. See [`catalogames::store_inventory::steam`].

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;

use catalogames::store_inventory::steam::codec::{self, base36};
use catalogames::store_inventory::steam::doctor;
use catalogames::store_inventory::steam::overrides::{self, Fact, Outcome};
use catalogames::store_inventory::steam::snapshot::{self, COLUMNS, UNKNOWN};
use serde::Deserialize;

const USER_AGENT: &str = concat!("catalogames-steam-catalogue/", env!("CARGO_PKG_VERSION"));

/// Valve's tag dictionary, so a stored `tagid` can be read back as a name.
const TAG_LIST: &str = "https://api.steampowered.com/IStoreService/GetTagList/v1/?language=english";

/// The store's own search, which is what enumerates app-ids. `category1=998` is "games", so
/// software, videos and hardware never enter the list.
const SEARCH: &str = "https://store.steampowered.com/search/results/\
                      ?query&dynamic_data=&category1=998&infinite=1&ignore_preferences=1\
                      &sort_by=Name_ASC";

/// The countries the sweep asks as, in order. Their catalogues are UNIONED.
///
/// **One country is not enough, and the 25 `unresolved` rows proved it.** Those apps were
/// enumerated by an IL-geolocated search and then refused by an item service asked as `US`: 0 of
/// 25 resolved under US, 6 under CA, 18 under JP, all 25 under IL. Steam sells a different
/// catalogue in every country and none contains another. An earlier note here claimed CA was
/// effectively a superset; that came from a 200-id sample per country — 0.1% of a 188,000-game
/// catalogue whose exclusives are rarer still — and could not have detected them.
///
/// So this is a spread, not a ranking: one country per inhabited continent, chosen among the
/// permissive ones. Totals measured 2026-09-23 — PL 188,161, ZA 188,132, CA 188,169, AU 188,115,
/// IN 188,110, IL 188,106, JP 187,897, BR 186,756. Deliberately absent are DE (157,474), CN
/// (177,744) and RU (183,908): heavily censored, so most of what they hold is held elsewhere too,
/// and an enumeration costs about 47 minutes whatever it returns.
///
/// The FIRST is the one most apps are described from; the rest are tried only for the few it will
/// not describe. Order therefore affects speed, not what ends up in the file.
///
/// # This list is larger than it needs to be, and the sweep of 2026-09-25 is why
///
/// Run over all eight, the seven after CA contributed 140 games between them for about nine hours
/// of enumeration. Re-querying those 140 under 18 country codes, **JP alone can describe 124** —
/// the next best are KR and SG at 70 — so **CA + JP covers 188,434 of the 188,450 found**, and 13
/// of the 16 it misses are reachable from no country at all.
///
/// So a future sweep should pass `CA,JP` and take about 3.3 hours instead of 10.2. The full list
/// is kept as the default only because it is what produced the snapshot in `data/`; shortening it
/// is a decision about the file's contents, not a tidy-up. See `crate::store_inventory::steam` for
/// the whole coverage table.
const COUNTRIES: &[&str] = &["CA", "PL", "ZA", "IL", "IN", "JP", "AU", "BR"];

/// Batched item lookup. 150 ids per request measured fine; 500 exceeds the front-end's URL limit.
const ITEMS: &str = "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/?input_json=";

/// The store's whole category table, keyless, in one request — the exact counterpart of
/// [`TAG_LIST`], which is why `feats` can be stored as Valve's numbers like `tagids` is.
const CATEGORY_LIST: &str = "https://api.steampowered.com/IStoreBrowseService/GetStoreCategories/v1/\
                             ?input_json=%7B%22language%22%3A%22english%22%7D";

/// How many app-ids one search page yields. The cap is the store's, not ours.
const PAGE: usize = 100;

/// How many ids go into one item request.
const BATCH: usize = 150;

/// Waited between requests, because the store DOES rate-limit and the limit is not published.
///
/// Measured rather than guessed: an unpaced sweep drew HTTP 429 at roughly the thirtieth search
/// page, within about twenty seconds. A second and a half puts this well under a request a
/// second sustained, which is slower than a person clicking "next page" and is the same spirit
/// as the Humble client's own spacing.
const SPACING: std::time::Duration = std::time::Duration::from_millis(1_500);

/// How long to wait after a 429 before trying that request again, doubling each time.
const BACKOFF: std::time::Duration = std::time::Duration::from_secs(20);

/// How many times one request is retried before the run gives up on it.
const RETRIES: u32 = 4;

#[derive(Deserialize)]
struct TagResponse {
    response: TagList,
}
#[derive(Deserialize)]
struct TagList {
    tags: Vec<TagEntry>,
}
#[derive(Deserialize)]
struct TagEntry {
    tagid: u32,
    name: String,
}
#[derive(Deserialize)]
struct CategoryResponse {
    response: CategoryList,
}
#[derive(Deserialize)]
struct CategoryList {
    categories: Vec<CategoryEntry>,
}
#[derive(Deserialize)]
struct CategoryEntry {
    categoryid: u32,
    /// Which of the three per-game arrays an id belongs to. Recorded once here rather than per
    /// game, which is what lets `feats` merge the arrays.
    #[serde(default)]
    r#type: u32,
    #[serde(default)]
    display_name: String,
}

#[derive(Deserialize)]
struct SearchPage {
    total_count: usize,
    results_html: String,
}

#[derive(Deserialize)]
struct ItemResponse {
    response: ItemList,
}
#[derive(Deserialize, Default)]
struct ItemList {
    #[serde(default)]
    store_items: Vec<Item>,
}

/// One game, with every field this tool reads. Anything Valve sends that is not named here is
/// dropped at the parse rather than carried and ignored.
#[derive(Deserialize, Default)]
struct Item {
    #[serde(default)]
    appid: u32,
    /// The same number again — except when it is the ONLY one.
    ///
    /// **Valve refuses an app in two shapes and only one of them fills `appid`.** App 8040 comes
    /// back `success: 15` with `appid: 8040`; app 4278390 comes back `success: 15` with
    /// `appid: 0` and `id: 4278390`. Reading `appid` alone gave thirteen rows identified as app 0
    /// — written to the file, counted as duplicates, and reported as thirteen `unresolved` issues
    /// against an app-id that does not exist — while the thirteen real apps were logged
    /// `unanswered` beside them.
    #[serde(default)]
    id: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    success: i64,
    #[serde(default = "yes")]
    visible: bool,
    /// Valve's own spelling, typo included: the field really is `unvailable_...`, so anyone
    /// searching the response for "unavailable" concludes it does not exist.
    ///
    /// **This is what separates a game that is gone from one this sweep merely cannot buy.** A
    /// refusal carries `success: 15` either way, but a country-restricted app also comes back
    /// with its NAME and this flag set, while a removed one carries neither. Verified: app 231390
    /// asked as US is refused with a name and the flag; asked as JP it is simply live.
    #[serde(default)]
    unvailable_for_country_restriction: bool,
    #[serde(default)]
    r#type: i64,
    #[serde(default)]
    reviews: Reviews,
    #[serde(default)]
    release: Release,
    #[serde(default)]
    platforms: Platforms,
    #[serde(default)]
    tags: Vec<TagRef>,
    #[serde(default)]
    basic_info: BasicInfo,
    #[serde(default)]
    included_items: Included,
    #[serde(default)]
    categories: Categories,
    #[serde(default)]
    supported_languages: Vec<SpokenIn>,
}

/// The feature checkboxes a store page lists, which Valve returns as three arrays by kind.
///
/// They are merged into one column: the kind is a property of the ID, not of the game, and
/// `steam-categories.tsv` already records which kind each id belongs to. Keeping three columns
/// would repeat that per game, 162,371 times, to say nothing a lookup does not.
#[derive(Deserialize, Default)]
struct Categories {
    #[serde(default)]
    supported_player_categoryids: Vec<u32>,
    #[serde(default)]
    feature_categoryids: Vec<u32>,
    #[serde(default)]
    controller_categoryids: Vec<u32>,
}

impl Categories {
    /// Every id, ascending and deduplicated, so the column does not depend on Valve's array
    /// order — which, unlike the tag order, carries no meaning worth preserving.
    fn merged(&self) -> Vec<u32> {
        let mut all: Vec<u32> = self
            .supported_player_categoryids
            .iter()
            .chain(&self.feature_categoryids)
            .chain(&self.controller_categoryids)
            .copied()
            .collect();
        all.sort_unstable();
        all.dedup();
        all
    }
}

/// One row of the language table on a store page: Valve's `supported`, `full_audio` and
/// `subtitles`, which are its Interface, Full Audio and Subtitles columns.
///
/// Confirmed against `appdetails` on app 620, which prints the same 27 languages and stars
/// exactly the 5 this endpoint flags `full_audio`.
#[derive(Deserialize, Default)]
struct SpokenIn {
    #[serde(default)]
    elanguage: i32,
    #[serde(default)]
    supported: bool,
    #[serde(default)]
    subtitles: bool,
    #[serde(default)]
    full_audio: bool,
}
const fn yes() -> bool {
    true
}

/// One more country's answer about an app, folded into what earlier countries said.
///
/// A description replaces a refusal, rather than adding a second row. **A refusal is kept too**:
/// it can carry what an earlier one did not — a name, or the flag saying the app is sold elsewhere
/// — and an app that only a later country answered for at all still gets its row and its ledger
/// entry. Dropping later refusals once meant a name or a region flag that only JP sent never
/// reached either file.
fn merge_answer(items: &mut Vec<Item>, item: Item) {
    if item.app_id() == 0 {
        return;
    }
    match items.iter_mut().find(|held| held.app_id() == item.app_id()) {
        Some(slot) if item.success == 1 => *slot = item,
        Some(slot) => slot.absorb_refusal(&item),
        None => items.push(item),
    }
}

impl Item {
    /// Takes from a later country's refusal whatever this record lacks: a name, and the flag that
    /// says the app is sold elsewhere. It only ever adds — a refusal cannot un-name an app — and a
    /// description is never touched by one.
    fn absorb_refusal(&mut self, later: &Self) {
        if self.success == 1 {
            return;
        }
        if self.name.trim().is_empty() && !later.name.trim().is_empty() {
            self.name.clone_from(&later.name);
        }
        self.unvailable_for_country_restriction |= later.unvailable_for_country_restriction;
    }

    /// Whether the refusal was a country restriction rather than the app being gone.
    const fn platforms_country_restricted(&self) -> bool {
        self.unvailable_for_country_restriction
    }

    /// The app this record is about, from whichever field Valve filled.
    ///
    /// Zero means neither was, and such a record identifies nothing: the caller drops it and lets
    /// the `unanswered` check report the app-ids that never came back.
    const fn app_id(&self) -> u32 {
        if self.appid != 0 { self.appid } else { self.id }
    }
}

/// Who made it and who sold it.
///
/// **Names, not the `creator_clan_account_id` beside them**, even though an id would be shorter
/// and would match how tags are stored. Measured over 100 games: 38% of developers and 16% of
/// publishers carry NO clan id, so an id-plus-dictionary scheme would simply lose a third of the
/// developers. Names also repeat far less than tags do — 118 distinct developers across those
/// same 100 games — so a dictionary would save little even where it worked.
#[derive(Deserialize, Default)]
struct BasicInfo {
    #[serde(default)]
    developers: Vec<Creator>,
    #[serde(default)]
    publishers: Vec<Creator>,
}

#[derive(Deserialize)]
struct Creator {
    #[serde(default)]
    name: String,
}

/// Other apps Steam ties to this one through its store item.
///
/// **Twice mis-described before it was measured, so here is what it actually is.** It was first
/// called the DLC-to-parent link, then corrected to "the parent, on supplementary items only".
/// Both were too neat. Over 30,000 games, 7,158 carry at least one — so it is not a
/// supplementary-item field — and the relation runs in no consistent direction: app 10
/// (Counter-Strike) names app 80 (Condition Zero) and app 80 names app 10 back, while app 323180
/// (Portal 2 Soundtrack) names app 620 and app 620 names NOTHING.
///
/// So read it as association, not parenthood: these apps arrive together in one store item. It is
/// still the best lead toward a product-type column, but it cannot be treated as a pointer to a
/// parent without checking each direction.
///
/// Only the apps are kept. The `included_packages` beside them are purchasing SKUs — Portal 2
/// returns one package called "Portal 2" — which say nothing about the games.
#[derive(Deserialize, Default)]
struct Included {
    #[serde(default)]
    included_apps: Vec<IncludedApp>,
}

#[derive(Deserialize)]
struct IncludedApp {
    #[serde(default)]
    id: u32,
}

#[derive(Deserialize, Default)]
struct Reviews {
    #[serde(default)]
    summary_filtered: Summary,
}
#[derive(Deserialize, Default)]
struct Summary {
    #[serde(default)]
    review_count: u32,
    #[serde(default)]
    percent_positive: u32,
    /// Valve's own band, 0-9, where 0 is "too few reviews to name one".
    #[serde(default)]
    review_score: u32,
}
#[derive(Deserialize, Default)]
struct Release {
    #[serde(default)]
    steam_release_date: i64,
    #[serde(default)]
    is_early_access: bool,
    /// Announced but not out. **This is what separates the two reasons a date can be missing**,
    /// and the endpoint says so plainly once you look: an unreleased game sends
    /// `is_coming_soon: true` with `custom_release_date_message: "Coming soon"` and NO
    /// `steam_release_date` at all, while Sleeping Dogs — long out, ten thousand reviews — sends
    /// `steam_release_date: 0`. Without it, 26,855 unreleased games and a handful of genuinely
    /// dateless ones all reported the same fault.
    #[serde(default)]
    is_coming_soon: bool,
}
#[derive(Deserialize, Default)]
struct Platforms {
    #[serde(default)]
    windows: bool,
    #[serde(default)]
    mac: bool,
    #[serde(default)]
    steamos_linux: bool,
    /// An OBJECT of headset flags, not a boolean — which is what a first pass got wrong, and the
    /// symptom was serde failing to decode a response that was otherwise perfectly valid.
    #[serde(default)]
    vr_support: VrSupport,
    #[serde(default)]
    steam_deck_compat_category: u32,
    /// The three verdicts beside the Deck one, on that same 0-3 scale. They share a single
    /// base-64 digit; see [`catalogames::store_inventory::steam::codec::Compat`].
    #[serde(default)]
    steam_os_compat_category: u32,
    #[serde(default)]
    steam_frame_compat_category: u32,
    #[serde(default)]
    steam_machine_compat_category: u32,
}

/// Which headsets a game names. Absent keys mean "not stated", so any flag set is VR support.
#[derive(Deserialize, Default)]
struct VrSupport {
    #[serde(default)]
    vrhmd: bool,
    #[serde(default)]
    htc_vive: bool,
    #[serde(default)]
    oculus_rift: bool,
    #[serde(default)]
    windows_mr: bool,
    #[serde(default)]
    valve_index: bool,
}

impl VrSupport {
    const fn any(&self) -> bool {
        self.vrhmd || self.htc_vive || self.oculus_rift || self.windows_mr || self.valve_index
    }
}
#[derive(Deserialize)]
struct TagRef {
    tagid: u32,
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(said) => {
            eprintln!("{said}");
            std::process::ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("steam_catalogue: {why}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    /// A sweep into `out`, of at most `wanted` apps, asking as each of `countries`.
    Sweep {
        out: std::path::PathBuf,
        wanted: usize,
        countries: Vec<String>,
    },
    /// The facts in a directory's `steam-overrides.tsv`, applied to its snapshot. No requests.
    ApplyOverrides(std::path::PathBuf),
    /// The doctor's findings and census for a directory's snapshot. No requests, no writes.
    Doctor(std::path::PathBuf),
}

const USAGE: &str = "usage: steam_catalogue <out-dir> [how-many|all] [cc,cc,…]\n       \
                     steam_catalogue --apply-overrides <dir>\n       \
                     steam_catalogue --doctor <dir>";

/// The command line, read.
///
/// **An out-dir that looks like an option is refused.** This tool has no `--help`, and `--help`
/// was once taken for a directory: a full sweep of live requests started into it.
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let first = args.next().ok_or(USAGE)?;
    let option = match first.as_str() {
        "--apply-overrides" => Some(Command::ApplyOverrides as fn(std::path::PathBuf) -> Command),
        "--doctor" => Some(Command::Doctor as fn(std::path::PathBuf) -> Command),
        _ => None,
    };
    if let Some(option) = option {
        let dir = args.next().ok_or(USAGE)?;
        if let Some(extra) = args.next() {
            return Err(format!("{extra:?} after the directory\n{USAGE}"));
        }
        return Ok(option(dir.into()));
    }
    if first.starts_with('-') {
        return Err(format!(
            "{first:?} is not an option this tool knows\n{USAGE}"
        ));
    }
    // `all` rather than a number large enough to mean it: the sweep stops when the store runs
    // out either way, and a magic 999999 in a shell history is a worse record of intent.
    let wanted: usize = match args.next().as_deref() {
        Some("all") => usize::MAX,
        Some(n) => n
            .parse()
            .map_err(|_| "how-many must be a number, or `all`".to_string())?,
        None => 10_000,
    };
    let countries: Vec<String> = match args.next() {
        Some(given) => given
            .split(',')
            .map(|code| code.trim().to_uppercase())
            .collect(),
        None => COUNTRIES.iter().map(|code| (*code).to_string()).collect(),
    };
    if let Some(bad) = countries
        .iter()
        .find(|code| code.len() != 2 || !code.chars().all(|c| c.is_ascii_alphabetic()))
    {
        return Err(format!("country must be a two-letter code, not {bad:?}"));
    }
    Ok(Command::Sweep {
        out: first.into(),
        wanted,
        countries,
    })
}

fn run() -> Result<String, String> {
    let (out, wanted, countries) = match parse_args(std::env::args().skip(1))? {
        Command::ApplyOverrides(dir) => return apply_overrides(&dir),
        Command::Doctor(dir) => return doctor_report(&dir),
        Command::Sweep {
            out,
            wanted,
            countries,
        } => (out, wanted, countries),
    };
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    // Read before anything is fetched: a typo in a hand-written file should cost a second, not the
    // hours of a sweep that would then fail at the end.
    let facts = read_facts(&out)?;

    let http = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| e.to_string())?;

    // **The field models are Rust, not data.** A tag id or a category id is resolved by every
    // caller that renders a row, so a table the compiler has already read beats a file parsed at
    // startup — and a misspelled constant is then a build error rather than a lookup returning
    // None. They are written into the source tree because that is where they have to compile
    // from; `MODELS` is fixed relative to the crate, since this binary only ever runs from it.
    eprintln!("steam_catalogue: tag model");
    let tags = tag_names(&http)?;
    write(&models("tags.rs"), &tag_model(&tags))?;
    eprintln!("steam_catalogue: category model");
    let categories = category_names(&http)?;
    write(&models("categories.rs"), &category_model(&categories))?;
    // No band model: `catalogames::inventory::steam::Rating` already is one, with the phrases
    // Steam prints and `from_valve_score` to reach them. A generated second copy would be a
    // parallel mechanism for a table the crate has carried all along.

    // **Enumeration is the expensive half and it is idempotent, so it is kept.** A full sweep
    // spends about 1,900 search pages getting the app-ids and about 1,250 item batches
    // describing them; losing the first to a failure in the second would mean paying for it
    // twice. The list is written as soon as it is complete and read back on a later run that
    // wants no more than it holds.
    let roster = out.join("steam-appids.tsv");
    let ids = match reuse(&roster, wanted) {
        Some(held) => {
            eprintln!(
                "steam_catalogue: reusing {} app-id(s) from {}",
                held.len(),
                roster.display()
            );
            held
        }
        None => enumerate(&http, wanted, &roster, &countries)?,
    };
    eprintln!("steam_catalogue: {} app-id(s) to describe", ids.len());

    // Kept as (app-id, line) rather than as text to be re-parsed when sorting. The ordering is
    // NUMERIC — 90 before 1000, not after it, which is what a lexical sort of the same digits
    // would give — and carrying the number makes that true by construction instead of by a
    // `parse().unwrap_or(0)` that would drop an unreadable row silently at position one.
    let mut rows: Vec<(u32, String)> = Vec::with_capacity(ids.len());
    let mut refusals: BTreeMap<u32, (bool, String)> = BTreeMap::new();
    let mut problems: Vec<(u32, String)> = Vec::new();
    let mut seen = 0usize;
    for chunk in ids.chunks(BATCH) {
        let items = describe(&http, chunk, &countries)?;
        let answered: std::collections::BTreeSet<u32> = items.iter().map(|i| i.app_id()).collect();
        for missing in chunk.iter().filter(|id| !answered.contains(id)) {
            problems.push((
                *missing,
                format!("{missing}\tunanswered\tthe item service returned no record"),
            ));
        }
        for item in &items {
            // What the refusal itself says, for the ledger: a country-restricted app comes back
            // named and flagged, a removed one comes back with neither.
            if item.success != 1 {
                refusals.insert(
                    item.app_id(),
                    (
                        item.platforms_country_restricted(),
                        item.name.replace(['\t', '\n'], " "),
                    ),
                );
            }
            let (line, refused) = record(item);
            problems.extend(
                faults(item)
                    .into_iter()
                    .chain(refused)
                    .map(|said| (item.app_id(), said)),
            );
            rows.push((item.app_id(), line));
        }
        seen += chunk.len();
        eprintln!("steam_catalogue: {seen}/{} …", ids.len());
    }

    // **The repair pass, before sorting.** Only the apps the bulk endpoint left undated are asked
    // about individually, so the cost is one request per gap rather than per game.
    let undated: Vec<u32> = problems
        .iter()
        .filter(|(_, said)| said.contains("\tno-release-date\t"))
        .map(|(id, _)| *id)
        .collect();
    if !undated.is_empty() {
        eprintln!(
            "steam_catalogue: {} app(s) arrived with no date; asking appdetails",
            undated.len()
        );
        let fixed = repair_dates(&http, &mut rows, &undated, &mut problems, &countries);
        eprintln!(
            "steam_catalogue: settled {fixed} of {}: a date, a planned date, TBA or X",
            undated.len()
        );
    }

    // What the store's search printed for each app: a name for an app the item service left
    // untitled, and the only name a delisted app keeps.
    let mut roster_names: BTreeMap<u32, String> = BTreeMap::new();
    for line in std::fs::read_to_string(&roster).unwrap_or_default().lines() {
        if line.starts_with('#') {
            continue;
        }
        if let (Ok(id), Some(title)) = (roster_id(line), roster_title(line)) {
            roster_names.insert(id, title.to_string());
        }
    }
    let titled = name_from_roster(&mut rows, &roster_names, &mut problems);
    if titled > 0 {
        eprintln!("steam_catalogue: named {titled} untitled app(s) from the search");
    }

    // The delisted ledger, from what this run could not describe. Both kinds count: `unresolved`
    // is the store refusing to describe an app it listed, `unanswered` is the batch coming back
    // without it at all, and neither leaves anything to write a row from.
    let vanished: std::collections::BTreeSet<u32> = problems
        .iter()
        .filter(|(_, said)| said.contains("\tunresolved\t") || said.contains("\tunanswered\t"))
        .map(|(id, _)| *id)
        .collect();
    let ledger = out.join("steam-delisted-games.tsv");
    let existing = std::fs::read_to_string(&ledger).unwrap_or_default();
    let today = catalogames::clock::Timestamp::now().date();
    write(
        &ledger,
        &delisted_ledger(&existing, &vanished, &roster_names, &refusals, &today),
    )?;
    eprintln!(
        "steam_catalogue: {} app(s) no country would describe",
        vanished.len()
    );

    // **Hand-found facts go on last**, over everything the sweep read for itself, so that a fact
    // Steam has since published — or contradicted — is seen as such. See steam-overrides.tsv.
    for (fact, outcome) in overrides::apply_all(&mut rows, &facts) {
        if let Some(said) = fact_fault(fact, &outcome, true) {
            problems.push((fact.app_id, said));
        }
    }

    // Ascending by app-id, numerically, which is what makes a binary search over the file
    // possible. The issues file takes the same ordering: one sorted lexically would put app 1000
    // before app 90 and read as though it had been shuffled.
    rows.sort_by_key(|(app_id, _)| *app_id);
    problems.sort();
    let rows: Vec<String> = rows.into_iter().map(|(_, line)| line).collect();
    let problems: Vec<String> = problems.into_iter().map(|(_, said)| said).collect();
    let written = header(&countries) + &rows.join("\n") + "\n";
    write(&out.join("steam-games.tsv"), &written)?;
    write(
        &out.join("steam-issues.tsv"),
        &(faults_header() + &problems.join("\n") + "\n"),
    )?;
    // What the reader would quietly work around, said now: see `store_inventory::steam::doctor`.
    let findings = doctor::check(&written, &(faults_header() + &problems.join("\n")));
    for finding in findings.iter().take(20) {
        eprintln!("steam_catalogue: doctor: {finding}");
    }
    if findings.len() > 20 {
        eprintln!(
            "steam_catalogue: doctor: … and {} more",
            findings.len() - 20
        );
    }
    eprintln!("steam_catalogue: doctor: {}", doctor::census(&written));

    Ok(format!(
        "steam_catalogue: wrote {} game(s) and {} issue(s) into {}\n{}",
        rows.len(),
        problems.len(),
        out.display(),
        backlog(problems.iter().map(String::as_str), &facts)
    ))
}

/// One line per game. Tab-separated because a tab cannot occur in any field here and a name can
/// contain every other separator worth having; the name goes last for the same reason.
fn record(item: &Item) -> (String, Vec<String>) {
    let say = |kind: &str, detail: String| format!("{}\t{kind}\t{detail}", item.app_id());
    let mut refused = Vec::new();

    // An app the store lists and will not describe still gets a row, with `?` everywhere nothing
    // is known. Leaving it out would make the file quietly disagree with the store's own index;
    // writing zeroes would make it a real game with a zero band, no date and no tags, which is
    // indistinguishable from a genuinely unreviewed new release. `?` is neither, and it is the
    // same marker a missing date already uses.
    //
    // **A reader has to expect it.** Every column except the app-id can be `?`, so a parser that
    // assumes a number will find one that is not. That is the point: these rows are meant to be
    // found and repaired, not silently consumed.
    if item.success != 1 {
        // Derived from COLUMNS rather than written out: a hand-typed count silently stops
        // matching the day a column is added, and a short row is a mis-parse, not an error.
        return (
            format!("{}{}", item.app_id(), "\t?".repeat(COLUMNS.len() - 1)),
            refused,
        );
    }
    let tags: Vec<String> = item.tags.iter().map(|t| base36(t.tagid)).collect();
    let parents: Vec<String> = item
        .included_items
        .included_apps
        .iter()
        .map(|app| base36(app.id))
        .collect();
    let feats: Vec<String> = item.categories.merged().into_iter().map(base36).collect();

    let verdicts = codec::Compat {
        os: item.platforms.steam_os_compat_category,
        frame: item.platforms.steam_frame_compat_category,
        machine: item.platforms.steam_machine_compat_category,
    };
    let compat = verdicts.digit().map_or_else(
        |why| {
            refused.push(say("compat-out-of-range", why.to_string()));
            "?".to_string()
        },
        |digit| digit.to_string(),
    );

    let spoken = item.supported_languages.iter().map(|row| codec::Spoken {
        id: row.elanguage,
        supported: row.supported,
        subtitles: row.subtitles,
        full_audio: row.full_audio,
    });
    let languages = codec::Languages::gather(spoken).map_or_else(
        |why| {
            let kind = match why {
                codec::Malformed::RegionalOnly => "language-regional-only",
                _ => "language-out-of-range",
            };
            refused.push(say(kind, why.to_string()));
            [
                UNKNOWN.to_string(),
                UNKNOWN.to_string(),
                UNKNOWN.to_string(),
            ]
        },
        codec::Languages::columns,
    );

    // A positive stamp that still will not read is the silent case this file used to have: the
    // row said `?` and nothing recorded that the store HAD sent something.
    //
    // **Coming-soon apps are excluded, for the same reason `no-release-date` excludes them.** An
    // unannounced game is parked at a placeholder far in the future — measured on the three that
    // fired here, 2104-03-01, 2104-12-31 and 2105-01-29 — which the year-2100 guard refuses. It
    // is not a unit error: a milliseconds value would be near 1.7e12, three orders larger. The
    // cell already says `TBA` — not out, no date announced — so flagging it says nothing new, and
    // an issues file that reports the expected is one people stop reading.
    let released = release_cell(&item.release);
    if released == UNKNOWN && item.release.steam_release_date > 0 && !item.release.is_coming_soon {
        refused.push(say(
            "unreadable-release-date",
            format!(
                "the store sent {}, which is not an instant this tool can read",
                item.release.steam_release_date
            ),
        ));
    }

    let line = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        item.app_id(),
        item.r#type,
        item.reviews.summary_filtered.review_score,
        item.reviews.summary_filtered.percent_positive,
        item.reviews.summary_filtered.review_count,
        released,
        platform_bits(&item.platforms),
        u32::from(item.platforms.vr_support.any()),
        u32::from(item.release.is_early_access),
        item.platforms.steam_deck_compat_category,
        compat,
        languages[0],
        languages[1],
        languages[2],
        tags.join(","),
        parents.join(","),
        feats.join(","),
        names(&item.basic_info.developers),
        names(&item.basic_info.publishers),
        item.name.replace(['\t', '\n'], " "),
    );
    (line, refused)
}

/// The commented header every reader meets first.
///
/// Written for someone who knows Steam and has never seen this file: each column says what Valve
/// calls the thing, and where a number is Valve's own it says so and names the sidecar that
/// decodes it. The three properties a reader cannot see by looking — the sort order, the tag
/// order, and which review population these numbers come from — are stated, because each one is
/// invisible in the data and wrong to guess.
fn header(countries: &[String]) -> String {
    format!(
        "# {}\n\
         # Steam's catalogue as IStoreBrowseService/GetItems/v1 reported it on {today}, AS SEEN FROM\n\
         # {country}. The countries are not incidental: Steam sells a different catalogue in each,\n\
         # from 188,169 games in CA down to 157,474 in DE, and none contains another — so these are\n\
         # UNIONED, and an app is described by whichever of them will describe it.\n\
         # Both the enumeration and the description use this list, which is what keeps a game from\n\
         # being listed and then refused. Regenerate with\n\
         #   cargo run --release --features tools --bin steam_catalogue -- <out-dir> [how-many|all] [country]\n\
         # SORTED BY app_id, NUMERICALLY ASCENDING — 90 before 1000 — so the file can be binary-searched\n\
         # Any cell but app_id may be `?`, which means unknown to us and nothing else. A row of nothing\n\
         # but `?` is an app known only by its id; steam-delisted-games.tsv says which of those the\n\
         # store refuses, and steam-issues.tsv why any `?` was written\n\
         # An EMPTY list column (tagids, incl, feats, devs, pubs) means NONE, which is not the same\n\
         # as `?`. `?` is \"not known\"; empty is \"known to be nothing\". steam-issues.tsv records the\n\
         # cases where an empty list is itself suspect, such as a game with no tags at all\n\
         #\n\
         # type       Valve's own product kind: 0 game, 11 music. A games-only sweep is all 0s; widening\n\
         #            the search past category1=998 is what makes the column earn its place\n\
         # band       0-9, Valve's own `review_score`; 0 is \"too few reviews to name a band\".\n\
         #            Decode through steam-bands.tsv\n\
         # approval   percent of those reviews that are positive — the \"% Positive\" a store page prints\n\
         # reviews    how many reviews that percentage is over\n\
         #            band, approval and reviews all come from `summary_filtered`, whose population is\n\
         #            ALL LANGUAGES but `purchase_type=steam`. THAT DIFFERS FROM THE REST OF THIS\n\
         #            PROGRAM, which asks for `purchase_type=all` so that key-activated copies count —\n\
         #            the population a bundle catalogue cares about. Valve publishes no bulk summary\n\
         #            over it, so expect these three to disagree with the curated table in src/inventory\n\
         # released   YYYY-MM-DD out on that day; ~YYYY-MM-DD not out yet, planned for that day; TBA\n\
         #            not out yet, no date announced; X out, no date published; ? unknown — the item\n\
         #            service gave none and appdetails could not say (no-release-date), or the date\n\
         #            would not read (unreadable-release-date)\n\
         # os         a bit mask: 1 windows, 2 mac, 4 linux\n\
         # vr         1 when the store names any headset (Vive, Rift, Index, Windows MR or a generic HMD)\n\
         # early      1 when the store marks it Early Access\n\
         # deck       Steam Deck compatibility: 0 unknown, 1 unsupported, 2 playable, 3 verified\n\
         # compat     ONE base-64 digit (0-9A-Za-z+/) holding the three verdicts beside the Deck one, on\n\
         #            that same 0-3 scale: bits 0-1 SteamOS, bits 2-3 Steam Frame, bits 4-5 Steam Machine\n\
         # lang_*     base-36 64-bit masks over Valve's `elanguage`, where bit N is language N; 0 is a\n\
         #            store listing none, and `?` a list the masks cannot express. The three\n\
         #            are Valve's `supported`, `full_audio` and `subtitles` — the Interface, Full Audio\n\
         #            and Subtitles columns of the language table on a store page. Checked against\n\
         #            `appdetails` on app 620: both report 27 languages and the same 5 with full audio.\n\
         #            A row carrying ONLY an `eadditionallanguage` (a regional variant such as Spanish -\n\
         #            Latin America, elanguage -1) sets no bit; its base language keeps its own\n\
         # tagids     Valve's own numbers in base 36, resolved by steam-tags.tsv. IN VALVE'S OWN ORDER,\n\
         #            which is DESCENDING BY VOTE WEIGHT — the first tag is the strongest, not the\n\
         #            alphabetically first. Up to 20 per game\n\
         # incl       other apps Steam ties to this one through its store item, base 36. NOT a parent\n\
         #            pointer and NOT reliably mutual: app 10 and app 80 name each other, while app 620's\n\
         #            soundtrack names 620 and 620 names nothing. 7,158 of 30,000 games carry at least one\n\
         # feats      store feature ids in base 36, resolved by steam-categories.tsv: the checkboxes a\n\
         #            store page lists (Single-player, Co-op, Steam Cloud, Full controller support …).\n\
         #            Valve returns them as three arrays by kind; they are merged here and ascending,\n\
         #            because the kind belongs to the id and the sidecar already records it\n\
         # devs, pubs names joined by `;`, since a studio name can contain a comma\n{FACTS_NOTE}",
        COLUMNS.join("\t"),
        today = catalogames::clock::Timestamp::now().date(),
        country = countries.join(", "),
    )
}

/// Every kind this sweep can report, named in the header.
///
/// A reader meeting 209 rows of seven kinds cannot tell whether they have seen the whole
/// vocabulary or only the kinds this run happened to hit. Listing them makes an absent kind mean
/// "did not occur" instead of "may not exist".
const FAULT_KINDS: &[(&str, &str)] = &[
    (
        "unresolved",
        "the item service returned no record for an app the store's search lists",
    ),
    ("unanswered", "the batch came back without this app at all"),
    ("no-name", "the store published no title"),
    (
        "no-release-date",
        "no date from the item service, not coming soon, and appdetails could not say either way",
    ),
    (
        "unreadable-release-date",
        "a released app sent a date this tool could not read (coming-soon excluded)",
    ),
    ("no-tags", "no player tags published"),
    ("no-platform", "no operating system flagged"),
    (
        "not-a-game",
        "Valve's own `type` is not 0 — the only non-game signal the store publishes",
    ),
    (
        "language-out-of-range",
        "an `elanguage` past 63; the language masks need widening",
    ),
    (
        "language-regional-only",
        "only regional language variants listed, which the masks cannot express",
    ),
    (
        "compat-out-of-range",
        "a compatibility verdict past 3; Valve added a value",
    ),
    (
        "override-redundant",
        "Steam now publishes a steam-overrides.tsv fact itself; delete the fact",
    ),
    (
        "override-stale",
        "the sweep no longer writes what a steam-overrides.tsv fact corrected, so it was NOT applied",
    ),
    (
        "override-orphaned",
        "a steam-overrides.tsv fact names an app the file has no row for, so it was NOT applied",
    ),
];

fn faults_header() -> String {
    let mut out = "# app_id\tkind\tdetail\n\
                   # Sorted by app_id like steam-games.tsv. One app may appear more than once, and\n\
                   # an app listed here still gets a row there. Every kind that can appear:\n"
        .to_string();
    // Widest name plus a gap, computed rather than typed: `name-suggests-not-a-game` is already
    // 24 characters, so a literal width leaves it one space today and none the day a longer kind
    // is added — the column would silently close up rather than fail.
    let widest = FAULT_KINDS
        .iter()
        .map(|(kind, _)| kind.len())
        .max()
        .unwrap_or(0);
    for (kind, meaning) in FAULT_KINDS {
        let _ = writeln!(out, "#   {kind:<widest$}  {meaning}");
    }
    out
}

/// `--doctor <dir>`: everything the doctor finds in the snapshot in `<dir>`, cross-checked against
/// the issues file beside it, and what the file does not know. No requests, and nothing written.
///
/// Fails — a non-zero exit, for a script to stop on — when anything is [`doctor::Severity::Invalid`].
fn doctor_report(dir: &std::path::Path) -> Result<String, String> {
    let path = dir.join("steam-games.tsv");
    let snapshot =
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let issues = std::fs::read_to_string(dir.join("steam-issues.tsv")).unwrap_or_default();
    let findings = doctor::check(&snapshot, &issues);
    for finding in &findings {
        eprintln!("steam_catalogue: doctor: {finding}");
    }
    let census = doctor::census(&snapshot);
    for (app_id, columns) in &census.partly_known {
        eprintln!(
            "steam_catalogue: doctor: app {app_id} does not know {}",
            columns.join(", ")
        );
    }
    let invalid = findings
        .iter()
        .filter(|finding| finding.severity == doctor::Severity::Invalid)
        .count();
    let summary = format!(
        "steam_catalogue: {}: {invalid} invalid, {} notice(s); {census}",
        path.display(),
        findings.len() - invalid
    );
    if invalid > 0 {
        return Err(summary);
    }
    Ok(summary)
}

/// The snapshot header's last paragraph — and the one a snapshot patched by `--apply-overrides`
/// gains if it was written before facts existed. Without it, a cell a fact filled would read as
/// something the item service reported.
const FACTS_NOTE: &str = "#\n\
    # Where the item service publishes nothing, a cell may carry a fact found elsewhere instead;\n\
    # every one is listed, with where it was seen, in steam-overrides.tsv\n";

/// The facts in `<dir>/steam-overrides.tsv` — none where there is no such file, since a directory
/// no one has written facts into has none.
fn read_facts(dir: &std::path::Path) -> Result<Vec<Fact>, String> {
    let path = dir.join("steam-overrides.tsv");
    match std::fs::read_to_string(&path) {
        Ok(text) => overrides::parse(&text).map_err(|why| format!("{}: {why}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// `--apply-overrides <dir>`: the facts in `<dir>/steam-overrides.tsv`, applied to the snapshot
/// already in `<dir>` — no requests. How a fact found by hand takes effect before the next sweep.
///
/// What needs a person is said on stderr, in the words the sweep would write into
/// `steam-issues.tsv`; that file itself is the sweep's report and is not rewritten here.
fn apply_overrides(dir: &std::path::Path) -> Result<String, String> {
    let facts = read_facts(dir)?;
    let path = dir.join("steam-games.tsv");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (patched, outcomes) = apply_to_snapshot(&text, &facts);
    for said in outcomes
        .iter()
        .filter_map(|(fact, outcome)| fact_fault(fact, outcome, false))
    {
        eprintln!("steam_catalogue: {said}");
    }
    if patched != text {
        write(&path, &patched)?;
    }
    let count = |wanted: fn(&Outcome) -> bool| outcomes.iter().filter(|(_, o)| wanted(o)).count();
    let issues = std::fs::read_to_string(dir.join("steam-issues.tsv")).unwrap_or_default();
    Ok(format!(
        "steam_catalogue: {} of {} fact(s) applied to {}, {} already there, {} not applied\n{}",
        count(|o| *o == Outcome::Applied),
        facts.len(),
        path.display(),
        count(|o| *o == Outcome::AlreadyThere),
        count(|o| matches!(o, Outcome::Changed(_) | Outcome::Gone)),
        backlog(issues.lines(), &facts)
    ))
}

/// A snapshot's text with the facts applied: its header kept as written — gaining [`FACTS_NOTE`]
/// once, if it predates facts — and every row in its place.
fn apply_to_snapshot<'f>(text: &str, facts: &'f [Fact]) -> (String, Vec<(&'f Fact, Outcome)>) {
    let lines: Vec<&str> = text.lines().collect();
    let body = lines
        .iter()
        .position(|line| !line.starts_with('#'))
        .unwrap_or(lines.len());
    let (header, body) = lines.split_at(body);
    let mut rows: Vec<(u32, String)> = body
        .iter()
        .filter(|line| !line.is_empty())
        .map(|line| (roster_id(line).unwrap_or(0), (*line).to_owned()))
        .collect();
    let outcomes = overrides::apply_all(&mut rows, facts);
    let mut out = header.join("\n");
    out.push('\n');
    if !out.contains(FACTS_NOTE) {
        out.push_str(FACTS_NOTE);
    }
    for (_, row) in &rows {
        out.push_str(row);
        out.push('\n');
    }
    (out, outcomes)
}

/// The `steam-issues.tsv` line a fact's outcome calls for — ending in what to do about it — or
/// `None` for an outcome that needs nobody.
///
/// `fresh` says the row was read from Steam this run, which is what makes a cell already holding
/// the fact's value news: Steam now publishes it itself, and the fact can go. In a snapshot already
/// on disk the same cell means only that an earlier run applied it.
fn fact_fault(fact: &Fact, outcome: &Outcome, fresh: bool) -> Option<String> {
    let (id, column, value) = (fact.app_id, fact.column_name(), &fact.value);
    let seen = format!(
        "seen {} on {}: {}",
        fact.seen,
        fact.surface.name(),
        fact.source
    );
    match outcome {
        Outcome::Applied => None,
        Outcome::AlreadyThere if !fresh => None,
        Outcome::AlreadyThere => Some(format!(
            "{id}\toverride-redundant\t{column}: Steam now publishes {value:?} itself; delete \
             the fact from steam-overrides.tsv ({seen})"
        )),
        Outcome::Changed(now) => Some(format!(
            "{id}\toverride-stale\t{column}: the fact expected the sweep to write {:?} and it \
             writes {now:?}, so {value:?} was NOT applied; re-check it ({seen}), then update \
             sweep_wrote or delete the fact",
            fact.sweep_wrote
        )),
        Outcome::Gone => Some(format!(
            "{id}\toverride-orphaned\t{column}: no row for this app any more, so {value:?} was \
             NOT applied; delete the fact, or keep a name in steam-delisted-games.tsv ({seen})"
        )),
    }
}

/// Which column a fact must set to settle an issue of each kind. Read by the backlog count and
/// nothing else: an issue is not suppressed when a fact settles it, because it records what the
/// item service published.
const SETTLES: &[(&str, &str)] = &[
    ("no-name", "name"),
    ("no-platform", "os"),
    ("no-release-date", "released"),
    ("unreadable-release-date", "released"),
    ("no-tags", "tagids(base36)"),
];

/// One line saying how many of each kind of hole the facts have settled — `no-name 17/18 ·
/// no-platform 6/18 · …` — so the overrides file reads as a backlog of known depth rather than
/// as a pile.
fn backlog<'a>(issues: impl IntoIterator<Item = &'a str>, facts: &[Fact]) -> String {
    let open: Vec<(u32, &str)> = issues
        .into_iter()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut field = line.split('\t');
            Some((field.next()?.parse().ok()?, field.next()?))
        })
        .collect();
    let mut kinds: Vec<&str> = SETTLES.iter().map(|(kind, _)| *kind).collect();
    kinds.dedup();
    let tally: Vec<String> = kinds
        .into_iter()
        .filter_map(|kind| {
            let columns: Vec<&str> = SETTLES
                .iter()
                .filter(|(of, _)| *of == kind)
                .map(|(_, column)| *column)
                .collect();
            let apps: Vec<u32> = open
                .iter()
                .filter(|(_, of)| *of == kind)
                .map(|(app_id, _)| *app_id)
                .collect();
            let settled = apps
                .iter()
                .filter(|app_id| {
                    facts.iter().any(|fact| {
                        fact.app_id == **app_id && columns.contains(&fact.column_name())
                    })
                })
                .count();
            (!apps.is_empty()).then(|| format!("{kind} {settled}/{}", apps.len()))
        })
        .collect();
    if tally.is_empty() {
        return "steam_catalogue: no open issue is of a kind a fact can settle".into();
    }
    format!(
        "steam_catalogue: settled by steam-overrides.tsv — {}",
        tally.join(" · ")
    )
}

/// Creator names, joined by `;`.
///
/// Semicolons rather than the commas the id columns use, because a studio name contains commas —
/// "Bandai Namco Entertainment Inc., Ltd." would otherwise read as two companies.
fn names(creators: &[Creator]) -> String {
    creators
        .iter()
        .map(|creator| creator.name.replace(['\t', '\n', ';'], " "))
        .collect::<Vec<_>>()
        .join(";")
}

/// Windows 1, mac 2, linux 4 — three flags in one digit, since a game may run on any combination.
const fn platform_bits(p: &Platforms) -> u32 {
    (p.windows as u32) | ((p.mac as u32) << 1) | ((p.steamos_linux as u32) << 2)
}

/// `YYYY-MM-DD` from a UNIX timestamp, or `?` when the store published none.
///
/// **Read through the HISTORIC reader, not the deadline one.** [`catalogames::clock::from_epoch_seconds`]
/// refuses anything before the year 2000, which is right for a sale deadline and wrong here: apps
/// 20, 50 and 70 are Valve\'s 1998-99 titles, the store sends good timestamps for all three, and
/// this rendered `?` for each while the issues file said nothing — the date looked merely absent.
/// A stamp that is positive and still unreadable now raises `unreadable-release-date`.
fn date(stamp: i64) -> String {
    if stamp <= 0 {
        return "?".to_string();
    }
    catalogames::clock::from_historic_epoch_seconds(stamp)
        .map_or_else(|| "?".to_string(), |when| when.date())
}

/// Everything about one record worth a human's attention later.
///
/// Recorded rather than acted on. What to do about a soundtrack sold as a game, or a game with no
/// reviews, is a decision for whoever reads the file — this only makes sure the decision can be
/// made from evidence instead of from a spot check.
fn faults(item: &Item) -> Vec<String> {
    let say = |kind: &str, detail: String| format!("{}\t{kind}\t{detail}", item.app_id());
    let mut found = Vec::new();
    // **One root cause, one issue.** An unresolved item carries no record at all, so every check
    // below would fire on the absence of data and describe the same fact six ways — which is
    // exactly what the first run did: four games produced twenty-four of its seventy-three
    // issues. Nothing is known about this app, and that is the whole finding.
    if item.success != 1 {
        return vec![say(
            "unresolved",
            format!(
                "success={}; the store's search lists this app but the item service returns no \
                 record for it — delisted, region-locked or withdrawn",
                item.success
            ),
        )];
    }
    if !item.visible {
        found.push(say(
            "invisible",
            "not shown on the store; likely delisted".into(),
        ));
    }
    // **Valve's own verdict, and the only one there is.** A `name-suggests-not-a-game` check used
    // to sit beside this, flagging titles containing "demo", "dlc", "ost" and the like. It was
    // removed on 2026-09-25 because every signal Steam publishes contradicted it: all 78 apps it
    // flagged came back `type: 0`, `item_type: 0`, carrying no product-kind category (10 "Game
    // demo", 21 "DLC", 19 "Mods") and `"game"` from `appdetails`. Decisively, Steam files demos
    // under `category1=10` — 38,904 of them — and none of 100 sampled appears in this sweep,
    // because `category1=998` already excludes them. So an app reaching this point is a game by
    // Steam's own filter, whatever its title says, and the check also had real false positives:
    // "DLC Quest" is a game.
    if item.r#type != 0 {
        found.push(say("not-a-game", format!("type={}", item.r#type)));
    }
    if item.name.trim().is_empty() {
        found.push(say("no-name", "the store published no title".into()));
    }
    // An unreleased game having no release date is not a fault, it is the definition. Before
    // this split, 26,855 of 152,592 rows reported one — 17.6% of the file, all of it noise.
    if item.release.steam_release_date <= 0 && !item.release.is_coming_soon {
        // **Not "unreleased", and the data refuses that reading.** All ten in the first ten
        // thousand are long-released games with thousands of reviews — Sleeping Dogs, Shogun 2,
        // Mafia, Gothic — and `appdetails` reports `coming_soon: false` for them. Valve simply
        // publishes no date on these entries. `appdetails` sometimes still has one where this
        // endpoint does not (Mafia: `28 Aug, 2002`), so a repair pass over the handful that land
        // here is possible; a bulk pass over the other 99.9% is not.
        found.push(say(
            "no-release-date",
            "the store publishes no date for this entry".into(),
        ));
    }
    if item.tags.is_empty() {
        found.push(say("no-tags", "no player tags published".into()));
    }
    if platform_bits(&item.platforms) == 0 {
        found.push(say("no-platform", "runs on nothing the store names".into()));
    }
    found
}

/// Valve's category table, as `id -> (kind, name)`.
fn category_names(
    http: &reqwest::blocking::Client,
) -> Result<BTreeMap<u32, (u32, String)>, String> {
    let body: CategoryResponse = get(http, CATEGORY_LIST, "category list")?;
    Ok(body
        .response
        .categories
        .into_iter()
        .map(|c| (c.categoryid, (c.r#type, c.display_name)))
        .collect())
}

fn tag_names(http: &reqwest::blocking::Client) -> Result<BTreeMap<u32, String>, String> {
    let body: TagResponse = get(http, TAG_LIST, "tag list")?;
    Ok(body
        .response
        .tags
        .into_iter()
        .map(|t| (t.tagid, t.name))
        .collect())
}

/// One batch, described — falling back through the countries for whatever the first will not answer.
///
/// **This is what drives `unresolved` to zero.** A union of several countries' catalogues contains
/// games that any single country refuses to describe, so asking only the first would re-create the
/// very fault the union exists to fix, just with different apps. The retry is cheap because it is
/// rare: only the stragglers are re-asked, and measured on the 25 that failed under US, one extra
/// country answered 18 of them and a second answered the rest.
fn describe(
    http: &reqwest::blocking::Client,
    ids: &[u32],
    countries: &[String],
) -> Result<Vec<Item>, String> {
    let mut items = fetch(http, ids, &countries[0])?;
    // A record naming no app cannot be written, matched or repaired. Dropping it here leaves the
    // `unanswered` check to report the app-ids that never came back, which is the honest account.
    items.retain(|item| item.app_id() != 0);
    let mut pending: Vec<u32> = {
        let answered: std::collections::BTreeSet<u32> = items
            .iter()
            .filter(|i| i.success == 1)
            .map(|i| i.app_id())
            .collect();
        ids.iter()
            .filter(|id| !answered.contains(id))
            .copied()
            .collect()
    };

    for country in &countries[1..] {
        if pending.is_empty() {
            break;
        }
        let retried = fetch(http, &pending, country)?;
        let mut still = Vec::new();
        for item in retried {
            merge_answer(&mut items, item);
        }
        let answered: std::collections::BTreeSet<u32> = items
            .iter()
            .filter(|i| i.success == 1)
            .map(|i| i.app_id())
            .collect();
        still.extend(pending.iter().filter(|id| !answered.contains(id)).copied());
        pending = still;
    }
    Ok(items)
}

/// `appdetails`, the per-app endpoint, asked only for the date.
///
/// **A repair layer, not a source.** It answers one app per request where `GetItems` answers 150,
/// so it is worth reaching for only where the bulk endpoint published nothing — measured, it
/// supplies a date for about a quarter of the apps that arrive without one.
const APP_DETAILS: &str = "https://store.steampowered.com/api/appdetails";

#[derive(Deserialize)]
struct DetailsEnvelope {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    data: DetailsData,
}
#[derive(Deserialize, Default)]
struct DetailsData {
    /// The app this payload is really about — **not** necessarily the key it arrived under.
    ///
    /// A single-id `appdetails` request can answer keyed under a DIFFERENT app-id. Measured on
    /// five consecutive requests with `filters=basic`: asking 620 answered under "323180", asking
    /// 201270 under "201272", asking 102500 under "204600". The payload was correct every time;
    /// only the key was wrong. Indexing the response by the id you asked for therefore finds
    /// nothing for a perfectly live game, and this repair pass would silently skip it.
    ///
    /// With `filters=release_date` the keys happened to be correct when re-tested, which makes
    /// the old code accidentally right rather than right — so the id is read from the payload.
    #[serde(default)]
    steam_appid: u32,
    #[serde(default)]
    release_date: DetailsDate,
}
#[derive(Deserialize, Default)]
struct DetailsDate {
    /// Whether the store says the app is not out yet. **Read because the item service does not
    /// always say so:** 1061280 and 1087030 come back from it with no `is_coming_soon` at all
    /// while `appdetails` marks both coming soon, and they were written as released games with no
    /// date.
    #[serde(default)]
    coming_soon: bool,
    #[serde(default)]
    date: String,
}

/// The months `appdetails` prints, in the order Valve numbers them.
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// `7 Jul, 2009` — and the other shapes Valve prints — as `YYYY-MM-DD`.
///
/// **A partial date is refused rather than completed.** Valve also prints "Jul 2009", "2009" and
/// "Coming soon"; inventing the first of the month for those would put a date in the file that
/// the store never published, which is worse than the `?` it replaces, because a `?` is visibly
/// unknown and a wrong day is not. Word order is not assumed either: the day is whichever number
/// is not the year.
fn read_store_date(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let month = words
        .iter()
        .find_map(|w| MONTHS.iter().position(|m| w.starts_with(m)))?
        + 1;
    let numbers: Vec<u32> = words.iter().filter_map(|w| w.parse().ok()).collect();
    let year = *numbers.iter().find(|n| (1970..=2100).contains(*n))?;
    let day = *numbers
        .iter()
        .find(|n| (1..=31).contains(*n) && **n != year)?;
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// Settles the `released` cell from `appdetails` where the bulk endpoint gave no date: a day, a
/// planned day, `TBA`, or `X` — see [`settle`].
///
/// **Every country is asked in turn, until one answers.** `appdetails` refuses an app sold only
/// elsewhere, and a refusal is not an answer: 231390 is refused as US, CA and DE and answers as
/// JP. Asking the first country alone left such an app `?` — unknown — when the store does say.
///
/// Patches the field in place rather than rebuilding the row: the column index is shared with the
/// reader through `COLUMNS`, and a row whose width changed would be caught by the assertion here
/// before it reached the file.
fn repair_dates(
    http: &reqwest::blocking::Client,
    rows: &mut [(u32, String)],
    undated: &[u32],
    problems: &mut Vec<(u32, String)>,
    countries: &[String],
) -> usize {
    let released = column("released");
    let mut fixed = 0;
    // Counted so a pass that attributes NOTHING is visible as a fault rather than as data that
    // simply has no dates. The two differ only where an app genuinely carries none.
    let mut attributed = 0;
    for (done, app_id) in undated.iter().enumerate() {
        if done % 25 == 0 {
            eprintln!("steam_catalogue: repairing dates {done}/{}", undated.len());
        }
        // By payload, never by key — see `DetailsData::steam_appid`. A response whose payload
        // names a different app is dropped rather than applied to the wrong game.
        // **`basic` is asked for so the payload carries `steam_appid`.** With `release_date`
        // alone the payload is ONLY the date — no id to match on — so matching by payload found
        // nothing and this pass recovered 0 of 92 where it had recovered 30.
        let Some(release) = countries.iter().find_map(|country| {
            let url = format!(
                "{APP_DETAILS}?appids={app_id}&cc={country}&l=english&filters=basic,release_date"
            );
            let body = get::<BTreeMap<String, DetailsEnvelope>>(http, &url, "appdetails").ok()?;
            body.into_values()
                .find(|envelope| envelope.success && envelope.data.steam_appid == *app_id)
                .map(|envelope| envelope.data.release_date)
        }) else {
            continue;
        };
        attributed += 1;
        let Some(found) = settle(&release) else {
            continue;
        };
        let Some((_, line)) = rows.iter_mut().find(|(id, _)| id == app_id) else {
            continue;
        };
        let mut field: Vec<&str> = line.split('\t').collect();
        if field.len() != COLUMNS.len() || field[released] != UNKNOWN {
            continue;
        }
        field[released] = &found;
        *line = field.join("\t");
        problems.retain(|(id, said)| !(id == app_id && said.contains("no-release-date")));
        fixed += 1;
    }
    if attributed == 0 && !undated.is_empty() {
        eprintln!(
            "steam_catalogue: WARNING — appdetails answered for NONE of the {} undated app(s); \
             the response shape has changed and no date could be attributed",
            undated.len()
        );
    }
    fixed
}

/// The `released` cell `appdetails` settles for an app the item service left undated.
///
/// Coming soon with a whole date is a planned day, and without one `TBA` — "Q4 2026" names no day.
/// Out with a whole date is that day, and with none at all `X`: the store answered and publishes
/// no date. Out with a PARTIAL date — "Jul 2009", "2009" — settles nothing: a date exists and this
/// cannot hold it, which is what `?` means.
fn settle(release: &DetailsDate) -> Option<String> {
    let day = read_store_date(&release.date);
    match (release.coming_soon, day) {
        (true, Some(day)) => Some(snapshot::Release::Planned(&day).cell()),
        (true, None) => Some(snapshot::Release::Unannounced.cell()),
        (false, Some(day)) => Some(day),
        (false, None) if release.date.trim().is_empty() => Some(snapshot::Release::Undated.cell()),
        (false, None) => None,
    }
}

/// The `released` cell for what the item service said: the day, the planned day, `TBA` — or `?`
/// for a game it does not call coming soon and gave no readable date, which the repair pass then
/// asks `appdetails` about.
///
/// An unannounced game is often parked at a placeholder past 2100, which [`date`] refuses; coming
/// soon and no readable day is `TBA` either way.
fn release_cell(release: &Release) -> String {
    let day = date(release.steam_release_date);
    match (release.is_coming_soon, day == UNKNOWN) {
        (true, false) => snapshot::Release::Planned(&day).cell(),
        (true, true) => snapshot::Release::Unannounced.cell(),
        (false, _) => day,
    }
}

/// Where a column sits in a row, by its header name — so a new column cannot shift a hand-typed
/// index.
fn column(name: &str) -> usize {
    COLUMNS
        .iter()
        .position(|at| *at == name)
        .unwrap_or_else(|| panic!("{name:?} is not a column of steam-games.tsv"))
}

/// One GET, paced, retried on a rate limit, and reported by STATUS rather than by a failure to
/// parse what came back.
///
/// Reading the status first matters: a 429 answers with a gzipped error body, so the only symptom
/// of being rate-limited was serde failing to decode it — a confusing way to learn that the
/// request was refused rather than malformed.
fn get<T: serde::de::DeserializeOwned>(
    http: &reqwest::blocking::Client,
    url: &str,
    what: &str,
) -> Result<T, String> {
    let mut wait = BACKOFF;
    for attempt in 0..=RETRIES {
        std::thread::sleep(SPACING);
        let response = http.get(url).send().map_err(|e| format!("{what}: {e}"))?;
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            if attempt == RETRIES {
                return Err(format!(
                    "{what}: still rate-limited after {RETRIES} retries"
                ));
            }
            eprintln!(
                "steam_catalogue: rate-limited, waiting {}s before retrying {what}",
                wait.as_secs()
            );
            std::thread::sleep(wait);
            wait *= 2;
            continue;
        }
        // **A 5xx is the server having a moment, so it is retried; a 4xx is this program being
        // wrong, so it is not.** A full sweep died at search page 109,200 on a bare 502 after
        // surviving a mangled body earlier in the same run — the store gets flaky under an hour
        // of sustained paging, and every one of those failures has cleared on a retry.
        if status.is_server_error() && attempt < RETRIES {
            eprintln!(
                "steam_catalogue: {what} answered {status}; retrying in {}s",
                wait.as_secs()
            );
            std::thread::sleep(wait);
            wait *= 2;
            continue;
        }
        if !status.is_success() {
            return Err(format!("{what}: HTTP {status}"));
        }
        // **A 200 whose body will not parse is retried, not fatal.** A full sweep died at search
        // page 106,900 of about 1,900 on exactly this: the same URL returned perfectly good JSON
        // when asked again seconds later, so it was one truncated or mangled response under
        // sustained load. Forty minutes of enumeration were thrown away for it.
        match response.json() {
            Ok(parsed) => return Ok(parsed),
            Err(why) if attempt < RETRIES => {
                eprintln!("steam_catalogue: {what} came back unreadable ({why}); retrying");
                std::thread::sleep(wait);
                wait *= 2;
            }
            Err(why) => return Err(format!("{what}: {why}")),
        }
    }
    Err(format!("{what}: gave up"))
}

/// Where the generated models land: beside the module that reads them.
fn models(file: &str) -> std::path::PathBuf {
    std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/store_inventory/steam"
    ))
    .join(file)
}

/// The banner every generated module carries, naming the command that rewrites it.
fn generated(what: &str) -> String {
    format!(
        "//! {what}, as Valve publishes them. **Generated — do not edit.**\n\
         //!\n\
         //! Rewritten by `cargo run --release --features tools --bin steam_catalogue -- <out-dir>`,\n\
         //! which fetches the table from Valve before it describes any game. Edit that binary, not\n\
         //! this file: the next sweep overwrites whatever is here.\n\n"
    )
}

/// A Rust string literal: the only characters Valve's names have ever contained that matter here
/// are the quote and the backslash, but escaping is done properly rather than hopefully, because
/// a name that breaks the generated file breaks the build for everyone.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Valve's tag table as a sorted constant, with a binary search over it.
fn tag_model(tags: &BTreeMap<u32, String>) -> String {
    let mut out = generated("Steam's player tags");
    out.push_str(
        "/// Every tag Valve publishes, ascending by id so [`name`] can binary-search it.\n\
         ///\n\
         /// The `tagids` column of `steam-games.tsv` holds these numbers in base 36. They are\n\
         /// Valve's own and are stored unmapped, because a private numbering would have to be\n\
         /// remapped every time Valve adds a tag, and Valve's survives a re-sweep unchanged.\n\
         pub const TAGS: &[(u32, &str)] = &[\n",
    );
    for (id, name) in tags {
        let _ = writeln!(out, "    ({id}, {}),", quoted(name));
    }
    out.push_str("];\n\n");
    out.push_str(
        "/// What Valve calls this tag, or `None` for an id no longer in the table.\n\
         ///\n\
         /// A tag can be retired, and a snapshot taken before that still names it, so a caller\n\
         /// must be able to meet an id this table does not hold.\n\
         #[must_use]\n\
         pub fn name(tagid: u32) -> Option<&'static str> {\n\
         \x20   TAGS.binary_search_by_key(&tagid, |(id, _)| *id)\n\
         \x20       .ok()\n\
         \x20       .map(|at| TAGS[at].1)\n\
         }\n",
    );
    out
}

/// Valve's store-category table: the feature checkboxes a store page lists.
fn category_model(categories: &BTreeMap<u32, (u32, String)>) -> String {
    let mut out = generated("Steam's store categories");
    out.push_str(
        "/// Which of Valve's three per-game arrays an id belongs to.\n\
         ///\n\
         /// Recorded once per id rather than per game, which is what lets the `feats` column of\n\
         /// `steam-games.tsv` merge the three arrays into one sorted list.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]\n\
         pub enum Kind {\n\
         \x20   /// Demo, DLC, Mods — a product kind, never in a game's own feature list.\n\
         \x20   Product,\n\
         \x20   /// Valve's `supported_player_categoryids`: Single-player, Co-op, and so on.\n\
         \x20   PlayerMode,\n\
         \x20   /// Valve's `feature_categoryids`: Achievements, Cloud, the accessibility set.\n\
         \x20   Feature,\n\
         \x20   /// Valve's `controller_categoryids`: full support, DualSense, Steam Input.\n\
         \x20   Controller,\n\
         \x20   /// A kind Valve has added since this table was generated.\n\
         \x20   Unknown,\n\
         }\n\n\
         impl Kind {\n\
         \x20   /// Valve's own number for the kind.\n\
         \x20   const fn of(value: u32) -> Self {\n\
         \x20       match value {\n\
         \x20           0 => Self::Product,\n\
         \x20           1 => Self::PlayerMode,\n\
         \x20           2 => Self::Feature,\n\
         \x20           3 => Self::Controller,\n\
         \x20           _ => Self::Unknown,\n\
         \x20       }\n\
         \x20   }\n\
         }\n\n",
    );
    out.push_str(
        "/// Every category Valve publishes, ascending by id so [`of`] can binary-search it.\n\
         ///\n\
         /// **Names are not unique.** 30 and 51 are both \"Steam Workshop\", 55 and 56 both\n\
         /// \"DualShock Controller Support\", 57 and 58 both \"DualSense Controller Support\". This\n\
         /// is a lookup from id to name and never the reverse.\n\
         pub const CATEGORIES: &[(u32, u32, &str)] = &[\n",
    );
    for (id, (kind, name)) in categories {
        let _ = writeln!(out, "    ({id}, {kind}, {}),", quoted(name));
    }
    out.push_str("];\n\n");
    out.push_str(
        "/// What Valve calls this category and which array it comes from, or `None` for an id\n\
         /// this table does not hold.\n\
         #[must_use]\n\
         pub fn of(categoryid: u32) -> Option<(Kind, &'static str)> {\n\
         \x20   CATEGORIES\n\
         \x20       .binary_search_by_key(&categoryid, |(id, ..)| *id)\n\
         \x20       .ok()\n\
         \x20       .map(|at| (Kind::of(CATEGORIES[at].1), CATEGORIES[at].2))\n\
         }\n\n\
         /// What Valve calls this category, for a caller that does not need the kind.\n\
         #[must_use]\n\
         pub fn name(categoryid: u32) -> Option<&'static str> {\n\
         \x20   of(categoryid).map(|(_, named)| named)\n\
         }\n",
    );
    out
}

/// An earlier run's app-ids, when there are at least as many as this run wants.
///
/// Order is preserved, because it is the store's own and the item phase walks it. Fewer than
/// wanted means the earlier run was smaller or stopped early, and enumerating again is the only
/// way to know which.
fn reuse(path: &std::path::Path, wanted: usize) -> Option<Vec<u32>> {
    let text = std::fs::read_to_string(path).ok()?;
    let ids: Vec<u32> = text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| roster_id(line).ok())
        .collect();
    // `usize::MAX` means "everything the store has", which no saved list can be known to be.
    (wanted != usize::MAX && ids.len() >= wanted).then(|| ids.into_iter().take(wanted).collect())
}

/// Every app-id the store lists, unioned across [`COUNTRIES`].
///
/// **Each country is walked from offset zero.** With `sort_by=Name_ASC` the ordering is stable —
/// the same offset returns the same page minutes apart, where the relevance order returned a
/// completely different hundred — so an offset is a position and a resumed run can trust it. The
/// previous version resumed at `ids.len()`, which is not the same number once duplicates have
/// been dropped, and so re-walked pages it already had.
///
/// Countries are recorded as they finish, with a `# done CC` line in the roster. A resumed run
/// skips those and restarts whichever country it died inside: re-walking one is deterministic
/// under a stable sort and the duplicates are dropped, so the cost is time rather than a gap.
fn enumerate(
    http: &reqwest::blocking::Client,
    wanted: usize,
    roster: &std::path::Path,
    countries: &[String],
) -> Result<Vec<u32>, String> {
    // Not `with_capacity(wanted)`: `all` is `usize::MAX`, and reserving that many is an instant
    // capacity overflow. One page's worth is enough of a hint; the vector grows from there.
    let mut ids: Vec<u32> = Vec::with_capacity(PAGE);
    let mut seen = std::collections::BTreeSet::new();
    let mut finished = std::collections::BTreeSet::new();
    // Titles seen this run, for ids the roster may already hold without one.
    let mut learnt: BTreeMap<u32, String> = BTreeMap::new();

    // **Resumed from whatever a previous run got through.** Writing the roster only once the
    // whole sweep finished meant a failure at page 1,069 of about 1,900 lost every one of the
    // 100,908 app-ids already in hand.
    for line in std::fs::read_to_string(roster).unwrap_or_default().lines() {
        if let Some(code) = line.strip_prefix(DONE) {
            finished.insert(code.trim().to_string());
        } else if !line.starts_with('#')
            && let Ok(id) = roster_id(line)
            && seen.insert(id)
        {
            ids.push(id);
        }
    }
    if !ids.is_empty() {
        eprintln!(
            "steam_catalogue: resuming with {} app-id(s) from {} finished country/ies",
            ids.len(),
            finished.len()
        );
    }

    for country in countries {
        if ids.len() >= wanted {
            break;
        }
        if finished.contains(country) {
            eprintln!("steam_catalogue: {country} already enumerated, skipping");
            continue;
        }
        let before = ids.len();
        let mut start = 0usize;
        let mut total = 0usize;
        while ids.len() < wanted {
            let url = format!("{SEARCH}&cc={country}&start={start}&count={PAGE}");
            let page: SearchPage = get(http, &url, &format!("search {country} at {start}"))?;
            // Progress during enumeration, not only during the item phase. Without it a full
            // sweep is silent for the best part of an hour, indistinguishable from a hang.
            eprintln!(
                "steam_catalogue: enumerating {country} {}/{} (page at {start}) …",
                ids.len(),
                page.total_count
            );
            total = page.total_count;
            let found = app_ids(&page.results_html);
            if found.is_empty() {
                break;
            }
            let mut fresh = Vec::new();
            for (id, title) in found {
                // **Titles are collected for every result, not only for new ones.** The roster
                // holds app-ids from sweeps that predate titles being kept, and those ids are
                // already in `seen`; a walk that recorded only NEW rows would leave every one of
                // them nameless for ever, which is precisely the case this exists to serve.
                if !title.is_empty() {
                    learnt.entry(id).or_insert_with(|| title.clone());
                }
                // Paging can still repeat a row when the catalogue is edited mid-walk, and every
                // country after the first repeats most of what the previous ones already found.
                if seen.insert(id) && ids.len() < wanted {
                    ids.push(id);
                    fresh.push((id, title));
                }
            }
            append(roster, &fresh)?;
            start += PAGE;
        }
        // **Rewritten once per country, not per page.** Appending cannot revise a line already
        // written, so the titles learnt above would otherwise never reach the file. Pages still
        // append as they go, so a crash mid-country loses titles but never app-ids; the rewrite
        // folds them in at the one moment the walk is known to be complete.
        rewrite_roster(roster, &ids, &learnt, &finished, country)?;
        finished.insert(country.clone());
        eprintln!(
            "steam_catalogue: {country} contributed {} new app-id(s) ({} walked of {total} \
             reported); {} held in all",
            ids.len() - before,
            start,
            ids.len()
        );
    }
    Ok(ids)
}

/// Games the store listed and will no longer describe, kept as a LEDGER rather than a snapshot.
///
/// **First-hand observation, not a third-party list.** Every sweep enumerates from the store's own
/// search and then asks the item service about each app; a handful are refused by every country,
/// and fetching their store pages returns a redirect to Steam's front page — which is what Steam
/// does for an app that no longer exists. Measured on one run: 16 of 17 redirected that way. The
/// seventeenth was region-restricted rather than gone, which is why this file says what it
/// observed rather than claiming to know why.
///
/// The date is the day a sweep FIRST could not describe the app, so it survives later runs — a
/// snapshot rewritten each time would keep resetting it to today and the column would mean
/// nothing. An app that becomes describable again is dropped: it is not delisted any more, and a
/// ledger of things that are currently gone is more useful than an archive of things that once
/// were.
///
/// Names come from the roster, which records what the SEARCH printed, and otherwise from this
/// file's own earlier rows — which is also where a name found by hand is kept, for an app the
/// search printed no title for. By definition nothing will describe a delisted app now.
fn delisted_ledger(
    existing: &str,
    vanished: &std::collections::BTreeSet<u32>,
    names: &BTreeMap<u32, String>,
    refusals: &BTreeMap<u32, (bool, String)>,
    today: &str,
) -> String {
    // What earlier runs already recorded: app-id -> (name, first missing).
    let mut held: BTreeMap<u32, (String, String)> = BTreeMap::new();
    for line in existing.lines() {
        if line.starts_with('#') {
            continue;
        }
        let mut field = line.split('\t');
        let (Some(id), Some(name), Some(_state), Some(since)) =
            (field.next(), field.next(), field.next(), field.next())
        else {
            continue;
        };
        if let Ok(id) = id.trim().parse::<u32>() {
            held.insert(id, (name.to_string(), since.to_string()));
        }
    }

    let mut out = "# app_id\tname\tstate\tfirst_missing\n\
                   # state: `removed` — no country named it, so either it is gone or it never\n\
                   #        existed; Valve's API cannot tell those apart, and a bogus app-id and a\n\
                   #        withdrawn one return byte-identical refusals.\n\
                   #        `region_restricted` — some country named it and set Valve's own\n\
                   #        `unvailable_for_country_restriction` flag (their spelling). The app is\n\
                   #        alive; this sweep's countries just cannot buy it.\n\
                   # Apps the store's own search listed and that NO country will describe. Observed\n\
                   # by this sweep, not taken from any third-party list.\n\
                   # first_missing is the day a sweep first failed to describe the app, kept across\n\
                   # later runs. An app that becomes describable again is removed from this file.\n\
                   # name is what the store's SEARCH printed, from steam-appids.tsv, or, where it\n\
                   # printed none, a name found by hand and written here, which later runs keep.\n\
                   # `?` where nothing has named the app.\n\
                   # NOT a statement of WHY: an app can be withdrawn, region-locked everywhere this\n\
                   # sweep asked, or removed outright, and the item service refuses all three alike.\n"
        .to_string();
    for id in vanished {
        let (name, since) = match held.remove(id) {
            // Already known gone: keep the day it was first missed, and take a better name if the
            // roster has since learnt one.
            Some((known, since)) => {
                let name = names
                    .get(id)
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .unwrap_or(known);
                (name, since)
            }
            None => (
                names
                    .get(id)
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .unwrap_or_else(|| "?".to_string()),
                today.to_string(),
            ),
        };
        // A name arriving WITH the refusal beats the roster's: it is what Valve calls the app now.
        let (restricted, refused_name) = refusals
            .get(id)
            .map_or((false, String::new()), |(flag, named)| {
                (*flag, named.clone())
            });
        let name = if refused_name.is_empty() {
            name
        } else {
            refused_name
        };
        let name = name.replace(['\t', '\n'], " ");
        let state = if restricted {
            "region_restricted"
        } else {
            "removed"
        };
        let _ = writeln!(out, "{id}\t{name}\t{state}\t{since}");
    }
    out
}

/// The app-id a roster line leads with.
///
/// Lines are `app_id<TAB>title`, and a line with no tab is read as a bare id — which is what every
/// roster written before titles were kept looks like, so an old file still loads.
fn roster_id(line: &str) -> Result<u32, std::num::ParseIntError> {
    line.split('\t').next().unwrap_or(line).trim().parse()
}

/// The title a roster line carries, or `None` for a line written before titles were kept.
fn roster_title(line: &str) -> Option<&str> {
    let (_, title) = line.split_once('\t')?;
    let title = title.trim();
    (!title.is_empty()).then_some(title)
}

/// Names each described row the item service left untitled with the title the store's search
/// printed, where it printed one, and drops that row's `no-name` fault — the rule the repair pass
/// follows for a date it recovers. Returns how many it named.
///
/// A row the file knows only by its id takes the name too: every cell is known or unknown on its
/// own, and that the store refuses the app is the delisted ledger's to record, not the name's.
fn name_from_roster(
    rows: &mut [(u32, String)],
    roster: &BTreeMap<u32, String>,
    problems: &mut Vec<(u32, String)>,
) -> usize {
    let name = column("name");
    let mut named = 0;
    for (app_id, line) in rows.iter_mut() {
        let Some(title) = roster.get(app_id).filter(|title| !title.is_empty()) else {
            continue;
        };
        let title = title.replace(['\t', '\n'], " ");
        let mut field: Vec<&str> = line.split('\t').collect();
        if field.len() != COLUMNS.len() || !(field[name].is_empty() || field[name] == UNKNOWN) {
            continue;
        }
        field[name] = &title;
        *line = field.join("\t");
        problems.retain(|(id, said)| !(*id == *app_id && said.contains("\tno-name\t")));
        named += 1;
    }
    named
}

/// The roster line that records a finished country.
const DONE: &str = "# done ";

/// Rewrites the roster with every app-id, its best known title, and the countries finished.
///
/// The append path cannot revise a line it already wrote, so this is what lets a title reach an
/// app-id an earlier sweep recorded bare. It runs when a country finishes — the one moment the
/// walk is known complete — and records that country as done in the same write.
///
/// A title already in the file survives where this run saw none: a country whose store does not
/// carry a game prints no title for it, and an older name beats no name at all.
fn rewrite_roster(
    path: &std::path::Path,
    ids: &[u32],
    learnt: &BTreeMap<u32, String>,
    finished: &std::collections::BTreeSet<String>,
    just_done: &str,
) -> Result<(), String> {
    let mut held: BTreeMap<u32, String> = BTreeMap::new();
    for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
        if line.starts_with('#') {
            continue;
        }
        if let (Ok(id), Some(title)) = (roster_id(line), roster_title(line)) {
            held.insert(id, title.to_string());
        }
    }
    let mut out = String::with_capacity(ids.len() * 24);
    out.push_str(ROSTER_HEADER);
    for country in finished.iter().map(String::as_str).chain([just_done]) {
        let _ = writeln!(out, "{DONE}{country}");
    }
    for id in ids {
        let title = learnt
            .get(id)
            .or_else(|| held.get(id))
            .map_or("", String::as_str);
        let _ = writeln!(out, "{id}\t{title}");
    }
    std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))
}

/// Adds app-ids to the roster as they are found, so nothing is lost if the sweep stops.
/// The roster's own header, written once when the file is created.
///
/// It was the only one of these files with nothing at the top saying what it holds. Both readers
/// already skip a line that will not parse as a number, so a header costs them nothing.
const ROSTER_HEADER: &str = "\
# app_id<TAB>title — every app the store's own search listed, one per line.\n\
# The title is the one the SEARCH printed, and it is kept because it is the last place a name\n\
# appears: an app that leaves the store stops being describable, so by the time a sweep notices it\n\
# is gone, nothing else knows what it was called. See steam-delisted-games.tsv.\n\
# IN THE STORE'S OWN ORDER, which is its relevance ranking, NOT sorted and NOT stable between\n\
# runs. steam-games.tsv is the sorted view; this is the work list that produced it.\n\
# Appended as enumeration proceeds, so a failure in the describe phase does not cost it. A later\n\
# run resumes from what is here, and the store's shifting order means resuming can re-walk pages\n\
# it already has — duplicates are dropped, so the cost is time rather than correctness.\n";

fn append(path: &std::path::Path, ids: &[(u32, String)]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let fresh = !path.exists();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if fresh {
        file.write_all(ROSTER_HEADER.as_bytes())
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let block: String = ids
        .iter()
        .map(|(id, title)| format!("{id}\t{title}\n"))
        .collect();
    file.write_all(block.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Every app in a search page, with the title the store printed beside it.
///
/// **The title is kept because it is the only name a delisted app will ever have.** An app that
/// leaves the store stops being describable — no country will answer for it — so by the time the
/// sweep notices it is gone, the chance to learn what it was called has passed. The search result
/// that listed it is the last place the name appears, and it costs nothing to write down.
///
/// The title follows its id in the markup, inside the next `<span class="title">`. A row whose
/// title cannot be found keeps an empty one rather than borrowing the next row's.
fn app_ids(html: &str) -> Vec<(u32, String)> {
    let mut found = Vec::new();
    for piece in html.split("data-ds-appid=\"").skip(1) {
        let Some(end) = piece.find('"') else { continue };
        let Ok(id) = piece[..end].parse() else {
            continue;
        };
        // Bounded to this row: the search repeats `data-ds-appid` per result, so the title has to
        // come before the next one or it belongs to a different game.
        let row = piece.split("data-ds-appid=\"").next().unwrap_or(piece);
        let title = row
            .split_once("<span class=\"title\">")
            .and_then(|(_, rest)| rest.split_once("</span>"))
            .map(|(name, _)| clean(name))
            .unwrap_or_default();
        found.push((id, title));
    }
    found
}

/// A store title as the roster holds it: no markup, no tabs, no newlines, one space between words.
fn clean(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    let mut space = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            _ if c.is_whitespace() => space = !out.is_empty(),
            _ => {
                if space {
                    out.push(' ');
                    space = false;
                }
                out.push(c);
            }
        }
    }
    // The store writes entities in titles — `&amp;` is common, `&#39;` less so.
    out.replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn fetch(
    http: &reqwest::blocking::Client,
    ids: &[u32],
    country: &str,
) -> Result<Vec<Item>, String> {
    let request = serde_json::json!({
        "ids": ids.iter().map(|id| serde_json::json!({"appid": id})).collect::<Vec<_>>(),
        "context": {"language": "english", "country_code": country, "steam_realm": 1},
        "data_request": {
            "include_reviews": true, "include_release": true,
            "include_platforms": true, "include_tag_count": 20,
            "include_basic_info": true, "include_included_items": true,
            "include_supported_languages": true, "include_category_data": true
        }
    })
    .to_string();
    let url = format!("{ITEMS}{}", urlencode(&request));
    let body: ItemResponse = get(http, &url, "items")?;
    Ok(body.response.store_items)
}

fn urlencode(text: &str) -> String {
    percent_encoding::utf8_percent_encode(text, percent_encoding::NON_ALPHANUMERIC).to_string()
}

fn write(path: &std::path::Path, body: &str) -> Result<(), String> {
    let mut file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(body.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three writers of a row's width — the header, a described game and a `?`-filled
    /// placeholder — must agree, and none of them counts by hand.
    ///
    /// This is the check that was missing when the width was the literal `15` inside `record`:
    /// adding a column changed the header and left every unresolved row one field short, which a
    /// reader would see as a shifted name rather than as an error.
    #[test]
    fn every_row_is_exactly_as_wide_as_the_header() {
        let width = COLUMNS.len();
        assert_eq!(
            header(&[COUNTRIES[0].to_string()])
                .lines()
                .next()
                .expect("a header line")
                .matches('\t')
                .count()
                + 1,
            width,
            "the header line does not list every column"
        );

        let (placeholder, refused) = record(&Item::default());
        assert_eq!(
            placeholder.split('\t').count(),
            width,
            "the unresolved-app row"
        );
        assert!(
            refused.is_empty(),
            "an unresolved app is reported by faults(), not by record()"
        );

        let described = Item {
            appid: 620,
            name: "Portal 2".to_string(),
            success: 1,
            ..Item::default()
        };
        let (line, refused) = record(&described);
        assert_eq!(line.split('\t').count(), width, "a described game's row");
        assert!(
            refused.is_empty(),
            "nothing in a default item is out of range"
        );
        assert!(line.starts_with("620\t"), "the app-id leads the row");
        assert!(
            line.ends_with("\tPortal 2"),
            "the name is last, so it may contain anything but a tab"
        );
    }

    /// Valve refuses an app in two shapes, and only one of them fills `appid`.
    ///
    /// Recorded live: app 8040 answers `success: 15` with `appid: 8040`, while app 4278390
    /// answers `success: 15` with `appid: 0` and `id: 4278390`. Reading `appid` alone wrote
    /// thirteen rows identified as app 0 into a file whose whole premise is one row per app-id.
    #[test]
    fn an_app_is_identified_by_whichever_field_valve_filled() {
        let both = Item {
            appid: 8040,
            id: 8040,
            ..Item::default()
        };
        assert_eq!(both.app_id(), 8040, "when both agree");
        let only_id = Item {
            appid: 0,
            id: 4_278_390,
            ..Item::default()
        };
        assert_eq!(
            only_id.app_id(),
            4_278_390,
            "the refusal shape that leaves appid at zero"
        );
        let only_appid = Item {
            appid: 620,
            id: 0,
            ..Item::default()
        };
        assert_eq!(only_appid.app_id(), 620);
        assert_eq!(
            Item::default().app_id(),
            0,
            "neither filled identifies nothing"
        );
    }

    /// The search's own markup, as the store serves it — title inside the row, entities and all.
    #[test]
    fn a_search_page_yields_each_app_with_its_title() {
        let html = concat!(
            r#"<a data-ds-appid="620" href="x"><span class="title">Portal 2</span></a>"#,
            r#"<a data-ds-appid="440" href="y"><span class="title">Tom &amp; Jerry&#39;s</span></a>"#,
            r#"<a data-ds-appid="70" href="z"><span class="title">  Half-Life
             </span></a>"#,
        );
        assert_eq!(
            app_ids(html),
            vec![
                (620, "Portal 2".to_string()),
                (440, "Tom & Jerry's".to_string()),
                (70, "Half-Life".to_string()),
            ]
        );
    }

    /// A row with no title keeps an empty one rather than borrowing the next row's — which is the
    /// failure that would silently mis-name a game. Measured: on a real sweep all 17 blank titles
    /// were apps the item service also refuses to name, and none was a mis-attribution.
    #[test]
    fn a_titleless_row_does_not_borrow_its_neighbours_name() {
        let html = concat!(
            r#"<a data-ds-appid="111"></a>"#,
            r#"<a data-ds-appid="222"><span class="title">Real Name</span></a>"#,
        );
        assert_eq!(
            app_ids(html),
            vec![(111, String::new()), (222, "Real Name".to_string())]
        );
    }

    /// The ledger is a ledger: a date once recorded survives later runs, an app that comes back is
    /// dropped, and a name the roster has since learnt replaces a `?`.
    #[test]
    fn the_delisted_ledger_keeps_dates_and_forgets_returning_games() {
        let existing = "# header\n\
                        100\tGone Long Ago\tremoved\t2026-09-01\n\
                        200\t?\tremoved\t2026-09-10\n\
                        300\tCame Back\tremoved\t2026-09-05\n";
        // 100 and 200 are still missing; 300 is describable again; 400 is newly missing.
        let vanished: std::collections::BTreeSet<u32> = [100, 200, 400].into_iter().collect();
        let mut names = BTreeMap::new();
        names.insert(200, "Learnt Since".to_string());
        names.insert(400, "Newly Gone".to_string());

        let written = delisted_ledger(existing, &vanished, &names, &BTreeMap::new(), "2026-09-26");
        let rows: Vec<&str> = written.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(rows.len(), 3, "300 came back and must be dropped: {rows:?}");
        assert!(
            rows.contains(&"100\tGone Long Ago\tremoved\t2026-09-01"),
            "date survives: {rows:?}"
        );
        assert!(
            rows.contains(&"200\tLearnt Since\tremoved\t2026-09-10"),
            "name upgraded, date kept: {rows:?}"
        );
        assert!(
            rows.contains(&"400\tNewly Gone\tremoved\t2026-09-26"),
            "new entry dated today: {rows:?}"
        );
        assert!(
            !written.contains("Came Back"),
            "a returning app leaves no trace"
        );
    }

    /// An app nothing ever named still gets a row, marked unknown rather than blank.
    #[test]
    fn a_delisted_app_with_no_known_name_is_marked_unknown() {
        let vanished: std::collections::BTreeSet<u32> = [999].into_iter().collect();
        let written = delisted_ledger(
            "",
            &vanished,
            &BTreeMap::new(),
            &BTreeMap::new(),
            "2026-09-26",
        );
        assert!(written.contains("999\t?\tremoved\t2026-09-26"), "{written}");
    }

    /// Valve's country-restriction flag separates two facts the refusal alone conflates.
    ///
    /// A refusal carries `success: 15` whether the app is gone or merely unsold here. The
    /// difference is that a restricted app comes back NAMED and flagged. Recording both as
    /// "removed" would put live games in a delisted file.
    #[test]
    fn a_country_restricted_app_is_not_recorded_as_removed() {
        let vanished: std::collections::BTreeSet<u32> = [11, 22].into_iter().collect();
        let mut refusals = BTreeMap::new();
        refusals.insert(11, (true, "Sold Elsewhere".to_string()));
        refusals.insert(22, (false, String::new()));
        let written = delisted_ledger("", &vanished, &BTreeMap::new(), &refusals, "2026-09-26");
        assert!(
            written.contains("11\tSold Elsewhere\tregion_restricted\t2026-09-26"),
            "a named, flagged refusal is a live game: {written}"
        );
        assert!(written.contains("22\t?\tremoved\t2026-09-26"), "{written}");
    }

    /// The shapes Valve actually prints, and the ones that must be refused.
    ///
    /// Recorded from live `appdetails` replies for apps the bulk endpoint left undated.
    #[test]
    fn a_store_date_is_read_or_refused_but_never_completed() {
        for (printed, expected) in [
            ("7 Jul, 2009", "2009-07-07"),
            ("21 May, 2008", "2008-05-21"),
            ("23 Sep, 2009", "2009-09-23"),
            ("Aug 28, 2002", "2002-08-28"),
            ("1 Dec, 1998", "1998-12-01"),
            ("31 December, 2020", "2020-12-31"),
        ] {
            assert_eq!(
                read_store_date(printed).as_deref(),
                Some(expected),
                "{printed:?}"
            );
        }
        // A partial date is refused: inventing a day would put a date in the file that the store
        // never published, and `?` at least shows it is unknown.
        for refused in [
            "Jul 2009",
            "2009",
            "Coming soon",
            "To be announced",
            "Q3 2026",
            "",
        ] {
            assert_eq!(
                read_store_date(refused),
                None,
                "{refused:?} should not become a date"
            );
        }
    }

    /// Both phases must ask as the same country, which is the fault that produced every
    /// `unresolved` row: the search was geolocated and the item service was asked as `US`.
    #[test]
    fn the_country_reaches_both_phases() {
        assert!(
            !SEARCH.contains("cc="),
            "the search URL must take its country as an argument, not carry a fixed one"
        );
        assert!(
            COUNTRIES.len() >= 2,
            "the point of the list is that one country is not enough"
        );
        for code in COUNTRIES {
            assert_eq!(code.len(), 2, "{code} is not a two-letter code");
            assert!(
                code.chars().all(|c| c.is_ascii_uppercase()),
                "{code} must be upper-case"
            );
        }
        let mut sorted = COUNTRIES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), COUNTRIES.len(), "a country is listed twice");

        let picked: Vec<String> = COUNTRIES.iter().map(|c| (*c).to_string()).collect();
        let written = header(&picked);
        for code in COUNTRIES {
            assert!(
                written.contains(code),
                "the header must name every country the snapshot is a union of; {code} is missing"
            );
        }
    }

    /// A value Valve has not invented yet must reach the issues file rather than the row.
    #[test]
    fn a_value_the_format_cannot_hold_is_reported_and_the_row_survives() {
        let widened = Item {
            appid: 1,
            success: 1,
            platforms: Platforms {
                steam_frame_compat_category: 4,
                ..Platforms::default()
            },
            supported_languages: vec![SpokenIn {
                elanguage: 64,
                supported: true,
                ..SpokenIn::default()
            }],
            ..Item::default()
        };
        let (line, refused) = record(&widened);
        assert_eq!(
            line.split('\t').count(),
            COLUMNS.len(),
            "the row is still written"
        );
        let kinds: Vec<&str> = refused
            .iter()
            .filter_map(|said| said.split('\t').nth(1))
            .collect();
        assert_eq!(kinds, ["compat-out-of-range", "language-out-of-range"]);
        for kind in kinds {
            assert!(
                FAULT_KINDS.iter().any(|(named, _)| *named == kind),
                "{kind} is raised but not listed in the header's vocabulary"
            );
        }
    }

    /// Every kind the header promises is one a reader can actually meet, and the reverse is
    /// checked above — so the list cannot drift into fiction in either direction.
    fn args(line: &str) -> Result<Command, String> {
        parse_args(line.split_whitespace().map(str::to_owned))
    }

    /// `--help` once started a sweep into a directory of that name; an option this tool does not
    /// know is now an error, and the one it does know takes exactly one directory.
    #[test]
    fn the_command_line_refuses_an_option_it_does_not_know() {
        assert_eq!(
            args("--apply-overrides data/steam"),
            Ok(Command::ApplyOverrides("data/steam".into()))
        );
        assert_eq!(
            args("--doctor data/steam"),
            Ok(Command::Doctor("data/steam".into()))
        );
        for bad in [
            "--help",
            "-h",
            "--apply-overrides",
            "--apply-overrides a b",
            "--doctor",
            "--doctor a b",
            "",
        ] {
            let err = args(bad).expect_err(bad);
            assert!(err.contains("usage:"), "{bad:?}: {err}");
        }
        assert_eq!(
            args("out 25 ca,pl"),
            Ok(Command::Sweep {
                out: "out".into(),
                wanted: 25,
                countries: vec!["CA".into(), "PL".into()],
            })
        );
        assert!(matches!(
            args("out all"),
            Ok(Command::Sweep {
                wanted: usize::MAX,
                ..
            })
        ));
        assert!(args("out many").is_err() && args("out 5 CAN").is_err());
    }

    fn fact_file(rows: &str) -> Vec<Fact> {
        overrides::parse(&format!(
            "# app_id\tcolumn\tsweep_wrote\tvalue\tsurface\tsource\tseen\n{rows}"
        ))
        .expect("the facts parse")
    }

    /// A described row, as wide as the format: `0` in every cell but the name, which is given.
    fn described(app_id: u32, name: &str) -> String {
        let mut cells: Vec<String> = COLUMNS.iter().map(|_| "0".to_owned()).collect();
        cells[0] = app_id.to_string();
        cells[column("name")] = name.to_owned();
        cells.join("\t")
    }

    /// Applying facts to a snapshot on disk keeps every line where it was, says once in the header
    /// that cells may come from facts, and changes nothing when run a second time.
    #[test]
    fn a_snapshot_on_disk_takes_its_facts_once_and_keeps_its_shape() {
        let facts = fact_file(
            "3244680\tname\t\tCards of Gluttony\tsteamdb\thttps://steamdb.info/app/3244680/\t2026-09-30\n",
        );
        let before = format!(
            "# app_id\ttype\n# an older header\n{}\n{}\n",
            described(620, "Portal 2"),
            described(3_244_680, "")
        );
        let (after, outcomes) = apply_to_snapshot(&before, &facts);
        assert_eq!(outcomes, vec![(&facts[0], Outcome::Applied)]);
        let lines: Vec<&str> = after.lines().collect();
        assert_eq!(&lines[..2], ["# app_id\ttype", "# an older header"]);
        assert_eq!(after.matches(FACTS_NOTE).count(), 1, "{after}");
        assert_eq!(lines[lines.len() - 2], described(620, "Portal 2"));
        assert_eq!(
            lines[lines.len() - 1],
            described(3_244_680, "Cards of Gluttony")
        );
        let (again, outcomes) = apply_to_snapshot(&after, &facts);
        assert_eq!(again, after, "a second application changes nothing");
        assert_eq!(outcomes, vec![(&facts[0], Outcome::AlreadyThere)]);
        assert!(
            header(&["CA".to_string()]).ends_with(FACTS_NOTE),
            "a new sweep writes it too"
        );
    }

    /// Only what needs a person is reported, and the report ends in what to do. A cell already
    /// holding the value is news only in a row just read from Steam.
    #[test]
    fn a_fact_is_reported_only_when_it_needs_a_person() {
        let facts =
            fact_file("2649520\tos\t0\t1\tstore-page\thttps://store.test/2649520\t2026-09-29\n");
        let fact = &facts[0];
        assert_eq!(fact_fault(fact, &Outcome::Applied, true), None);
        assert_eq!(fact_fault(fact, &Outcome::AlreadyThere, false), None);
        let said = |outcome: Outcome| fact_fault(fact, &outcome, true).expect("reported");
        let redundant = said(Outcome::AlreadyThere);
        assert!(
            redundant.starts_with("2649520\toverride-redundant\tos: "),
            "{redundant}"
        );
        assert!(redundant.contains("delete the fact"), "{redundant}");
        let stale = said(Outcome::Changed("3".into()));
        assert!(stale.starts_with("2649520\toverride-stale\t"), "{stale}");
        for part in [
            "\"0\"",
            "\"3\"",
            "NOT applied",
            "2026-09-29",
            "store-page",
            "re-check",
        ] {
            assert!(stale.contains(part), "{part} in {stale}");
        }
        let orphaned = said(Outcome::Gone);
        assert!(
            orphaned.starts_with("2649520\toverride-orphaned\t"),
            "{orphaned}"
        );
        assert!(orphaned.contains("steam-delisted-games.tsv"), "{orphaned}");
        for line in [redundant, stale, orphaned] {
            let kind = line.split('\t').nth(1).expect("a kind");
            assert!(
                FAULT_KINDS.iter().any(|(known, _)| *known == kind),
                "{kind}"
            );
        }
    }

    #[test]
    fn the_backlog_counts_the_holes_facts_have_settled_by_kind() {
        let facts = fact_file(
            "1\tname\t\tOne\tcommunity-hub\ts\t2026-09-29\n\
             3\tos\t0\t1\tstore-page\ts\t2026-09-29\n",
        );
        let issues = "# app_id\tkind\tdetail\n\
                      1\tno-name\tthe store published no title\n\
                      2\tno-name\tthe store published no title\n\
                      3\tno-platform\tno operating system flagged\n\
                      3\tno-release-date\tno date published\n\
                      4\tunresolved\tsuccess=15\n";
        assert_eq!(
            backlog(issues.lines(), &facts),
            "steam_catalogue: settled by steam-overrides.tsv — no-name 1/2 · no-platform 1/1 · \
             no-release-date 0/1",
            "an os fact settles the os hole, not the date hole of the same app"
        );
        assert!(backlog("4\tunresolved\tx".lines(), &facts).contains("no open issue"));
    }

    #[test]
    fn what_a_fact_settles_is_named_in_real_kinds_and_real_columns() {
        for (kind, column_name) in SETTLES {
            assert!(FAULT_KINDS.iter().any(|(known, _)| known == kind), "{kind}");
            let _ = column(column_name);
        }
    }

    /// The search's title names an app the item service left untitled — its `no-name` fault goes
    /// with it — and an app known only by its id; an already-named one is left alone.
    #[test]
    fn a_blank_name_takes_the_title_the_search_printed() {
        let mut rows = vec![
            (3_400_140, described(3_400_140, "")),
            (620, described(620, "Portal 2")),
            (
                4_278_390,
                format!("4278390{}", "\t?".repeat(COLUMNS.len() - 1)),
            ),
            (708_030, described(708_030, "")),
        ];
        let before = rows.clone();
        let roster: BTreeMap<u32, String> = [
            (3_400_140, "Summer Feast".to_string()),
            (620, "Portal Two".to_string()),
            (4_278_390, "My Beautiful Winter".to_string()),
        ]
        .into_iter()
        .collect();
        let mut problems = vec![
            (
                3_400_140,
                "3400140\tno-name\tthe store published no title".to_string(),
            ),
            (
                708_030,
                "708030\tno-name\tthe store published no title".to_string(),
            ),
        ];
        assert_eq!(name_from_roster(&mut rows, &roster, &mut problems), 2);
        assert_eq!(rows[0].1, described(3_400_140, "Summer Feast"));
        assert_eq!(rows[1], before[1], "an app already named keeps its name");
        assert_eq!(
            rows[2].1,
            format!(
                "4278390{}\tMy Beautiful Winter",
                "\t?".repeat(COLUMNS.len() - 2)
            ),
            "an unknown name is filled, and every other unknown cell stays unknown"
        );
        assert_eq!(rows[3], before[3], "no title printed, nothing to take");
        assert_eq!(
            problems.len(),
            1,
            "only the named app's fault goes: {problems:?}"
        );
        assert_eq!(problems[0].0, 708_030);
    }

    /// What `appdetails` says settles the cell in the file's own words: every one of the four
    /// answers it can give, and the one that settles nothing.
    #[test]
    fn appdetails_settles_an_undated_app_in_the_released_grammar() {
        let said = |coming_soon: bool, date: &str| {
            settle(&DetailsDate {
                coming_soon,
                date: date.into(),
            })
        };
        assert_eq!(
            said(true, "15 Nov, 2026"),
            Some("~2026-11-15".into()),
            "planned"
        );
        assert_eq!(said(true, "To be announced"), Some("TBA".into()));
        assert_eq!(
            said(true, "Q4 2026"),
            Some("TBA".into()),
            "a quarter names no day"
        );
        assert_eq!(said(false, "7 Jul, 2009"), Some("2009-07-07".into()), "out");
        assert_eq!(
            said(false, ""),
            Some("X".into()),
            "answered, and no date published"
        );
        assert_eq!(
            said(false, "Jul 2009"),
            None,
            "a date exists that the cell cannot hold"
        );
    }

    /// The item service's own word goes into the cell the same way — and a game it does not call
    /// coming soon with no readable date is left unknown for the repair pass.
    #[test]
    fn the_item_services_release_is_written_in_the_released_grammar() {
        let release = |steam_release_date: i64, is_coming_soon: bool| {
            release_cell(&Release {
                steam_release_date,
                is_coming_soon,
                ..Release::default()
            })
        };
        // 2011-04-19 00:00 UTC.
        assert_eq!(release(1_303_171_200, false), "2011-04-19");
        assert_eq!(release(1_303_171_200, true), "~2011-04-19");
        assert_eq!(release(0, true), "TBA");
        assert_eq!(
            release(4_234_567_890, true),
            "TBA",
            "a placeholder past 2100 is no day"
        );
        assert_eq!(release(0, false), "?", "for the repair pass to ask about");
    }

    fn answer(app_id: u32, success: i64, name: &str, restricted: bool) -> Item {
        Item {
            appid: app_id,
            success,
            name: name.to_owned(),
            unvailable_for_country_restriction: restricted,
            ..Item::default()
        }
    }

    /// Every country's answer counts: a later refusal adds the name and the region flag an
    /// earlier one lacked, a later description replaces a refusal, and an app only a later
    /// country answered for is kept rather than lost.
    #[test]
    fn a_later_countrys_answer_adds_what_the_earlier_ones_lacked() {
        let mut items = vec![answer(1, 15, "", false), answer(2, 15, "", false)];
        merge_answer(&mut items, answer(1, 15, "Sold Elsewhere", true));
        merge_answer(&mut items, answer(1, 15, "", false));
        merge_answer(&mut items, answer(2, 1, "Described", false));
        merge_answer(&mut items, answer(3, 15, "Only Here", false));
        merge_answer(&mut items, answer(0, 15, "No App", false));
        let seen: Vec<(u32, i64, &str, bool)> = items
            .iter()
            .map(|i| {
                (
                    i.app_id(),
                    i.success,
                    i.name.as_str(),
                    i.platforms_country_restricted(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                (1, 15, "Sold Elsewhere", true),
                (2, 1, "Described", false),
                (3, 15, "Only Here", false),
            ],
            "a later refusal can never un-name or un-flag an app, and a record naming no app is dropped"
        );
        let mut described = vec![answer(4, 1, "Real Name", false)];
        merge_answer(&mut described, answer(4, 15, "Other", true));
        assert_eq!(
            (described[0].success, described[0].name.as_str()),
            (1, "Real Name"),
            "a refusal never touches a description"
        );
    }

    #[test]
    fn the_fault_vocabulary_has_no_repeats() {
        let mut named: Vec<&str> = FAULT_KINDS.iter().map(|(kind, _)| *kind).collect();
        named.sort_unstable();
        let listed = named.len();
        named.dedup();
        assert_eq!(named.len(), listed, "a kind is listed twice");
    }
}
