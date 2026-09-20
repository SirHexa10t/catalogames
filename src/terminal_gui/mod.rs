//! Everything this program draws on a terminal, and nothing else.
//!
//! **Binary-only, and that is the dividing line.** An embedding project calls the library
//! functions directly and has no use for a form, a menu or a colour gate — the same reason clap
//! is behind the `cli` feature. Nothing in here is reachable from [`catalogames`] the library.
//!
//! [`cli`] is the argument surface, [`picker`] the sales form, [`setup`] the credentials form,
//! and [`logger`] the narration that `-v` turns on. They are together because they share a
//! constraint rather than a subject: each needs a terminal to exist, and each must degrade to
//! something a pipe can carry when there is not one.

pub mod cli;
pub mod logger;
pub mod picker;
pub mod setup;
