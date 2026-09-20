//! Games as itch.io presents them — the sibling of [`crate::inventory::steam`] for itch.io.
//!
//! **A placeholder.** No entries yet, and no types of its own on purpose: what a itch.io
//! entry should carry is not yet known, and inventing a shape before there is a single real
//! entry to hold would only have to be undone. When the first entries arrive, copy
//! [`crate::inventory::steam`]'s shape — a module per category, a `pub const` table of
//! entries, derived helpers, and tests holding the house rules mechanically — and keep only
//! the fields itch.io actually publishes.
//!
//! One thing already known to differ: nothing here should assume Steam's review bands or
//! app-ids. itch.io identifies products its own way, and a shared identifier type would be a
//! guess at this stage.
