//! What each store's catalogue says about the games in it.
//!
//! One module per store, because they publish different things and agree on almost nothing: a
//! Steam entry is keyed by an app-id and carries two review windows, a Deck verdict and player
//! tags, while an Epic entry has only Epic's own identifier and a title. Flattening them into a
//! shared shape would mean inventing the fields the quieter stores do not publish, which is the
//! one thing this crate refuses to do everywhere else.
//!
//! These are **catalogues, not accounts**. What a particular person owns lives in
//! [`crate::commands::gamelib`] and [`crate::user_games::holdings`]; what a store SELLS lives here. The mapping between
//! two of them is [`crate::user_games::crossover`], which belongs to neither.
//!
//! [`steam`] is the only one with real data in it today. [`itch`] and [`gamejolt`] are
//! placeholders carrying no types on purpose: what those stores publish is not yet known, and a
//! shape invented before there is one real entry to hold would only have to be undone.

pub mod epic;
pub mod gamejolt;
pub mod itch;
pub mod steam;
