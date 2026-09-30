//! How a catalogue field becomes characters and back again.
//!
//! Every encoder here is half of a pair, and the pairs exist because the snapshot is written by
//! `steam_catalogue` and read by this crate at runtime. A format with only an encoder is a format
//! nobody has checked: the first reader is where a silent truncation or an off-by-one in a bit
//! position finally shows up, as games quietly missing a language they support.
//!
//! So the tests below are **round trips**, not written-out expectations. `encode(decode(x)) == x`
//! over every value the type admits catches a shifted bit; a table of hand-typed strings only
//! catches the ones whoever typed it thought of.
//!
//! # Nothing is clamped
//!
//! Out-of-range input is an error, never a saturated value. Valve owns these numbers and can
//! widen them whenever it likes — a 33rd language, a fifth Deck verdict — and the failure mode of
//! clamping is that the sweep keeps running and writes wrong rows for years. [`Malformed`] makes
//! that day loud: the run records the app and the value it could not represent, and the fix is a
//! deliberate widening of the format rather than a shrug.

use std::fmt;

/// A value the row format cannot represent, carrying the number that did not fit.
///
/// Each variant names the field so a problems file says which column to widen, not merely that
/// something was wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Malformed {
    /// A character outside the field's alphabet.
    Digit(char),
    /// A number too wide for the field — a base-36 run exceeding [`u32::MAX`], say.
    Overflow,
    /// An empty field where a number was required.
    Empty,
    /// An `elanguage` outside `0..=63`, which is the day [`Languages`] must grow past 64 bits.
    Language(i32),
    /// A language list made only of regional variants with no base language, which the masks
    /// cannot express. Writing it as no languages at all would claim something the store did not
    /// say, so the cell is unknown instead.
    RegionalOnly,
    /// A `*_compat_category` outside `0..=3`, which is the day Valve added a fifth verdict.
    Compat(u32),
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Digit(c) => write!(f, "{c:?} is not a digit of this field's alphabet"),
            Self::Overflow => write!(f, "the number is wider than the field holds"),
            Self::Empty => write!(f, "the field is empty"),
            Self::Language(id) => {
                write!(
                    f,
                    "elanguage {id} is outside 0..=63; the language mask needs widening"
                )
            }
            Self::RegionalOnly => write!(
                f,
                "only regional variants are listed, which the language masks cannot express"
            ),
            Self::Compat(v) => write!(f, "compat category {v} is outside 0..=3"),
        }
    }
}

impl std::error::Error for Malformed {}

/// `0-9a-z`, lowercase, least significant digit last. Valve's own numbers written shorter.
const BASE36: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// A `u32` in base 36.
///
/// **Measured, because "use a bigger base" is the obvious next thought and it barely pays.** Over
/// the first ten thousand games the tag block is 894 KB in decimal, 694 KB in base 36 and 643 KB
/// in base 62 — so base 62 buys 51 KB more than base 36, about 4% of the file, in exchange for a
/// case-sensitive alphabet. The widest id is four characters either way, which is why the extra
/// symbols have so little to do.
///
/// What WOULD pay is not a bigger base: only 429 distinct tags appear across those ten thousand
/// games, in an id space reaching 1,352,486. The ids are sparse, so most of the width is spent
/// on a range nothing occupies. See `steam-tags.tsv`.
#[must_use]
pub fn base36(value: u32) -> String {
    base36_wide(u64::from(value))
}

/// A `u64` in base 36: the same digits [`base36`] writes, for the one field wider than 32 bits —
/// the language masks, see [`Languages`]. A value both widths hold is spelled identically by both.
#[must_use]
pub fn base36_wide(mut value: u64) -> String {
    if value == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while value > 0 {
        out.push(BASE36[(value % 36) as usize]);
        value /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Reads back what [`base36`] wrote, rejecting anything it would not have produced.
///
/// Uppercase is rejected rather than folded: the writer never emits it, so seeing it means the
/// field came from somewhere else and the rest of the row is suspect too.
///
/// # Errors
///
/// [`Malformed::Empty`] for an empty field, [`Malformed::Digit`] for a character outside `0-9a-z`,
/// and [`Malformed::Overflow`] past [`u32::MAX`].
pub fn from_base36(text: &str) -> Result<u32, Malformed> {
    u32::try_from(from_base36_wide(text)?).map_err(|_| Malformed::Overflow)
}

/// Reads back what [`base36_wide`] wrote: [`from_base36`]'s rules, with the ceiling at
/// [`u64::MAX`].
///
/// # Errors
///
/// As [`from_base36`], with [`Malformed::Overflow`] past [`u64::MAX`].
pub fn from_base36_wide(text: &str) -> Result<u64, Malformed> {
    if text.is_empty() {
        return Err(Malformed::Empty);
    }
    let mut value: u64 = 0;
    for c in text.chars() {
        let digit = match c {
            '0'..='9' => u64::from(c) - u64::from('0'),
            'a'..='z' => u64::from(c) - u64::from('a') + 10,
            _ => return Err(Malformed::Digit(c)),
        };
        value = value
            .checked_mul(36)
            .and_then(|shifted| shifted.checked_add(digit))
            .ok_or(Malformed::Overflow)?;
    }
    Ok(value)
}

/// Digits first, so the value every other row carries — zero — reads as `0` rather than as `A`.
///
/// Deliberately NOT RFC 4648, whose alphabet starts at `A`. That standard exists so two systems
/// can exchange a byte stream; nothing here is exchanged with anything, and a single digit gains
/// none of its interoperability while paying its whole legibility cost. Measured over 100 games,
/// `steam_frame_compat_category` alone is 0 in 98 of them.
const BASE64: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz+/";

/// The three compat verdicts Valve publishes beside the Deck one, packed into a single character.
///
/// Each is a four-value scale — 0 unknown, 1 unsupported, 2 playable, 3 verified — so three of
/// them are exactly six bits and exactly one digit of 64, with nothing wasted. Deck stays in its
/// own column: it is the one of the four anyone reads by eye, and a fourth field here would spill
/// into a second character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Compat {
    /// SteamOS, in bits 0-1.
    pub os: u32,
    /// Steam Frame, in bits 2-3.
    pub frame: u32,
    /// Steam Machine, in bits 4-5.
    pub machine: u32,
}

impl Compat {
    /// The six-bit packing, low field first.
    ///
    /// # Errors
    ///
    /// [`Malformed::Compat`] naming the first field outside `0..=3`.
    pub const fn bits(self) -> Result<u32, Malformed> {
        // A loop would need an array and an index; three checks read as the three fields they are.
        if self.os > 3 {
            return Err(Malformed::Compat(self.os));
        }
        if self.frame > 3 {
            return Err(Malformed::Compat(self.frame));
        }
        if self.machine > 3 {
            return Err(Malformed::Compat(self.machine));
        }
        Ok(self.os | (self.frame << 2) | (self.machine << 4))
    }

    /// The single character the row carries.
    ///
    /// # Errors
    ///
    /// [`Malformed::Compat`] naming the first field outside `0..=3`.
    pub fn digit(self) -> Result<char, Malformed> {
        Ok(char::from(BASE64[self.bits()? as usize]))
    }

    /// Unpacks a six-bit value. Never fails: every value of `0..64` is a valid triple.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self {
            os: bits & 3,
            frame: (bits >> 2) & 3,
            machine: (bits >> 4) & 3,
        }
    }

    /// Reads back what [`Self::digit`] wrote.
    ///
    /// # Errors
    ///
    /// [`Malformed::Digit`] for a character outside the alphabet.
    pub fn from_digit(c: char) -> Result<Self, Malformed> {
        let found = u8::try_from(c)
            .ok()
            .and_then(|byte| BASE64.iter().position(|&d| d == byte))
            .ok_or(Malformed::Digit(c))?;
        // `position` over a 64-entry table cannot exceed u32.
        Ok(Self::from_bits(found as u32))
    }
}

/// The widest `elanguage` the mask holds.
///
/// **64 bits because 32 were full.** Valve's ids ran `0..=31` when this was measured, and the
/// largest mask in the snapshot of 2026-09-26 was exactly `u32::MAX`: every bit in use, so the
/// next language Valve adds had nowhere to go. Bit 31 was also a hazard of its own — anything
/// reading the mask as a signed 32-bit number takes it for a negative one. The file's base-36
/// text is unchanged by the widening; only the numbers it can spell grew.
pub const WIDEST_LANGUAGE: i32 = 63;

/// An `elanguage` of `-1` is a row carrying only an `eadditionallanguage` — a regional variant
/// with no base language, seen in 2 of 1,105 entries. It is a shape Valve already publishes, not a
/// widening, so it is skipped rather than raised; what the mask cannot express, it does not claim.
/// A list made of nothing BUT such rows is refused as [`Malformed::RegionalOnly`], though: its
/// masks would be all zero, which the file reads as "no languages", and the store listed some.
pub const REGIONAL_ONLY: i32 = -1;

/// One language row as Valve publishes it, before the masks fold the list into three numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spoken {
    /// Valve's `elanguage`, or [`REGIONAL_ONLY`].
    pub id: i32,
    /// The store lists the language at all.
    pub supported: bool,
    /// Subtitles in it.
    pub subtitles: bool,
    /// Dubbed audio in it.
    pub full_audio: bool,
}

/// Which languages a game offers, as three independent bit masks over Valve's `elanguage`.
///
/// Three rather than one because the flags are independent, not nested: measured over 3,683 rows,
/// `supported` holds for 3,676, `subtitles` for 2,360 and `full_audio` for 1,348, and `supported`
/// is false 7 times, so none of the three is derivable from another.
///
/// **What this drops**, deliberately: `eadditionallanguage`, which distinguishes Spanish-Spain
/// from Spanish-Latin America and appears on 13.6% of rows. Its ids run past 54, so carrying it
/// would need a second, wider mask per flag for a distinction this catalogue never queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Languages {
    /// Listed by the store at all — the store page's Interface column.
    pub supported: u64,
    /// Dubbed — the Full Audio column.
    pub full_audio: u64,
    /// Subtitled — the Subtitles column.
    pub subtitles: u64,
}

impl Languages {
    /// Folds Valve's list into the three masks.
    ///
    /// # Errors
    ///
    /// [`Malformed::Language`] naming the first id outside `0..=63` that is not [`REGIONAL_ONLY`].
    /// That is the signal to widen the masks again, and it is an error rather than a skip because
    /// a dropped language is invisible in the output while a refused row is not. And
    /// [`Malformed::RegionalOnly`] for a list of regional variants alone — see [`REGIONAL_ONLY`].
    pub fn gather(rows: impl IntoIterator<Item = Spoken>) -> Result<Self, Malformed> {
        let mut masks = Self::default();
        let (mut listed, mut expressed) = (false, false);
        for row in rows {
            listed = true;
            if row.id == REGIONAL_ONLY {
                continue;
            }
            expressed = true;
            if row.id < 0 || row.id > WIDEST_LANGUAGE {
                return Err(Malformed::Language(row.id));
            }
            // Bounded by WIDEST_LANGUAGE just above, so the shift cannot overflow.
            let bit = 1u64 << row.id;
            if row.supported {
                masks.supported |= bit;
            }
            if row.subtitles {
                masks.subtitles |= bit;
            }
            if row.full_audio {
                masks.full_audio |= bit;
            }
        }
        if listed && !expressed {
            return Err(Malformed::RegionalOnly);
        }
        Ok(masks)
    }

    /// The three masks in the row's column order: `lang_ui`, `lang_audio`, `lang_subs`.
    ///
    /// **The order is the store page's own — Interface, Full Audio, Subtitles — and it is pinned
    /// by a test, because nothing else can catch it being wrong.** A round trip through
    /// [`Self::from_columns`] is self-consistent whatever order the two agree on, so an array
    /// ordered differently from the header names still passes every round-trip check while
    /// writing dubbing figures into the subtitles column. That is precisely what it did, and a
    /// live cross-check against Valve is what found it.
    #[must_use]
    pub fn columns(self) -> [String; 3] {
        [
            base36_wide(self.supported),
            base36_wide(self.full_audio),
            base36_wide(self.subtitles),
        ]
    }

    /// Reads back what [`Self::columns`] wrote.
    ///
    /// # Errors
    ///
    /// Whatever [`from_base36_wide`] raises for the first column that will not parse.
    pub fn from_columns(columns: [&str; 3]) -> Result<Self, Malformed> {
        Ok(Self {
            supported: from_base36_wide(columns[0])?,
            full_audio: from_base36_wide(columns[1])?,
            subtitles: from_base36_wide(columns[2])?,
        })
    }

    /// Whether a given `elanguage` is listed. `false` for any id the mask cannot hold.
    #[must_use]
    pub const fn speaks(self, id: i32) -> bool {
        id >= 0 && id <= WIDEST_LANGUAGE && self.supported & (1u64 << id) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every value the field admits, not a sample of them — the point of a six-bit field is that
    /// exhaustion is 64 cases, so "we tested the ones we thought of" has no excuse here.
    #[test]
    fn every_compat_triple_survives_a_round_trip() {
        for os in 0..=3 {
            for frame in 0..=3 {
                for machine in 0..=3 {
                    let original = Compat { os, frame, machine };
                    let digit = original.digit().expect("0..=3 is in range");
                    assert_eq!(
                        Compat::from_digit(digit),
                        Ok(original),
                        "{original:?} encoded to {digit:?} and came back as something else"
                    );
                }
            }
        }
    }

    /// A duplicated character in the table would make two triples decode identically, and the
    /// round trip above would still pass for whichever one `position` happened to find first.
    #[test]
    fn the_alphabets_have_no_repeated_characters() {
        for (name, alphabet) in [("base36", &BASE36[..]), ("base64", &BASE64[..])] {
            let mut seen = alphabet.to_vec();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), alphabet.len(), "{name} repeats a character");
        }
    }

    /// Zero is the value most rows carry, so it is the one whose legibility was paid for.
    #[test]
    fn the_unknown_triple_reads_as_zero() {
        assert_eq!(Compat::default().digit(), Ok('0'));
        assert_eq!(Compat::from_digit('0'), Ok(Compat::default()));
    }

    #[test]
    fn a_compat_verdict_valve_has_not_invented_yet_is_refused() {
        assert_eq!(
            Compat {
                os: 4,
                ..Compat::default()
            }
            .digit(),
            Err(Malformed::Compat(4))
        );
        assert_eq!(
            Compat {
                frame: 9,
                ..Compat::default()
            }
            .bits(),
            Err(Malformed::Compat(9))
        );
        assert_eq!(
            Compat {
                machine: 4,
                ..Compat::default()
            }
            .bits(),
            Err(Malformed::Compat(4))
        );
    }

    #[test]
    fn a_character_outside_the_alphabet_is_refused() {
        assert_eq!(Compat::from_digit('-'), Err(Malformed::Digit('-')));
        assert_eq!(Compat::from_digit('\t'), Err(Malformed::Digit('\t')));
        assert_eq!(Compat::from_digit('é'), Err(Malformed::Digit('é')));
    }

    /// Each field must occupy its own two bits. A shifted mask still round-trips through its own
    /// pair of functions, so the packing is pinned against the layout the docs promise.
    #[test]
    fn each_compat_field_owns_its_own_bits() {
        assert_eq!(
            Compat {
                os: 3,
                frame: 0,
                machine: 0
            }
            .bits(),
            Ok(0b00_00_11)
        );
        assert_eq!(
            Compat {
                os: 0,
                frame: 3,
                machine: 0
            }
            .bits(),
            Ok(0b00_11_00)
        );
        assert_eq!(
            Compat {
                os: 0,
                frame: 0,
                machine: 3
            }
            .bits(),
            Ok(0b11_00_00)
        );
    }

    /// 64 positions by 8 flag combinations: exhaustive over one language at a time, which is what
    /// catches a bit written into the wrong mask or shifted by one.
    #[test]
    fn every_language_bit_lands_in_every_mask_independently() {
        for id in 0..=WIDEST_LANGUAGE {
            for combination in 0..8u8 {
                let row = Spoken {
                    id,
                    supported: combination & 1 != 0,
                    subtitles: combination & 2 != 0,
                    full_audio: combination & 4 != 0,
                };
                let masks = Languages::gather([row]).expect("0..=63 is in range");
                let bit = 1u64 << id;
                assert_eq!(
                    masks.supported,
                    if row.supported { bit } else { 0 },
                    "{row:?}"
                );
                assert_eq!(
                    masks.subtitles,
                    if row.subtitles { bit } else { 0 },
                    "{row:?}"
                );
                assert_eq!(
                    masks.full_audio,
                    if row.full_audio { bit } else { 0 },
                    "{row:?}"
                );

                let columns = masks.columns();
                let borrowed = [
                    columns[0].as_str(),
                    columns[1].as_str(),
                    columns[2].as_str(),
                ];
                assert_eq!(
                    Languages::from_columns(borrowed),
                    Ok(masks),
                    "{row:?} via {columns:?}"
                );
            }
        }
    }

    /// Every language at once, with the flags differing per language, so a mask that accidentally
    /// aliased another would show up as the wrong answer rather than as a coincidence.
    #[test]
    fn a_full_language_list_survives_a_round_trip() {
        let rows: Vec<Spoken> = (0..=WIDEST_LANGUAGE)
            .map(|id| Spoken {
                id,
                supported: true,
                subtitles: id % 2 == 0,
                full_audio: id % 3 == 0,
            })
            .collect();
        let masks = Languages::gather(rows.iter().copied()).expect("0..=63 is in range");
        assert_eq!(masks.supported, u64::MAX, "all 64 listed");
        for row in &rows {
            let bit = 1u64 << row.id;
            assert_eq!(
                masks.subtitles & bit != 0,
                row.subtitles,
                "subtitles for {}",
                row.id
            );
            assert_eq!(
                masks.full_audio & bit != 0,
                row.full_audio,
                "audio for {}",
                row.id
            );
            assert!(
                masks.speaks(row.id),
                "language {} should read back as spoken",
                row.id
            );
        }
        let columns = masks.columns();
        let borrowed = [
            columns[0].as_str(),
            columns[1].as_str(),
            columns[2].as_str(),
        ];
        assert_eq!(Languages::from_columns(borrowed), Ok(masks));
    }

    /// Which column is which, pinned with three masks no two of which are equal.
    ///
    /// Every other test here round-trips, and a round trip cannot see this: the writer and the
    /// reader agree with each other no matter which slot holds which flag. Only naming the slots
    /// against distinguishable values catches a swap, and a live cross-check against Valve found
    /// one that all sixteen round trips had passed.
    #[test]
    fn the_columns_are_interface_then_audio_then_subtitles() {
        let masks = Languages {
            supported: 0b111,
            full_audio: 0b010,
            subtitles: 0b101,
        };
        let columns = masks.columns();
        assert_eq!(
            columns[0],
            base36(0b111),
            "column 0 is `lang_ui`, Valve's `supported`"
        );
        assert_eq!(
            columns[1],
            base36(0b010),
            "column 1 is `lang_audio`, Valve's `full_audio`"
        );
        assert_eq!(
            columns[2],
            base36(0b101),
            "column 2 is `lang_subs`, Valve's `subtitles`"
        );

        let borrowed = [
            columns[0].as_str(),
            columns[1].as_str(),
            columns[2].as_str(),
        ];
        assert_eq!(
            Languages::from_columns(borrowed),
            Ok(masks),
            "the reader agrees"
        );

        // And the reader is pinned independently, so the pair cannot agree on a wrong order.
        let read = Languages::from_columns(["1", "2", "4"]).expect("plain digits");
        assert_eq!((read.supported, read.full_audio, read.subtitles), (1, 2, 4));
    }

    /// The 33rd language fits now; the day Valve adds a 65th, the sweep must say so rather than
    /// drop it.
    #[test]
    fn a_language_wider_than_the_mask_is_refused_rather_than_dropped() {
        let thirty_third = Spoken {
            id: 32,
            supported: true,
            subtitles: false,
            full_audio: false,
        };
        assert_eq!(
            Languages::gather([thirty_third]).map(|masks| masks.supported),
            Ok(1u64 << 32),
            "the id that had nowhere to go in 32 bits"
        );
        let widened = Spoken {
            id: 64,
            ..thirty_third
        };
        assert_eq!(Languages::gather([widened]), Err(Malformed::Language(64)));
        let absurd = Spoken {
            id: -2,
            supported: true,
            subtitles: false,
            full_audio: false,
        };
        assert_eq!(Languages::gather([absurd]), Err(Malformed::Language(-2)));
    }

    /// A shape Valve already publishes is not a widening, so beside a base language it is skipped
    /// without complaint — and, crucially, without setting a bit that would claim a language the
    /// game does not list.
    #[test]
    fn a_regional_row_beside_a_base_language_is_skipped() {
        let regional = Spoken {
            id: REGIONAL_ONLY,
            supported: true,
            subtitles: true,
            full_audio: true,
        };
        let english = Spoken {
            id: 0,
            supported: true,
            subtitles: false,
            full_audio: false,
        };
        assert_eq!(
            Languages::gather([regional, english]),
            Ok(Languages {
                supported: 1,
                ..Languages::default()
            })
        );
    }

    /// A list of regional variants and nothing else would come out as three zeroes — which the
    /// file reads as "no languages", when the store listed some. So it is refused, and the sweep
    /// writes the cells as unknown.
    #[test]
    fn a_list_of_regional_variants_alone_is_refused_rather_than_read_as_none() {
        let regional = Spoken {
            id: REGIONAL_ONLY,
            supported: true,
            subtitles: true,
            full_audio: true,
        };
        assert_eq!(
            Languages::gather([regional, regional]),
            Err(Malformed::RegionalOnly)
        );
    }

    #[test]
    fn an_empty_language_list_is_three_zeroes() {
        let masks = Languages::gather([]).expect("nothing to reject");
        assert_eq!(
            masks.columns(),
            ["0".to_string(), "0".to_string(), "0".to_string()]
        );
        assert!(!masks.speaks(0));
    }

    #[test]
    fn speaks_refuses_ids_the_mask_cannot_hold() {
        let masks = Languages {
            supported: u64::MAX,
            ..Languages::default()
        };
        assert!(masks.speaks(0) && masks.speaks(32) && masks.speaks(WIDEST_LANGUAGE));
        assert!(
            !masks.speaks(64),
            "an id past the mask is absent, never wrapped around"
        );
        assert!(!masks.speaks(-1) && !masks.speaks(i32::MIN));
    }

    /// Exhaustive across the range every real field occupies, plus the boundaries where a base
    /// conversion actually breaks: each power of 36, and the widest value a u32 holds.
    #[test]
    fn base36_survives_a_round_trip() {
        let mut checked = 0u32;
        let powers = (0..7)
            .map(|e| 36u64.pow(e))
            .flat_map(|p| [p.saturating_sub(1), p, p + 1]);
        let edges = [0, 1, 35, 36, u32::from(u16::MAX), u32::MAX - 1, u32::MAX].into_iter();
        let sweep = (0..=100_000u32).map(u64::from);
        // A deterministic LCG beats a random one: a failure is reproducible without a seed to log.
        let mut state = 0x2545_F491u64;
        let scattered = std::iter::repeat_with(move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 32) & 0xFFFF_FFFF
        })
        .take(200_000);

        for value in sweep
            .chain(powers)
            .chain(edges.map(u64::from))
            .chain(scattered)
        {
            let Ok(value) = u32::try_from(value) else {
                continue;
            };
            let text = base36(value);
            assert_eq!(from_base36(&text), Ok(value), "{value} encoded to {text:?}");
            assert!(
                !text.is_empty() && (text == "0" || !text.starts_with('0')),
                "{text:?}"
            );
            checked += 1;
        }
        assert!(checked > 300_000, "the sweep shrank to {checked} values");
    }

    #[test]
    fn base36_refuses_what_it_would_never_have_written() {
        assert_eq!(from_base36(""), Err(Malformed::Empty));
        assert_eq!(
            from_base36("A"),
            Err(Malformed::Digit('A')),
            "uppercase is not folded"
        );
        assert_eq!(from_base36("1 2"), Err(Malformed::Digit(' ')));
        assert_eq!(from_base36("-1"), Err(Malformed::Digit('-')));
        assert_eq!(from_base36("1.5"), Err(Malformed::Digit('.')));
    }

    #[test]
    fn base36_refuses_a_number_too_wide_for_the_column() {
        assert_eq!(from_base36(&base36(u32::MAX)), Ok(u32::MAX));
        assert_eq!(
            from_base36("1z141z4"),
            Err(Malformed::Overflow),
            "one past u32::MAX"
        );
        assert_eq!(from_base36("zzzzzzzzzz"), Err(Malformed::Overflow));
    }

    /// The wide pair spells every value the narrow pair does identically — so widening the masks
    /// changed no existing cell — and carries on to the full 64 bits.
    #[test]
    fn the_wide_base36_agrees_with_the_narrow_one_and_reaches_64_bits() {
        for value in [0, 1, 35, 36, 1_295, 1_296, u32::MAX - 1, u32::MAX] {
            assert_eq!(base36_wide(u64::from(value)), base36(value), "{value}");
        }
        assert_eq!(
            base36_wide(u64::from(u32::MAX)),
            "1z141z3",
            "the saturated 32-bit mask"
        );
        assert_eq!(from_base36_wide(&base36_wide(u64::MAX)), Ok(u64::MAX));
        assert_eq!(from_base36_wide("1z141z4"), Ok(u64::from(u32::MAX) + 1));
        assert_eq!(
            from_base36("1z141z4"),
            Err(Malformed::Overflow),
            "the narrow one still refuses"
        );
        assert_eq!(
            from_base36_wide("zzzzzzzzzzzzzz"),
            Err(Malformed::Overflow),
            "past u64::MAX"
        );
    }

    /// Whatever the field holds, writing what was read must reproduce the field exactly — the
    /// property a reader relies on when it rewrites a row it did not originate.
    #[test]
    fn decoding_then_encoding_reproduces_the_text() {
        for text in ["0", "1", "z", "10", "1z141z3", "abc123"] {
            let value = from_base36(text).expect("valid base 36");
            assert_eq!(
                base36(value),
                text,
                "{text:?} did not survive the return trip"
            );
        }
        for bits in 0..64u32 {
            let digit = Compat::from_bits(bits)
                .digit()
                .expect("from_bits yields 0..=3 fields");
            assert_eq!(Compat::from_digit(digit).and_then(Compat::bits), Ok(bits));
        }
    }
}
