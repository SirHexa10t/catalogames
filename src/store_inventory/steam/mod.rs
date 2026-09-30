//! A snapshot of Steam's catalogue: the plan, the measurements behind it, and what was built.
//!
//! **Built:** the row format and [`codec`], the `steam_catalogue` tool that writes it, a snapshot
//! in `data/steam/` compiled in by [`snapshot`], and the reader — [`snapshot::by_id`] over a
//! binary search of the sorted file, [`snapshot::by_name`] over an index sorted by folded title
//! and built at first use, with an edition-wording fallback. The program reaches it through
//! [`crate::inventory::steam::app_id_of`], after the curated table. Titles need no file of their
//! own: that index is built from the snapshot — measured in release, 225 ms once per process and
//! 29 µs a title after.
//!
//! # What goes in it: exactly what a [`crate::inventory::steam::SteamGame`] needs, and no more
//!
//! Not the reviews themselves, not descriptions, not images. The fields the program already uses
//! to describe a game, for every game rather than for 415 of them.
//!
//! | field | source | cardinality |
//! |---|---|---|
//! | `app_id` | the key | u32 |
//! | `name` | bulk | text |
//! | `released` | bulk, as a UNIX timestamp rather than a display string | date |
//! | `all_time` band + approval + count | **see the population problem below** | 10 bands / 0-100 / u32 |
//! | `tags` | bulk, with vote weights | 446 values, 9 bits each |
//! | `os` | bulk | 3 flags |
//! | `vr` | bulk | small set |
//! | `deck` | bulk | 4 values, 2 bits |
//! | `features` | bulk, as category ids | small set |
//!
//! All of the above are written, and so are `type` (Valve's product kind), `early`, `compat` (the
//! three `*_compat_category` verdicts beside the Deck one, in a single base-64 digit), the three
//! language masks, `incl`, `devs` and `pubs` — and `released` says whether a game is out, planned
//! or unannounced, which once took a column of its own. Any cell but the app-id may be `?`, which
//! means unknown to us and nothing else. **No count is given here on purpose:**
//! [`snapshot::COLUMNS`] is the authoritative list, shared by the sweep that writes the file and
//! the reader, and a number repeated in prose would be wrong the next time one is added. The
//! file's own commented header explains every column to a reader who has nothing but the file.
//!
//! Two fields a snapshot **cannot** carry, and they are the reason the curated table does not
//! simply disappear. `aliases` is hand-written judgement — the disambiguating `(YYYY)` suffixes
//! and the edition wordings another store prints bare — and no store publishes it. `recent`, the
//! thirty-day review window, is not in any bulk surface either; only the all-time summary is.
//!
//! # Coverage: where the games were found, and what it cost
//!
//! Measured on the sweep of 2026-09-25, which wrote **188,450 games**.
//!
//! ## Two different things were wrong, and only one of them was about countries
//!
//! The catalogue went from 162,371 to 188,450, and almost all of that is a SORT fix rather than a
//! country fix. Enumeration used to page over Steam's relevance ranking, which is reshuffled
//! continuously — the same offset fetched twice 40 seconds apart returned 0 of 100 identical
//! positions and not even the same set. Paging an offset over a moving ranking re-reads rows it
//! has seen and never reaches others. `sort_by=Name_ASC` is stable (100 of 100 identical) and
//! recovered **25,939 games on its own**.
//!
//! The country union added **140** on top of that.
//!
//! | country | new app-ids it contributed |
//! |---|---|
//! | CA (walked first, so it carries the sort fix) | 25,939 |
//! | JP | 69 |
//! | PL | 23 |
//! | ZA | 12 |
//! | IN | 10 |
//! | BR | 10 |
//! | IL | 8 |
//! | AU | 8 |
//!
//! Those are MARGINAL counts in walk order — each country was credited only with what no earlier
//! one had found — so they understate any country taken on its own. JP's 69 is the striking one:
//! it was walked sixth, after five countries had already taken their share, and still found more
//! than all five together.
//!
//! ## Two country codes are enough: **CA and JP**
//!
//! Re-querying the 140 games CA's own walk missed, under 18 country codes, JP can describe **124**
//! of them — the next best are KR and SG at 70.
//!
//! | pair | covers | misses |
//! |---|---|---|
//! | **CA + JP** | **188,434** | **16** |
//! | CA + KR | 188,380 | 70 |
//! | CA + SG | 188,380 | 70 |
//! | CA + PL | 188,375 | 75 |
//!
//! And 13 of those 16 are unreachable from ANY country (see below), so CA + JP misses three games
//! that anything could have found. **A future sweep should use CA and JP and stop there**: the
//! other six cost about eleven hours between them and contributed 71 games, and this table is what
//! that bought.
//!
//! A wrinkle worth keeping: being LISTED by a country's search and being DESCRIBABLE by it are
//! different. CA can describe 52 of the 140 games its own search never listed.
//!
//! ## What it costs to run
//!
//! Measured at the sweep's 1.5-second pacing, which drew no rate limit in any run:
//!
//! | phase | cost |
//! |---|---|
//! | one country's enumeration | 1,885 pages at 2.13 s = **67 min** |
//! | all eight | **8.9 h** |
//! | describing 188,450 apps | 1,256 batches of 150 = **56 min** |
//! | the `appdetails` date repair | 92 single requests, about 3 min |
//! | **the whole 8-country run** | **10.2 h** |
//! | **a CA + JP run would be** | **about 3.3 h** |
//!
//! Enumeration is nine tenths of the bill, and it is the part that scales with the country list.
//! Describing does not: an app is described once whatever it was found by.
//!
//! ## What no country reaches
//!
//! Thirteen apps of 188,450 were listed by the search during enumeration and then refused by every
//! one of 18 country codes, and by `appdetails` too. They are not a defect in the sweep: a full
//! run takes hours, and an app unpublished inside that window is listed by the phase that ran
//! first and gone by the phase that ran second. The only lever is a shorter gap.
//!
//! # The population problem, which is the whole difficulty
//!
//! [`crate::steam::store::reviews_url`] asks for `language=all` and `purchase_type=all`, because
//! key-activated copies are exactly what a bundle catalogue is about. Valve's bulk surfaces
//! expose two summaries and **neither is that one**: `summary_language_specific` is english plus
//! steam, `summary_filtered` is all-languages plus steam. The all-purchase-type figure is
//! available only from `appreviews`, one app at a time.
//!
//! Measured over the 415 committed entries: taking Valve's filtered band instead would give the
//! identical band for 384 of them (92%), a different band for 26 (6%), and no band at all for 5.
//! A 6% degradation — and a silent one, since every band would still look plausible.
//!
//! **The way out is scope, not a better source.** The band is only needed for the few hundred
//! games a listing actually shows, never for 187,144. So the snapshot carries the bulk band and
//! says which population it is, and anything the program puts on screen can have the right one
//! fetched for it — which is what the program already does today.
//!
//! # The shape on disk
//!
//! One file, plain text and sorted by app-id, because sorted text is the cheapest thing there is
//! to query without parsing it first: one record per line, ascending, so a binary search over
//! line offsets answers a lookup in a handful of seeks without reading or parsing the rest. It is
//! `data/steam/steam-games.tsv`, beside its companions: `steam-appids.tsv` (the work list the
//!   sweep walked), and `steam-tags.tsv`, `steam-categories.tsv` and `steam-bands.tsv`, which
//!   name the three columns stored as Valve's own numbers. Faults go to `steam-issues.tsv`, apps
//!   the store stopped describing to `steam-delisted-games.tsv` — which [`delisted`] reads, and is
//!   the only place the program learns that the store refuses an app — and facts found by hand
//!   where Steam's item service publishes nothing come from `steam-overrides.tsv` — see
//!   [`overrides`]. [`doctor`] checks the file against the grammar its reader uses.
//!
//! A lookup by title needs no second file: [`snapshot::by_name`] sorts one pointer per row by
//! folded title at first use and binary-searches that. Titles are not unique, so it refuses a
//! title several games share rather than picking one.
//!
//! Text rather than a binary format on purpose, at least to begin with: it diffs, it greps, and
//! a wrong value can be seen. Bit-packing the enums is a later refinement and the cardinality
//! column above is what it would use; there is no point compressing a format still being
//! learned.

pub mod codec;

pub mod categories;
pub mod delisted;
pub mod doctor;
pub mod overrides;
pub mod snapshot;
pub mod tags;
