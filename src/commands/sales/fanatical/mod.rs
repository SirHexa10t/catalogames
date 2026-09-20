//! Fanatical.
//!
//! # Where the data comes from
//!
//! Two sources, for two different reasons.
//!
//! The bundle **listing** comes from `api/all/en`, a plain JSON endpoint that Fanatical's
//! `robots.txt` affirmatively `Allow`s. The bundle **contents** come from Fanatical's MCP
//! server, which its `llms.txt` asks clients to prefer over reading pages, and which answers
//! with Steam application ids directly.
//!
//! Neither is a scrape of a rendered page, and that is not for want of trying: the pick-and-mix
//! page is a JavaScript shell with no embedded data at all — no `__NEXT_DATA__`, no preloaded
//! state, nothing of the sort Humble embeds. The endpoints were found by watching what a real
//! browser requested, and the responses are saved as fixtures so nobody has to repeat that.
//!
//! Nothing in this module is generative. MCP is JSON-RPC over HTTP; see [`mcp`].
//!
//! # How a pick-and-mix bundle differs from Humble's
//!
//! A Humble bundle's tiers are nested *content sets*, so the largest tier is the whole bundle
//! and finding it takes work. Fanatical's tiers are *price points* over one shared pool —
//! "pick 2 for £6.99, 3 for £9.99, 5 for £14.99" — and every tier draws from the same list. So
//! there is no largest-tier computation here: the pool is the game list.
//!
//! Only bundles typed `bundle` are games. The listing also carries book, elearning, software
//! and comic bundles, and the type separates them exactly.
//!
//! # Steam ids
//!
//! Roughly three quarters of products carry one, and taking it deletes the name-matching that
//! elsewhere in this crate resolved "Ashen" to "Ashen Empires". The products without one are
//! multi-game packs and editions; where such a product names a base product, that is followed,
//! which recovers about half of them.
//!
//! A trap worth naming: the sibling `steam.type` field reads `"app"` even for products whose id
//! is `null`. It does not discriminate. Only a non-null id does.
//!
//! # Terms
//!
//! Fanatical publishes terms for AI clients and they make one positive demand: product URLs
//! are to be used exactly as returned, query parameters included, and Fanatical cited when its
//! data is displayed. They also ask that product descriptions, images and metadata not be
//! republished in bulk — so this module reads none of those fields.

mod client;
pub mod mcp;
mod parse;
mod schema;

pub use client::Client;
pub use parse::{CATALOGUE_URL, Entry, MCP_URL, bundle_url, parse_bundle, parse_catalogue};
