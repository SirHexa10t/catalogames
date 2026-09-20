//! Fetches Steam's catalogue into the files [`catalogames::store_inventory::steam`] describes.
//!
//! **Not part of the library.** Built only under the `tools` feature, like `steam_lookup` and
//! `steam_tags`.
//!
//! ```text
//! cargo run --release --features tools --bin steam_catalogue -- <out-dir> [how-many]
//! ```
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

use serde::Deserialize;

const USER_AGENT: &str = concat!("catalogames-steam-catalogue/", env!("CARGO_PKG_VERSION"));

/// Valve's tag dictionary, so a stored `tagid` can be read back as a name.
const TAG_LIST: &str = "https://api.steampowered.com/IStoreService/GetTagList/v1/?language=english";

/// The store's own search, which is what enumerates app-ids. `category1=998` is "games", so
/// software, videos and hardware never enter the list.
const SEARCH: &str = "https://store.steampowered.com/search/results/\
                      ?query&dynamic_data=&category1=998&infinite=1&ignore_preferences=1";

/// Batched item lookup. 150 ids per request measured fine; 500 exceeds the front-end's URL limit.
const ITEMS: &str = "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/?input_json=";

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
    #[serde(default)]
    name: String,
    #[serde(default)]
    success: i64,
    #[serde(default = "yes")]
    visible: bool,
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
}
const fn yes() -> bool {
    true
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

fn run() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .ok_or("usage: steam_catalogue <out-dir> [how-many]")?;
    // `all` rather than a number large enough to mean it: the sweep stops when the store runs
    // out either way, and a magic 999999 in a shell history is a worse record of intent.
    let wanted: usize = match args.next().as_deref() {
        Some("all") => usize::MAX,
        Some(n) => n
            .parse()
            .map_err(|_| "how-many must be a number, or `all`".to_string())?,
        None => 10_000,
    };
    let out = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;

    let http = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| e.to_string())?;

    eprintln!("steam_catalogue: tag dictionary");
    let tags = tag_names(&http)?;
    write(&out.join("steam-tags.tsv"), &dictionary(&tags))?;

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
        None => {
            let found = enumerate(&http, wanted)?;
            let listed = found
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            write(&roster, &(listed + "\n"))?;
            found
        }
    };
    eprintln!("steam_catalogue: {} app-id(s) to describe", ids.len());

    let mut rows: Vec<String> = Vec::with_capacity(ids.len());
    let mut problems: Vec<String> = Vec::new();
    let mut seen = 0usize;
    for chunk in ids.chunks(BATCH) {
        let items = fetch(&http, chunk)?;
        let answered: std::collections::BTreeSet<u32> = items.iter().map(|i| i.appid).collect();
        for missing in chunk.iter().filter(|id| !answered.contains(id)) {
            problems.push(format!(
                "{missing}\tunanswered\tthe item service returned no record"
            ));
        }
        for item in &items {
            problems.extend(faults(item));
            rows.push(record(item));
        }
        seen += chunk.len();
        eprintln!("steam_catalogue: {seen}/{} …", ids.len());
    }

    // Ascending by app-id, which is what makes a binary search over the file possible.
    rows.sort_by_key(|line| {
        line.split('\t')
            .next()
            .unwrap_or("")
            .parse::<u32>()
            .unwrap_or(0)
    });
    problems.sort();
    write(
        &out.join("steam-games.tsv"),
        &(header() + &rows.join("\n") + "\n"),
    )?;
    write(
        &out.join("steam-issues.tsv"),
        &(faults_header() + &problems.join("\n") + "\n"),
    )?;

    Ok(format!(
        "steam_catalogue: wrote {} game(s) and {} issue(s) into {}",
        rows.len(),
        problems.len(),
        out.display()
    ))
}

/// One line per game. Tab-separated because a tab cannot occur in any field here and a name can
/// contain every other separator worth having; the name goes last for the same reason.
fn record(item: &Item) -> String {
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
        return format!("{}{}", item.appid, "\t?".repeat(14));
    }
    let tags: Vec<String> = item.tags.iter().map(|t| base36(t.tagid)).collect();
    let parents: Vec<String> = item
        .included_items
        .included_apps
        .iter()
        .map(|app| base36(app.id))
        .collect();
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        item.appid,
        item.r#type,
        item.reviews.summary_filtered.review_score,
        item.reviews.summary_filtered.percent_positive,
        item.reviews.summary_filtered.review_count,
        date(item.release.steam_release_date),
        platform_bits(&item.platforms),
        u32::from(item.platforms.vr_support.any()),
        u32::from(item.release.is_early_access),
        item.platforms.steam_deck_compat_category,
        tags.join(","),
        parents.join(","),
        names(&item.basic_info.developers),
        names(&item.basic_info.publishers),
        item.name.replace(['\t', '\n'], " "),
    )
}

fn header() -> String {
    "# app_id\ttype\tband\tapproval\treviews\treleased\tos\tvr\tearly\tdeck\t\
      tagids(base36)\tincl(base36)\tdevs\tpubs\tname\n\
     # band 0-9 (Valve's own review_score, 0 = too few to band), over language=all purchase_type=steam\n\
     # os is a bit mask: 1 windows, 2 mac, 4 linux; deck 0 unknown 1 unsupported 2 playable 3 verified\n\
     # vr 1 when the store names any headset (Vive, Rift, Index, Windows MR or a generic HMD), else 0\n\
     # early 1 when the store marks it Early Access, else 0\n\
     # type is Valve's own: 0 game, 11 music. A games-only sweep is all 0s; widen it to see others\n\
     # incl lists other apps Steam ties to this one through its store item, base 36. NOT a parent\n\
     # pointer and NOT reliably mutual: app 10 and app 80 name each other, while app 620's\n\
     # soundtrack names 620 and 620 names nothing. 7,158 of 30,000 games carry at least one\n\
     # devs and pubs are names joined by `;`, since a studio name can contain a comma\n\
     # tagids are Valve's own numbers in base 36, resolvable through steam-tags.tsv\n\
     # any column but app_id may be `?`: the store lists the app and publishes no record for it\n"
        .to_string()
}

fn faults_header() -> String {
    "# app_id\tkind\tdetail\n".to_string()
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

/// A tag id in base 36, which is Valve's own number written shorter and nothing else.
///
/// **Measured, because "use a bigger base" is the obvious next thought and it barely pays.** Over
/// the first ten thousand games the tag block is 894 KB in decimal, 694 KB in base 36 and 643 KB
/// in base 62 — so base 62 buys 51 KB more than base 36, about 4% of the file, in exchange for a
/// case-sensitive alphabet. The widest id is four characters either way, which is why the extra
/// symbols have so little to do.
///
/// What WOULD pay is not a bigger base: only 429 distinct tags appear across those ten thousand
/// games, in an id space reaching 1,352,486. The ids are sparse, so most of the width is spent on
/// a range nothing occupies. See `steam-tags.tsv`.
fn base36(mut value: u32) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while value > 0 {
        out.push(DIGITS[(value % 36) as usize]);
        value /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Windows 1, mac 2, linux 4 — three flags in one digit, since a game may run on any combination.
const fn platform_bits(p: &Platforms) -> u32 {
    (p.windows as u32) | ((p.mac as u32) << 1) | ((p.steamos_linux as u32) << 2)
}

/// `YYYY-MM-DD` from a UNIX timestamp, or `?` when the store published none.
fn date(stamp: i64) -> String {
    if stamp <= 0 {
        return "?".to_string();
    }
    catalogames::clock::from_epoch_seconds(stamp)
        .map_or_else(|| "?".to_string(), |when| when.date())
}

/// Everything about one record worth a human's attention later.
///
/// Recorded rather than acted on. What to do about a soundtrack sold as a game, or a game with no
/// reviews, is a decision for whoever reads the file — this only makes sure the decision can be
/// made from evidence instead of from a spot check.
fn faults(item: &Item) -> Vec<String> {
    let say = |kind: &str, detail: String| format!("{}\t{kind}\t{detail}", item.appid);
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
    if item.r#type != 0 {
        found.push(say("not-a-game", format!("type={}", item.r#type)));
    }
    if item.name.trim().is_empty() {
        found.push(say("no-name", "the store published no title".into()));
    }
    if item.release.steam_release_date <= 0 {
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
    if let Some(word) = suspect_word(&item.name) {
        found.push(say(
            "name-suggests-not-a-game",
            format!("{word:?} in the title"),
        ));
    }
    found
}

/// A whole word in the title suggesting the thing is not a game, or `None`.
///
/// **Matched as a WORD, never as a substring**, which is the trap this crate already documents
/// and which a first version of this walked straight into: `"ost "` matched "Gh*ost* Recon" twice
/// in the first two hundred games. "ost" also sits inside Ghostwire Tokyo and Samorost, which is
/// why the inventory refuses to filter on names at all.
///
/// Reported and never acted on. Whether a soundtrack sold as a game should be dropped is a
/// decision for whoever reads the file; this only makes it findable.
fn suspect_word(name: &str) -> Option<&'static str> {
    const ALONE: &[&str] = &["ost", "soundtrack", "artbook", "demo", "dlc"];
    const PAIRS: &[[&str; 2]] = &[
        ["season", "pass"],
        ["art", "book"],
        ["original", "soundtrack"],
    ];

    let words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    if let Some(hit) = ALONE
        .iter()
        .find(|word| words.iter().any(|had| had == *word))
    {
        return Some(hit);
    }
    PAIRS
        .iter()
        .find(|pair| {
            words
                .windows(2)
                .any(|had| had[0] == pair[0] && had[1] == pair[1])
        })
        .map(|pair| pair[1])
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
        if !status.is_success() {
            return Err(format!("{what}: HTTP {status}"));
        }
        return response.json().map_err(|e| format!("{what}: {e}"));
    }
    Err(format!("{what}: gave up"))
}

fn dictionary(tags: &BTreeMap<u32, String>) -> String {
    let mut out = "# tagid\tname — Valve's own numbering, stored unmapped\n".to_string();
    for (id, name) in tags {
        let _ = writeln!(out, "{id}\t{name}");
    }
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
        .filter_map(|line| line.trim().parse().ok())
        .collect();
    // `usize::MAX` means "everything the store has", which no saved list can be known to be.
    (wanted != usize::MAX && ids.len() >= wanted).then(|| ids.into_iter().take(wanted).collect())
}

/// App-ids, in the order the store's own search lists them.
fn enumerate(http: &reqwest::blocking::Client, wanted: usize) -> Result<Vec<u32>, String> {
    // Not `with_capacity(wanted)`: `all` is `usize::MAX`, and reserving that many is an instant
    // capacity overflow. One page's worth is enough of a hint; the vector grows from there.
    let mut ids: Vec<u32> = Vec::with_capacity(PAGE);
    let mut seen = std::collections::BTreeSet::new();
    let mut start = 0usize;
    while ids.len() < wanted {
        let url = format!("{SEARCH}&start={start}&count={PAGE}");
        let page: SearchPage = get(http, &url, &format!("search at {start}"))?;
        // Progress during enumeration, not only during the item phase. Without it a 30,000 run
        // prints one line and then nothing for seven minutes, and a full-catalogue run would be
        // silent for over an hour — indistinguishable from a hang.
        eprintln!(
            "steam_catalogue: enumerating {} (page at {start}) …",
            ids.len()
        );
        let found = app_ids(&page.results_html);
        if found.is_empty() {
            eprintln!(
                "steam_catalogue: the store ran out at {start} of {}",
                page.total_count
            );
            break;
        }
        for id in found {
            // The store's paging can repeat a row when the catalogue shifts under it.
            if seen.insert(id) && ids.len() < wanted {
                ids.push(id);
            }
        }
        start += PAGE;
    }
    Ok(ids)
}

/// Every `data-ds-appid` in a search page, in the order it appears.
fn app_ids(html: &str) -> Vec<u32> {
    let mut found = Vec::new();
    for piece in html.split("data-ds-appid=\"").skip(1) {
        if let Some(end) = piece.find('"')
            && let Ok(id) = piece[..end].parse()
        {
            found.push(id);
        }
    }
    found
}

fn fetch(http: &reqwest::blocking::Client, ids: &[u32]) -> Result<Vec<Item>, String> {
    let request = serde_json::json!({
        "ids": ids.iter().map(|id| serde_json::json!({"appid": id})).collect::<Vec<_>>(),
        "context": {"language": "english", "country_code": "US", "steam_realm": 1},
        "data_request": {
            "include_reviews": true, "include_release": true,
            "include_platforms": true, "include_tag_count": 20,
            "include_basic_info": true, "include_included_items": true
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
