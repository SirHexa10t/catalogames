//! Checks a `steam-games.tsv` for what its reader quietly works around.
//!
//! The reader is lenient on purpose: a cell it cannot read costs that cell, not the row, so a file
//! with a fault in it still serves every lookup. The price is that a fault is invisible from the
//! program's side, and this is where it becomes visible. Every cell is checked against the same
//! grammar the reader uses — [`super::snapshot`]'s, one implementation for both — and so are the
//! properties no single row shows: the header naming the shared column list, and the ascending
//! order the binary search depends on.
//!
//! A test runs it over the committed file and fails on anything [`Severity::Invalid`];
//! `steam_catalogue` runs it over every file it writes, and `steam_catalogue --doctor <dir>` over
//! any. What the file does not know — its `?` cells — is not a fault, and is counted separately by
//! [`census`].

use std::collections::BTreeSet;
use std::fmt;

use super::snapshot::{COLUMNS, PLANNED, Release, UNKNOWN, check_cell};

/// How far past the sweep a planned date may lie before it reads as a placeholder rather than a
/// plan. Measured on the sweep of 2026-09-26: of 15,966 planned dates, 15,923 fall within four
/// years, and the tail past ten years is 2069, 2077, 2080 and eight at 2099 — parked, not planned.
const PLACEHOLDER_YEARS: u32 = 10;

/// The issue kinds that say a cell was written `?` because the format could not hold its value.
const OVERFLOWS: [&str; 3] = [
    "compat-out-of-range",
    "language-out-of-range",
    "language-regional-only",
];

/// How much a finding matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The file breaks its own format: a reader would misread a cell, skip a row, or be unable to
    /// search the file.
    Invalid,
    /// The file is well-formed but says something worth a person's look: a value the format could
    /// not hold, a planned date that reads like a placeholder, a game out on a day after the sweep.
    Notice,
}

/// One thing the doctor found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    /// The line in the file, counting from 1.
    pub line: usize,
    /// The app the line is about, where it names one.
    pub app_id: Option<u32>,
    /// The column, by its header name, where the finding is about one cell.
    pub column: Option<&'static str>,
    /// What is wrong, in words.
    pub problem: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}", self.line)?;
        if let Some(app_id) = self.app_id {
            write!(f, ", app {app_id}")?;
        }
        if let Some(column) = self.column {
            write!(f, ", {column}")?;
        }
        write!(f, ": {}", self.problem)
    }
}

/// Everything wrong with a snapshot's text, in file order.
///
/// `issues` is the `steam-issues.tsv` the same sweep wrote, or `""`. It is what tells a `?` the
/// format could not hold — which the sweep records there — from a `?` that is plain unknown.
#[must_use]
pub fn check(snapshot: &str, issues: &str) -> Vec<Finding> {
    let overflowed = overflowed(issues);
    let mut found = Vec::new();
    // Raw lines, split on `\n` alone: `str::lines` would strip a carriage return before the
    // check could see it, while the reader's binary search does not — a CRLF file reads one way
    // through a scan and another through a lookup by id. The newline ending the last row is not
    // a line of its own; a second one is.
    let mut lines = snapshot
        .strip_suffix('\n')
        .unwrap_or(snapshot)
        .split('\n')
        .enumerate()
        .map(|(at, line)| (at + 1, line));

    let header = format!("# {}", COLUMNS.join("\t"));
    if lines.next().map(|(_, line)| line) != Some(header.as_str()) {
        found.push(Finding {
            severity: Severity::Invalid,
            line: 1,
            app_id: None,
            column: None,
            problem: "the first line does not name the shared column list".into(),
        });
    }
    let as_of = swept_on(snapshot);

    let mut previous: Option<u32> = None;
    let mut in_header = true;
    for (line, text) in lines {
        // **The header is one run of `#` lines at the top, and nothing else may interrupt the
        // rows.** The binary search behind every lookup by id reads whatever line it lands on, and
        // a blank or a comment there reads as "not found" — for whichever apps' searches happen
        // to cross it.
        if in_header && text.starts_with('#') {
            continue;
        }
        in_header = false;
        if text.is_empty() || text.starts_with('#') {
            found.push(Finding {
                severity: Severity::Invalid,
                line,
                app_id: None,
                column: None,
                problem: "a blank or comment line among the rows: a lookup by id that lands on \
                          it finds nothing"
                    .into(),
            });
            continue;
        }
        let cells: Vec<&str> = text.split('\t').collect();
        let app_id = cells.first().and_then(|cell| cell.parse::<u32>().ok());
        let mut say = |severity, column: Option<&'static str>, problem: String| {
            found.push(Finding {
                severity,
                line,
                app_id,
                column,
                problem,
            });
        };
        if cells.len() != COLUMNS.len() {
            say(
                Severity::Invalid,
                None,
                format!(
                    "{} cells where the format has {}",
                    cells.len(),
                    COLUMNS.len()
                ),
            );
            continue;
        }
        for (at, cell) in cells.iter().enumerate() {
            if cell.contains('\r') {
                // Saved with Windows line endings: the last cell of every row, the title, would
                // carry the carriage return into every lookup and every line printed.
                say(
                    Severity::Invalid,
                    Some(COLUMNS[at]),
                    "a carriage return: the file was saved with CRLF line endings".into(),
                );
            } else if let Err(why) = check_cell(at, cell) {
                say(Severity::Invalid, Some(COLUMNS[at]), why);
            }
        }
        let Some(app_id) = app_id else {
            continue;
        };
        if let Some(before) = previous
            && app_id <= before
        {
            say(
                Severity::Invalid,
                None,
                format!("not after {before}: the rows must ascend, or the binary search misses"),
            );
        }
        previous = Some(app_id);

        if overflowed.contains(&app_id) {
            for (at, cell) in cells.iter().enumerate() {
                let name = COLUMNS[at];
                if *cell == UNKNOWN && (name == "compat" || name.starts_with("lang_")) {
                    say(
                        Severity::Notice,
                        Some(name),
                        "a value the format could not hold, written as unknown — \
                         see steam-issues.tsv; the format needs widening"
                            .into(),
                    );
                }
            }
        }
        if let Some(as_of) = as_of {
            let released = cells[COLUMNS.iter().position(|c| *c == "released").unwrap_or(0)];
            if let Some(problem) = odd_date(released, as_of) {
                say(Severity::Notice, Some("released"), problem);
            }
        }
    }
    found
}

/// What a snapshot does not know: its `?` cells, counted.
///
/// Not a fault — `?` is the file saying "unknown to us", which it is meant to say — so it is kept
/// apart from [`check`]'s findings, as the list of what is still worth finding out.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Census {
    /// Apps whose every cell but the id is unknown.
    pub known_only_by_id: Vec<u32>,
    /// Apps that know some cells and not others, each with the columns it does not know.
    pub partly_known: Vec<(u32, Vec<&'static str>)>,
}

impl Census {
    /// Unknown cells per column over the partly known rows, in column order — the id-only rows
    /// would add one to every column and say nothing about any.
    #[must_use]
    pub fn by_column(&self) -> Vec<(&'static str, usize)> {
        COLUMNS
            .iter()
            .map(|name| {
                let count = self
                    .partly_known
                    .iter()
                    .filter(|(_, columns)| columns.contains(name))
                    .count();
                (*name, count)
            })
            .filter(|(_, count)| *count > 0)
            .collect()
    }
}

impl fmt::Display for Census {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} app(s) known only by their id, {} partly known",
            self.known_only_by_id.len(),
            self.partly_known.len()
        )?;
        let columns: Vec<String> = self
            .by_column()
            .into_iter()
            .map(|(name, count)| format!("{name} {count}"))
            .collect();
        if !columns.is_empty() {
            write!(f, " — unknown {}", columns.join(", "))?;
        }
        Ok(())
    }
}

/// Every `?` in a snapshot's rows, by app. Lines that are not whole rows are [`check`]'s concern,
/// and are skipped here.
#[must_use]
pub fn census(snapshot: &str) -> Census {
    let mut census = Census::default();
    for text in snapshot
        .lines()
        .filter(|text| !text.starts_with('#') && !text.is_empty())
    {
        let cells: Vec<&str> = text.split('\t').collect();
        let Some(app_id) = cells.first().and_then(|cell| cell.parse::<u32>().ok()) else {
            continue;
        };
        if cells.len() != COLUMNS.len() {
            continue;
        }
        let unknown: Vec<&'static str> = cells
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, cell)| **cell == UNKNOWN)
            .map(|(at, _)| COLUMNS[at])
            .collect();
        if unknown.len() == COLUMNS.len() - 1 {
            census.known_only_by_id.push(app_id);
        } else if !unknown.is_empty() {
            census.partly_known.push((app_id, unknown));
        }
    }
    census
}

/// The apps `steam-issues.tsv` says were written `?` for a value the format could not hold.
fn overflowed(issues: &str) -> BTreeSet<u32> {
    issues
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut field = line.split('\t');
            let app_id = field.next()?.parse().ok()?;
            OVERFLOWS.contains(&field.next()?).then_some(app_id)
        })
        .collect()
}

/// The day the file says it was swept, from the header line that states it.
fn swept_on(snapshot: &str) -> Option<&str> {
    snapshot
        .lines()
        .take_while(|line| line.starts_with('#'))
        .find_map(|line| line.split("reported it on ").nth(1))
        .and_then(|rest| rest.get(..10))
        .filter(|day| crate::steam::printed_date(day).is_some())
}

/// What is odd about a `released` cell on a file swept on `as_of`, if anything.
fn odd_date(cell: &str, as_of: &str) -> Option<String> {
    let year = |day: &str| day.get(..4).and_then(|year| year.parse::<u32>().ok());
    match Release::from_cell(cell).ok().flatten()? {
        Release::Out(day) if day > as_of => Some(format!(
            "out on {day}, after the sweep of {as_of}: the store calls it released"
        )),
        Release::Planned(day) if year(day)? > year(as_of)? + PLACEHOLDER_YEARS => Some(format!(
            "planned for {day}, more than {PLACEHOLDER_YEARS} years after the sweep — \
             a placeholder more likely than a plan ({PLANNED}{day})"
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::snapshot::SNAPSHOT;
    use super::*;

    const ISSUES: &str = include_str!("../../../data/steam/steam-issues.tsv");

    /// A small file in the committed shape: the column line, the date line, then rows.
    fn file(rows: &[&str]) -> String {
        format!(
            "# {}\n# Steam's catalogue as IStoreBrowseService/GetItems/v1 reported it on \
             2026-09-26, AS SEEN FROM\n{}\n",
            COLUMNS.join("\t"),
            rows.join("\n")
        )
    }

    /// A described row: every cell given, or the value a real row would carry.
    fn row(app_id: u32, released: &str, compat: &str) -> String {
        let mut cells: Vec<String> = COLUMNS.iter().map(|_| "0".to_owned()).collect();
        cells[0] = app_id.to_string();
        for (name, value) in [
            ("released", released),
            ("compat", compat),
            ("tagids(base36)", ""),
            ("incl(base36)", ""),
            ("feats(base36)", ""),
            ("devs", ""),
            ("pubs", ""),
            ("name", "A Game"),
        ] {
            let at = COLUMNS.iter().position(|c| *c == name).expect("a column");
            cells[at] = value.to_owned();
        }
        cells.join("\t")
    }

    /// **The committed file is valid, and this is what keeps it so.** The reader works around a
    /// bad cell without a word, so without this a sweep writing one would be found only by
    /// someone noticing a game's details quietly missing.
    #[test]
    fn the_committed_snapshot_has_nothing_invalid_in_it() {
        let invalid: Vec<String> = check(SNAPSHOT, ISSUES)
            .into_iter()
            .filter(|finding| finding.severity == Severity::Invalid)
            .map(|finding| finding.to_string())
            .collect();
        assert!(
            invalid.is_empty(),
            "{} invalid, the first: {:#?}",
            invalid.len(),
            &invalid[..invalid.len().min(20)]
        );
    }

    #[test]
    fn a_well_formed_file_has_no_findings() {
        let good = file(&[&row(10, "2003-03-01", "0"), &row(20, "TBA", "?")]);
        assert_eq!(check(&good, ""), Vec::new());
    }

    #[test]
    fn a_header_not_naming_the_columns_is_invalid() {
        let bad = format!("# app_id\tname\n{}\n", row(10, "2003-03-01", "0"));
        let found = check(&bad, "");
        assert_eq!(found[0].line, 1);
        assert_eq!(found[0].severity, Severity::Invalid);
    }

    /// Each of the things a reader cannot see: a short row, a bad cell (by column), and a row out
    /// of order — each with its line.
    #[test]
    fn a_row_the_reader_would_misread_is_invalid_with_its_line() {
        let bad = file(&[
            &row(10, "2003-03-01", "0"),
            "20\t0\t0",
            &row(30, "2011-4-19", "0"),
            &row(25, "2003-03-01", "0"),
        ]);
        let found = check(&bad, "");
        let seen: Vec<(usize, Option<&str>)> = found.iter().map(|f| (f.line, f.column)).collect();
        assert_eq!(
            seen,
            [(4, None), (5, Some("released")), (6, None)],
            "{found:#?}"
        );
        assert!(found.iter().all(|f| f.severity == Severity::Invalid));
        assert!(
            found[2].problem.contains("not after 30"),
            "{}",
            found[2].problem
        );
    }

    /// A `?` is plain unknown unless the sweep recorded that it could not hold the value — only
    /// then is it worth a person's look.
    #[test]
    fn an_unknown_cell_is_a_notice_only_where_the_sweep_recorded_an_overflow() {
        let text = file(&[&row(10, "2003-03-01", "?")]);
        assert_eq!(check(&text, ""), Vec::new(), "plain unknown");
        let issues = "10\tcompat-out-of-range\tcompat category 4 is outside 0..=3\n";
        let found = check(&text, issues);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(
            (found[0].severity, found[0].column),
            (Severity::Notice, Some("compat"))
        );
    }

    /// A blank line, a comment and a carriage return inside the rows are each invalid: the first
    /// two break the binary search, the third rides into every title.
    #[test]
    fn a_line_that_is_no_row_among_the_rows_is_invalid() {
        let good = row(10, "2003-03-01", "0");
        let crlf = format!("{}\r", row(40, "2003-03-01", "0"));
        let text = file(&[&good, "", &row(20, "2003-03-01", "0"), "# a note", &crlf]);
        let found = check(&text, "");
        let seen: Vec<(usize, Severity)> = found.iter().map(|f| (f.line, f.severity)).collect();
        assert_eq!(
            seen,
            [
                (4, Severity::Invalid),
                (6, Severity::Invalid),
                (7, Severity::Invalid)
            ],
            "{found:#?}"
        );
        assert_eq!(
            found[2].column,
            Some("name"),
            "the carriage return rides on the title"
        );
    }

    #[test]
    fn the_census_counts_what_the_file_does_not_know() {
        let id_only = format!("30{}", "\t?".repeat(COLUMNS.len() - 1));
        let mut partly: Vec<String> = row(20, "TBA", "?").split('\t').map(str::to_owned).collect();
        let devs = COLUMNS.iter().position(|c| *c == "devs").expect("a column");
        partly[devs] = UNKNOWN.to_owned();
        let text = file(&[&row(10, "2003-03-01", "0"), &partly.join("\t"), &id_only]);
        let census = census(&text);
        assert_eq!(census.known_only_by_id, [30]);
        assert_eq!(census.partly_known, [(20, vec!["compat", "devs"])]);
        assert_eq!(census.by_column(), [("compat", 1), ("devs", 1)]);
        assert_eq!(
            census.to_string(),
            "1 app(s) known only by their id, 1 partly known — unknown compat 1, devs 1"
        );
    }

    #[test]
    fn a_placeholder_plan_and_a_release_after_the_sweep_are_notices() {
        let text = file(&[
            &row(10, "~2099-12-31", "0"),
            &row(20, "~2027-06-01", "0"),
            &row(30, "2027-06-09", "0"),
        ]);
        let found = check(&text, "");
        let seen: Vec<(u32, Severity)> = found
            .iter()
            .map(|f| (f.app_id.expect("an app"), f.severity))
            .collect();
        assert_eq!(
            seen,
            [(10, Severity::Notice), (30, Severity::Notice)],
            "{found:#?}"
        );
    }
}
