//! The committed snapshot, decoded by the committed codec.
//!
//! Everything else checks the writer: that a value survives a round trip, that a row is as wide
//! as its header, that a sample agrees with Valve. None of it opens `data/steam/steam-games.tsv`
//! and reads it, which is the one thing the file exists to allow — and a format nobody has read
//! end to end is a format whose first reader finds the bad row.
//!
//! So this decodes every encoded field of every row through the same functions a runtime query
//! would use, and fails naming the app and the column rather than the row count.

use catalogames::store_inventory::steam::codec::{Compat, Languages, base36, from_base36};
use catalogames::store_inventory::steam::snapshot::{COLUMNS, UNKNOWN};

/// A column's position, by the name the shared list gives it — never a hand-typed index, which a
/// column added or removed would silently shift.
fn column(name: &str) -> usize {
    COLUMNS
        .iter()
        .position(|at| *at == name)
        .unwrap_or_else(|| panic!("{name:?} is not a column"))
}

#[test]
fn every_row_of_the_snapshot_decodes() {
    // The compiled-in constant, not a file read. The library embeds the snapshot, so it cannot
    // build without it, and the "file absent" branch this test used to carry became unreachable —
    // a skip that can never fire is a check nobody knows is not running.
    //
    // The fields are still decoded by hand rather than through `snapshot::Entry`, deliberately:
    // this test exists to prove the CODEC reads back what the writer wrote, and routing it through
    // the reader would only prove the reader agrees with itself.
    let text = catalogames::store_inventory::steam::snapshot::SNAPSHOT;
    let (app, compat, lang) = (column("app_id"), column("compat"), column("lang_ui"));
    let (tagids, incl, feats) = (
        column("tagids(base36)"),
        column("incl(base36)"),
        column("feats(base36)"),
    );

    let mut rows = 0usize;
    let mut unresolved = 0usize;
    let mut previous = 0u32;
    let mut listed_languages = 0usize;
    let mut with_features = 0usize;
    let (mut listed, mut dubbed, mut subbed) = (0u64, 0u64, 0u64);

    for (line_no, line) in text.lines().enumerate() {
        if line.starts_with('#') {
            continue;
        }
        let at = line_no + 1;
        let field: Vec<&str> = line.split('\t').collect();
        assert_eq!(
            field.len(),
            COLUMNS.len(),
            "line {at} has {} columns",
            field.len()
        );

        let app_id: u32 = field[app]
            .parse()
            .unwrap_or_else(|_| panic!("line {at}: {:?} is not an app-id", field[app]));
        assert!(
            app_id > previous,
            "line {at}: app {app_id} does not follow {previous} — the file must stay sorted and unique"
        );
        previous = app_id;
        rows += 1;

        // A `?` cell is unknown and carries nothing to decode — and a row may be partly unknown,
        // as an app known only by its id is until a fact fills some of its cells. So `?` is
        // skipped cell by cell, never by row.
        if field[1..].iter().all(|cell| *cell == UNKNOWN) {
            unresolved += 1;
            continue;
        }

        if field[compat] != UNKNOWN {
            let digit = field[compat].chars().next().unwrap_or(' ');
            assert_eq!(
                field[compat].chars().count(),
                1,
                "app {app_id}: compat is one digit"
            );
            Compat::from_digit(digit)
                .unwrap_or_else(|why| panic!("app {app_id}: compat {digit:?} — {why}"));
        }

        let languages = [field[lang], field[lang + 1], field[lang + 2]];
        if languages.contains(&UNKNOWN) {
            continue;
        }
        let masks = Languages::from_columns(languages)
            .unwrap_or_else(|why| panic!("app {app_id}: language masks — {why}"));
        // **There is no per-row nesting to assert, and assuming one is wrong.** A first version
        // of this test demanded `full_audio` be a subset of `supported` and failed on app 21660,
        // Street Fighter IV, which Valve publishes with elanguage 10 marked
        // `supported: false, full_audio: true`. The three flags are independent, exactly as the
        // codec documents; the corpus check after the loop is what catches a crossed column.
        listed += u64::from(masks.supported.count_ones());
        dubbed += u64::from(masks.full_audio.count_ones());
        subbed += u64::from(masks.subtitles.count_ones());
        if masks.supported != 0 {
            listed_languages += 1;
        }

        for (name, column) in [("tagids", tagids), ("incl", incl), ("feats", feats)] {
            if field[column].is_empty() || field[column] == UNKNOWN {
                continue;
            }
            if column == feats {
                with_features += 1;
            }
            let mut ascending = None;
            for token in field[column].split(',') {
                let value = from_base36(token)
                    .unwrap_or_else(|why| panic!("app {app_id}: {name} token {token:?} — {why}"));
                assert_eq!(
                    base36(value),
                    token,
                    "app {app_id}: {name} {token:?} is not canonical"
                );
                // `feats` is documented as ascending and deduplicated; the other two are not.
                if column == feats {
                    if let Some(last) = ascending {
                        assert!(
                            value > last,
                            "app {app_id}: feats {token:?} is not ascending"
                        );
                    }
                    ascending = Some(value);
                }
            }
        }
    }

    assert!(rows > 0, "the snapshot has no rows");
    // Guards against a file that parses because it is nearly empty: the counts below come from
    // the 2026-09-21 sweep and are floors, not fixtures, so a later, larger sweep still passes.
    assert!(
        listed_languages > rows / 2,
        "only {listed_languages} of {rows} rows list a language"
    );
    assert!(
        with_features > rows / 4,
        "only {with_features} of {rows} rows carry features"
    );

    // The one cross-column invariant that is actually true, and it is a corpus property rather
    // than a row one: across a whole catalogue far more languages are listed than subtitled, and
    // more subtitled than dubbed. Measured over 3,683 sampled rows the ratio was 3,676 / 2,360 /
    // 1,348. Swapping any pair of these columns inverts one of these comparisons — which is the
    // failure that reached the written file once and that no round trip could see.
    assert!(
        listed > subbed,
        "listed {listed} should exceed subtitled {subbed}: lang_ui and lang_subs look crossed"
    );
    assert!(
        subbed > dubbed,
        "subtitled {subbed} should exceed dubbed {dubbed}: lang_subs and lang_audio look crossed"
    );
    eprintln!(
        "decoded {rows} rows ({unresolved} unresolved); language bits listed {listed}, subtitled {subbed}, dubbed {dubbed}; {with_features} rows with features"
    );
}
