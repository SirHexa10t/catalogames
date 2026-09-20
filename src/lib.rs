//! Track game bundle deals across stores.
//!
//! Every capability is a library function; the `catalogames` binary is a thin
//! shell over this crate so that embedding projects can reach all of it.
//!
//! ```no_run
//! use catalogames::user_games::holdings::Holdings;
//! use catalogames::render::{self, Palette};
//!
//! let listing = catalogames::commands::sales::humble::Client::new()?.list_bundles()?;
//! // `Holdings::none()` marks nothing. `Holdings::load(dir)` reads the library and wishlist
//! // files `gamelib` writes, and then a game already owned or wanted says so on its own line.
//! print!("{}", render::listing(&listing, Palette::Plain, &Holdings::none()));
//! # Ok::<(), catalogames::Error>(())
//! ```
//!
//! Fetching and parsing are separate: [`humble::parse_index`] and
//! [`humble::parse_bundle`] take page text and do no I/O, so a caller with its
//! own HTTP stack — async, cached, proxied — can skip [`humble::Client`] and
//! keep the parsing.

#![forbid(unsafe_code)]

pub mod clock;
pub mod commands;
pub mod error;
pub mod inventory;
pub mod links;
pub mod model;
pub mod render;
pub mod steam;
pub mod store_inventory;
pub mod user_games;
mod write;

pub use error::{Error, Result};
pub use model::{Bundle, Game, Listing, Money, Price, Problem, ProblemKind};
