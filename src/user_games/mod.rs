//! What the person running this program already owns, and what they still want.
//!
//! Distinct from [`crate::store_inventory`] and [`crate::inventory`], which describe what the
//! STORES sell. These two modules describe one account holder: [`holdings`] reads the library and
//! wishlist snapshots [`crate::commands::gamelib`] writes and answers "do I have this, do I want this", and
//! [`crossover`] answers the same question across stores, where the only thing two catalogues
//! share is a title.
//!
//! # A fact, never advice
//!
//! Both say what is true and stop. Owning a game on one store is not a reason to skip it on
//! another — an Epic copy carries no Steam key, so no achievements, no cards, no cloud saves and
//! no Deck verdict — and a wishlist is a note to yourself rather than an instruction. Wording
//! these as advice would also cost more when they are wrong: a mislabelled fact is checked
//! against your own library in seconds, a mislabelled instruction is acted on.

pub mod crossover;
pub mod holdings;
