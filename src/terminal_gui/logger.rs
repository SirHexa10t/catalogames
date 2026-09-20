//! Minimal `log` sink for the command line.
//!
//! Hand-written rather than pulled from a logging crate because the CLI needs
//! exactly one behaviour — verbosity chosen by `-v` — and the library side is
//! the `log` facade either way, so an embedding project routes records into
//! whatever logging it already runs and never sees this.

use log::{Level, LevelFilter, Log, Metadata, Record};

/// Writes records to stderr so that stdout stays a clean listing, pipeable
/// without the narration mixed in.
struct Stderr;

static SINK: Stderr = Stderr;

/// Log target this crate's own records carry.
const OURS: &str = "catalogames";

/// Whether a record came from this project rather than a dependency.
///
/// Without this, `-vv` is unusable: `scraper`'s HTML tokenizer logs every token
/// it processes at debug level and `reqwest`'s TLS stack logs each root
/// certificate, so tens of thousands of lines bury the handful that describe
/// what the program is actually doing. Dependency logs are still reachable by
/// an embedding project, which installs its own logger and sets its own
/// filters.
fn is_ours(target: &str) -> bool {
    target == OURS || target.starts_with(&format!("{OURS}::"))
}

impl Log for Stderr {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        is_ours(metadata.target()) && metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("[{}] {}", label(record.level()), record.args());
        }
    }

    fn flush(&self) {}
}

/// Lower-case level tag. An exhaustive `match` rather than a lookup table, so
/// a new level in the `log` crate would fail the build instead of going
/// unlabelled.
fn label(level: Level) -> &'static str {
    match level {
        Level::Error => "error",
        Level::Warn => "warn",
        Level::Info => "info",
        Level::Debug => "debug",
        Level::Trace => "trace",
    }
}

/// Installs the sink at a verbosity set by how often `-v` was given.
///
/// The default admits warnings and errors only. The library emits neither
/// during a healthy run — problems come back as [`catalogames::Problem`]
/// values instead — so a run without `-v` prints exactly what it always did.
pub fn install(verbosity: u8) {
    log::set_max_level(match verbosity {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    });
    // Only fails if a logger is already installed, which cannot happen here.
    let _ = log::set_logger(&SINK);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_from_this_crate_are_kept() {
        assert!(is_ours("catalogames"));
        assert!(is_ours("catalogames::commands::sales::humble::client"));
        assert!(is_ours("catalogames::commands::sales::humble::parse"));
    }

    #[test]
    fn records_from_dependencies_are_dropped() {
        // These two are the reason the filter exists at all.
        assert!(!is_ours("html5ever::tree_builder"));
        assert!(!is_ours("rustls::client::hs"));
        assert!(!is_ours("reqwest::connect"));
    }

    #[test]
    fn a_crate_merely_prefixed_like_ours_is_dropped() {
        assert!(!is_ours("catalogames_extras"));
        assert!(!is_ours("catalogames-plugin"));
    }

    #[test]
    fn every_level_has_a_label() {
        // Exhaustive by construction: `label` matches on `Level`, so a new
        // level would fail the build rather than print without a tag.
        for level in [
            Level::Error,
            Level::Warn,
            Level::Info,
            Level::Debug,
            Level::Trace,
        ] {
            assert!(!label(level).is_empty());
        }
    }
}
