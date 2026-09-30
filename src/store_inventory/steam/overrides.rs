//! Facts about a Steam app that the sweep cannot read, found by hand: `steam-overrides.tsv`.
//!
//! The sweep writes `steam-games.tsv` from what Steam's item service publishes, and some cells
//! come back blank where Steam itself knows better: a name the item service leaves empty while the
//! app's community hub carries it, an operating system the store page names under System
//! Requirements while both APIs flag none. A fact found that way is written into the overrides
//! file, one row per cell, and the sweep writes it over its own reading on every run.
//!
//! # A fact corrects one observed cell, so it is applied by compare-and-swap
//!
//! Each fact names the cell the sweep WROTE when the fact was found (`sweep_wrote`), and it
//! applies only while the sweep still writes exactly that — compared byte for byte, an empty field
//! meaning an empty cell. The predicate is about the sweep's reading rather than "the store",
//! because Steam's surfaces disagree with each other: app 3400140 has an empty name in the item
//! service and "Summer Feast" in `appdetails`, at the same moment. Anything else in the cell means
//! the ground moved under the fact:
//!
//! - the cell already holds the fact's value: [`Outcome::AlreadyThere`]. Written by an earlier
//!   application — or, in a row the sweep has just read from Steam, Steam now publishes it and the
//!   fact can be deleted;
//! - the cell holds a third value: [`Outcome::Changed`], and the fact is **not applied**. A game
//!   renamed at launch, a Mac port the page did not list yet. Writing the fact anyway would pin a
//!   value Steam no longer uses, silently and for good; refusing makes it a report a person reads;
//! - there is no row for the app at all: [`Outcome::Gone`]. The store no longer lists it; a name
//!   worth keeping belongs in `steam-delisted-games.tsv`, which the sweep keeps across runs.
//!
//! A row the file otherwise knows nothing about — the `?` of an app the store would not describe —
//! takes facts like any other: every cell is known or unknown on its own, and that the store
//! refused the app is recorded in the delisted ledger, not in its cells.
//!
//! Always-win was the alternative, and it fails invisibly: every one of those three cases becomes a
//! confident cell nobody is told to look at. The six Windows facts are the plainest case — read
//! off a page's System Requirements tab, not the platform field the sweep reads — so the day Steam
//! fills that field as `windows,mac`, an always-win rule would hide the Mac release.
//!
//! # What a fact's surface is for
//!
//! [`Surface`] says where the value was read, and so who can read it again. `appdetails` and the
//! roster are mechanical: a tool can re-check them, and the sweep's own readers have since been
//! taught to, so those facts retire themselves. A store page, a community hub or SteamDB was read by
//! a person, and only a person can re-check it.
//!
//! # Where this lives
//!
//! Beside [`super::codec`], for the same reason: the sweep writes with it and this crate checks the
//! committed files with it. The library reads no facts at runtime — the committed snapshot already
//! carries them — but the test pinning that it does must run in the default build, which never
//! compiles the sweep.

use super::snapshot::COLUMNS;

/// Where a fact was read, which decides who can check it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// Steam's per-app `appdetails` endpoint: a request a tool can repeat.
    AppDetails,
    /// The title the store's own search printed, which `steam-appids.tsv` keeps.
    Roster,
    /// A store page, read by a person.
    StorePage,
    /// An app's community hub, read by a person.
    CommunityHub,
    /// SteamDB's page for the app, read by a person: Valve's own app record, as SteamDB shows it.
    SteamDb,
}

impl Surface {
    pub const ALL: [Self; 5] = [
        Self::AppDetails,
        Self::Roster,
        Self::StorePage,
        Self::CommunityHub,
        Self::SteamDb,
    ];

    /// The word the file spells it with.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AppDetails => "appdetails",
            Self::Roster => "roster",
            Self::StorePage => "store-page",
            Self::CommunityHub => "community-hub",
            Self::SteamDb => "steamdb",
        }
    }

    /// Whether a tool can read the fact again, rather than only a person.
    #[must_use]
    pub const fn mechanical(self) -> bool {
        matches!(self, Self::AppDetails | Self::Roster)
    }

    fn named(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|surface| surface.name() == word)
    }
}

/// One fact: one cell of one app's row, and where it was seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    pub app_id: u32,
    /// An index into [`COLUMNS`] — never the app-id's own.
    pub column: usize,
    /// The cell as the sweep wrote it when the fact was found.
    pub sweep_wrote: String,
    /// The cell to write instead, in the file's own encoding: `os` is the 1/2/4 bit mask,
    /// `released` a day, `~day`, `TBA` or `X` — checked against the column's grammar when read.
    pub value: String,
    pub surface: Surface,
    /// Where exactly: a URL, or the file and field.
    pub source: String,
    /// The day it was seen, `YYYY-MM-DD`.
    pub seen: String,
}

impl Fact {
    /// The column's name, as the header of `steam-games.tsv` spells it.
    #[must_use]
    pub fn column_name(&self) -> &'static str {
        COLUMNS[self.column]
    }
}

/// What one fact did to its row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The cell said `sweep_wrote`; it now says the fact's value.
    Applied,
    /// The cell already said the fact's value, and was left alone.
    AlreadyThere,
    /// The cell says something else — carried here — and was left alone.
    Changed(String),
    /// No row for the app — the store no longer lists it — or a row not as wide as the format.
    Gone,
}

/// The facts in a `steam-overrides.tsv`, in file order.
///
/// # Errors
///
/// A message naming the line, for a row that is not seven fields; an app-id that is not a number;
/// a column that is not in [`COLUMNS`], or is the app-id; a surface this module does not know; a
/// `seen` that is not a whole date; a fact whose value is what the sweep wrote already — a row
/// born redundant; and a second fact for a cell another row already covers.
pub fn parse(text: &str) -> Result<Vec<Fact>, String> {
    let mut facts: Vec<Fact> = Vec::new();
    for (at, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fact = fact(line).map_err(|why| format!("line {}: {why}", at + 1))?;
        if let Some(earlier) = facts
            .iter()
            .find(|held| held.app_id == fact.app_id && held.column == fact.column)
        {
            return Err(format!(
                "line {}: a second fact for app {} {}; the first says {:?}",
                at + 1,
                fact.app_id,
                fact.column_name(),
                earlier.value
            ));
        }
        facts.push(fact);
    }
    Ok(facts)
}

/// One data row, checked.
fn fact(line: &str) -> Result<Fact, String> {
    let field: Vec<&str> = line.split('\t').collect();
    let [app_id, column, sweep_wrote, value, surface, source, seen] = field[..] else {
        return Err(format!(
            "{} fields where seven are wanted: app_id, column, sweep_wrote, value, surface, \
             source, seen",
            field.len()
        ));
    };
    let app_id = app_id
        .parse()
        .map_err(|_| format!("{app_id:?} is not an app-id"))?;
    let column = match COLUMNS.iter().position(|name| *name == column) {
        Some(0) => return Err("the app-id is what a fact is about, not a cell it can set".into()),
        Some(at) => at,
        None => return Err(format!("{column:?} is not a column of steam-games.tsv")),
    };
    let surface = Surface::named(surface).ok_or_else(|| {
        let known: Vec<&str> = Surface::ALL.iter().map(|surface| surface.name()).collect();
        format!("{surface:?} is not a surface; one of {}", known.join(", "))
    })?;
    if crate::steam::printed_date(seen).is_none() {
        return Err(format!("{seen:?} is not a date, YYYY-MM-DD"));
    }
    if sweep_wrote == value {
        return Err(format!(
            "the sweep already writes {value:?}; there is nothing to correct"
        ));
    }
    // The grammar the reader and the doctor use: a value the column cannot hold would be read as
    // unknown, silently, so it is refused here instead.
    super::snapshot::check_cell(column, value)
        .map_err(|why| format!("{}: {why}", COLUMNS[column]))?;
    Ok(Fact {
        app_id,
        column,
        sweep_wrote: sweep_wrote.to_owned(),
        value: value.to_owned(),
        surface,
        source: source.to_owned(),
        seen: seen.to_owned(),
    })
}

/// Applies each fact to the row it names, and says what each did — including the facts whose app
/// has no row at all.
///
/// `rows` are `steam-games.tsv` lines keyed by app-id, in any order; a row is changed only where a
/// fact is [`Outcome::Applied`]. Two facts about one app are two cells of one row, and apply
/// independently.
pub fn apply_all<'f>(rows: &mut [(u32, String)], facts: &'f [Fact]) -> Vec<(&'f Fact, Outcome)> {
    facts
        .iter()
        .map(|fact| {
            let outcome = rows
                .iter_mut()
                .find(|(app_id, _)| *app_id == fact.app_id)
                .map_or(Outcome::Gone, |(_, row)| apply(fact, row));
            (fact, outcome)
        })
        .collect()
}

/// One fact against one row. A row of the wrong width is no row this format describes, so the
/// fact is [`Outcome::Gone`] rather than written into a shifted cell.
fn apply(fact: &Fact, row: &mut String) -> Outcome {
    let mut cells: Vec<&str> = row.split('\t').collect();
    if cells.len() != COLUMNS.len() {
        return Outcome::Gone;
    }
    let cell = cells[fact.column];
    if cell == fact.value {
        return Outcome::AlreadyThere;
    }
    if cell != fact.sweep_wrote {
        return Outcome::Changed(cell.to_owned());
    }
    cells[fact.column] = fact.value.as_str();
    let patched = cells.join("\t");
    *row = patched;
    Outcome::Applied
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store_inventory::steam::snapshot;

    const HEADER: &str = "# app_id\tcolumn\tsweep_wrote\tvalue\tsurface\tsource\tseen\n";

    /// A described row, as wide as the format: every cell `0` except the ones given.
    fn row(app_id: u32, cells: &[(&str, &str)]) -> (u32, String) {
        let mut row: Vec<String> = COLUMNS.iter().map(|_| "0".to_owned()).collect();
        row[0] = app_id.to_string();
        let name = COLUMNS.len() - 1;
        row[name] = String::new();
        for (column, value) in cells {
            let at = COLUMNS.iter().position(|c| c == column).expect("a column");
            row[at] = (*value).to_owned();
        }
        (app_id, row.join("\t"))
    }

    /// A row the file knows nothing about but the id — an app the store would not describe.
    fn unknown(app_id: u32) -> (u32, String) {
        (
            app_id,
            format!("{app_id}{}", "\t?".repeat(COLUMNS.len() - 1)),
        )
    }

    fn cell(row: &(u32, String), column: &str) -> String {
        let at = COLUMNS.iter().position(|c| *c == column).expect("a column");
        row.1.split('\t').nth(at).expect("the cell").to_owned()
    }

    fn facts(rows: &str) -> Vec<Fact> {
        parse(&format!("{HEADER}{rows}")).expect("the facts parse")
    }

    fn name_fact() -> Vec<Fact> {
        facts(
            "3244680\tname\t\tCards of Gluttony\tsteamdb\thttps://steamdb.info/app/3244680/\t2026-09-30\n",
        )
    }

    #[test]
    fn a_fact_applies_while_the_sweep_still_writes_what_it_corrected() {
        let mut rows = vec![row(3_244_680, &[]), row(620, &[("name", "Portal 2")])];
        let facts = name_fact();
        let outcomes = apply_all(&mut rows, &facts);
        assert_eq!(outcomes, vec![(&facts[0], Outcome::Applied)]);
        assert_eq!(cell(&rows[0], "name"), "Cards of Gluttony");
        assert_eq!(
            cell(&rows[1], "name"),
            "Portal 2",
            "another app's row is not touched"
        );
        assert_eq!(
            rows[0].1.split('\t').count(),
            COLUMNS.len(),
            "the row keeps its width"
        );
    }

    /// Applying twice is applying once: the second pass finds the value and leaves it.
    #[test]
    fn a_cell_already_holding_the_value_is_left_alone_and_says_so() {
        let mut rows = vec![row(3_244_680, &[])];
        let facts = name_fact();
        apply_all(&mut rows, &facts);
        let once = rows.clone();
        assert_eq!(
            apply_all(&mut rows, &facts),
            vec![(&facts[0], Outcome::AlreadyThere)]
        );
        assert_eq!(rows, once);
    }

    /// The store moved: the fact is not written over a value it never saw.
    #[test]
    fn a_cell_holding_a_third_value_is_not_overwritten() {
        let mut rows = vec![row(
            3_244_680,
            &[("name", "Cards of Gluttony: Second Helping")],
        )];
        let before = rows.clone();
        let facts = name_fact();
        assert_eq!(
            apply_all(&mut rows, &facts),
            vec![(
                &facts[0],
                Outcome::Changed("Cards of Gluttony: Second Helping".into())
            )]
        );
        assert_eq!(rows, before);
    }

    /// No row, and a row of the wrong width, are both no row a fact can go into.
    #[test]
    fn an_app_with_no_row_is_gone() {
        let facts = name_fact();
        for mut rows in [
            vec![row(620, &[])],
            vec![(3_244_680, "3244680\t0\t0".to_owned())],
        ] {
            let before = rows.clone();
            assert_eq!(
                apply_all(&mut rows, &facts),
                vec![(&facts[0], Outcome::Gone)]
            );
            assert_eq!(rows, before, "nothing is written into it");
        }
    }

    /// A row known only by its id takes facts cell by cell, and every other cell stays unknown.
    #[test]
    fn a_fact_fills_a_cell_of_a_row_known_only_by_its_id() {
        let mut rows = vec![unknown(5_009_660)];
        let facts = facts(
            "5009660\tname\t?\tHaunted Love\tsteamdb\thttps://steamdb.info/app/5009660/\t2026-09-30\n",
        );
        assert_eq!(
            apply_all(&mut rows, &facts),
            vec![(&facts[0], Outcome::Applied)]
        );
        assert_eq!(cell(&rows[0], "name"), "Haunted Love");
        assert_eq!(
            cell(&rows[0], "type"),
            "?",
            "the other cells are left unknown"
        );
    }

    #[test]
    fn two_facts_about_one_app_set_two_cells_of_one_row() {
        let mut rows = vec![row(2_649_520, &[])];
        let facts = facts(
            "2649520\tos\t0\t1\tstore-page\thttps://store.steampowered.com/app/2649520/\t2026-09-29\n\
             2649520\tname\t\tA Name\tcommunity-hub\thttps://steamcommunity.com/app/2649520/\t2026-09-29\n",
        );
        let outcomes = apply_all(&mut rows, &facts);
        assert!(
            outcomes
                .iter()
                .all(|(_, outcome)| *outcome == Outcome::Applied),
            "{outcomes:?}"
        );
        assert_eq!(
            (cell(&rows[0], "os"), cell(&rows[0], "name")),
            ("1".into(), "A Name".into())
        );
    }

    /// An empty `sweep_wrote` is an empty cell, byte for byte: a cell of one space is a third value.
    #[test]
    fn the_comparison_is_byte_exact() {
        let mut rows = vec![row(3_244_680, &[("name", " ")])];
        let facts = name_fact();
        assert_eq!(
            apply_all(&mut rows, &facts),
            vec![(&facts[0], Outcome::Changed(" ".into()))]
        );
    }

    #[test]
    fn a_file_of_comments_and_blank_lines_holds_no_facts() {
        assert_eq!(parse(&format!("{HEADER}# a note\n\n")), Ok(Vec::new()));
    }

    /// Every way a hand-typed row can be wrong is refused with the line it is on, before it can
    /// reach a generated file.
    #[test]
    fn a_row_that_cannot_be_a_fact_is_refused_with_its_line() {
        let refusals = [
            (
                "3244680\tname\t\tCards of Gluttony\tsteamdb\t2026-09-30",
                "fields",
            ),
            ("x\tname\t\tA\tsteamdb\tsource\t2026-09-30", "not an app-id"),
            ("1\tnom\t\tA\tsteamdb\tsource\t2026-09-30", "not a column"),
            (
                "1\tapp_id\t1\t2\tsteamdb\tsource\t2026-09-30",
                "app-id is what",
            ),
            ("1\tname\t\tA\ta-forum\tsource\t2026-09-30", "not a surface"),
            ("1\tname\t\tA\tsteamdb\tsource\tSep 30", "not a date"),
            (
                "1\tos\t1\t1\tstore-page\tsource\t2026-09-30",
                "nothing to correct",
            ),
            (
                "1\treleased\t?\tsoon\tappdetails\tsource\t2026-09-30",
                "released",
            ),
            ("1\tos\t0\t8\tstore-page\tsource\t2026-09-30", "os mask"),
        ];
        for (line, why) in refusals {
            let err = parse(&format!("{HEADER}{line}\n")).expect_err(line);
            assert!(
                err.starts_with("line 2: ") && err.contains(why),
                "{line:?}: {err}"
            );
        }
        let twice = "1\tname\t\tA\tsteamdb\tsource\t2026-09-30\n\
                     1\tname\t\tB\tcommunity-hub\tsource\t2026-09-30\n";
        let err = parse(&format!("{HEADER}{twice}")).expect_err("a cell twice");
        assert!(
            err.starts_with("line 3: ") && err.contains("second fact"),
            "{err}"
        );
    }

    #[test]
    fn every_surface_reads_back_from_its_own_name_and_only_two_are_mechanical() {
        for surface in Surface::ALL {
            assert_eq!(Surface::named(surface.name()), Some(surface));
        }
        let mechanical: Vec<&str> = Surface::ALL
            .iter()
            .filter(|surface| surface.mechanical())
            .map(|surface| surface.name())
            .collect();
        assert_eq!(mechanical, ["appdetails", "roster"]);
    }

    /// **The committed snapshot carries every committed fact, and one it does not FAILS THE BUILD
    /// — deliberately.** A fact the last sweep could not apply (the store moved: stale), or whose
    /// app no longer has a row (orphaned), is a question for a person, and a green build over it
    /// is how such questions go unanswered for years. The remedy the message names is the one
    /// the sweep's own report names.
    ///
    /// The default build never compiles the sweep, so this is where an unapplied edit is caught
    /// too: a fact added to the file and never applied.
    #[test]
    fn a_committed_fact_the_committed_snapshot_does_not_carry_fails_the_build() {
        let facts = parse(include_str!("../../../data/steam/steam-overrides.tsv"))
            .expect("steam-overrides.tsv parses");
        let wanted: std::collections::BTreeSet<u32> = facts.iter().map(|f| f.app_id).collect();
        let mut rows: Vec<(u32, String)> = snapshot::rows()
            .filter_map(|line| {
                let app_id = line.split('\t').next()?.parse().ok()?;
                wanted.contains(&app_id).then(|| (app_id, line.to_owned()))
            })
            .collect();
        let unsettled: Vec<String> = apply_all(&mut rows, &facts)
            .into_iter()
            .filter(|(_, outcome)| *outcome != Outcome::AlreadyThere)
            .map(|(fact, outcome)| {
                let remedy = match outcome {
                    Outcome::Applied => {
                        "not applied yet: run `cargo run --release --features \
                                         tools --bin steam_catalogue -- --apply-overrides \
                                         data/steam`"
                    }
                    Outcome::Changed(_) => {
                        "stale: the sweep writes something else now; re-check \
                                            the fact, then update sweep_wrote or delete it"
                    }
                    _ => {
                        "orphaned: no row for this app any more; delete the fact, \
                          or keep a name in steam-delisted-games.tsv"
                    }
                };
                format!(
                    "{} {} ({outcome:?}): {remedy}",
                    fact.app_id,
                    fact.column_name()
                )
            })
            .collect();
        assert!(unsettled.is_empty(), "{unsettled:#?}");

        // Through the reader the program uses, not the text: every row a fact touched still
        // parses, and reads back a name fact as the name.
        for fact in &facts {
            let found = snapshot::by_id(fact.app_id)
                .unwrap_or_else(|| panic!("app {} no longer parses", fact.app_id));
            if fact.column_name() == "name" {
                assert_eq!(found.name, fact.value, "app {}", fact.app_id);
            }
        }
    }
}
