//! Serde mirrors of the two Fanatical payloads this crate reads.
//!
//! As with [`crate::commands::sales::humble::schema`], only the fields actually used are declared; everything
//! else is ignored, and what *is* declared is required so that a rename fails loudly rather
//! than yielding a bundle with no games.

use serde::Deserialize;

// ---------------------------------------------------------------------------------------------
// https://www.fanatical.com/api/all/en  — the bundle listing
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct Catalogue {
    /// Every "build your own" bundle, games and otherwise.
    pub pickandmix: Vec<Listed>,
}

#[derive(Debug, Deserialize)]
pub struct Listed {
    pub name: String,
    pub slug: String,
    /// Category. `"bundle"` is the games one; the rest are `"book-bundle"`,
    /// `"elearning-bundle"`, `"software-bundle"` and `"comic-bundle"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The pool, named but not detailed — `steam` appears only in the per-product data.
    pub products: Vec<ListedProduct>,
    /// When the bundle stops being sold, e.g. `"2026-09-24T07:00:00.000Z"`.
    ///
    /// Marked `Z`, so unlike Humble's there is nothing to infer. Optional because the listing
    /// genuinely omits it: the open-ended eLearning bundles carry `valid_from` alone.
    #[serde(default)]
    pub valid_until: Option<String>,
    /// Price points, as `quantity` picked for `price`. Unlike Humble's tiers these do not
    /// partition the pool; every tier draws from all of it.
    #[serde(default)]
    pub tiers: Vec<Tier>,
}

#[derive(Debug, Deserialize)]
pub struct ListedProduct {
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct Tier {
    /// How many products this price lets you pick.
    pub quantity: u32,
    /// Total for that many picks, in each currency's smallest unit — `{"USD": 699}` is $6.99.
    ///
    /// The whole tier, not a per-game rate: `{quantity: 25, price: {USD: 2369}}` is twenty-five
    /// games for $23.69, which is 95 cents each.
    ///
    /// Typed as a float despite counting whole units, because Fanatical publishes some of them
    /// as `204.99999999999997`. Declaring it an integer makes serde reject the whole catalogue
    /// over one such value — which is how this was found. [`crate::Money::from_minor`] carries
    /// the rounding rule and the measurement behind it.
    #[serde(default)]
    pub price: std::collections::BTreeMap<String, f64>,
}

// ---------------------------------------------------------------------------------------------
// mcp.fanatical.com — `get_products`
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct Products {
    /// ISO 4217 code the service folded every price in this response to.
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub products: Vec<Product>,
    /// Slugs the service could not resolve **in the requested region**.
    ///
    /// Reported rather than inferred from a count difference: availability is regional, so a
    /// product present in the catalogue can be genuinely absent from one region's store.
    #[serde(default)]
    pub not_found: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Product {
    pub name: String,
    pub slug: String,
    /// Steam application id, or `null`.
    ///
    /// `null` for multi-game packs and for editions that are not themselves a Steam product.
    /// Note that the sibling `steam.type` field is `"app"` even for those — it does not
    /// discriminate, and only a non-null id does.
    #[serde(default)]
    pub steam_id: Option<u32>,
    /// The base product, where this one is an edition or a pack of it.
    #[serde(default)]
    pub parent: Option<Parent>,

    /// What the service calls this: `"game"`, `"game-bundle"`, `"mystery-bundle"`.
    ///
    /// The declared type answers questions a count-based heuristic gets wrong. A mystery bundle
    /// is excluded by it rather than by guessing from its name, and a fixed-price bundle is
    /// known to be bought whole however many products its tier happens to list.
    #[serde(rename = "type", default)]
    pub kind: Option<String>,

    /// What the product costs, as a decimal in the response's currency.
    ///
    /// **This is what you pay, and the tier's own `price` is not.** On one real bundle the
    /// top-level price is 7.49 while its single tier says 59.99 — the retail worth of the games,
    /// equal to `full_price` — and the service's own message reads "Get 3 games for $7.49".
    /// Another bundle has both equal, so a rule read off that one alone cannot tell them apart.
    #[serde(default)]
    pub price: Option<f64>,

    /// When the product stops being sold, as **Unix seconds**.
    ///
    /// A third representation of an instant in this one service: the catalogue sends ISO-8601
    /// with a `Z`, and this sends a bare integer. Read by [`crate::clock::from_epoch_seconds`],
    /// which is the only reader that takes this shape.
    #[serde(default)]
    pub valid_until: Option<i64>,

    /// The games a multi-game pack actually delivers.
    ///
    /// This is the "This bundle includes:" list the store page prints. A pack such as
    /// `lazy-otter-double-pack` is a marketing wrapper with no Steam page of its own — its
    /// `steam_id` is null and it names no `parent` — so this array is the only thing that says
    /// what is really being sold.
    ///
    /// The nested entries carry a name and a slug and **no Steam id**, which is why resolving
    /// them takes a second lookup. See `client::resolve_packs`.
    ///
    /// Read through [`null_as_empty`] because the service sends an explicit `null` for an
    /// ordinary game rather than omitting the field, and `#[serde(default)]` covers only the
    /// omitted case. Declared as a plain `Vec` it rejected every real response.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub bundle_tiers: Vec<BundleTier>,

    /// Whether this record was built from a pack's contents list rather than looked up.
    ///
    /// Such a record carries a name and a slug and nothing else — no Steam id, no reviews, no
    /// release date — because the service declined to describe it. Set by `client::resolve_packs`
    /// and read by `parse::pack_contents_without_details`, which reports them; a reader has to be
    /// told that a row is a name and a link and not a verdict.
    #[serde(skip)]
    pub from_contents_list: bool,

    /// The contained products, once looked up. Filled by `client::resolve_packs`, never by the
    /// wire — the payload has no such field, and `bundle_tiers` above is what it is built from.
    ///
    /// Held here rather than in a table beside the products because the resolution passes
    /// already work this way: `resolve_parents` fills in `steam_id` after parsing, for the same
    /// reason and at the same point.
    #[serde(skip)]
    pub contents: Vec<Product>,
}

/// Reads a list that may arrive as `null`, as an empty list.
///
/// Distinct from `#[serde(default)]`, which only applies when a field is absent. Fanatical sends
/// `"bundle_tiers": null` for products that are not packs, and an explicit null is a value —
/// serde will try to deserialise it and fail.
fn null_as_empty<'de, D, T>(deserializer: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}

/// One purchase option of a multi-game pack.
///
/// A pack has exactly one in every record seen; the shape is a list because the field is shared
/// with pick-and-mix bundles, where the tiers are a ladder.
#[derive(Debug, Clone, Deserialize)]
pub struct BundleTier {
    /// How many products this option delivers, as the service counts them.
    ///
    /// Checked against the length of `products` rather than trusted: the two are computed
    /// separately, so a disagreement is the earliest sign that the list came back truncated.
    #[serde(default)]
    pub tier_product_count: Option<usize>,
    /// Products this option delivers.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub products: Vec<ContainedProduct>,
}

impl Product {
    /// A record with nothing in it, for a product the service would not describe.
    ///
    /// Every optional field stays `None` on purpose: what is not known must not be invented, and
    /// a caller reading this gets a name, a slug and the fact that nothing else arrived.
    #[must_use]
    pub fn unknown() -> Self {
        Self {
            name: String::new(),
            slug: String::new(),
            steam_id: None,
            parent: None,
            kind: None,
            price: None,
            valid_until: None,
            from_contents_list: true,
            bundle_tiers: Vec::new(),
            contents: Vec::new(),
        }
    }
}

/// A game named inside a pack's or a bundle's contents list.
#[derive(Debug, Clone, Deserialize)]
pub struct ContainedProduct {
    /// The store's own name for it, sent right here in the pack's list.
    ///
    /// Worth taking even though a second lookup usually supplies a fuller record, because
    /// sometimes that lookup returns nothing: every one of `rock-of-ages-1-3-complete-bundle`'s
    /// three games came back in `not_found` while the pack still named all three. A name from the
    /// store beats a title derived from a slug, and beats the game vanishing.
    #[serde(default)]
    pub name: String,
    pub slug: String,
}

/// What `search_products` answers with.
#[derive(Debug, Deserialize)]
pub struct SearchResults {
    #[serde(default, deserialize_with = "null_as_empty")]
    pub results: Vec<Product>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Parent {
    pub slug: String,
}
