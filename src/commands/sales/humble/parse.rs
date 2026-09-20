//! Turning Humble's pages into [`Bundle`]s. Pure: no I/O happens here.
//!
//! Kept separate from [`super::client`] so the whole of the fragile part — the
//! part that breaks when Humble redesigns — is testable from a saved page with
//! no network involved.

use log::debug;
use scraper::{Html, Selector};

use super::schema::{BundleData, BundlePage, LandingPage, TierDisplay};
use crate::clock::{self, Timestamp};
use crate::{Bundle, Error, Game, Money, Price, Problem, ProblemKind, Result};
use std::collections::{BTreeMap, BTreeSet};

/// The page listing current game bundles.
pub const INDEX_URL: &str = "https://www.humblebundle.com/games";

/// Origin used to absolutise the site-relative links on the index page.
const ORIGIN: &str = "https://www.humblebundle.com";

pub(super) const INDEX_BLOCK: &str = "landingPage-json-data";
pub(super) const BUNDLE_BLOCK: &str = "webpack-bundle-page-data";

/// One row of the index page: enough to fetch and check a bundle's own page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub title: String,
    /// Absolute URL of the bundle's page.
    pub url: String,
    /// Game count as Humble advertises it on the index, when it says one.
    pub advertised_games: Option<usize>,
    /// When Humble stops selling the bundle, when it says.
    pub ends_at: Option<Timestamp>,
}

/// Reads the JSON Humble embeds in a `<script type="application/json">` tag.
///
/// The inner text needs no unescaping before `serde_json` sees it. Humble emits
/// the payload with JSON escaping and no HTML entities — measured on both pages:
/// zero literal `<`, ~300 `\u003c` escapes, zero `&lt;`/`&amp;`. The markup that
/// appears in blurbs (`<em>`, `<br>`) is escaped along with everything else, so
/// a literal `</script>` cannot occur inside the payload either.
///
/// An HTML parser is still used to *find* the element, rather than scanning for
/// the id, so that attribute order and spacing in the opening tag stay Humble's
/// business rather than ours.
fn json_block(html: &str, block: &'static str, url: &str) -> Result<String> {
    let selector = Selector::parse(&format!("script#{block}"))
        .expect("block ids are crate constants and are valid CSS identifiers");

    Html::parse_document(html)
        .select(&selector)
        .next()
        .map(|element| {
            let json = element.inner_html();
            debug!("  read {} bytes from <script id=\"{block}\">", json.len());
            json
        })
        .ok_or_else(|| Error::MissingDataBlock {
            block,
            url: url.into(),
        })
}

fn decode<T: serde::de::DeserializeOwned>(json: &str, block: &'static str, url: &str) -> Result<T> {
    serde_json::from_str(json).map_err(|source| Error::Schema {
        block,
        url: url.into(),
        source,
    })
}

/// Extracts the bundle rows from the games index page.
pub fn parse_index(html: &str, url: &str) -> Result<Vec<Entry>> {
    let page: LandingPage = decode(&json_block(html, INDEX_BLOCK, url)?, INDEX_BLOCK, url)?;

    let entries: Vec<Entry> = page
        .data
        .games
        .mosaic
        .into_iter()
        .flat_map(|mosaic| mosaic.products)
        .map(|product| Entry {
            url: absolutise(&product.product_url),
            advertised_games: advertised_game_count(&product.hover_highlights),
            ends_at: product
                .end_date
                .as_deref()
                .and_then(|date| clock::parse_reported(date, &product.tile_name)),
            title: product.tile_name,
        })
        .collect();

    if entries.is_empty() {
        return Err(Error::NoBundles { url: url.into() });
    }
    Ok(entries)
}

fn absolutise(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_owned()
    } else {
        format!("{ORIGIN}{path}")
    }
}

/// Pulls the count out of a caption like `"7 games"`.
///
/// Returns `None` rather than guessing when no caption is shaped that way: the
/// count is a cross-check, and a cross-check that invents its own numbers is
/// worse than none.
fn advertised_game_count(highlights: &[String]) -> Option<usize> {
    highlights.iter().find_map(|highlight| {
        let mut words = highlight.split_whitespace();
        let count: usize = words.next()?.parse().ok()?;
        let noun = words.next()?.to_ascii_lowercase();
        noun.starts_with("game").then_some(count)
    })
}

/// Builds a [`Bundle`] from one bundle page, plus anything suspicious about it.
///
/// `entry` supplies the title and URL, which the index page states and the
/// bundle page does not repeat in the same form.
pub fn parse_bundle(html: &str, entry: &Entry) -> Result<(Bundle, Vec<Problem>)> {
    let page: BundlePage = decode(
        &json_block(html, BUNDLE_BLOCK, &entry.url)?,
        BUNDLE_BLOCK,
        &entry.url,
    )?;
    let data = page.bundle_data;

    let (tier_name, tier) =
        largest_tier(&data.tier_display_data).ok_or_else(|| Error::NoTiers {
            title: entry.title.clone(),
        })?;

    debug!(
        "  {} tier(s); largest is {:?} with {} item(s)",
        data.tier_display_data.len(),
        tier_name,
        tier.tier_item_machine_names.len()
    );

    let (games, unresolved) = resolve_titles(tier, &data);
    debug!("  resolved {} game title(s)", games.len());
    if games.is_empty() {
        return Err(Error::NoGames {
            title: entry.title.clone(),
        });
    }

    let mut problems = Vec::new();
    let mut report = |kind| {
        problems.push(Problem {
            bundle: entry.title.clone(),
            kind,
        })
    };

    if !unresolved.is_empty() {
        report(ProblemKind::UnresolvedItems {
            machine_names: unresolved,
        });
    }
    for smaller in non_cumulative_tiers(tier_name, tier, &data.tier_display_data) {
        report(ProblemKind::TiersNotCumulative {
            largest: tier_name.clone(),
            smaller,
        });
    }
    if let Some(advertised) = entry.advertised_games {
        debug!(
            "  vendor advertises {advertised}, found {} ({})",
            games.len(),
            if advertised == games.len() {
                "agrees"
            } else {
                "DISAGREES"
            }
        );
    }
    if let Some(advertised) = entry.advertised_games
        && advertised != games.len()
    {
        report(ProblemKind::GameCountMismatch {
            advertised,
            found: games.len(),
        });
    }

    // The price of the tier whose games are the ones listed. Humble sells a bundle as one
    // payment per tier, so the largest tier's price is exactly "the least this many games can
    // be bought for" — which is the question a bundle listing is asked.
    let price = data
        .tier_pricing_data
        .get(tier_name)
        .and_then(|tier| Money::from_major(tier.price.amount, &tier.price.currency))
        .map(Price::Whole);
    if price.is_none() {
        debug!("  no usable price for tier {tier_name:?}");
    }

    let bundle = Bundle {
        title: entry.title.clone(),
        url: entry.url.clone(),
        price,
        ends_at: entry.ends_at,
        games,
    };
    Ok((bundle, problems))
}

/// The tier holding the most items, ties broken by tier name.
///
/// Chosen by size rather than by position: Humble's own `tier_order` runs from
/// most expensive to cheapest, so "last" and "largest" are not the same thing,
/// and relying on either ordering would break quietly the day it changed.
fn largest_tier(tiers: &BTreeMap<String, TierDisplay>) -> Option<(&String, &TierDisplay)> {
    tiers.iter().max_by(|(a_name, a), (b_name, b)| {
        a.tier_item_machine_names
            .len()
            .cmp(&b.tier_item_machine_names.len())
            .then_with(|| a_name.cmp(b_name))
    })
}

/// Looks each of the tier's machine names up in the item table.
///
/// Returns the games it could name and the machine names it could not, in the
/// vendor's own order.
fn resolve_titles(tier: &TierDisplay, data: &BundleData) -> (Vec<Game>, Vec<String>) {
    let mut games = Vec::with_capacity(tier.tier_item_machine_names.len());
    let mut unresolved = Vec::new();

    for machine_name in &tier.tier_item_machine_names {
        match data
            .tier_item_data
            .get(machine_name)
            .and_then(|item| item.human_name.as_deref())
        {
            Some(title) => games.push(Game {
                title: title.to_owned(),
                machine_name: machine_name.clone(),
                // Humble publishes no Steam id: its bundle data carries only flags saying a
                // key is delivered through Steam. Checked exhaustively against a live page.
                steam_app_id: None,
                // Humble's item table names one title per entry and nothing beneath it, so
                // there is no pack to take apart here.
                contains: Vec::new(),
            }),
            None => unresolved.push(machine_name.clone()),
        }
    }
    (games, unresolved)
}

/// Names the tiers that are *not* contained in the largest one.
///
/// Always empty while tiers stay cumulative, which is the only condition under
/// which the largest tier means "everything in the bundle".
fn non_cumulative_tiers(
    largest_name: &str,
    largest: &TierDisplay,
    tiers: &BTreeMap<String, TierDisplay>,
) -> Vec<String> {
    let contents: BTreeSet<&str> = largest
        .tier_item_machine_names
        .iter()
        .map(String::as_str)
        .collect();

    tiers
        .iter()
        .filter(|(name, _)| name.as_str() != largest_name)
        .filter(|(_, tier)| {
            !tier
                .tier_item_machine_names
                .iter()
                .all(|name| contents.contains(name.as_str()))
        })
        .map(|(name, _)| name.clone())
        .collect()
}
