//! Instants, in UTC, without a date crate.
//!
//! One home for date arithmetic so the same conversion is not written twice. What is needed is
//! small and exact: parse the timestamps two stores publish, say how long is left, and print an
//! instant a person can read.
//!
//! # Why not `chrono` or `time`
//!
//! Both are excellent and both are larger than the problem. This crate needs no time zones, no
//! locales, no calendars, no formatting language — it needs seconds-since-the-epoch and one
//! display format. The conversion below is Howard Hinnant's `days_from_civil`, which is exact
//! for every proleptic Gregorian date and is about twenty lines in each direction. Adding a
//! dependency to avoid twenty tested lines would be the wrong trade for a crate whose README
//! counts its own transitive dependencies.
//!
//! If this ever needs a second time zone, a locale, or leap seconds, that trade flips — take
//! the dependency rather than growing this file.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// An instant, as seconds since the Unix epoch, always UTC.
///
/// Held as a signed count so that arithmetic against a time in the past is ordinary subtraction
/// rather than a special case.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Wraps a count of seconds since the Unix epoch.
    #[must_use]
    pub const fn from_unix(seconds: i64) -> Self {
        Self(seconds)
    }

    /// Seconds since the Unix epoch.
    #[must_use]
    pub const fn unix(self) -> i64 {
        self.0
    }

    /// Now, read from the system clock.
    ///
    /// A clock set wrongly makes every "time left" wrong by the same amount, which is worth
    /// knowing when a remaining time looks implausible. Nothing here tries to correct for it:
    /// the alternative is trusting a store's own clock, and a store that publishes a bad
    /// timestamp would then be undetectable.
    #[must_use]
    pub fn now() -> Self {
        Self(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_secs() as i64),
        )
    }

    /// How many seconds separate this instant from `now`. Negative once it has passed.
    #[must_use]
    pub const fn seconds_from(self, now: Self) -> i64 {
        self.0 - now.0
    }

    /// `2026-09-17_1332` — for a file name, where a date has to sort and carry no spaces.
    #[must_use]
    pub fn stamp(self) -> String {
        let (year, month, day) = civil_from_days(self.0.div_euclid(86_400));
        let seconds = self.0.rem_euclid(86_400);
        format!(
            "{year:04}-{month:02}-{day:02}_{:02}{:02}",
            seconds / 3_600,
            (seconds % 3_600) / 60
        )
    }

    /// `2026-09-17` — the date alone, for a note recording which day something was read.
    #[must_use]
    pub fn date(self) -> String {
        let (year, month, day) = civil_from_days(self.0.div_euclid(86_400));
        format!("{year:04}-{month:02}-{day:02}")
    }

    /// Reads `YYYY-MM-DDTHH:MM:SS`, with an optional fraction and an optional `Z`.
    ///
    /// Both shapes the tracked stores publish: Fanatical marks its timestamps `Z`, and Humble
    /// publishes a bare one. Humble's being UTC is **measured, not assumed** — a bundle page
    /// carries the server's own `at_time|datetime`, which read `2026-09-16T19:23:19` against a
    /// wall clock reading `19:23:14 UTC`.
    ///
    /// A timestamp carrying an explicit offset such as `+02:00` is **refused** rather than read
    /// as UTC. Silently dropping an offset is exactly the class of mistake this function exists
    /// to prevent, and no tracked store sends one — so a refusal is a signal that something has
    /// changed, not an inconvenience.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().strip_suffix('Z').unwrap_or(text.trim());
        // Drop a fractional second; sub-second precision has no bearing on a bundle deadline.
        let text = text.split_once('.').map_or(text, |(whole, _)| whole);

        let (date, time) = text.split_once(['T', ' '])?;
        let mut parts = date.split('-');
        let year: i64 = parts.next()?.parse().ok()?;
        let month: u32 = parts.next()?.parse().ok()?;
        let day: u32 = parts.next()?.parse().ok()?;
        if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }

        let mut parts = time.split(':');
        let hour: i64 = parts.next()?.parse().ok()?;
        let minute: i64 = parts.next()?.parse().ok()?;
        // Seconds are optional: `2026-09-17T04:00` is a valid ISO-8601 instant.
        let second: i64 = parts.next().map_or(Ok(0), str::parse).ok()?;
        if parts.next().is_some() || hour > 23 || minute > 59 || second > 60 {
            return None;
        }

        Some(Self(
            days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second,
        ))
    }
}

/// `2026-09-17 04:00 UTC` — minutes, because no deadline here is stated to the second.
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day) = civil_from_days(self.0.div_euclid(86_400));
        let seconds = self.0.rem_euclid(86_400);
        write!(
            f,
            "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
            seconds / 3_600,
            (seconds % 3_600) / 60
        )
    }
}

/// A store DEADLINE older than this is a unit error rather than a deadline.
const YEAR_2000: i64 = 946_684_800;

/// The guard both readers share. Past it the value is milliseconds, not seconds: the same instant
/// a thousand times larger lands in the year 58,000.
const YEAR_2100: i64 = 4_102_444_800;

/// Seconds since the epoch, as some services publish a deadline.
///
/// Range-checked rather than wrapped blindly, because the mistake this guards is not a malformed
/// value but a **correctly formed one in the wrong unit**: the same instant in milliseconds is a
/// thousand times larger, lands in the year 58,000, and renders as an enormous "days left" that
/// looks like data rather than a bug. The window is 2000 to 2100 — a store deadline outside it
/// is not a deadline.
///
/// There is no reader here that takes both this and the textual form. [`Timestamp::parse`]
/// refuses a bare integer and this refuses anything that is not one, so a value can only be read
/// by the reader for its own representation.
#[must_use]
pub fn from_epoch_seconds(seconds: i64) -> Option<Timestamp> {
    (YEAR_2000..YEAR_2100)
        .contains(&seconds)
        .then_some(Timestamp(seconds))
}

/// The same millisecond guard as [`from_epoch_seconds`], without its floor, for an instant that
/// is allowed to predate Steam itself.
///
/// **The two callers disagree about what "too old to be real" means, and sharing one window was a
/// silent bug.** A store deadline in 1998 is a unit error; a RELEASE date in 1998 is Half-Life.
/// The catalogue sweep read release dates through [`from_epoch_seconds`] and so rendered `?` for
/// apps 20, 50 and 70 — Valve's own pre-2000 catalogue — although the store had sent a perfectly
/// good timestamp, and the row claimed the date was merely absent.
///
/// Zero is still refused: Valve sends `0` for "no date published", not for 1970-01-01.
#[must_use]
pub fn from_historic_epoch_seconds(seconds: i64) -> Option<Timestamp> {
    (1..YEAR_2100)
        .contains(&seconds)
        .then_some(Timestamp(seconds))
}

/// Reads a deadline a store published, warning rather than failing when it will not parse.
///
/// A bundle whose end date is unreadable is still a bundle worth listing, so this never sinks a
/// run. It does warn, because an unparseable timestamp means the store changed its format and
/// that is worth noticing before every bundle silently loses its deadline.
#[must_use]
pub fn parse_reported(text: &str, subject: &str) -> Option<Timestamp> {
    let parsed = Timestamp::parse(text);
    if parsed.is_none() {
        log::warn!("{subject}: could not read the end date {text:?}; the format has changed");
    }
    parsed
}

/// `2026-09-17 04:00:00Z`, for a record that wants the seconds.
#[must_use]
pub fn to_iso8601(at: Timestamp) -> String {
    let (year, month, day) = civil_from_days(at.unix().div_euclid(86_400));
    let seconds = at.unix().rem_euclid(86_400);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z",
        seconds / 3_600,
        (seconds % 3_600) / 60,
        seconds % 60
    )
}

/// Days from `1970-01-01` to the given civil date. Howard Hinnant's algorithm.
///
/// Shifts the year to start in March so that a leap day lands at the end of the cycle and the
/// month-length pattern becomes a straight line — which is what makes the whole thing
/// branchless and exact rather than a table of month lengths with special cases.
const fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 } as i64;
    let day_of_year = (153 * shifted_month + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The inverse of [`days_from_civil`].
const fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;

    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_instants_convert_both_ways() {
        // Pinned against instants that can be checked by hand, because the arithmetic is
        // hand-rolled and a silent off-by-one here would misdate every deadline.
        for (text, unix) in [
            ("1970-01-01T00:00:00", 0),
            ("2001-09-09T01:46:40", 1_000_000_000),
            ("2026-09-17T04:00:00", 1_789_617_600),
        ] {
            let parsed = Timestamp::parse(text).expect("parses");
            assert_eq!(parsed.unix(), unix, "{text}");
            assert_eq!(to_iso8601(parsed), format!("{text}Z").replace('T', " "));
        }
    }

    #[test]
    fn a_leap_day_is_a_real_day() {
        // The whole reason the algorithm shifts the year to March.
        let leap = Timestamp::parse("2024-02-29T00:00:00").expect("parses");
        assert_eq!(leap.unix(), 1_709_164_800);
        assert_eq!(to_iso8601(leap), "2024-02-29 00:00:00Z");
        // 2000 is a leap year and 1900 is not; a naive rule gets one of them wrong.
        assert_eq!(
            days_from_civil(2000, 3, 1) - days_from_civil(2000, 2, 28),
            2
        );
        assert_eq!(
            days_from_civil(1900, 3, 1) - days_from_civil(1900, 2, 28),
            1
        );
    }

    #[test]
    fn the_boundaries_a_calendar_gets_wrong_are_pinned_by_name() {
        // Each of these breaks a different naive rule: the century exceptions, the epoch, and
        // the 32-bit overflow date that no longer matters here but is the classic regression.
        for (text, unix) in [
            ("1970-01-01T00:00:00", 0),
            ("1900-02-28T00:00:00", -2_203_977_600), // 1900 is NOT a leap year
            ("2000-02-29T00:00:00", 951_782_400),    // 2000 IS one
            ("2024-02-29T00:00:00", 1_709_164_800),
            ("2100-02-28T00:00:00", 4_107_456_000), // 2100 is NOT one
            ("2028-02-29T00:00:00", 1_835_395_200), // an ordinary leap year
            ("2100-03-01T00:00:00", 4_107_542_400), // the day after a skipped leap day
            ("2038-01-19T03:14:07", 2_147_483_647), // the signed 32-bit ceiling
        ] {
            assert_eq!(
                Timestamp::parse(text).expect("parses").unix(),
                unix,
                "{text}"
            );
        }
        // The day after each non-leap February 28th is March 1st, not February 29th.
        assert_eq!(
            civil_from_days(days_from_civil(1900, 2, 28) + 1),
            (1900, 3, 1)
        );
        assert_eq!(
            civil_from_days(days_from_civil(2100, 2, 28) + 1),
            (2100, 3, 1)
        );
        assert_eq!(
            civil_from_days(days_from_civil(2000, 2, 28) + 1),
            (2000, 2, 29)
        );
    }

    #[test]
    fn an_epoch_reader_and_a_text_reader_each_refuse_the_other_s_shape() {
        // Cross-rejection, because a reader accepting its own format proves nothing — it is a
        // reader accepting the *other* format that silently misdates everything.
        let text = "2026-09-30T07:00:00";
        let epoch = 1_790_751_600;
        assert_eq!(Timestamp::parse(text).expect("parses").unix(), epoch);
        assert_eq!(from_epoch_seconds(epoch), Timestamp::parse(text));
        assert_eq!(Timestamp::parse(&epoch.to_string()), None);
    }

    #[test]
    fn an_epoch_in_the_wrong_unit_is_refused_rather_than_read_as_a_far_future_date() {
        // The same instant in milliseconds lands in the year 58,000 and renders as an enormous
        // "days left"; in the other direction a seconds value read as milliseconds lands in
        // 1970 and renders as long ended. Both are well-formed and both are wrong.
        assert_eq!(from_epoch_seconds(1_790_751_600_000), None);
        assert_eq!(from_epoch_seconds(1_790_751), None);
        assert_eq!(from_epoch_seconds(0), None);
        assert_eq!(from_epoch_seconds(-1), None);
    }

    /// The three real timestamps the shared window used to drop. Written out as Valve sends them,
    /// so the test states the contract in the store's own numbers rather than in a round date.
    #[test]
    fn a_release_date_predating_steam_is_read_rather_than_discarded() {
        for (app, seconds, expected) in [
            (70, 911_499_840, "1998-11-19"),
            (20, 922_953_600, "1999-04-01"),
            (50, 941_443_200, "1999-11-01"),
        ] {
            let read = from_historic_epoch_seconds(seconds)
                .unwrap_or_else(|| panic!("app {app}: {seconds} should be a readable date"));
            assert_eq!(read.date(), expected, "app {app}");
            assert_eq!(
                from_epoch_seconds(seconds),
                None,
                "app {app}: the deadline reader is meant to keep refusing this"
            );
        }
    }

    /// The looser floor must not loosen the guard the floor was standing in for.
    #[test]
    fn the_historic_reader_still_refuses_milliseconds_and_absent_dates() {
        assert_eq!(
            from_historic_epoch_seconds(1_790_751_600_000),
            None,
            "milliseconds"
        );
        assert_eq!(
            from_historic_epoch_seconds(0),
            None,
            "Valve's \"no date published\""
        );
        assert_eq!(from_historic_epoch_seconds(-1), None);
        assert_eq!(
            from_historic_epoch_seconds(4_102_444_800),
            None,
            "the year 2100 itself"
        );
    }

    #[test]
    fn a_unix_epoch_integer_is_not_mistaken_for_a_timestamp() {
        // Fanatical publishes bare epoch integers elsewhere in the same payload
        // (`stardeal.available_valid_until`). Nothing here reads that field, and if something
        // ever does it needs its own reader — this one must not silently accept the shape.
        assert_eq!(Timestamp::parse("1789398000"), None);
    }

    #[test]
    fn every_day_of_a_leap_cycle_survives_a_round_trip() {
        // 146,097 days is the full Gregorian cycle: if the two directions agree across all of
        // it they agree everywhere, which is stronger than any list of examples.
        for days in -50_000..96_097 {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(
                days_from_civil(year, month, day),
                days,
                "{year}-{month}-{day}"
            );
        }
    }

    #[test]
    fn both_shapes_the_stores_publish_are_read_as_the_same_instant() {
        // Fanatical marks its timestamps; Humble does not, and its bare form is UTC.
        let marked = Timestamp::parse("2026-09-24T07:00:00.000Z").expect("parses");
        let bare = Timestamp::parse("2026-09-24T07:00:00").expect("parses");
        assert_eq!(marked, bare);
    }

    #[test]
    fn seconds_may_be_left_out() {
        assert_eq!(
            Timestamp::parse("2026-09-17T04:00"),
            Timestamp::parse("2026-09-17T04:00:00")
        );
    }

    #[test]
    fn an_explicit_offset_is_refused_rather_than_quietly_read_as_utc() {
        // Reading +02:00 as UTC would misdate a deadline by two hours with nothing to show for
        // it. A refusal is visible; a silent two-hour error is not.
        assert_eq!(Timestamp::parse("2026-09-17T04:00:00+02:00"), None);
        assert_eq!(Timestamp::parse("2026-09-17T04:00:00-07:00"), None);
    }

    #[test]
    fn nonsense_is_refused_rather_than_guessed_at() {
        for text in [
            "",
            "soon",
            "2026-09-17",
            "2026-13-01T00:00:00",
            "2026-09-32T00:00:00",
            "2026-09-17T24:00:00",
            "2026-09-17T04:60:00",
            "2026-09-17T04:00:00:00",
            "2026-09-17-01T04:00:00",
        ] {
            assert_eq!(Timestamp::parse(text), None, "accepted {text:?}");
        }
    }

    #[test]
    fn a_deadline_counts_down_and_then_goes_negative() {
        let now = Timestamp::from_unix(1_000_000);
        assert_eq!(Timestamp::from_unix(1_003_600).seconds_from(now), 3_600);
        assert_eq!(Timestamp::from_unix(999_000).seconds_from(now), -1_000);
    }

    #[test]
    fn a_displayed_instant_says_which_zone_it_is_in() {
        let at = Timestamp::parse("2026-09-17T04:00:00").expect("parses");
        assert_eq!(at.to_string(), "2026-09-17 04:00 UTC");
    }
}
