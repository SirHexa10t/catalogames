//! A snapshot of Steam's catalogue. **Not built — the plan and the measurements behind it.**
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
//! Two fields a snapshot **cannot** carry, and they are the reason the curated table does not
//! simply disappear. `aliases` is hand-written judgement — the disambiguating `(YYYY)` suffixes
//! and the edition wordings another store prints bare — and no store publishes it. `recent`, the
//! thirty-day review window, is not in any bulk surface either; only the all-time summary is.
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
//! Two files, both plain text, both sorted, because sorted text is the cheapest thing there is
//! to query without parsing it first:
//!
//! - **by app-id**, one record per line, ascending. A binary search over line offsets answers a
//!   lookup in a handful of seeks without reading or parsing the rest.
//! - **by name**, normalised through [`crate::inventory::steam::comparable`], mapping to app-id.
//!   This is the file that retires `[owned on Steam?]`: a store publishing no app-id — Humble
//!   publishes none — currently resolves through 415 entries and falls back to comparing titles
//!   against the account's own library.
//!
//! Text rather than a binary format on purpose, at least to begin with: it diffs, it greps, and
//! a wrong value can be seen. Bit-packing the enums is a later refinement and the cardinality
//! column above is what it would use; there is no point compressing a format still being
//! learned.
//!
//! Names are not unique — 4 of 2,282 measured names reach two app-ids, and all four are
//! GOTY/remaster/edition pairs — so the by-name file has to admit collisions rather than assume
//! them away.
