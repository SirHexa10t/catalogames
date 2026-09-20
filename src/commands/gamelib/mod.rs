//! One-off snapshots of the account holder's own game libraries.
//!
//! A snapshot, not a sync: each run writes a point-in-time record and stops. Nothing here holds
//! a session open, refreshes a token in the background, or runs on a schedule.
//!
//! Each store gets its own handler — [`steam`], [`epic`] — because what they need and how they
//! are reached have nothing in common: Steam has a documented, key-gated Web API; Epic has no
//! public API at all and is read through a third-party tool. Only the parts that are genuinely
//! shared live here: the snapshot shape, writing it safely, and reading credentials.
//!
//! # What the files are
//!
//! Your own purchase history and playtime. Not secret, but more revealing than it first looks —
//! written `0600`, and never anywhere but the directory you asked for.

pub mod config;
pub mod epic;
pub mod steam;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::{Error, Result};

/// What one `gamelib` run produced.
///
/// A store may publish more than one thing — Steam has a library and a wishlist — and they fail
/// independently: a public library beside a private wishlist is an ordinary configuration.
/// Whatever could be written is written, and whatever could not says why, in the same shape
/// [`crate::Listing`] uses for a partly-successful bundle run.
#[derive(Debug, Default)]
pub struct Report {
    /// Files written, in the order they were taken.
    pub written: Vec<PathBuf>,
    /// Snapshots that were not taken, and why.
    pub failures: Vec<Failure>,
}

impl Report {
    /// True when the run completed with nothing to report.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.failures.is_empty()
    }

    /// Runs one snapshot, recording either the file it wrote or the reason it did not.
    ///
    /// Takes a closure rather than a `Result` so that a snapshot is not even attempted when an
    /// earlier failure has already been recorded for it — and so the call site reads as one
    /// line per thing the store publishes.
    pub fn take(
        &mut self,
        what: Kind,
        directory: &Path,
        produce: impl FnOnce() -> Result<Snapshot>,
    ) {
        match produce().and_then(|snapshot| snapshot.write(directory)) {
            Ok(path) => self.written.push(path),
            Err(reason) => self.failures.push(Failure {
                what: Some(what),
                reason,
            }),
        }
    }
}

/// One snapshot that could not be taken.
#[derive(Debug)]
pub struct Failure {
    /// Which snapshot failed, or `None` when nothing could be read from the store at all.
    ///
    /// The distinction keeps a setup message from being repeated once per snapshot: a missing
    /// API key stops the library and the wishlist alike, and saying so twice would suggest two
    /// separate problems.
    pub what: Option<Kind>,
    pub reason: Error,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(what) = self.what {
            write!(f, "{what}: ")?;
        }
        f.write_str(&crate::error::chain(&self.reason))
    }
}

/// What a snapshot records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Games the account owns.
    Library,
    /// Games the account has wishlisted.
    Wishlist,
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Library => "library",
            Self::Wishlist => "wishlist",
        })
    }
}

impl Kind {
    /// Every kind there is, for a caller that has to visit all of them.
    ///
    /// A list rather than a derive, and held to the enum by a test that indexes one against an
    /// exhaustive match: a new variant stops that test compiling until it is named, and then
    /// fails it until it is added here in the same position.
    pub const ALL: [Self; 2] = [Self::Library, Self::Wishlist];

    /// The middle of the file name: `game_<part>_<store>`.
    fn file_part(self) -> &'static str {
        match self {
            Self::Library => "lib",
            Self::Wishlist => "wishlist",
        }
    }

    /// Whether a smaller snapshot than the last one is normal for this kind.
    ///
    /// A wishlist shrinks every time a game on it is bought, so shrinking is its ordinary
    /// behaviour. A library only shrinks on a refund, which is rare enough that a shorter read
    /// is far more likely to be a privacy setting, the wrong account, or a truncated response —
    /// see [`Snapshot::write`], which refuses that case rather than overwriting good data.
    fn shrinking_is_normal(self) -> bool {
        match self {
            Self::Wishlist => true,
            Self::Library => false,
        }
    }
}

/// One game in a snapshot.
///
/// Every field but the store's own id is optional, because the stores do not publish the same
/// things and an absent field is a fact worth keeping distinct from a zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// The store's own identifier: a Steam app-id as text, an Epic `app_name`.
    pub id: String,
    /// Steam application id, where the store has one. The join key to
    /// [`crate::inventory::steam`]; `None` for stores that do not use Steam ids.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playtime_minutes: Option<u32>,
    /// Wishlist position, lower first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
}

/// Where a snapshot came from, so a file found later can be judged.
///
/// The count the service reported sits beside the number of entries actually written. Two
/// numbers for one fact: while they agree the read was complete, and a disagreement is the
/// earliest sign it was not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenance {
    /// What was asked, so a reader knows which definition they are holding.
    pub source_url: String,
    /// The account the data belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// The count the service stated, when it stated one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reported_count: Option<u32>,
    /// The tool that read it, when something other than this crate did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_with: Option<String>,
}

/// A point-in-time record of one library or wishlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub store: &'static str,
    pub kind: Kind,
    /// When it was taken, UTC, as `YYYY-MM-DD HH:MM:SSZ`.
    pub captured: String,
    /// Seconds since the Unix epoch — the same instant, for anything comparing snapshots.
    pub captured_unix: u64,
    pub provenance: Provenance,
    /// Sorted by id, so two snapshots diff into "what changed" rather than a reshuffle.
    pub games: Vec<Entry>,
}

impl Snapshot {
    /// Builds a snapshot, stamping it with the time and putting the entries in a stable order.
    pub fn new(
        store: &'static str,
        kind: Kind,
        provenance: Provenance,
        mut games: Vec<Entry>,
    ) -> Self {
        // Numeric where there is an app-id, so 1202130 does not sort before 550320.
        games.sort_by(|a, b| match (a.app_id, b.app_id) {
            (Some(x), Some(y)) => x.cmp(&y),
            _ => a.id.cmp(&b.id),
        });
        games.dedup_by(|a, b| a.id == b.id);

        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self {
            store,
            kind,
            captured: utc_timestamp(unix),
            captured_unix: unix,
            provenance,
            games,
        }
    }

    /// `game_lib_steam`, `game_wishlist_steam`, `game_lib_epic`.
    pub fn file_name(&self) -> String {
        file_name(self.kind, self.store)
    }

    /// Writes the snapshot into `directory`, replacing any previous one.
    ///
    /// A library that came back smaller than the one already on disk is refused. Steam removes
    /// a game only on a refund, so a shorter read is far more likely to be a privacy setting
    /// that changed, the wrong account, or a truncated response than a real loss — and
    /// replacing good data with bad is the one failure a snapshot file cannot recover from.
    /// The old file is kept and the caller is told what to check.
    ///
    /// A wishlist is exempt: it shrinks by design every time something on it is bought.
    pub fn write(&self, directory: &Path) -> Result<PathBuf> {
        let target = directory.join(self.file_name());
        if !self.kind.shrinking_is_normal()
            && let Some(previous) = entry_count(&target)
            && previous > self.games.len()
        {
            return Err(Error::Drift {
                subject: target.display().to_string(),
                detail: format!(
                    "the snapshot already there holds {previous} games and this one holds {} — \
                     a library does not shrink, so the old file has been kept. Check that the \
                     account is the right one and that its game details are still public, then \
                     run again. If the loss is real, move the old file aside first",
                    self.games.len()
                ),
            });
        }

        let body = serde_json::to_vec_pretty(self).map_err(|source| Error::Payload {
            url: self.provenance.source_url.as_str().into(),
            source,
        })?;
        // Owner-only: a library and a wishlist are more revealing than they look.
        crate::write::replace(&target, &body, 0o600)?;
        Ok(target)
    }
}

/// What a snapshot of `kind` from `store` is called: `game_lib_steam`, `game_wishlist_steam`.
///
/// Public and free-standing because the files are read back as well as written — see
/// [`crate::user_games::holdings`] — and a reader that worked out the name for itself would be a second
/// answer to drift away from this one.
#[must_use]
pub fn file_name(kind: Kind, store: &str) -> String {
    format!("game_{}_{}", kind.file_part(), store)
}

/// How many games the snapshot at `path` holds, or `None` if there is no readable one.
fn entry_count(path: &Path) -> Option<usize> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(value.get("games")?.as_array()?.len())
}

/// `YYYY-MM-DD HH:MM:SSZ` from seconds since the Unix epoch.
///
/// Hand-rolled rather than pulling a date crate in for one field. The civil-from-days part is
/// Howard Hinnant's algorithm, which is exact for every date after 1970 — the only range a
/// capture timestamp can occupy.
fn utc_timestamp(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86_400) as i64;
    let seconds = unix_seconds % 86_400;

    // Shift the epoch to 0000-03-01 so a leap day lands at the end of the cycle.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;

    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z",
        seconds / 3_600,
        (seconds % 3_600) / 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, app_id: Option<u32>) -> Entry {
        Entry {
            id: id.to_owned(),
            app_id,
            name: None,
            playtime_minutes: None,
            priority: None,
        }
    }

    fn snapshot(games: Vec<Entry>) -> Snapshot {
        Snapshot::new(
            "steam",
            Kind::Library,
            Provenance {
                source_url: "https://example.test".to_owned(),
                account: None,
                reported_count: None,
                read_with: None,
            },
            games,
        )
    }

    #[test]
    fn a_timestamp_is_the_date_it_should_be() {
        // Pinned against known instants, because the arithmetic is hand-rolled.
        assert_eq!(utc_timestamp(0), "1970-01-01 00:00:00Z");
        assert_eq!(utc_timestamp(1_000_000_000), "2001-09-09 01:46:40Z");
        // A leap day, which is the whole reason the algorithm shifts the year to March.
        assert_eq!(utc_timestamp(1_709_164_800), "2024-02-29 00:00:00Z");
        assert_eq!(utc_timestamp(1_758_000_000), "2025-09-16 05:20:00Z");
    }

    #[test]
    fn entries_are_sorted_numerically_so_two_snapshots_diff_meaningfully() {
        // Sorted as text, 1202130 would come before 550320 and every diff would be noise.
        let snapshot = snapshot(vec![
            entry("550320", Some(550_320)),
            entry("1202130", Some(1_202_130)),
            entry("210970", Some(210_970)),
        ]);
        let ids: Vec<u32> = snapshot.games.iter().filter_map(|g| g.app_id).collect();
        assert_eq!(ids, [210_970, 550_320, 1_202_130]);
    }

    #[test]
    fn a_game_listed_twice_is_recorded_once() {
        let snapshot = snapshot(vec![entry("1", Some(1)), entry("1", Some(1))]);
        assert_eq!(snapshot.games.len(), 1);
    }

    #[test]
    fn stores_without_steam_ids_sort_by_their_own_identifier() {
        let snapshot = Snapshot::new(
            "epic",
            Kind::Library,
            Provenance {
                source_url: "legendary".to_owned(),
                account: None,
                reported_count: None,
                read_with: None,
            },
            vec![entry("Zulu", None), entry("Alpha", None)],
        );
        let ids: Vec<&str> = snapshot.games.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids, ["Alpha", "Zulu"]);
    }

    #[test]
    fn the_file_is_named_for_its_store_and_kind() {
        assert_eq!(snapshot(Vec::new()).file_name(), "game_lib_steam");
        let wishlist = Snapshot::new(
            "steam",
            Kind::Wishlist,
            Provenance {
                source_url: String::new(),
                account: None,
                reported_count: None,
                read_with: None,
            },
            Vec::new(),
        );
        assert_eq!(wishlist.file_name(), "game_wishlist_steam");
    }

    #[test]
    fn a_written_snapshot_is_readable_only_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let directory =
            std::env::temp_dir().join(format!("catalogames-test-{}", std::process::id()));
        let written = snapshot(vec![entry("1", Some(1))])
            .write(&directory)
            .expect("writes");

        let mode = std::fs::metadata(&written)
            .expect("exists")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o077,
            0,
            "group or world can read {written:?}: {mode:o}"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_wishlist_may_shrink_because_buying_a_game_takes_it_off_the_list() {
        let directory =
            std::env::temp_dir().join(format!("catalogames-wish-{}", std::process::id()));
        let full = Snapshot::new(
            "steam",
            Kind::Wishlist,
            Provenance {
                source_url: String::new(),
                account: None,
                reported_count: None,
                read_with: None,
            },
            vec![entry("1", Some(1)), entry("2", Some(2))],
        );
        full.write(&directory).expect("writes");

        let smaller = Snapshot::new(
            "steam",
            Kind::Wishlist,
            Provenance {
                source_url: String::new(),
                account: None,
                reported_count: None,
                read_with: None,
            },
            vec![entry("1", Some(1))],
        );
        let path = smaller
            .write(&directory)
            .expect("a shorter wishlist is ordinary");
        let text = std::fs::read_to_string(&path).expect("exists");
        assert_eq!(text.matches("\"id\"").count(), 1);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_shrunken_library_is_refused_and_the_old_one_kept() {
        // A library does not lose games. A smaller read is a privacy setting, the wrong
        // account, or a partial response — never news, and never worth losing the old file for.
        let directory =
            std::env::temp_dir().join(format!("catalogames-shrink-{}", std::process::id()));
        let full = snapshot(vec![
            entry("1", Some(1)),
            entry("2", Some(2)),
            entry("3", Some(3)),
        ]);
        let path = full.write(&directory).expect("writes");

        let failure = snapshot(vec![entry("1", Some(1))])
            .write(&directory)
            .unwrap_err();
        assert!(failure.to_string().contains("does not shrink"), "{failure}");

        let kept = std::fs::read_to_string(&path).expect("still there");
        assert_eq!(
            kept.matches("\"id\"").count(),
            3,
            "the old snapshot was lost"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_snapshot_the_same_size_or_larger_replaces_the_old_one() {
        let directory =
            std::env::temp_dir().join(format!("catalogames-grow-{}", std::process::id()));
        snapshot(vec![entry("1", Some(1))])
            .write(&directory)
            .expect("writes");
        let path = snapshot(vec![entry("1", Some(1)), entry("2", Some(2))])
            .write(&directory)
            .expect("replaces");

        let text = std::fs::read_to_string(&path).expect("exists");
        assert_eq!(text.matches("\"id\"").count(), 2);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn nothing_is_left_behind_when_a_write_succeeds() {
        let directory =
            std::env::temp_dir().join(format!("catalogames-tidy-{}", std::process::id()));
        snapshot(vec![entry("1", Some(1))])
            .write(&directory)
            .expect("writes");

        let leftovers: Vec<_> = std::fs::read_dir(&directory)
            .expect("exists")
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "temporary file left behind");
        let _ = std::fs::remove_dir_all(&directory);
    }
}
