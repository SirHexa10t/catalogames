//! What Steam says about one app: everything needed for a [`crate::inventory::steam`] entry.
//!
//! Four requests per app, because no one source carries it all — and each has a trap that is
//! documented where it is handled:
//!
//! | Request | Carries | Lacks |
//! |---|---|---|
//! | `appdetails` | type, name, release date, platforms, feature list, purchase options | tags, any review figure, Deck |
//! | `appreviews` | the all-time review summary | the 30-day summary (its `day_range` changes nothing) |
//! | the store page | tags, the 30-day summary | — (and it age-gates) |
//! | the Deck report | Valve's Deck rating | — (absent from `appdetails` entirely) |
//!
//! Fetching and parsing are separate, as everywhere in this crate: the `parse_*` functions take
//! response text and do no I/O, so a caller with its own HTTP — the inventory generator keeps a
//! disk cache, for one — can drive them from saved bodies, and the tests drive them from
//! fixtures.

use std::time::Duration;

use log::debug;
use scraper::{Html, Selector};
use serde::Deserialize;

use crate::error::SafeUrl;
use crate::inventory::steam::{Deck, Os, Rating, Reviews, Vr};
use crate::{Error, Result};

// ---------------------------------------------------------------------------------------------
// Where things are
// ---------------------------------------------------------------------------------------------

/// The `appdetails` fields this crate reads, named individually.
///
/// Named rather than omitted: `basic,release_date` alone silently drops `categories`,
/// `platforms` and `package_groups`, leaving compatibility and edition detection empty instead
/// of failing; dropping the filter entirely nearly doubles the payload (24KB against 13KB) with
/// descriptions, screenshots and movies. And Steam ignores a filter name it does not recognise
/// rather than rejecting it — `package_groups` is the plausible wrong spelling of `packages` —
/// which is why [`parse_app_details`] checks that a filtered-for field actually arrived.
pub const APP_DETAILS_FILTERS: &str = "basic,release_date,categories,platforms,packages";

/// Satisfies the store's age gate. Any timestamp comfortably over eighteen years ago works;
/// this is 1979-01-01. Without it a mature title's page comes back as a check page with no tags
/// — which parses as "this game has no tags", so the gate must be cleared, not detected later.
pub const AGE_GATE_COOKIE: &str = "birthtime=283993200";

/// `l=en` is pinned on every request: the release date is a localized display string with no
/// machine-readable form behind it, and category descriptions are localized too, so the locale
/// decides what the table will contain.
pub fn app_details_url(app_id: u32) -> String {
    format!(
        "https://store.steampowered.com/api/appdetails?appids={app_id}&cc=US&l=en\
         &filters={APP_DETAILS_FILTERS}"
    )
}

/// `language=all` and `purchase_type=all` are both explicit and neither may be dropped: Steam
/// defaults `purchase_type` to `steam`, which excludes key-activated copies — about a fifth of
/// the reviews on a popular title, and precisely the copies a bundle catalogue exists to talk
/// about. `num_per_page=0` asks for the summary without review bodies, the cheapest form.
pub fn reviews_url(app_id: u32) -> String {
    format!(
        "https://store.steampowered.com/appreviews/{app_id}?json=1&num_per_page=0\
         &language=all&purchase_type=all"
    )
}

/// The Deck compatibility report. A separate request because `appdetails` carries nothing about
/// the Deck — no key matching "deck" anywhere in it — and the page renders its badge
/// client-side from this same call.
pub fn deck_url(app_id: u32) -> String {
    format!(
        "https://store.steampowered.com/saleaction/ajaxgetdeckappcompatibilityreport\
         ?nAppID={app_id}&l=english"
    )
}

/// The store page, in English so the review-row labels and verdict strings are the ones this
/// crate matches on.
pub fn page_url(app_id: u32) -> String {
    format!("https://store.steampowered.com/app/{app_id}/?l=english")
}

// ---------------------------------------------------------------------------------------------
// The result
// ---------------------------------------------------------------------------------------------

/// Everything the inventory records about a game, fetched live, with owned strings.
///
/// The runtime twin of [`crate::inventory::steam::SteamGame`], which is `Copy` over `'static`
/// strings because it lives in a generated `const` table; a value fetched at runtime cannot be
/// one. The generator turns this into a table row; a program can use it directly.
///
/// Constructible from outside the crate, unlike `SteamGame`: a plain data record that the
/// generator's own tests build by hand, and that a consumer may want to build from another
/// source. Adding a field is therefore a breaking change here, and is not done lightly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDetails {
    pub app_id: u32,
    /// Title as Steam's English store prints it.
    pub name: String,
    /// What Steam types the app as. Kept rather than filtered on, because "is this a game" is
    /// the caller's question — the generator keeps only [`AppKind::Game`]; a program looking up
    /// a DLC it owns should not be refused. And it is the only source of that fact: the store
    /// search types everything `"app"`.
    pub kind: AppKind,
    /// Release date exactly as the English store renders it, e.g. `"Jun 16, 2022"`.
    pub released: String,
    /// The last thirty days, when Steam shows a row for them.
    pub recent: Option<Reviews>,
    /// Every review, all languages, bought or key-activated.
    pub all_time: Reviews,
    /// Player tags, most-voted first, as Steam names them.
    ///
    /// Strings, not [`crate::inventory::steam::Tag`] — on purpose, and against the inventory's
    /// own rule. The enum is fail-closed: a tag Valve added since it was generated is an error
    /// there, which is right for a table being emitted and wrong for a runtime fetch, where it
    /// would make every lookup of an arbitrary title fail the day Valve adds one tag. The
    /// mapping, and its strictness, live where the table is written: in the generator.
    pub tags: Vec<String>,
    pub os: Os,
    pub vr: Vr,
    pub deck: Deck,
    /// Steam's feature list — single-player, co-op, achievements, controller support and the
    /// rest — as English descriptions.
    pub features: Vec<String>,
    /// Purchase *packages* named by `appdetails`, e.g. `"Steelrising - Bastille Edition"`,
    /// prices stripped.
    ///
    /// Named for what it is, because it is the smaller half of what the store sells: an edition
    /// sold as a package appears here, but an edition sold as a store *bundle* — the more common
    /// shape, six of twelve in a survey — exists only on the page and is not in this list.
    pub package_options: Vec<String>,
}

/// What Steam types an app as.
///
/// Closed over the types seen, with an escape for one that has not been — never a default to
/// `Game`, because that is the one wrong answer a caller cannot detect afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppKind {
    Game,
    Dlc,
    Demo,
    Music,
    Video,
    /// A type this crate has not seen; carries Steam's word for it.
    Other(String),
}

impl AppKind {
    /// Reads Steam's `type` field.
    pub fn from_steam(kind: &str) -> Self {
        match kind {
            "game" => Self::Game,
            "dlc" => Self::Dlc,
            "demo" => Self::Demo,
            "music" => Self::Music,
            "video" => Self::Video,
            other => Self::Other(other.to_owned()),
        }
    }

    /// Steam's word for it.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Game => "game",
            Self::Dlc => "dlc",
            Self::Demo => "demo",
            Self::Music => "music",
            Self::Video => "video",
            Self::Other(other) => other,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// appdetails
// ---------------------------------------------------------------------------------------------

/// What `appdetails` says about one app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDetails {
    pub app_id: u32,
    pub name: String,
    pub kind: AppKind,
    /// Steam's display string, or `None` for a title with no date.
    pub released: Option<String>,
    pub os: Os,
    /// The feature list as `(id, description)` pairs.
    ///
    /// The id is the part worth matching on. Descriptions are localized — the same category
    /// reads "Single-player" under `l=en` and "Einzelspieler" under `l=de` — so a string match
    /// silently produces an empty result the moment Steam picks a different language.
    pub categories: Vec<(u64, String)>,
    /// Purchase packages with their prices stripped; see [`GameDetails::package_options`].
    pub package_options: Vec<String>,
}

/// Reads an `appdetails` response. `None` when Steam has no such app (`success: false`).
pub fn parse_app_details(json: &str, app_id: u32) -> Result<Option<AppDetails>> {
    let root: serde_json::Value = serde_json::from_str(json).map_err(|source| Error::Payload {
        url: app_details_url(app_id).into(),
        source,
    })?;
    let entry = &root[app_id.to_string()];
    if entry["success"].as_bool() != Some(true) {
        return Ok(None);
    }
    let data = &entry["data"];

    // A field the filter list is supposed to deliver must actually be present. Steam ignores an
    // unrecognised filter name silently, and a list it recognises nothing in returns an empty
    // response with no error — so a wrong name in `APP_DETAILS_FILTERS` would otherwise show up
    // as every compatibility field being empty, which looks like data.
    if !data["platforms"].is_object() {
        return Err(Error::Drift {
            subject: format!("appdetails {app_id}"),
            detail: "no platforms in the response — a name in the filter list is not one Steam \
                     recognises"
                .to_owned(),
        });
    }

    let flag = |key: &str| data["platforms"][key].as_bool().unwrap_or(false);
    Ok(Some(AppDetails {
        app_id,
        name: data["name"].as_str().unwrap_or_default().to_owned(),
        kind: AppKind::from_steam(data["type"].as_str().unwrap_or_default()),
        released: data["release_date"]["date"].as_str().map(str::to_owned),
        os: Os {
            windows: flag("windows"),
            mac: flag("mac"),
            linux: flag("linux"),
        },
        categories: data["categories"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|c| {
                        Some((c["id"].as_u64()?, c["description"].as_str()?.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        package_options: data["package_groups"]
            .as_array()
            .map(|groups| {
                groups
                    .iter()
                    .filter_map(|g| g["subs"].as_array())
                    .flatten()
                    .filter_map(|sub| sub["option_text"].as_str())
                    .map(strip_price)
                    .collect()
            })
            .unwrap_or_default(),
    }))
}

/// Drops the trailing price from a purchase option: `"X - $49.99"` -> `"X"`.
///
/// Split from the right, because option names contain `" - "` themselves.
pub fn strip_price(option: &str) -> String {
    match option.rsplit_once(" - ") {
        Some((name, tail)) if tail.starts_with('$') => name.trim().to_owned(),
        _ => option.trim().to_owned(),
    }
}

/// Category id meaning a headset is required.
const CATEGORY_VR_ONLY: u64 = 54;
/// Category id meaning a headset is optional.
const CATEGORY_VR_SUPPORTED: u64 = 53;

/// Reads VR support out of Steam's feature list.
///
/// There is no dedicated VR field; VR is an ordinary category. Four look relevant and only two
/// discriminate:
///
/// - **54 "VR Only"** — a headset is required.
/// - **53 "VR Supported"** — a headset is optional.
/// - 52 "Tracked Controller Support" — appears on both, so it says nothing.
/// - 31 "VR Support" — a legacy umbrella, present on Half-Life: Alyx and *absent* from both
///   Beat Saber and No Man's Sky, so it discriminates nothing either.
///
/// 54 is tested first and 31 is never consulted. Both matter: Alyx carries 54 **and** 31, so
/// VR-only is not the absence of VR-support, and reading 31 as "optional" would file the
/// flagship VR-only title as playable on a monitor.
///
/// Matched by id because descriptions are localized.
pub fn vr_support(categories: &[(u64, String)]) -> Vr {
    if categories.iter().any(|(id, _)| *id == CATEGORY_VR_ONLY) {
        Vr::Only
    } else if categories
        .iter()
        .any(|(id, _)| *id == CATEGORY_VR_SUPPORTED)
    {
        Vr::Supported
    } else {
        Vr::None
    }
}

// ---------------------------------------------------------------------------------------------
// appreviews
// ---------------------------------------------------------------------------------------------

/// Reads the all-time review summary, checking the derived band against Valve's own.
pub fn parse_reviews(json: &str, app_id: u32) -> Result<Reviews> {
    let subject = || format!("appreviews {app_id}");
    let root: serde_json::Value = serde_json::from_str(json).map_err(|source| Error::Payload {
        url: reviews_url(app_id).into(),
        source,
    })?;
    let summary = &root["query_summary"];

    // `total_reviews`, never `num_reviews` — the latter counts the reviews in this response,
    // which `num_per_page=0` makes zero.
    let count = summary["total_reviews"]
        .as_u64()
        .ok_or_else(|| Error::Drift {
            subject: subject(),
            detail: "no total_reviews in the summary".to_owned(),
        })? as u32;
    let positive = summary["total_positive"]
        .as_u64()
        .ok_or_else(|| Error::Drift {
            subject: subject(),
            detail: "no total_positive in the summary".to_owned(),
        })? as u32;
    let reviews = Reviews {
        approval: approval_percent(positive, count),
        count,
    };

    // Valve ships its own band index in the same response. Comparing the derived band against
    // it costs nothing and is the only thing standing between a quietly retuned threshold and a
    // table full of subtly wrong verdicts. Steam assigns the band from the exact ratio rather
    // than a rounded percentage, which is why the stored figure is floored — flooring lands in
    // the same band as the exact ratio, and rounding measurably does not.
    if let Some(score) = summary["review_score"].as_u64() {
        let valve = u8::try_from(score)
            .ok()
            .and_then(Rating::from_valve_score)
            .ok_or_else(|| Error::Drift {
                subject: subject(),
                detail: format!("unknown review_score {score}"),
            })?;
        let derived = reviews.rating();
        if valve != derived {
            return Err(Error::Drift {
                subject: subject(),
                detail: format!(
                    "Steam says {valve:?} but {}% over {count} reviews derives {derived:?} — the \
                     band thresholds have moved",
                    reviews.approval
                ),
            });
        }
    }
    Ok(reviews)
}

/// Positive share as a whole percent, floored.
///
/// Floored rather than rounded, and that is not a rounding preference: Steam assigns a band
/// from the exact ratio, so a title on 79.98% is "Mostly Positive" even though it displays as
/// 80%. Flooring reproduces that; rounding crosses the boundary and picks the wrong band.
pub fn approval_percent(positive: u32, total: u32) -> u8 {
    if total == 0 {
        return 0;
    }
    ((u64::from(positive) * 100) / u64::from(total)) as u8
}

// ---------------------------------------------------------------------------------------------
// the Deck report
// ---------------------------------------------------------------------------------------------

/// Reads Valve's Deck verdict out of the compatibility report.
pub fn parse_deck(json: &str, app_id: u32) -> Result<Deck> {
    let subject = || format!("deck report {app_id}");
    let root: serde_json::Value = serde_json::from_str(json).map_err(|source| Error::Payload {
        url: deck_url(app_id).into(),
        source,
    })?;

    // `success: 1` comes back even for an app-id that does not exist, so it is not a
    // found-check. What differs is the shape of `results`: an object for a real app, an empty
    // array otherwise. That distinction has to survive — "no report" and a genuine `Unknown`
    // rating are different facts, and collapsing the first into the second would hide every
    // failed lookup behind a plausible value.
    let results = &root["results"];
    if !results.is_object() {
        return Err(Error::Drift {
            subject: subject(),
            detail: "no compatibility report for this app".to_owned(),
        });
    }

    // Read `resolved_category` by exact name: the report carries three more fields whose names
    // differ by one word — steamos_, machine_ and frame_resolved_category — and they disagree.
    // Terraria is 3 here and 2 under steamos_.
    //
    // Absent is an error, not `Unknown`. A title Valve has not rated reports an explicit `0`
    // (measured on an unrated DLC), so "no such field" is a shape this crate has not seen — and
    // `Unknown` has a defined meaning in the inventory's house rules. Mapping a missing field
    // onto it would let a field rename relabel every game as unrated with nothing flagged.
    let Some(category) = results["resolved_category"].as_u64() else {
        return Err(Error::Drift {
            subject: subject(),
            detail: "report has no resolved_category".to_owned(),
        });
    };
    u8::try_from(category)
        .ok()
        .and_then(Deck::from_category)
        .ok_or_else(|| Error::Drift {
            subject: subject(),
            detail: format!("unknown compatibility category {category}"),
        })
}

// ---------------------------------------------------------------------------------------------
// the store page
// ---------------------------------------------------------------------------------------------

/// What only the store page carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFacts {
    /// The thirty-day summary, when Steam shows a row for it.
    pub recent: Option<Reviews>,
    /// Player tags, most-voted first, as Steam names them. See [`GameDetails::tags`].
    pub tags: Vec<String>,
}

/// Reads tags and the thirty-day review summary out of a store page.
pub fn parse_store_page(html: &str, app_id: u32) -> Result<PageFacts> {
    let document = Html::parse_document(html);
    Ok(PageFacts {
        recent: recent_reviews(&document, app_id)?,
        tags: tags(html, app_id)?,
    })
}

/// The last-thirty-days summary, cross-checked against itself.
///
/// The page states the same count twice — in the row's tooltip and in its visible figure — from
/// different parts of the template. Comparing them costs nothing and catches a layout change
/// that would otherwise yield a confident wrong number. The verdict the page prints is then
/// compared against the one derived from the figures, for the same reason as in
/// [`parse_reviews`].
fn recent_reviews(document: &Html, app_id: u32) -> Result<Option<Reviews>> {
    let subject = || format!("store page {app_id}");
    let drift = |detail: String| Error::Drift {
        subject: subject(),
        detail,
    };
    let row = Selector::parse(".user_reviews_summary_row").expect("static selector");
    let subtitle = Selector::parse(".subtitle").expect("static selector");
    let summary = Selector::parse(".game_review_summary").expect("static selector");
    let count = Selector::parse(".responsive_hidden").expect("static selector");

    for element in document.select(&row) {
        let label = element
            .select(&subtitle)
            .next()
            .map(|e| e.text().collect::<String>());
        if !label.unwrap_or_default().contains("Recent") {
            continue;
        }
        let text = element
            .select(&summary)
            .next()
            .map(|e| e.text().collect::<String>())
            .ok_or_else(|| drift("recent row has no verdict".to_owned()))?;
        let shown_rating = Rating::from_steam(&text)
            .ok_or_else(|| drift(format!("unknown recent rating {text:?}")))?;

        let shown = element
            .select(&count)
            .next()
            .map(|e| e.text().collect::<String>())
            .and_then(|t| trailing_count(&t))
            .ok_or_else(|| drift("recent row has no count".to_owned()))?;

        let tooltip = element.value().attr("data-tooltip-html");
        let stated = tooltip
            .and_then(tooltip_count)
            .ok_or_else(|| drift("recent row has no tooltip count".to_owned()))?;
        if shown != stated {
            return Err(drift(format!(
                "recent count disagrees with itself ({shown} vs {stated}); the page layout has \
                 changed"
            )));
        }

        let approval = tooltip
            .and_then(leading_percent)
            .ok_or_else(|| drift("recent tooltip states no percentage".to_owned()))?;
        let reviews = Reviews {
            approval,
            count: shown,
        };
        let derived = reviews.recent_rating();
        if derived != shown_rating {
            return Err(drift(format!(
                "page says {shown_rating:?} for the last 30 days but {approval}% over {shown} \
                 reviews derives {derived:?}"
            )));
        }
        return Ok(Some(reviews));
    }
    Ok(None)
}

#[derive(Deserialize)]
struct TagJson {
    name: String,
}

/// Player tags, taken from the tag modal's own JSON rather than the visible links — the modal
/// carries the full ordered list, while the links are truncated and some are hidden by CSS.
fn tags(html: &str, app_id: u32) -> Result<Vec<String>> {
    let drift = |detail: String| Error::Drift {
        subject: format!("store page {app_id}"),
        detail,
    };
    let Some(at) = html.find("InitAppTagModal") else {
        // A caller with its own HTTP stack never sees the redirect the gate performs, so the
        // content is its only evidence. The gate page is positively identifiable — it names
        // itself dozens of times — and a caller who forgot the cookie deserves the exact
        // diagnosis rather than "something changed".
        return Err(drift(if html.contains("agecheck") {
            "age gate not cleared — the page is Steam's age check, not the game".to_owned()
        } else {
            "no tag data; the page layout has changed".to_owned()
        }));
    };
    let rest = &html[at..];
    let open = rest
        .find('[')
        .ok_or_else(|| drift("tag data has no list".to_owned()))?;

    // Let serde find the end of the array: it stops at the closing bracket and ignores the
    // trailing JavaScript, which hand-matching brackets would get wrong on a name containing one.
    let tags: Vec<TagJson> = serde_json::Deserializer::from_str(&rest[open..])
        .into_iter()
        .next()
        .ok_or_else(|| drift("tag list is empty".to_owned()))?
        .map_err(|source| Error::Payload {
            url: page_url(app_id).into(),
            source,
        })?;

    Ok(tags.into_iter().map(|tag| tag.name).collect())
}

/// The review count a summary row's tooltip states.
///
/// The recent tooltip carries three numbers — `"83% of the 73 user reviews in the last 30 days
/// are positive."` — and the count is the **middle** one. Truncating at `" user reviews"` first
/// leaves `"83% of the 73"`, whose trailing number is the count. Taking the trailing number of
/// the whole tooltip would instead yield the day window, `30`: a perfectly plausible review
/// count that would be identical on every title, which is the kind of wrong that never
/// announces itself. The all-time tooltip has only two numbers, so the same rule reads it too.
fn tooltip_count(tooltip: &str) -> Option<u32> {
    trailing_count(tooltip.split(" user reviews").next()?)
}

/// The percentage a tooltip opens with: `"83% of the 73 user reviews…"` -> `83`.
fn leading_percent(tooltip: &str) -> Option<u8> {
    let (number, _) = tooltip.split_once('%')?;
    number.trim().parse().ok()
}

/// The **last** number in `text`, thousands separators included.
///
/// Last, not first, and not every digit concatenated: the strings this reads carry more than
/// one number. A tooltip reads `"75% of the 12 user reviews in the last 30 days are positive."`
/// — the percentage comes first and the count second, so taking every digit yields `7512` and
/// taking the first yields the percentage. Only the trailing number is the count.
fn trailing_count(text: &str) -> Option<u32> {
    let mut number = String::new();
    let mut started = false;
    for character in text.chars().rev() {
        match character {
            c if c.is_ascii_digit() => {
                number.insert(0, c);
                started = true;
            }
            ',' if started => continue,
            _ if started => break,
            _ => continue,
        }
    }
    number.parse().ok()
}

// ---------------------------------------------------------------------------------------------
// Putting it together
// ---------------------------------------------------------------------------------------------

/// Combines the four sources into one record.
pub fn assemble(
    details: AppDetails,
    page: PageFacts,
    all_time: Reviews,
    deck: Deck,
) -> GameDetails {
    GameDetails {
        app_id: details.app_id,
        name: details.name,
        kind: details.kind,
        // A title with no date is a real thing — pre-orders say "Coming soon" — but an empty
        // string in a table reads as a parse failure, so it is named as what it is.
        released: details.released.unwrap_or_else(|| "Unknown".to_owned()),
        recent: page.recent,
        all_time,
        tags: page.tags,
        os: details.os,
        vr: vr_support(&details.categories),
        deck,
        features: details
            .categories
            .into_iter()
            .map(|(_, name)| name)
            .collect(),
        package_options: details.package_options,
    }
}

// ---------------------------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------------------------

/// How this crate names itself to Steam by default. Honest — Steam serves it exactly what it
/// serves a browser, checked against the live site — and carrying no personal identifier.
pub const DEFAULT_USER_AGENT: &str = concat!("catalogames/", env!("CARGO_PKG_VERSION"));

/// Pause between the four requests one lookup makes.
///
/// Steam publishes no rate limit and none is probed for here — establishing one means tripping
/// it. One request per second is slower than a person browsing and is the defensible floor. A
/// library with no pacing lets every consumer hammer Steam by default, with this project's name
/// on the request.
pub const DEFAULT_SPACING: Duration = Duration::from_secs(1);

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Fetches what Steam says about an app.
///
/// ```no_run
/// let game = catalogames::steam::store::Client::new()?.fetch(1202130)?;
/// println!("{}: {}", game.name, game.all_time.rating().as_str());
/// # Ok::<(), catalogames::Error>(())
/// ```
///
/// Three things about how it talks to Steam are policy a library should not impose silently,
/// so each is a documented default a consumer can change through [`Client::configure`]:
///
/// - **User-Agent.** Defaults to [`DEFAULT_USER_AGENT`]. A program embedding this crate should
///   name itself in front of it (`myapp/2.1 catalogames/0.1`), the convention well-behaved
///   clients follow — otherwise every consumer presents to Valve as this project, which is
///   misleading and means one careless consumer gets the project's name throttled.
/// - **Pacing.** Defaults to [`DEFAULT_SPACING`] between requests.
/// - **The age gate.** Steam hides mature titles behind an age check; without clearing it, a
///   fetch returns a check page with no tags and no review rows — silently incomplete data.
///   Clearing it means sending a birth date, which asserts on the consumer's behalf that the
///   viewer is an adult. On by default for completeness, because the alternative is quietly
///   worse data; a consumer that must not make that assertion turns it off and gets a loud
///   [`Error::Drift`] on a gated title instead of a thinner record.
#[derive(Debug)]
pub struct Client {
    http: reqwest::blocking::Client,
    spacing: Duration,
    clear_age_gate: bool,
}

impl Client {
    /// The defaults: this crate's own User-Agent, one request per second, age gate cleared.
    pub fn new() -> Result<Self> {
        Self::configure(DEFAULT_USER_AGENT, DEFAULT_SPACING, true)
    }

    /// A client with explicit policy. See the type's documentation for what each setting means.
    pub fn configure(user_agent: &str, spacing: Duration, clear_age_gate: bool) -> Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(user_agent.to_owned())
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|source| Error::Fetch {
                url: "https://store.steampowered.com".into(),
                source: source.into(),
            })?;
        Ok(Self {
            http,
            spacing,
            clear_age_gate,
        })
    }

    /// The pause this client keeps between requests, for a caller pacing a series of its own.
    #[must_use]
    pub const fn spacing(&self) -> Duration {
        self.spacing
    }

    /// Everything the inventory records about `app_id`, in four paced requests.
    pub fn fetch(&self, app_id: u32) -> Result<GameDetails> {
        let details = parse_app_details(&self.get(&app_details_url(app_id), false)?, app_id)?
            .ok_or(Error::NoSuchApp { app_id })?;
        std::thread::sleep(self.spacing);
        let page = parse_store_page(&self.get(&page_url(app_id), true)?, app_id)?;
        std::thread::sleep(self.spacing);
        let all_time = parse_reviews(&self.get(&reviews_url(app_id), false)?, app_id)?;
        std::thread::sleep(self.spacing);
        let deck = parse_deck(&self.get(&deck_url(app_id), false)?, app_id)?;
        Ok(assemble(details, page, all_time, deck))
    }

    /// One request to Steam, with this client's policy applied.
    ///
    /// Public so that the one correct way to talk to Steam exists once: the inventory generator
    /// keeps a disk cache and wraps this rather than re-implementing the headers, the gate and
    /// the throttle handling. `age_gated` is set only for the store page, the one request that
    /// needs the cookie.
    pub fn get(&self, url: &str, age_gated: bool) -> Result<String> {
        // Redacted once; see the same construction in `humble::Client::get`.
        let safe = SafeUrl::from(url);
        let fetch_error = |source: reqwest::Error| Error::Fetch {
            url: safe.clone(),
            source: source.into(),
        };
        debug!("GET {safe}");
        let mut request = self.http.get(url);
        if age_gated && self.clear_age_gate {
            request = request.header("Cookie", AGE_GATE_COOKIE);
        }
        let response = request.send().map_err(fetch_error)?;

        // Checked before the body is read: a throttle or an error page is not data.
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited {
                url: safe.clone(),
                retry_after: response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned),
            });
        }
        if !status.is_success() {
            return Err(Error::Status {
                url: safe.clone(),
                status: status.as_u16(),
            });
        }
        // The age gate redirects rather than failing, so a 200 is not proof of a real page.
        if age_gated && response.url().path().contains("/agecheck/") {
            return Err(Error::Drift {
                subject: format!("GET {safe}"),
                detail: "age gate not cleared".to_owned(),
            });
        }
        let body = response.text().map_err(fetch_error)?;
        debug!("  {} bytes", body.len());
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_is_read_from_the_visible_parenthesised_number() {
        assert_eq!(trailing_count("(7,217)"), Some(7217));
        assert_eq!(trailing_count("\n  (73)\n"), Some(73));
        assert_eq!(trailing_count("no digits here"), None);
        assert_eq!(trailing_count(""), None);
    }

    #[test]
    fn a_recent_tooltip_yields_the_review_count_not_the_day_window() {
        // Three numbers, count in the middle. Two ways to get this wrong, both pinned:
        // concatenating every digit gives 8373, and the whole string's trailing number gives
        // 30 — the day window, which is plausible AND identical on every title.
        let tooltip = "83% of the 73 user reviews in the last 30 days are positive.";
        assert_eq!(tooltip_count(tooltip), Some(73));
        assert_ne!(
            tooltip_count(tooltip),
            Some(30),
            "read the day window as the count"
        );
        assert_ne!(
            tooltip_count(tooltip),
            Some(8373),
            "concatenated every digit"
        );
    }

    #[test]
    fn a_recent_tooltip_whose_count_is_not_the_day_window_still_reads_correctly() {
        assert_eq!(
            tooltip_count("75% of the 12 user reviews in the last 30 days are positive."),
            Some(12)
        );
        assert_eq!(
            tooltip_count("96% of the 9,556 user reviews in the last 30 days are positive."),
            Some(9556)
        );
    }

    #[test]
    fn an_all_time_tooltip_has_only_two_numbers_and_reads_the_same_way() {
        assert_eq!(
            tooltip_count("88% of the 7,217 user reviews in your language are positive"),
            Some(7217)
        );
    }

    #[test]
    fn a_tooltip_percentage_is_read_from_the_front() {
        assert_eq!(
            leading_percent("83% of the 73 user reviews in the last 30 days."),
            Some(83)
        );
        assert_eq!(leading_percent("100% of the 12 user reviews"), Some(100));
        assert_eq!(leading_percent("no percentage here"), None);
    }

    #[test]
    fn an_approval_percentage_is_floored_not_rounded() {
        // Steam assigns a band from the exact ratio, so 79.97% is "Mostly Positive" even
        // though it displays as 80%. Flooring lands in the same band; rounding does not.
        assert_eq!(approval_percent(2272, 2841), 79);
        assert_eq!(approval_percent(19563, 21898), 89);
        assert_eq!(approval_percent(1, 1), 100);
        assert_eq!(approval_percent(0, 10), 0);
        assert_eq!(
            approval_percent(0, 0),
            0,
            "nothing reviewed is zero, not a panic"
        );
    }

    #[test]
    fn a_purchase_option_loses_its_price_but_keeps_its_own_dashes() {
        assert_eq!(
            strip_price("Steelrising - Bastille Edition - $59.99"),
            "Steelrising - Bastille Edition"
        );
        assert_eq!(strip_price("Frostpunk - $29.99"), "Frostpunk");
        assert_eq!(
            strip_price("Disco Elysium - The Final Cut"),
            "Disco Elysium - The Final Cut"
        );
    }

    #[test]
    fn steams_types_are_read_and_an_unfamiliar_one_is_kept_rather_than_assumed() {
        assert_eq!(AppKind::from_steam("game"), AppKind::Game);
        assert_eq!(AppKind::from_steam("dlc"), AppKind::Dlc);
        assert_eq!(AppKind::from_steam("demo"), AppKind::Demo);
        // Never a default to Game: that is the one wrong answer a caller cannot detect.
        assert_eq!(
            AppKind::from_steam("hardware"),
            AppKind::Other("hardware".to_owned())
        );
        assert_eq!(AppKind::from_steam("hardware").as_str(), "hardware");
        assert_ne!(AppKind::from_steam(""), AppKind::Game);
    }

    #[test]
    fn vr_only_wins_over_the_legacy_umbrella_category() {
        // Half-Life: Alyx carries BOTH 54 "VR Only" and 31 "VR Support". Reading 31 as
        // "optional" would file the flagship VR-only title as playable on a monitor.
        let alyx = [
            (2, "Single-player".to_owned()),
            (54, "VR Only".to_owned()),
            (31, "VR Support".to_owned()),
            (52, "Tracked Controller Support".to_owned()),
        ];
        assert_eq!(vr_support(&alyx), Vr::Only);
    }

    #[test]
    fn vr_support_is_read_from_ids_not_descriptions() {
        let optional = [(53, "Von VR unterstützt".to_owned())];
        assert_eq!(vr_support(&optional), Vr::Supported);
        let none = [
            (2, "Einzelspieler".to_owned()),
            (52, "Tracked Controller Support".to_owned()),
        ];
        assert_eq!(vr_support(&none), Vr::None);
    }
}
