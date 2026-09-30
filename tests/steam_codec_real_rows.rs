//! The codec against responses Valve actually sent, rather than against rows this project made up.
//!
//! The unit tests in `store_inventory::steam::codec` are exhaustive over the values the types
//! admit, which proves the bit arithmetic. What they cannot prove is that Valve's data fits those
//! types: the shapes below are five real `GetItems` responses, recorded 2026-09-21, and the check
//! is that every language the store listed is still listed after a trip through the masks.
//!
//! Nothing here re-encodes the data a second way. A parallel encoder written for a test agrees
//! with itself and with nothing else; reconstructing through the decoder is what makes a dropped
//! or shifted bit visible.

use catalogames::store_inventory::steam::codec::{Compat, Languages, REGIONAL_ONLY, Spoken};

/// `(app id, name, rows of (elanguage, supported, subtitles, full_audio), (os, frame, machine))`.
type Recorded = (
    u32,
    &'static str,
    &'static [(i32, bool, bool, bool)],
    (u32, u32, u32),
);

const RESPONSES: &[Recorded] = &[
    (
        620,
        "Portal 2",
        &[
            (0, true, true, true),
            (2, true, true, true),
            (1, true, true, true),
            (5, true, true, true),
            (19, true, true, false),
            (13, true, true, false),
            (14, true, true, false),
            (15, true, true, false),
            (18, true, true, false),
            (3, true, true, false),
            (10, true, true, false),
            (4, true, true, false),
            (16, true, true, false),
            (12, true, true, false),
            (11, true, true, false),
            (20, true, true, false),
            (8, true, true, true),
            (6, true, true, false),
            (17, true, true, false),
            (9, true, true, false),
            (7, true, true, false),
            (21, true, true, false),
            (23, true, true, false),
            (24, true, true, false),
            (22, true, true, false),
            (27, true, true, false),
            (26, true, true, false),
        ],
        (2, 3, 3),
    ),
    (
        1091500,
        "Cyberpunk 2077",
        &[
            (0, true, true, true),
            (2, true, true, true),
            (3, true, true, true),
            (1, true, true, true),
            (5, true, true, true),
            (25, true, true, false),
            (19, true, true, false),
            (18, true, true, false),
            (10, true, true, true),
            (4, true, true, true),
            (12, true, true, true),
            (22, true, true, true),
            (8, true, true, true),
            (6, true, true, true),
            (27, true, true, false),
            (9, true, true, false),
            (7, true, true, false),
            (21, true, true, false),
            (26, true, true, false),
        ],
        (2, 0, 3),
    ),
    (
        570,
        "Dota 2",
        &[
            (23, true, false, false),
            (19, true, false, false),
            (13, true, false, false),
            (14, true, false, false),
            (0, true, false, true),
            (15, true, false, false),
            (2, true, false, false),
            (1, true, false, false),
            (24, true, false, false),
            (18, true, false, false),
            (3, true, false, false),
            (10, true, false, false),
            (4, true, false, true),
            (16, true, false, false),
            (12, true, false, false),
            (11, true, false, false),
            (22, true, false, false),
            (20, true, false, false),
            (8, true, false, false),
            (6, true, false, true),
            (5, true, false, false),
            (17, true, false, false),
            (9, true, false, false),
            (7, true, false, false),
            (21, true, false, false),
            (26, true, false, false),
            (27, true, false, false),
            (28, true, false, false),
        ],
        (2, 0, 2),
    ),
    (
        292030,
        "The Witcher 3: Wild Hunt - Complete Edition",
        &[
            (0, true, true, true),
            (2, true, true, true),
            (3, true, true, false),
            (1, true, true, true),
            (5, true, true, false),
            (25, true, true, false),
            (19, true, true, false),
            (18, true, true, false),
            (10, true, true, true),
            (4, true, true, true),
            (12, true, true, true),
            (22, true, true, true),
            (8, true, true, true),
            (7, true, true, false),
            (21, true, true, false),
            (6, true, true, true),
            (27, true, true, false),
        ],
        (2, 0, 3),
    ),
    (
        413150,
        "Stardew Valley",
        &[
            (0, true, false, false),
            (1, true, false, false),
            (5, true, false, false),
            (10, true, false, false),
            (22, true, false, false),
            (8, true, false, false),
            (6, true, false, false),
            (2, true, false, false),
            (3, true, false, false),
            (18, true, false, false),
            (4, true, false, false),
            (21, true, false, false),
        ],
        (2, 0, 3),
    ),
];

/// Every language the store listed comes back listed, with its subtitle and dubbing flags intact.
#[test]
fn real_language_lists_survive_the_masks() {
    for &(app, name, rows, _) in RESPONSES {
        let spoken: Vec<Spoken> = rows
            .iter()
            .map(|&(id, supported, subtitles, full_audio)| Spoken {
                id,
                supported,
                subtitles,
                full_audio,
            })
            .collect();
        let masks = Languages::gather(spoken.iter().copied()).unwrap_or_else(|why| {
            panic!("app {app} ({name}) has a language the mask cannot hold: {why}")
        });

        let columns = masks.columns();
        let borrowed = [
            columns[0].as_str(),
            columns[1].as_str(),
            columns[2].as_str(),
        ];
        let read_back = Languages::from_columns(borrowed).unwrap_or_else(|why| {
            panic!("app {app} ({name}) wrote {columns:?}, which will not parse: {why}")
        });
        assert_eq!(
            read_back, masks,
            "app {app} ({name}) did not survive {columns:?}"
        );

        for row in &spoken {
            if row.id == REGIONAL_ONLY {
                continue;
            }
            let bit = 1u64 << row.id;
            assert_eq!(
                read_back.supported & bit != 0,
                row.supported,
                "{name}: supported for elanguage {}",
                row.id
            );
            assert_eq!(
                read_back.subtitles & bit != 0,
                row.subtitles,
                "{name}: subtitles for elanguage {}",
                row.id
            );
            assert_eq!(
                read_back.full_audio & bit != 0,
                row.full_audio,
                "{name}: audio for elanguage {}",
                row.id
            );
        }
    }
}

/// No mask claims a language the response never mentioned — the failure a set-comparison catches
/// and a per-row check does not.
#[test]
fn the_masks_claim_nothing_the_store_did_not_list() {
    for &(app, name, rows, _) in RESPONSES {
        let spoken: Vec<Spoken> = rows
            .iter()
            .map(|&(id, supported, subtitles, full_audio)| Spoken {
                id,
                supported,
                subtitles,
                full_audio,
            })
            .collect();
        let masks = Languages::gather(spoken.iter().copied()).expect("recorded ids are in range");
        for id in 0..=31 {
            let listed = rows
                .iter()
                .any(|&(other, supported, ..)| other == id && supported);
            assert_eq!(
                masks.speaks(id),
                listed,
                "app {app} ({name}) disagrees about elanguage {id}"
            );
        }
    }
}

/// The columns land in the order the header names them, checked on games whose three masks
/// genuinely differ — Portal 2 lists 27 languages and dubs 5 of them.
///
/// This is the failure a live cross-check caught after sixteen round-trip tests had passed: the
/// writer emitted `[supported, subtitles, full_audio]` while the header said `lang_ui,
/// lang_audio, lang_subs`, so every game's dubbing figures were written into the subtitles
/// column. Round trips cannot see it; naming the columns against different values can.
#[test]
fn the_columns_carry_what_the_header_says_they_carry() {
    let mut differing = 0;
    for &(app, name, rows, _) in RESPONSES {
        let spoken: Vec<Spoken> = rows
            .iter()
            .map(|&(id, supported, subtitles, full_audio)| Spoken {
                id,
                supported,
                subtitles,
                full_audio,
            })
            .collect();
        let masks = Languages::gather(spoken).expect("recorded ids are in range");
        let columns = masks.columns();

        let count = |mask: u64| mask.count_ones() as usize;
        let listed = rows.iter().filter(|(id, s, ..)| *id >= 0 && *s).count();
        let dubbed = rows.iter().filter(|(id, _, _, a)| *id >= 0 && *a).count();
        let subbed = rows.iter().filter(|(id, _, b, _)| *id >= 0 && *b).count();

        assert_eq!(
            count(masks.supported),
            listed,
            "{name}: lang_ui counts the listed languages"
        );
        assert_eq!(
            count(masks.full_audio),
            dubbed,
            "app {app} ({name}): lang_audio counts the dubbed ones"
        );
        assert_eq!(
            count(masks.subtitles),
            subbed,
            "app {app} ({name}): lang_subs counts the subtitled ones"
        );

        let read = Languages::from_columns([&columns[0], &columns[1], &columns[2]])
            .expect("what columns() wrote, from_columns() reads");
        assert_eq!(
            read.full_audio.count_ones() as usize,
            dubbed,
            "{name}: column 1 is audio, not subtitles"
        );
        assert_eq!(
            read.subtitles.count_ones() as usize,
            subbed,
            "{name}: column 2 is subtitles, not audio"
        );

        if dubbed != subbed {
            differing += 1;
        }
    }
    assert!(
        differing > 0,
        "no recorded game distinguishes audio from subtitles; the check is vacuous"
    );
}

/// Real compat triples through the single digit, including the (2, 3, 3) that exercises the top
/// two bits — the ones a packing error leaves untouched on the far commoner all-zero rows.
#[test]
fn real_compat_triples_survive_the_digit() {
    for &(app, name, _, (os, frame, machine)) in RESPONSES {
        let original = Compat { os, frame, machine };
        let digit = original.digit().unwrap_or_else(|why| {
            panic!("app {app} ({name}) has a compat verdict out of range: {why}")
        });
        assert_eq!(
            Compat::from_digit(digit),
            Ok(original),
            "app {app} ({name}) encoded to {digit:?}"
        );
    }
}
