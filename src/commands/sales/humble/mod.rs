//! Humble Bundle.
//!
//! # Where the data comes from
//!
//! Humble has no public API for browsing bundles, so this reads the same JSON
//! the site's own front-end reads: both pages embed it in a
//! `<script type="application/json">` element, already rendered server-side, so
//! no JavaScript engine is involved.
//!
//! Two requests per bundle are unavoidable. `/games` names the bundles but not
//! their contents, so each bundle's own page has to be fetched for its game
//! list — and it stores that list under a *different* script id.
//!
//! # How a bundle's games are identified
//!
//! A bundle sells in price tiers. Each tier lists its contents in full rather
//! than only what it adds, so the largest tier is the whole bundle. Two details
//! of the shape are easy to get wrong and are handled explicitly:
//!
//! - Humble's `tier_order` runs most-expensive first, so the largest tier is
//!   found by counting, never by position.
//! - The page's item table includes entries belonging to no tier at all — the
//!   charity is one — so games are read from tier membership instead.
//!
//! Items are not filtered by their `item_content_type`. The Godot bundle's
//! items are typed `software`, yet Humble itself advertises them as "13 games";
//! filtering on the type would silently empty that bundle.
//!
//! # Terms of use
//!
//! Humble's terms prohibit automated access, and their `robots.txt` carries a
//! prose header prohibiting scraping — though note its machine-readable rules
//! do *not* disallow `/games`, so a robots parser alone reads as permission.
//! Using this module is a decision for whoever ships it. See the README.

mod client;
mod parse;
mod schema;

pub use client::Client;
pub use parse::{Entry, INDEX_URL, parse_bundle, parse_index};
