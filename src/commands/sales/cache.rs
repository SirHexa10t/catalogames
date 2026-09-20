//! Keeping a fetched listing on disk, so the next run can offer it at once.
//!
//! Fetching every bundle from every store takes minutes, and the answer barely moves between one
//! run and the next: a bundle lasts days or weeks. So a run saves what it read, and the next one
//! puts it straight on the screen.
//!
//! **What is stored is instants, never durations.** A deadline is kept as the moment it falls,
//! and how long is left is worked out each time the listing is drawn. A cache reloaded two hours
//! later therefore shows two hours less on every bundle, with no arithmetic anywhere and nothing
//! to keep in step.

use std::path::{Path, PathBuf};
use std::time::Duration;

use log::debug;
use serde::{Deserialize, Serialize};

use crate::clock::Timestamp;
use crate::{Bundle, Error, Result};

/// How old a capture may be and still be offered.
///
/// A day, because that is the grain the data moves at: bundles run for weeks, and their prices
/// and contents do not change within an afternoon. Long enough that a person checking twice in
/// one day waits once; short enough that a bundle added this morning is not missed all week.
pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// What every cache file is called after its timestamp.
const SUFFIX: &str = "_sales.json";

/// One store's part of a capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    /// The site, as the listing cites it.
    pub site: String,
    pub bundles: Vec<Bundle>,
    /// What went wrong when this was read, already rendered.
    ///
    /// Text, because [`crate::Problem`] carries an [`Error`] that does not serialise — and
    /// because nothing needs to reconstruct one. What a reader needs is to know that what they
    /// are being shown is incomplete.
    ///
    /// **Dropping these would make a cache lie.** A store whose pages half failed leaves a
    /// capture missing those bundles and no record that they are missing; reloaded, it would put
    /// a cheerful "saved earlier" note over a partial listing for up to a day, where a live run
    /// at least scrolled the reasons past once.
    #[serde(default)]
    pub problems: Vec<String>,
}

/// A whole capture: what was read, from where, and when.
///
/// # What a capture cannot tell you
///
/// Both tests that retire one — its age, and a bundle in it having ended — fire on the capture
/// going *wrong*. Neither fires on the world gaining something new. A capture taken at ten,
/// holding bundles all good for another week, is reused at noon even though a bundle launched at
/// eleven; nothing looks amiss and the new bundle is simply absent. That is the cache's real
/// failure mode, it is invisible by construction, and `-f` is the way past it.
///
/// It bites hardest where a store publishes no deadline at all. Fanatical's is sparse — absent
/// on two of five bundles in one capture — and for those the age limit is the only test doing
/// anything at all.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capture {
    /// The version of this crate that wrote it.
    ///
    /// A serialised format is a compatibility contract, and these types are not frozen: `Game`
    /// has gained a field twice. An old file meeting a new type either fails to parse or, worse,
    /// fills the gap with a default — a bundle whose games all carry no contents being
    /// indistinguishable from one with no packs. A cache is the one thing that may always simply
    /// be thrown away, so a capture from another version is refetched rather than migrated.
    pub version: String,
    /// When it was read, as `YYYY-MM-DD HH:MM UTC`, for a person.
    pub captured: String,
    /// The instant, kept **inside** the record and not only in the file name: copying or
    /// renaming a file must not change what its contents claim about themselves, and a
    /// correctness decision should not come from parsing a name.
    pub captured_at: Timestamp,
    pub stores: Vec<Store>,
}

/// Why a capture cannot stand in for a fresh read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stale {
    /// Older than [`MAX_AGE`], or stamped in the future by a clock that moved.
    TooOld,
    /// Written by a different version of this crate, whose types may differ.
    OtherVersion(String),
    /// A bundle in it has already ended, so the whole capture is behind the world.
    Ended(String),
    /// It covers different stores than the ones asked for.
    OtherStores,
}

impl std::fmt::Display for Stale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooOld => write!(f, "over {} hours old", MAX_AGE.as_secs() / 3_600),
            Self::OtherVersion(version) => write!(f, "written by version {version}"),
            Self::Ended(title) => write!(f, "{title:?} has ended since it was taken"),
            Self::OtherStores => write!(f, "it covers other stores"),
        }
    }
}

impl Capture {
    /// Builds a capture of what was just read.
    #[must_use]
    pub fn of(stores: Vec<Store>) -> Self {
        let now = Timestamp::now();
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            captured: now.to_string(),
            captured_at: now,
            stores,
        }
    }

    /// Why this capture cannot be shown instead of fetching, or `None` when it can.
    ///
    /// The expired-bundle test is the one that earns its place. Age alone would happily show a
    /// bundle that ended an hour ago as still on sale, and a listing whose whole purpose is
    /// deadlines has no business doing that.
    #[must_use]
    pub fn stale(&self, wanted: &[&str], now: Timestamp) -> Option<Stale> {
        if self.version != env!("CARGO_PKG_VERSION") {
            return Some(Stale::OtherVersion(self.version.clone()));
        }
        // Compared as a set: asking for two stores in the other order is the same question. A
        // capture covers a store only if that store's fetch returned, so a store that failed is
        // simply absent here, the sets stop matching, and the next run refetches rather than
        // reusing a capture with a whole store missing from it.
        let mut sites: Vec<&str> = self
            .stores
            .iter()
            .map(|store| store.site.as_str())
            .collect();
        let mut wanted: Vec<&str> = wanted.to_vec();
        sites.sort_unstable();
        wanted.sort_unstable();
        if sites != wanted {
            return Some(Stale::OtherStores);
        }
        let age = now.seconds_from(self.captured_at).unsigned_abs();
        if now.seconds_from(self.captured_at) < 0 || age > MAX_AGE.as_secs() {
            return Some(Stale::TooOld);
        }
        self.stores
            .iter()
            .flat_map(|store| &store.bundles)
            .find(|bundle| bundle.ends_at.is_some_and(|at| at.seconds_from(now) <= 0))
            .map(|bundle| Stale::Ended(bundle.title.clone()))
    }

    /// What this capture is called on disk: its timestamp first, so a listing sorts by age.
    #[must_use]
    pub fn file_name(&self) -> String {
        format!("{}{SUFFIX}", self.captured_at.stamp())
    }

    /// Everything that went wrong when this was read, across every store.
    #[must_use]
    pub fn problems(&self) -> Vec<&str> {
        self.stores
            .iter()
            .flat_map(|store| store.problems.iter().map(String::as_str))
            .collect()
    }
}

/// Whether a file name is one this module writes: a `YYYY-mm-DD_HHMM` stamp and nothing else.
///
/// Matched by shape rather than by suffix alone, because these files are deleted and the
/// directory belongs to whatever else wants to use it.
fn ours(name: &str) -> bool {
    let Some(stamp) = name.strip_suffix(SUFFIX) else {
        return false;
    };
    stamp.len() == "0000-00-00_0000".len()
        && stamp
            .chars()
            .zip("0000-00-00_0000".chars())
            .all(|(had, shape)| {
                if shape == '0' {
                    had.is_ascii_digit()
                } else {
                    had == shape
                }
            })
}

/// Where captures are kept: `$XDG_CACHE_HOME/catalogames`, else `~/.cache/catalogames`.
///
/// A cache directory rather than a configuration one, because that is what this is: everything
/// here can be deleted at any moment and the only cost is one slow run.
pub fn directory() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME")
        && !xdg.is_empty()
    {
        return Ok(PathBuf::from(xdg).join(env!("CARGO_PKG_NAME")));
    }
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => Ok(PathBuf::from(home)
            .join(".cache")
            .join(env!("CARGO_PKG_NAME"))),
        _ => Err(Error::Setup {
            detail: "neither XDG_CACHE_HOME nor HOME is set, so there is nowhere to keep a cache"
                .to_owned(),
        }),
    }
}

/// The most recent capture in `directory`, if there is one that parses.
///
/// A capture that will not parse is ignored rather than reported: an old file from a previous
/// version of this program is not a fault, and the only consequence is fetching.
#[must_use]
pub fn newest(directory: &Path) -> Option<Capture> {
    let mut captures: Vec<Capture> = std::fs::read_dir(directory)
        .ok()?
        .filter_map(std::result::Result::ok)
        .filter(|entry| ours(&entry.file_name().to_string_lossy()))
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    captures.sort_by_key(|capture: &Capture| capture.captured_at);
    captures.pop()
}

/// Writes a capture, and removes the ones it supersedes.
///
/// Older captures are deleted because a timestamped name means they would otherwise accumulate
/// forever, and only the newest is ever read. Nothing else in the directory is touched: only
/// files this module names are removed, so a cache directory shared with something else is safe.
pub fn save(directory: &Path, capture: &Capture) -> Result<PathBuf> {
    let failed = |detail: String| Error::Drift {
        subject: directory.display().to_string(),
        detail,
    };
    std::fs::create_dir_all(directory).map_err(|source| failed(format!("{source}")))?;

    let path = directory.join(capture.file_name());
    let body = serde_json::to_vec_pretty(capture).map_err(|source| Error::Payload {
        url: "cache".into(),
        source,
    })?;
    // Not owner-only: a listing of public store pages is not private, and a cache a person
    // cannot read is a cache they cannot check.
    crate::write::replace(&path, &body, 0o644)?;

    // Only after the new file is safely down, and only over names this module makes: a cache
    // directory is not ours alone, and a failed write must not leave us with nothing.
    for stale in std::fs::read_dir(directory).into_iter().flatten().flatten() {
        let name = stale.file_name();
        if ours(&name.to_string_lossy()) && stale.path() != path {
            debug!("removing superseded capture {}", name.to_string_lossy());
            let _ = std::fs::remove_file(stale.path());
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Game;

    fn bundle(title: &str, ends_at: Option<&str>) -> Bundle {
        Bundle {
            title: title.to_owned(),
            url: "https://example.test".to_owned(),
            price: None,
            ends_at: ends_at.map(|text| Timestamp::parse(text).expect("parses")),
            games: vec![Game {
                title: "A Game".to_owned(),
                machine_name: "a-game".to_owned(),
                steam_app_id: Some(1),
                contains: Vec::new(),
            }],
        }
    }

    fn capture(at: &str, bundles: Vec<Bundle>) -> Capture {
        let taken = Timestamp::parse(at).expect("parses");
        Capture {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            captured: taken.to_string(),
            captured_at: taken,
            stores: vec![Store {
                site: "humblebundle.com".to_owned(),
                bundles,
                problems: Vec::new(),
            }],
        }
    }

    fn at(text: &str) -> Timestamp {
        Timestamp::parse(text).expect("parses")
    }

    #[test]
    fn a_recent_capture_of_the_right_stores_is_offered_as_it_is() {
        let held = capture(
            "2026-09-17T10:00:00",
            vec![bundle("A", Some("2026-09-30T00:00:00"))],
        );
        assert_eq!(
            held.stale(&["humblebundle.com"], at("2026-09-17T18:00:00")),
            None
        );
    }

    #[test]
    fn a_capture_older_than_a_day_is_refused() {
        let held = capture(
            "2026-09-16T10:00:00",
            vec![bundle("A", Some("2026-09-30T00:00:00"))],
        );
        assert_eq!(
            held.stale(&["humblebundle.com"], at("2026-09-17T11:00:00")),
            Some(Stale::TooOld)
        );
        // And the hour before that is still inside the day.
        assert_eq!(
            held.stale(&["humblebundle.com"], at("2026-09-17T09:00:00")),
            None
        );
    }

    #[test]
    fn a_capture_holding_a_bundle_that_has_since_ended_is_refused() {
        // Age alone would show a bundle that ended an hour ago as still on sale, which a
        // listing about deadlines has no business doing.
        let held = capture(
            "2026-09-17T10:00:00",
            vec![
                bundle("Still going", Some("2026-09-30T00:00:00")),
                bundle("Over", Some("2026-09-17T12:00:00")),
            ],
        );
        assert_eq!(
            held.stale(&["humblebundle.com"], at("2026-09-17T13:00:00")),
            Some(Stale::Ended("Over".to_owned()))
        );
    }

    #[test]
    fn a_bundle_with_no_published_deadline_never_makes_a_capture_stale() {
        let held = capture("2026-09-17T10:00:00", vec![bundle("A", None)]);
        assert_eq!(
            held.stale(&["humblebundle.com"], at("2026-09-17T18:00:00")),
            None
        );
    }

    #[test]
    fn a_capture_of_other_stores_is_not_used_for_this_question() {
        // Asking for one store then for all of them must not answer the second from the first.
        let held = capture("2026-09-17T10:00:00", vec![bundle("A", None)]);
        assert_eq!(
            held.stale(
                &["humblebundle.com", "fanatical.com"],
                at("2026-09-17T11:00:00")
            ),
            Some(Stale::OtherStores)
        );
    }

    #[test]
    fn a_capture_from_the_future_is_refused_rather_than_trusted() {
        // A clock that moved backwards would otherwise make a capture look eternally fresh.
        let held = capture("2026-09-18T10:00:00", vec![bundle("A", None)]);
        assert_eq!(
            held.stale(&["humblebundle.com"], at("2026-09-17T10:00:00")),
            Some(Stale::TooOld)
        );
    }

    #[test]
    fn a_capture_written_by_another_version_is_refetched_rather_than_migrated() {
        // These types are not frozen — Game has gained a field twice — and a missing field
        // deserialises into a default rather than failing, so an old capture would quietly
        // claim every bundle has no packs. A cache may always simply be thrown away.
        let mut held = capture("2026-09-17T10:00:00", vec![bundle("A", None)]);
        held.version = "0.0.1".to_owned();
        assert_eq!(
            held.stale(&["humblebundle.com"], at("2026-09-17T11:00:00")),
            Some(Stale::OtherVersion("0.0.1".to_owned()))
        );
    }

    #[test]
    fn the_stores_are_compared_as_a_set_so_the_order_asked_in_does_not_matter() {
        let mut held = capture("2026-09-17T10:00:00", vec![bundle("A", None)]);
        held.stores.push(Store {
            site: "fanatical.com".to_owned(),
            bundles: vec![bundle("B", None)],
            problems: Vec::new(),
        });
        for asked in [
            ["humblebundle.com", "fanatical.com"],
            ["fanatical.com", "humblebundle.com"],
        ] {
            assert_eq!(
                held.stale(&asked, at("2026-09-17T11:00:00")),
                None,
                "{asked:?}"
            );
        }
    }

    #[test]
    fn what_went_wrong_when_it_was_read_is_kept_with_it() {
        // A store whose pages half failed leaves a capture missing those bundles. Without this
        // the reload would put a cheerful note over a partial listing for a day.
        let mut held = capture("2026-09-17T10:00:00", vec![bundle("A", None)]);
        held.stores[0].problems = vec!["B: could not reach it".to_owned()];

        let text = serde_json::to_string(&held).expect("serialises");
        let back: Capture = serde_json::from_str(&text).expect("parses");
        assert_eq!(back.problems(), ["B: could not reach it"]);
    }

    #[test]
    fn only_files_this_module_names_are_ever_deleted() {
        // The cache directory is not ours alone.
        assert!(ours("2026-09-17_1332_sales.json"));
        assert!(!ours("sales.json"));
        assert!(!ours("notes_sales.json"));
        assert!(!ours("2026-09-17_sales.json"));
        assert!(!ours("2026-09-17_1332_sales.json.bak"));
    }

    #[test]
    fn the_file_is_named_for_when_it_was_taken_so_the_directory_sorts_by_age() {
        let held = capture("2026-09-17T13:32:00", Vec::new());
        assert_eq!(held.file_name(), "2026-09-17_1332_sales.json");
    }

    #[test]
    fn saving_keeps_only_the_newest_and_leaves_anything_else_alone() {
        let directory =
            std::env::temp_dir().join(format!("catalogames-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("creates");
        std::fs::write(directory.join("notes.txt"), "mine").expect("writes");

        let older = capture("2026-09-16T09:00:00", vec![bundle("Old", None)]);
        let newer = capture("2026-09-17T09:00:00", vec![bundle("New", None)]);
        save(&directory, &older).expect("saves");
        save(&directory, &newer).expect("saves");

        let names: Vec<String> = std::fs::read_dir(&directory)
            .expect("reads")
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&newer.file_name()), "{names:?}");
        assert!(!names.contains(&older.file_name()), "{names:?}");
        assert!(
            names.contains(&"notes.txt".to_owned()),
            "it touched a file that is not its own"
        );

        let read = newest(&directory).expect("reads the capture back");
        assert_eq!(read.stores[0].bundles[0].title, "New");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_deadline_survives_the_round_trip_as_an_instant() {
        // The whole reason instants are stored rather than "in x hours": how long is left is
        // worked out when the listing is drawn, so a reload shows less time with no arithmetic.
        let held = capture(
            "2026-09-17T10:00:00",
            vec![bundle("A", Some("2026-09-18T04:00:00"))],
        );
        let text = serde_json::to_string(&held).expect("serialises");
        let back: Capture = serde_json::from_str(&text).expect("parses");

        let deadline = back.stores[0].bundles[0].ends_at.expect("a deadline");
        assert_eq!(deadline.seconds_from(at("2026-09-17T10:00:00")), 18 * 3_600);
        assert_eq!(deadline.seconds_from(at("2026-09-17T12:00:00")), 16 * 3_600);
    }
}
