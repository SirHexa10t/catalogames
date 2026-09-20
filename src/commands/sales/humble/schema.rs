//! Serde mirrors of the JSON Humble Bundle embeds in its pages.
//!
//! Only the fields this crate actually reads are declared. Everything else —
//! images, blurbs, prices, exchange rates — is ignored by serde, which is the
//! tolerance we want: decorative fields can come and go freely.
//!
//! The fields that *are* declared here are deliberately required. If Humble
//! renames `tier_display_data`, deserialisation fails loudly with
//! [`crate::Error::Schema`]. Defaulting them to empty collections instead would
//! turn a page-layout change into a bundle that parses "successfully" with zero
//! games — a wrong answer wearing a success, which is far more expensive to
//! notice than a failed run.

use serde::Deserialize;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// https://www.humblebundle.com/games  —  <script id="landingPage-json-data">
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LandingPage {
    pub data: LandingData,
}

#[derive(Debug, Deserialize)]
pub struct LandingData {
    /// The games section. The page also carries a `popular` section that mixes
    /// in book and software bundles, which is not what `sales humblebundle`
    /// asks for.
    pub games: Section,
}

#[derive(Debug, Deserialize)]
pub struct Section {
    pub mosaic: Vec<Mosaic>,
}

#[derive(Debug, Deserialize)]
pub struct Mosaic {
    pub products: Vec<Product>,
}

#[derive(Debug, Deserialize)]
pub struct Product {
    /// Display title, e.g. `"Beyond the Metroidverse Bundle"`.
    pub tile_name: String,
    /// Site-relative path, e.g. `"/games/beyond-metroidverse-bundle"`.
    pub product_url: String,
    /// When the bundle stops being sold, e.g. `"2026-09-17T04:00:00"`.
    ///
    /// UTC, with no marker saying so — **measured on this field, not inferred from another**.
    /// Humble's page renders its own countdown client-side from this value, so it can be read
    /// back: at `2026-09-16T19:31:24 UTC` the rendered page said "Offer ends in 8 hours : 28
    /// minutes" for a bundle whose `end_date` is `2026-09-17T04:00:00`. That is the value read
    /// as UTC, to within the minute. Read as Pacific the page would have said 15h28m.
    ///
    /// The *hour* is not a contract: these cluster on 04:00 and 18:00, which are midnight US
    /// Eastern and 11:00 US Pacific, so a daylight-saving change shifts them by one. Nothing
    /// asserts a particular hour for that reason.
    ///
    /// Optional, unlike the fields above it: a bundle with no announced end is a thing Humble
    /// can sell, and failing the whole index over a missing date would trade a listing for a
    /// deadline.
    #[serde(default, rename = "end_date|datetime")]
    pub end_date: Option<String>,
    /// Marketing captions such as `["7 games", "US$140 Value"]`.
    ///
    /// Decorative, hence defaulted — but the game count inside it is computed
    /// independently of the tier data on the bundle page, so it doubles as a
    /// check on our reading of that data. See [`crate::ProblemKind`].
    #[serde(default)]
    pub hover_highlights: Vec<String>,
}

// ---------------------------------------------------------------------------
// https://www.humblebundle.com/games/<slug>  —  <script id="webpack-bundle-page-data">
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct BundlePage {
    #[serde(rename = "bundleData")]
    pub bundle_data: BundleData,
}

#[derive(Debug, Deserialize)]
pub struct BundleData {
    /// Tier identifier (`"initial"`, `"bt10"`, …) to that tier's contents.
    ///
    /// A `BTreeMap` rather than a `HashMap` so that iteration order is stable:
    /// picking "the largest tier" has to return the same tier every run, even
    /// when two tiers hold the same number of items.
    pub tier_display_data: BTreeMap<String, TierDisplay>,

    /// What each tier costs, keyed by the same tier ids as `tier_display_data`.
    ///
    /// Defaulted rather than required: a bundle that stopped publishing prices should lose its
    /// price, not its games. The two maps are separate in Humble's payload, so a tier can in
    /// principle appear in one and not the other, which is why the lookup is by id.
    #[serde(default)]
    pub tier_pricing_data: BTreeMap<String, TierPricing>,

    /// Every item the page knows about, keyed by machine name.
    ///
    /// Wider than the tiers: it also carries entries that belong to no tier at
    /// all, such as the bundle's charity (`"roomtoread"`). That is why the game
    /// list is built from tier membership and never from these keys.
    pub tier_item_data: BTreeMap<String, TierItem>,
}

/// What one tier costs.
#[derive(Debug, Deserialize)]
pub struct TierPricing {
    /// `{"currency": "USD", "amount": 10.0}` — what must be paid to unlock this tier.
    ///
    /// Read by its exact key. `average_purchase_price|money` sits directly beside it in the
    /// same object, one word apart and the same shape — a field whose value is plausible,
    /// wrong, and impossible to spot afterwards.
    ///
    /// For a beat-the-average tier (`is_bta`) this is the moving average rather than a fixed
    /// price. Still the right number — it is what the tier costs to unlock right now — but not
    /// one that is stable between runs.
    #[serde(rename = "price|money")]
    pub price: MoneyField,
}

/// Humble's own money shape, a decimal beside its currency code.
#[derive(Debug, Deserialize)]
pub struct MoneyField {
    pub currency: String,
    pub amount: f64,
}

#[derive(Debug, Deserialize)]
pub struct TierDisplay {
    pub tier_item_machine_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TierItem {
    /// Display title. Optional so that one nameless item is reported as a
    /// single unresolved entry rather than failing the whole bundle; if the
    /// field is renamed wholesale, every item goes unresolved and the bundle
    /// fails with [`crate::Error::NoGames`], which is the loud outcome we want.
    #[serde(default)]
    pub human_name: Option<String>,
}
