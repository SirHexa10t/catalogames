# How big is Steam

Measurements taken on **2026-09-14**. Every number below was measured that day unless
marked *estimate*; re-measure before relying on it, because the catalogue grows daily.

## Catalogue size

From Steam's own store search, which is keyless and pages 100 rows at a time
(`store.steampowered.com/search/results/?infinite=1&ignore_preferences=1&category1=…`):

| Category | `category1` | Count |
|---|---:|---:|
| **Games** | 998 | **186,157** |
| DLC | 21 | 61,461 |
| Demos | 10 | 37,714 |
| Software | 994 | 2,720 |
| Videos | 997 | 148 |
| All store items | — | 299,324 |

Two things to know before quoting these:

- **`ignore_preferences=1` is load-bearing.** Without it, games read 176,334 — 9,823 titles
  (5.3%) are hidden from an anonymous caller by content preferences.
- **These are storefront items, not app-ids.** Delisted apps are absent, and the store's
  category taxonomy is not `appdetails`' `type` field. Secondary trackers commonly quote
  ~138,000 "games"; the gap is definitional, not a contradiction.

What could not be established from a first-party source: the delisted count. SteamDB tracks
it but blocks automated access. Community figures (~900 delisted, ~1,500 purchase-disabled)
are *unverified* and should not be repeated as fact.

## How many have actually been played

**"Played by at least 100 people" cannot be measured.** Every available proxy has a
resolution floor orders of magnitude above 100:

- Valve publishes no per-app owner or player counts.
- SteamSpy's lowest owner bucket is `0 .. 20,000`. Thirty thousand apps into its
  owner-sorted index, every entry is in that bucket.
- Reviews run at roughly 1.2–3% of owners (the "Boxleiter number": 30–80× owners per review,
  with a stated ~2× uncertainty), so 100 owners is one to three reviews — and Steam shows no
  review score at all below about ten.

What *can* be measured is review counts. From a random sample of 4,000 games (40 random
offsets × 100 rows, 2.1% of the catalogue), with 95% confidence intervals:

| Cut-line | Share | Games | Implies owners (30–80×) |
|---|---:|---:|---|
| ≥10 reviews | 37.3% | ~69,400 (66,600–72,200) | 300–800+ |
| ≥50 reviews | 16.6% | ~30,900 (28,800–33,000) | 1,500–4,000+ |
| ≥100 reviews | 12.0% | ~22,300 (20,400–24,200) | 3,000–8,000+ |
| ≥500 reviews | 4.7% | ~8,700 (7,500–9,900) | 15,000–40,000+ |
| ≥1000 reviews | 3.4% | ~6,300 (5,200–7,300) | 30,000–80,000+ |

62.7% of games show no review score at all.

Caveat on those counts: search rows carry Steam's *default-filtered* review count (request
language, Steam purchases only). One app measured 33,376 there against 54,478 with
`language=all&purchase_type=all` — the figure `steam_inventory` standardises on. The
all-language ≥100 figure is therefore higher, plausibly 25,000–30,000 (*estimate*,
extrapolated from a single app's ratio).

## Can it be compiled in?

Measured by generating tables at this crate's real entry size (742 bytes per `SteamGame`
literal) and timing a clean `cargo build --release`:

| Cut-line | Entries | Source | Release build |
|---|---:|---:|---:|
| today (bundle games only) | 137 | 0.1 MB | <1 s |
| ≥100 reviews | 22,300 | 16 MB | 25 s |
| ≥10 reviews | 69,400 | 49 MB | 48 s |
| every game | 186,157 | 132 MB | 508 s |

The whole catalogue compiles — in eight and a half minutes, from a 132 MB generated file.
Feasible, and miserable. `≥100 reviews` is the comfortable line. Beyond roughly 50,000
entries the answer is the one `software_inventory` reached for its 40,000 IEEE entries: a
packed data file read by `build.rs`, not Rust source.

## What collecting it costs

At one request per second, which is the pacing this crate treats as the defensible floor:

| Data | Requests | Wall clock |
|---|---:|---:|
| search paging only (id, name, release date, tag ids, platforms, filtered review summary) | 1,863 | 31 min |
| + standardised review figure (`appreviews` per app) | 188,020 | 2.2 days |
| + `appdetails` per app (type, genres, categories) | 374,177 | 4.3 days |
| + store page per app (30-day figure, full tag list) | 560,334 | 6.5 days |

`appdetails` batches app-ids only for the `price_overview` filter; every other filter returns
HTTP 400 for more than one id, so it is strictly one request per app. Search paging filters
to games server-side and never returns delisted apps.

The real decision is not "hours or days" but **whether Steam's default-filtered review counts
are acceptable**. Accept them: half an hour. Insist on the standardised all-language figure:
two days.

## Bulk sources

None first-party. SteamSpy's `request=all` (1,000 apps per page, one request per minute,
refreshed daily) is the nearest sanctioned bulk source and its review counts are sound; its
owner data is the coarse buckets above. Kaggle datasets exist but are third-party
redistributions of Valve's data, at least one restricted to "educational and research
purposes" — fine for a sanity check, not for anything shipped.
