//! Reading `steam-delisted-games.tsv`: the apps the store's search listed and that no country
//! the sweep asked would describe.
//!
//! **This is where "the store refuses this app" is recorded, and nowhere else.** A row of
//! [`super::snapshot::UNKNOWN`] in `steam-games.tsv` means only that the file knows nothing about
//! the app — and a refused app can still carry cells found elsewhere — so a reader that needs the
//! refusal asks here.
//!
//! Compiled in, like the snapshot, and small: seventeen rows on 2026-09-26.

/// Steam's own ledger of what it would not describe, as the sweep last wrote it.
pub const LEDGER: &str = include_str!("../../../data/steam/steam-delisted-games.tsv");

/// What a sweep saw when the store would not describe an app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// No country the sweep asked named it: gone, or never existed — Steam answers both alike.
    Removed,
    /// Some country named it and flagged it as not for sale there. The app is alive; the sweep's
    /// countries just cannot buy it, and a person elsewhere may well be able to.
    RegionRestricted,
}

/// One app the store would not describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delisted {
    pub app_id: u32,
    /// The last name anything gave it, or `None` where nothing has.
    pub name: Option<&'static str>,
    pub state: State,
    /// The day a sweep first failed to describe it, `YYYY-MM-DD`.
    pub first_missing: &'static str,
}

/// The ledger's entry for `app_id`, if the store refused to describe it.
#[must_use]
pub fn get(app_id: u32) -> Option<Delisted> {
    all().find(|entry| entry.app_id == app_id)
}

/// Every app in the ledger, in its order — ascending by app-id. A line that is not a whole entry
/// is skipped; the ledger's own test is what insists there are none.
pub fn all() -> impl Iterator<Item = Delisted> {
    LEDGER.lines().filter_map(parse)
}

fn parse(line: &'static str) -> Option<Delisted> {
    if line.starts_with('#') {
        return None;
    }
    let mut field = line.split('\t');
    let (Some(app_id), Some(name), Some(state), Some(first_missing), None) = (
        field.next(),
        field.next(),
        field.next(),
        field.next(),
        field.next(),
    ) else {
        return None;
    };
    Some(Delisted {
        app_id: app_id.parse().ok()?,
        name: (name != super::snapshot::UNKNOWN && !name.is_empty()).then_some(name),
        state: match state {
            "removed" => State::Removed,
            "region_restricted" => State::RegionRestricted,
            _ => return None,
        },
        first_missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every data line is a whole entry — a state spelled differently by the sweep one day would
    /// otherwise drop the app from this reader silently.
    #[test]
    fn every_line_of_the_ledger_is_an_entry() {
        let lines = LEDGER.lines().filter(|line| !line.starts_with('#')).count();
        assert_eq!(all().count(), lines);
        assert!(lines > 0, "the ledger is empty");
        let ids: Vec<u32> = all().map(|entry| entry.app_id).collect();
        assert!(
            ids.windows(2).all(|pair| pair[0] < pair[1]),
            "ascending: {ids:?}"
        );
    }

    #[test]
    fn a_removed_app_and_a_region_restricted_one_read_as_what_the_sweep_saw() {
        let region = get(4_600_150).expect("4600150 is in the ledger");
        assert_eq!(region.state, State::RegionRestricted);
        assert_eq!(region.name, Some("yulgang next"));
        let removed = get(5_009_660).expect("5009660 is in the ledger");
        assert_eq!(removed.state, State::Removed);
        assert_eq!(removed.name, Some("Haunted Love"));
        assert_eq!(removed.first_missing, "2026-09-26");
        assert_eq!(get(620), None, "a live app is not in it");
    }

    #[test]
    fn a_name_nothing_has_given_is_none_rather_than_the_marker() {
        let line = "1\t?\tremoved\t2026-09-26";
        assert_eq!(parse(line).map(|entry| entry.name), Some(None));
        assert_eq!(
            parse("1\tA\tvanished\t2026-09-26"),
            None,
            "an unknown state"
        );
        assert_eq!(parse("1\tA\tremoved"), None, "a short line");
    }
}
