//! Snapshots of what the stores sell, fetched once and queried locally.
//!
//! The counterpart to [`crate::inventory`], and eventually its replacement. An inventory module
//! is a **curated table compiled into the binary** — 415 Steam games, hand-reviewed, regenerated
//! rarely. That is why a game a bundle sells is so often matched by title rather than by id: 415
//! entries cannot cover a store with 187,144 of them, and everything outside the table falls back
//! to comparing names.
//!
//! What lives here instead is a **snapshot on disk, read at runtime**. Big enough to cover a
//! store, small enough to query in microseconds, and refreshed by running a tool rather than by
//! recompiling.
//!
//! One module per store, and each owns its own file format, because the stores do not publish
//! comparable things and a shared schema would mean inventing the fields the quieter ones lack —
//! the same reason [`crate::inventory`] keeps them apart.
//!
//! # What is built, and what is still a plan
//!
//! [`steam`] is built end to end: its row format and codec, the `steam_catalogue` tool that
//! writes the snapshot, a snapshot in `data/steam/` compiled into the binary, and a reader
//! ([`steam::snapshot`]) that answers by app-id or by title. The program consults it through the
//! one join in [`crate::inventory::steam`] — `app_id_of` and `of_game` — AFTER the curated table,
//! whose hand-written aliases and chosen review population the snapshot cannot carry. What the
//! snapshot does not hold: the thirty-day review window, and a by-name index (title lookups scan).
//!
//! [`epic`] is notes only.
//!
//! The research behind both is in `TODO.txt`; what is settled, what is measured and what is still
//! someone's decision is recorded per store, so that whoever finishes it is not re-deriving any
//! of it.

pub mod epic;
pub mod steam;
