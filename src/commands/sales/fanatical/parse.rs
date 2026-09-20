//! Turning Fanatical's two payloads into [`Bundle`]s. Pure: no I/O happens here.

use std::collections::BTreeMap;

use super::schema::{Catalogue, ContainedProduct, Listed, Product, Products, SearchResults, Tier};
use crate::clock::{self, Timestamp};
use crate::{Bundle, Error, Game, Money, Price, Problem, ProblemKind, Result};
use log::debug;

/// The bundle listing. Explicitly `Allow`ed in Fanatical's `robots.txt`.
pub const CATALOGUE_URL: &str = "https://www.fanatical.com/api/all/en";

/// Fanatical's MCP endpoint, which its `llms.txt` asks clients to prefer.
pub const MCP_URL: &str = "https://mcp.fanatical.com/mcp";

/// The `type` marking a bundle of games.
///
/// Matched positively, never by excluding the known non-game types. `"bundle"` is the unmarked
/// default, so a deny-list would silently admit any category Fanatical adds later — and it
/// would look like it was working.
const GAMES_BUNDLE: &str = "bundle";

/// Where a bundle lives on the site.
pub fn bundle_url(slug: &str) -> String {
    format!("https://www.fanatical.com/en/pick-and-mix/{slug}")
}

/// One row of the listing: enough to fetch a bundle's contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub title: String,
    pub slug: String,
    pub url: String,
    /// Product slugs in the pool, as the listing states them.
    pub product_slugs: Vec<String>,
    /// How many products the cheapest tier lets you pick, when tiers are stated.
    pub smallest_pick: Option<u32>,
    /// What the listed games cost, read from the tier that gives the most of them.
    pub price: Option<Price>,
    /// When Fanatical stops selling the bundle, when the listing says.
    pub ends_at: Option<Timestamp>,
}

/// Reads the games bundles out of the catalogue.
///
/// Non-game categories are dropped here rather than later: the listing's `type` separates them
/// exactly, and filtering at this point is what keeps the run to fifteen detail calls instead
/// of thirty-six. Filtering instead on "contains Steam games" is not possible from the listing
/// at all — its product entries carry no Steam data.
pub fn parse_catalogue(json: &str) -> Result<Vec<Entry>> {
    let catalogue: Catalogue = serde_json::from_str(json).map_err(|source| Error::Payload {
        url: CATALOGUE_URL.into(),
        source,
    })?;

    let entries: Vec<Entry> = catalogue
        .pickandmix
        .into_iter()
        .filter(|listed| listed.kind == GAMES_BUNDLE)
        .map(entry)
        .collect();

    if entries.is_empty() {
        return Err(Error::NoBundles {
            url: CATALOGUE_URL.into(),
        });
    }
    Ok(entries)
}

/// The currency the listing is read in.
///
/// `api/all/en` quotes every currency at once, so one has to be chosen, and it is chosen **by
/// key**: JSON object order is not a contract and "the first entry" would drift silently.
///
/// USD because that is what Fanatical's own API resolves this client to — its MCP responses
/// come back `region: US, currency: USD`. Taking prices from this catalogue rather than from the
/// MCP record is deliberate and should stay that way: the catalogue is already fetched, so it
/// costs no request, and it quotes every currency at once, whereas the MCP folds to whichever
/// region the *caller* resolves to. Switching to the MCP price would look like a simplification
/// and would quietly make the output depend on where it was run.
const QUOTED_CURRENCY: &str = "USD";

/// What the listed games cost, from the tier that grants the most of them.
///
/// Fanatical's tiers are price points over one shared pool — every tier draws from all of it,
/// and a tier's price is the total for that many picks. So the rate per pick is the price
/// divided by the count, and it genuinely is what one game costs at that tier. **This rule is
/// Fanatical's alone.** It must not be lifted anywhere else: a store whose tiers are cumulative
/// partitions rather than price points would produce an average nobody can pay.
///
/// Which shape applies is decided by asking whether the best tier takes the **whole pool**:
///
/// * **No tiers** — nothing is claimed. The listing published no price.
/// * **The best tier takes every product** — a whole-bundle price. That payment, those games,
///   no choice to make, which is how the charity and themed bundles are sold.
/// * **The best tier takes fewer** — a pick-and-mix, and the rate per pick is what it costs.
///
/// Counting tiers instead would be simpler and wrong, and Fanatical's own catalogue says so:
/// `your-digital-life-the-complete-manuals` has **four** tiers whose largest takes all 28 of its
/// 28 products. Under a tier-count rule that renders a per-game rate for a bundle that is only
/// sold whole. Measured across one capture, fifteen of the games bundles agree under either rule
/// and sixteen non-game ones do not — so the count happens to correlate today, inside the
/// category this crate reads, and is not the field carrying the meaning.
fn price_of(tiers: &[Tier], pool: usize) -> Option<Price> {
    // The most games on offer. `max_by_key` returns the last maximum, which for a ladder listed
    // in ascending order is the same tier either way.
    let best = tiers.iter().max_by_key(|tier| tier.quantity)?;
    let total = Money::from_hundredths(*best.price.get(QUOTED_CURRENCY)?, QUOTED_CURRENCY)?;

    if usize::try_from(best.quantity).is_ok_and(|picks| picks >= pool) {
        return Some(Price::Whole(total));
    }
    Some(Price::PerGame {
        each: total.each_of(best.quantity)?,
        games: best.quantity,
    })
}

fn entry(listed: Listed) -> Entry {
    let ends_at = listed
        .valid_until
        .as_deref()
        .and_then(|until| clock::parse_reported(until, &listed.name));
    let price = price_of(&listed.tiers, listed.products.len());
    Entry {
        url: bundle_url(&listed.slug),
        price,
        ends_at,
        title: listed.name,
        slug: listed.slug,
        product_slugs: listed.products.into_iter().map(|p| p.slug).collect(),
        smallest_pick: listed.tiers.iter().map(|tier| tier.quantity).min(),
    }
}

/// Builds a [`Bundle`] from a listing row and the product detail fetched for it.
///
/// `products` need not cover every slug the listing named: availability is regional, so the
/// service reports what it could not resolve and those become [`ProblemKind::UnresolvedItems`]
/// rather than silently shortening the bundle.
pub(super) fn build_bundle(entry: &Entry, detail: &Products) -> Result<(Bundle, Vec<Problem>)> {
    // Walked in the order the listing gave, not the order the answers came back in: that order
    // is the store's own curation, which the listing promises to preserve.
    let found: BTreeMap<&str, &Product> = detail
        .products
        .iter()
        .map(|product| (product.slug.as_str(), product))
        .collect();
    let mut derived = Vec::new();
    let games: Vec<Game> = entry
        .product_slugs
        .iter()
        .filter_map(|slug| match found.get(slug.as_str()) {
            Some(product) => Some(game(product)),
            // The service said outright that it does not know this slug, while the listing says
            // the bundle contains it. The slug is still the store's own identifier, so it
            // yields a readable title and a search link — which is what every game with no id
            // gets — rather than the product going missing from a bundle that sells it.
            None if detail.not_found.iter().any(|missing| missing == slug) => {
                derived.push(slug.clone());
                Some(Game {
                    title: title_from_slug(slug),
                    machine_name: slug.clone(),
                    steam_app_id: None,
                    contains: Vec::new(),
                })
            }
            // Neither answered nor refused. That is the service dropping a slug silently, and
            // the pool-size check below is what says so.
            None => None,
        })
        .collect();
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

    if !derived.is_empty() {
        report(ProblemKind::TitleFromSlug { slugs: derived });
    }

    // A pack's games are now always listed, because one the service would not describe is built
    // from the name the pack itself gave it. That fixes the row and does NOT fix the gap: such a
    // game has no Steam id, no reviews and no release date, so its line is a name and a search
    // link rather than a verdict. Reported for the same reason as before — a fix whose remaining
    // loss looks like success is worse than the loss being visible.
    let unopened = packs_left_unopened(detail);
    if !unopened.is_empty() {
        report(ProblemKind::UnresolvedItems {
            machine_names: unopened,
        });
    }

    for (pack, stated, listed) in short_contents_lists(detail) {
        report(ProblemKind::PackContentsIncomplete {
            pack,
            stated,
            listed,
        });
    }

    // The listing and the detail state the pool size independently. They normally agree — a
    // control bundle matched exactly, slug for slug — so a disagreement is a real signal rather
    // than routine noise. It is reported, not fatal: the listing is edge-cached and can lag a
    // bundle whose contents change mid-flight.
    let listed = entry.product_slugs.len();
    if listed != games.len() {
        report(ProblemKind::GameCountMismatch {
            advertised: listed,
            found: games.len(),
        });
    }

    let bundle = Bundle {
        title: entry.title.clone(),
        url: entry.url.clone(),
        price: entry.price.clone(),
        ends_at: entry.ends_at,
        games,
    };
    Ok((bundle, problems))
}

/// One product, as this crate's store-agnostic [`Game`].
///
/// The Steam id is taken only when the product itself has one. A pack or edition carries
/// `null` there — and its sibling `steam.type` still reads `"app"`, so the type field cannot be
/// used to tell them apart. Only a non-null id counts.
fn game(product: &Product) -> Game {
    Game {
        title: product.name.clone(),
        machine_name: product.slug.clone(),
        steam_app_id: product.steam_id,
        // **A product either is a game or stands for several; never both.** Held here rather
        // than only in the resolution order, because a value carrying an id *and* contents has
        // no meaning: the renderer would have to guess whether to link it or open it out. An
        // edition that is its own Steam app and also lists what it bundles stays one linkable
        // game, which is what a buyer of it actually gets.
        contains: if product.steam_id.is_some() {
            Vec::new()
        } else {
            product.contents.iter().map(game).collect()
        },
    }
}

/// Slugs worth a second lookup: products with no Steam id that name a base product.
///
/// An edition ("Everspace Ultimate Edition") is not itself a Steam product, but the base game
/// it is built from is, and the service names it. Following that recovers an id for roughly
/// half the products that lack one; the rest are genuine multi-game packs with no single Steam
/// equivalent.
/// A readable title from a slug, for a product the store named but published no details for.
///
/// Mechanical and not always right — `scp-fragmented-minds` becomes "Scp Fragmented Minds" where
/// the store writes "SCP: Fragmented Minds" — which is why it is reported rather than passed off
/// as the store's own wording. It is enough to search for and enough to recognise, and both beat
/// dropping a game the bundle actually sells.
fn title_from_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut letters = word.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + letters.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Where a whole bundle lives on the site, as against a pick-and-mix one.
#[must_use]
pub fn whole_bundle_url(slug: &str) -> String {
    format!("https://www.fanatical.com/en/bundle/{slug}")
}

/// What a record says it delivers, minus itself.
///
/// A bundle can appear inside its own contents list: one real collection lists four products,
/// of which the first is the collection. Left in, it would be rendered as a game inside itself,
/// with its own link, one line under its own title — the same "a wrapper is not a product"
/// mistake one level down — and it would make the stated count disagree with the list for no
/// reason.
pub(super) fn delivered(product: &Product) -> impl Iterator<Item = &ContainedProduct> {
    product
        .bundle_tiers
        .iter()
        .flat_map(|tier| tier.products.iter())
        .filter(|contained| contained.slug != product.slug)
}

/// Reads a `search_products` answer.
pub(super) fn parse_search(sse: &str) -> Result<Vec<Product>> {
    let result = json_rpc_result(sse)?;
    let payload = tool_payload(&result)?;
    let found: SearchResults = serde_json::from_str(payload).map_err(|source| Error::Payload {
        url: MCP_URL.into(),
        source,
    })?;
    Ok(found.results)
}

/// Turns the records of whole bundles into listing entries.
///
/// A fixed-price bundle is bought whole — one payment, those games, no choice to make — **so its
/// price is [`Price::Whole`] whatever its tier's count says**. The quantity-against-pool rule
/// that decides the shape for a pick-and-mix must not be applied here: one real collection
/// states a count of three over a list of four, which that rule would read as a choice and quote
/// a per-game rate for a bundle nobody can buy a part of.
pub(super) fn whole_bundle_entries(detail: &Products) -> Vec<Entry> {
    let currency = detail.currency.as_deref().unwrap_or(QUOTED_CURRENCY);
    detail
        .products
        .iter()
        .filter_map(|record| {
            let product_slugs: Vec<String> =
                delivered(record).map(|held| held.slug.clone()).collect();
            if product_slugs.is_empty() {
                debug!("  {} lists no contents; skipped", record.slug);
                return None;
            }
            Some(Entry {
                title: record.name.clone(),
                slug: record.slug.clone(),
                url: whole_bundle_url(&record.slug),
                product_slugs,
                smallest_pick: None,
                price: record
                    .price
                    .and_then(|amount| Money::from_major(amount, currency))
                    .map(Price::Whole),
                ends_at: record.valid_until.and_then(clock::from_epoch_seconds),
            })
        })
        .collect()
}

/// Slugs named inside multi-game packs, for the pass that looks them up.
///
/// Only packs that could not be resolved any other way: a product with a Steam id of its own is
/// a game, and one that names a base product is an edition — [`parents_worth_resolving`] handles
/// that and runs first, so an edition is never taken apart into its contents.
pub(super) fn packs_worth_expanding(detail: &Products) -> Vec<String> {
    let mut slugs: Vec<String> = detail
        .products
        .iter()
        .filter(|product| product.steam_id.is_none() && product.parent.is_none())
        .flat_map(delivered)
        .map(|contained| contained.slug.clone())
        .collect();
    // A game can appear in more than one pack in the same bundle; asking for it twice would
    // spend a request slot on an answer already in hand.
    slugs.sort();
    slugs.dedup();
    slugs
}

pub(super) fn parents_worth_resolving(detail: &Products) -> Vec<String> {
    let mut slugs: Vec<String> = detail
        .products
        .iter()
        .filter(|product| product.steam_id.is_none())
        .filter_map(|product| product.parent.as_ref())
        .map(|parent| parent.slug.clone())
        .collect();
    // Two editions of one game name the same base product; asking for it twice spends a slot in
    // a twenty-per-call batch on an answer already in hand.
    slugs.sort();
    slugs.dedup();
    slugs
}

/// Contained slugs of packs that were looked up and still came back with nothing.
///
/// The silent-failure guard for [`packs_worth_expanding`]. A pack renders as its name with its
/// contents beneath it, so a pack whose contents all failed to resolve renders as a bare name
/// with nothing under it — which looks exactly like a pack that was handled correctly. Reported
/// as unresolved items instead, which is what the bundle's own problem list is for.
pub(super) fn packs_left_unopened(detail: &Products) -> Vec<String> {
    detail
        .products
        .iter()
        .flat_map(|product| &product.contents)
        .filter(|contained| contained.from_contents_list)
        .map(|contained| contained.slug.clone())
        .collect()
}

/// Contents lists that are shorter than their own stated count.
///
/// **One-sided on purpose.** An equality check looked right and was wrong twice: a pick-and-mix
/// tier states how many you may pick out of a larger pool, so fewer stated than listed is its
/// normal shape, and one real collection lists itself among its contents so that its count and
/// its list differed by exactly one until [`delivered`] filtered it out. Only the other
/// direction is impossible — a tier cannot hand over more products than it names — so only that
/// direction is reported, and what it catches is a truncated list.
pub(super) fn short_contents_lists(detail: &Products) -> Vec<(String, usize, usize)> {
    detail
        .products
        .iter()
        .filter_map(|product| {
            let stated = product
                .bundle_tiers
                .first()
                .and_then(|tier| tier.tier_product_count)?;
            let listed = delivered(product).count();
            (stated > listed).then(|| (product.slug.clone(), stated, listed))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// MCP framing
// ---------------------------------------------------------------------------------------------

/// Pulls the JSON-RPC result out of an MCP response.
///
/// Responses are Server-Sent Events: a `data:` line per message, in a stream that may carry
/// more than one. The last is the answer to the request just made.
pub fn json_rpc_result(sse: &str) -> Result<serde_json::Value> {
    let last = sse
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .next_back()
        .ok_or_else(|| Error::Protocol {
            service: "fanatical mcp",
            detail: "response carried no data frame".to_owned(),
        })?;

    let message: serde_json::Value =
        serde_json::from_str(last).map_err(|source| Error::Payload {
            url: MCP_URL.into(),
            source,
        })?;

    // A JSON-RPC error is a well-formed response carrying a failure; surfacing it as one beats
    // reporting "no result field".
    if let Some(error) = message.get("error") {
        return Err(Error::Protocol {
            service: "fanatical mcp",
            detail: error.to_string(),
        });
    }
    message
        .get("result")
        .cloned()
        .ok_or_else(|| Error::Protocol {
            service: "fanatical mcp",
            detail: "response had neither result nor error".to_owned(),
        })
}

/// Reads a tool's payload out of a JSON-RPC result.
///
/// Tool results arrive as a content list whose first text item is itself a JSON document — a
/// string carrying JSON, not nested JSON, so it needs a second parse.
pub fn tool_payload(result: &serde_json::Value) -> Result<&str> {
    result["content"][0]["text"]
        .as_str()
        .ok_or_else(|| Error::Protocol {
            service: "fanatical mcp",
            detail: "tool result carried no text content".to_owned(),
        })
}

/// Parses an MCP `get_products` response, SSE framing and all.
pub(super) fn parse_products(sse: &str) -> Result<Products> {
    let result = json_rpc_result(sse)?;
    let payload = tool_payload(&result)?;
    serde_json::from_str(payload).map_err(|source| Error::Payload {
        url: MCP_URL.into(),
        source,
    })
}

/// Builds a bundle from a listing row and the raw MCP response naming its contents.
///
/// The pure counterpart to [`super::Client::list_bundles`]: everything fragile — the JSON-RPC
/// framing, the product schema, the cross-check — happens here, so it can be driven from a
/// saved response with no network involved.
pub fn parse_bundle(entry: &Entry, mcp_response: &str) -> Result<(Bundle, Vec<Problem>)> {
    let detail = parse_products(mcp_response)?;
    build_bundle(entry, &detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same capture the integration tests read, reached from here so that the rule can be
    /// checked against bundles `parse_catalogue` filters out — which is where its one
    /// counterexample lives.
    const CAPTURE: &str = include_str!("../../../../tests/fixtures/fanatical/all-en.json");

    fn listed(slug: &str) -> Listed {
        let catalogue: Catalogue = serde_json::from_str(CAPTURE).expect("the capture parses");
        catalogue
            .pickandmix
            .into_iter()
            .find(|entry| entry.slug == slug)
            .unwrap_or_else(|| panic!("no bundle {slug:?} in the capture"))
    }

    fn priced(slug: &str) -> Option<Price> {
        let entry = listed(slug);
        price_of(&entry.tiers, entry.products.len())
    }

    #[test]
    fn a_pick_and_mix_reports_the_rate_at_the_tier_that_gives_the_most_games() {
        // Nineteen products, best tier five picks: a choice, so a rate.
        let price = priced("fanatical-favorites-build-your-own-bundle").expect("a price");
        let Price::PerGame { each, games } = price else {
            panic!("expected a per-game rate, got {price:?}");
        };
        assert_eq!(games, 5);
        assert_eq!(each.currency, "USD");
    }

    #[test]
    fn a_tier_ladder_that_takes_the_whole_pool_is_a_whole_bundle_price() {
        // Four tiers, and the largest takes all 28 of the 28 products. Counting tiers would
        // call this a pick-and-mix and quote a per-item rate for something sold only whole.
        let price = priced("your-digital-life-the-complete-manuals-build-your-own-bundle");
        assert!(
            matches!(price, Some(Price::Whole(_))),
            "expected a whole-bundle price, got {price:?}"
        );
    }

    #[test]
    fn the_charity_bundle_shape_is_read_as_one_payment_for_everything() {
        // war-child-charity-bundle-2026, read from Fanatical's MCP server on 2026-09-16: one
        // tier, `tier_product_count: 11`, `price: 7.99`, over a pool of 11. Re-expressed here in
        // the catalogue's own shape because whole bundles are not carried in that payload, which
        // is also why this branch has no live listing to exercise it yet.
        let tiers = vec![Tier {
            quantity: 11,
            price: [("USD".to_owned(), 799.0)].into_iter().collect(),
        }];
        assert_eq!(
            price_of(&tiers, 11),
            Some(Price::Whole(Money::new(799, "USD")))
        );
    }

    // ------------------------------------------------------------------------------------
    // Multi-game packs
    // ------------------------------------------------------------------------------------

    use super::super::schema::{BundleTier, ContainedProduct, Parent};

    fn product(slug: &str, steam_id: Option<u32>) -> Product {
        Product {
            from_contents_list: false,
            name: slug.to_owned(),
            slug: slug.to_owned(),
            steam_id,
            parent: None,
            kind: Some("game".to_owned()),
            price: None,
            valid_until: None,
            bundle_tiers: Vec::new(),
            contents: Vec::new(),
        }
    }

    fn pack(slug: &str, holds: &[&str]) -> Product {
        Product {
            bundle_tiers: vec![BundleTier {
                tier_product_count: Some(holds.len()),
                products: holds
                    .iter()
                    .map(|inner| ContainedProduct {
                        name: (*inner).to_owned(),
                        slug: (*inner).to_owned(),
                    })
                    .collect(),
            }],
            ..product(slug, None)
        }
    }

    fn detail(products: Vec<Product>) -> Products {
        Products {
            currency: Some(QUOTED_CURRENCY.to_owned()),
            products,
            not_found: Vec::new(),
        }
    }

    #[test]
    fn a_product_that_is_not_a_pack_may_send_a_null_contents_list() {
        // The service sends an explicit `null` rather than omitting the field, which
        // `#[serde(default)]` alone does not cover — declared as a plain Vec it rejected every
        // real response.
        let parsed: Products = serde_json::from_str(
            r#"{"products":[{"name":"One","slug":"one","steam_id":1,"bundle_tiers":null}],
                "not_found":[]}"#,
        )
        .expect("a null contents list is read as no contents");
        assert!(parsed.products[0].bundle_tiers.is_empty());
    }

    #[test]
    fn a_pack_delivers_its_contents_and_never_claims_a_page_of_its_own() {
        let mut wrapper = pack("double-pack", &["one", "two"]);
        wrapper.contents = vec![product("one", Some(1)), product("two", Some(2))];

        let built = game(&wrapper);
        assert_eq!(built.steam_app_id, None, "a pack has no store page");
        assert_eq!(built.contains.len(), 2);
        assert_eq!(
            built
                .contains
                .iter()
                .filter_map(|g| g.steam_app_id)
                .collect::<Vec<_>>(),
            [1, 2],
            "each contained game keeps its own id"
        );
    }

    #[test]
    fn a_product_is_either_a_game_or_stands_for_several_but_never_both() {
        // An edition that is its own Steam app and also lists what it bundles stays one
        // linkable game. The state with an id *and* contents has no meaning — the renderer
        // would have to guess whether to link it or open it out — so it cannot be built.
        let mut edition = pack("final-cut", &["base", "dlc"]);
        edition.steam_id = Some(632_470);
        edition.contents = vec![product("base", Some(1)), product("dlc", Some(2))];

        let built = game(&edition);
        assert_eq!(built.steam_app_id, Some(632_470));
        assert!(
            built.contains.is_empty(),
            "it was opened out as well as linked"
        );
    }

    #[test]
    fn an_edition_naming_a_base_product_is_not_taken_apart_into_its_contents() {
        // Parents are resolved first, so a product with a base game is never a pack candidate.
        let mut edition = pack("ultimate-edition", &["base", "season-pass"]);
        edition.parent = Some(Parent {
            slug: "base".to_owned(),
        });
        assert!(packs_worth_expanding(&detail(vec![edition])).is_empty());
    }

    #[test]
    fn one_game_named_by_two_packs_is_looked_up_once() {
        let wanted = packs_worth_expanding(&detail(vec![
            pack("first-pack", &["shared", "one"]),
            pack("second-pack", &["shared", "two"]),
        ]));
        assert_eq!(wanted, ["one", "shared", "two"]);
    }

    #[test]
    fn a_pack_whose_contents_could_not_be_looked_up_still_lists_them_and_says_so() {
        // The bug this now states the fix for: `rock-of-ages-1-3-complete-bundle` had all three
        // of its slugs come back in `not_found`, its contents list emptied, and the whole pack
        // rendered as one bare row — beside `boomer-shooters-furious-4-bundle`, whose four slugs
        // resolved and which opened out correctly.
        //
        // A pack's own list names every game it sells, so the games are listed from that. What is
        // still missing is the DETAIL — no Steam id, no reviews, no date — and that is what the
        // problem reports. Both halves are asserted, because listing them without saying they are
        // thin would be a fix whose remaining loss looks like success.
        let entry = Entry {
            title: "A Bundle".to_owned(),
            slug: "a-bundle".to_owned(),
            url: bundle_url("a-bundle"),
            product_slugs: vec!["double-pack".to_owned()],
            smallest_pick: None,
            price: None,
            ends_at: None,
        };
        // What `client::resolve_packs` leaves behind when the lookup answers nothing: the games
        // are there, built from the pack's own names, each marked as carrying no detail.
        let mut unopened = pack("double-pack", &["one", "two"]);
        unopened.contents = ["one", "two"]
            .iter()
            .map(|slug| Product {
                name: (*slug).to_owned(),
                slug: (*slug).to_owned(),
                ..Product::unknown()
            })
            .collect();
        let (bundle, problems) = build_bundle(&entry, &detail(vec![unopened])).expect("builds");

        let held: Vec<&str> = bundle.games[0]
            .contains
            .iter()
            .map(|game| game.title.as_str())
            .collect();
        assert_eq!(held, ["one", "two"], "the pack still lists what it sells");
        assert!(
            bundle.games[0]
                .contains
                .iter()
                .all(|game| game.steam_app_id.is_none()),
            "nothing was invented for them"
        );
        assert!(
            problems.iter().any(|p| matches!(
                &p.kind,
                ProblemKind::UnresolvedItems { machine_names } if machine_names == &["one", "two"]
            )),
            "and the missing detail is still reported: {problems:?}"
        );
    }

    #[test]
    fn a_contents_list_shorter_than_its_own_count_is_reported() {
        // A tier cannot hand over more products than it names, so this direction is impossible
        // and worth reporting.
        let mut truncated = pack("double-pack", &["one"]);
        truncated.bundle_tiers[0].tier_product_count = Some(2);
        assert_eq!(
            short_contents_lists(&detail(vec![truncated])),
            [("double-pack".to_owned(), 2, 1)]
        );
    }

    #[test]
    fn a_pool_larger_than_the_count_is_ordinary_and_is_not_reported() {
        // The other direction is how a pick-and-mix reads — pick five of thirty — so an equality
        // check would have called every one of them broken.
        let mut ladder = pack("pick-and-mix", &["one", "two", "three"]);
        ladder.bundle_tiers[0].tier_product_count = Some(2);
        assert!(short_contents_lists(&detail(vec![ladder])).is_empty());
    }

    #[test]
    fn a_bundle_listed_inside_its_own_contents_is_not_delivered_by_itself() {
        // One real collection lists four products, the first of which is the collection. Left
        // in, it renders as a game inside itself and makes the stated count look wrong.
        let collection = pack(
            "arkham-collection",
            &["arkham-collection", "asylum", "city", "knight"],
        );
        let held: Vec<&str> = delivered(&collection).map(|c| c.slug.as_str()).collect();
        assert_eq!(held, ["asylum", "city", "knight"]);

        // And with the self-reference gone, its stated count of three agrees with its list.
        let mut counted = collection;
        counted.bundle_tiers[0].tier_product_count = Some(3);
        assert!(short_contents_lists(&detail(vec![counted])).is_empty());
    }

    #[test]
    fn a_fixed_price_bundle_is_bought_whole_however_many_products_its_tier_lists() {
        // The quantity-against-pool rule that shapes a pick-and-mix price must not reach here:
        // this record states three over a list of four, which that rule would read as a choice.
        let mut record = pack(
            "arkham-collection",
            &["arkham-collection", "asylum", "city", "knight"],
        );
        record.name = "Batman: Arkham Collection".to_owned();
        record.bundle_tiers[0].tier_product_count = Some(3);
        // What you pay is the record's own price. Its tier says 59.99, which is the retail
        // worth of the games and not a price anybody is charged.
        record.price = Some(7.49);
        record.valid_until = Some(1_790_467_140);

        let entries = whole_bundle_entries(&detail(vec![record]));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].price, Some(Price::Whole(Money::new(749, "USD"))));
        assert_eq!(entries[0].product_slugs, ["asylum", "city", "knight"]);
        assert_eq!(entries[0].url, whole_bundle_url("arkham-collection"));
        assert_eq!(
            entries[0].ends_at,
            crate::clock::from_epoch_seconds(1_790_467_140)
        );
    }

    #[test]
    fn a_pack_counts_as_the_one_product_the_listing_named() {
        // The pool-size check compares what the listing said against what came back. A pack
        // stays one game with its contents inside it, so opening packs out must not make every
        // bundle containing one look short.
        let mut wrapper = pack("double-pack", &["one", "two"]);
        wrapper.contents = vec![product("one", Some(1)), product("two", Some(2))];
        let entry = Entry {
            title: "A Bundle".to_owned(),
            slug: "a-bundle".to_owned(),
            url: bundle_url("a-bundle"),
            product_slugs: vec!["double-pack".to_owned(), "solo".to_owned()],
            smallest_pick: None,
            price: None,
            ends_at: None,
        };
        let (bundle, problems) =
            build_bundle(&entry, &detail(vec![wrapper, product("solo", Some(3))])).expect("builds");

        assert_eq!(bundle.games.len(), 2, "the pack is one entry, not three");
        assert!(
            !problems
                .iter()
                .any(|p| matches!(p.kind, ProblemKind::GameCountMismatch { .. })),
            "{problems:?}"
        );
    }

    const BUNDLE_SEARCH: &str =
        include_str!("../../../../tests/fixtures/fanatical/mcp-search-bundles.sse");

    #[test]
    fn the_bundle_search_is_read_as_the_list_of_bundles_it_is() {
        let found = parse_search(BUNDLE_SEARCH).expect("the capture parses");
        assert_eq!(found.len(), 5);
        assert!(found.iter().any(|p| p.slug == "nuntius-games-bundle"));
    }

    #[test]
    fn a_mystery_bundle_is_excluded_by_its_declared_type_not_by_its_name() {
        // Its contents are random by design, so there is nothing to list. Reading the type is
        // what keeps that from being a guess about wording — and the pick-and-mix bundles in
        // the same capture are typed "game-bundle" too, so the type alone does not separate
        // those; the slugs the catalogue already supplied do.
        let found = parse_search(BUNDLE_SEARCH).expect("parses");
        let mystery = found
            .iter()
            .find(|p| p.slug == "vip-mystery-bundle")
            .expect("the capture holds one");
        assert_eq!(mystery.kind.as_deref(), Some("mystery-bundle"));
        assert!(
            found
                .iter()
                .filter(|p| p.kind.as_deref() == Some("game-bundle"))
                .all(|p| p.slug != "vip-mystery-bundle")
        );
    }

    #[test]
    fn a_bundle_the_catalogue_already_listed_is_not_a_second_entry() {
        // The search returns pick-and-mix bundles as well, and those arrive from the catalogue
        // with their tier ladder. Listing them twice would double every one of them.
        let found = parse_search(BUNDLE_SEARCH).expect("parses");
        let titanium = found
            .iter()
            .find(|p| p.slug == "build-your-own-titanium-collection")
            .expect("the capture holds it");
        assert_eq!(titanium.kind.as_deref(), Some("game-bundle"));
    }

    #[test]
    fn a_bundle_quoting_no_tiers_is_left_without_a_price() {
        assert_eq!(price_of(&[], 10), None);
    }

    #[test]
    fn a_tier_missing_the_quoted_currency_yields_no_price_rather_than_another_currency() {
        // Never fall through to whichever currency happens to be first: in one real tier GBP,
        // EUR and USD are all 699, so a wrong currency is invisible in the digits.
        let tiers = vec![Tier {
            quantity: 3,
            price: [("GBP".to_owned(), 699.0)].into_iter().collect(),
        }];
        assert_eq!(price_of(&tiers, 20), None);
    }
}
