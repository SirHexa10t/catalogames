//! Prints `src/inventory/steam/tags.rs` — every tag Steam publishes, as an enum.
//!
//! **Not part of the library.** Built only under the `tools` feature, like its sibling
//! `steam_lookup`, and run far less often: the tag vocabulary changes when Valve adds a tag,
//! not when a bundle rotates.
//!
//! ```text
//! cargo run --release --features tools --bin steam_tags > src/inventory/steam/tags.rs
//! ```
//!
//! Prints to stdout and never writes in place, for the same reason as `steam_lookup`: the diff
//! is the review.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Deserialize;

const USER_AGENT: &str = concat!("catalogames-steam-tags/", env!("CARGO_PKG_VERSION"));

/// Valve's own tag list.
///
/// Preferred over `store.steampowered.com/tagdata/populartags/english`, which returned 429 tags
/// against this endpoint's 446 and is a strict subset of it — "popular" is a filtered view, and
/// a tag missing from the enum is one the game generator cannot record.
const TAG_LIST: &str = "https://api.steampowered.com/IStoreService/GetTagList/v1/?language=english";

#[derive(Deserialize)]
struct Tag {
    tagid: u32,
    name: String,
}

#[derive(Deserialize)]
struct Response {
    response: TagList,
}

#[derive(Deserialize)]
struct TagList {
    tags: Vec<Tag>,
}

/// Turns a tag's display name into a Rust variant.
///
/// Words are split on anything that is not alphanumeric and each is capitalised, which turns
/// `"Souls-like"` into `SoulsLike`, `"Beat 'em up"` into `BeatEmUp` and `"Football (Soccer)"`
/// into `FootballSoccer`. Only the first letter of each word is touched, so `"Sci-fi"` becomes
/// `SciFi` rather than `Scifi` and `"2.5D"` keeps its capital `D`.
///
/// Apostrophes are removed rather than treated as word breaks, because they appear in both
/// roles: `"1990's"` is one word and must not become `_1990S`, while `"Beat 'em up"` is three
/// and must become `BeatEmUp`. Deleting the apostrophe first gets both right.
///
/// Names that begin with a digit get a leading underscore, because Rust forbids an identifier
/// starting with one: `"2D"` becomes `_2D` and `"1990's"` becomes `_1990s`.
fn variant(name: &str) -> String {
    let name = name.replace(['\'', '\u{2019}'], "");
    let mut out = String::new();
    for word in name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let mut letters = word.chars();
        if let Some(first) = letters.next() {
            out.extend(first.to_uppercase());
            out.push_str(letters.as_str());
        }
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{out}")
    } else {
        out
    }
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("steam_tags: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let http = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    eprintln!("fetching {TAG_LIST}");
    let response = http.get(TAG_LIST).send().map_err(|e| format!("GET: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("GET: HTTP {}", response.status()));
    }
    let body = response.text().map_err(|e| format!("GET: {e}"))?;
    let mut tags = serde_json::from_str::<Response>(&body)
        .map_err(|e| format!("tag list: {e}"))?
        .response
        .tags;

    // Sorted by id so regeneration produces a reviewable diff rather than a reshuffled file,
    // and so a newly added tag lands at the end where it is easy to see.
    tags.sort_by_key(|tag| tag.tagid);
    eprintln!("{} tags", tags.len());

    // A name that manages to produce a variant another name already produced would silently
    // drop one of the two tags, so it fails the run instead.
    let mut claimed: BTreeMap<String, &str> = BTreeMap::new();
    for tag in &tags {
        let name = variant(&tag.name);
        if name.is_empty() {
            return Err(format!(
                "tag {:?} (id {}) yields no identifier",
                tag.name, tag.tagid
            ));
        }
        if let Some(other) = claimed.insert(name.clone(), &tag.name) {
            return Err(format!(
                "{:?} and {:?} both map to `{name}`",
                other, tag.name
            ));
        }
    }

    print!("{}", emit(&tags));
    Ok(())
}

fn emit(tags: &[Tag]) -> String {
    let mut out = String::new();
    out.push_str(
        "//! Every tag Steam publishes, as an enum.\n\
         //!\n\
         //! GENERATED — do not hand-edit; regenerate and review the diff:\n\
         //!\n\
         //! ```text\n\
         //! cargo run --release --features tools --bin steam_tags > src/inventory/steam/tags.rs\n\
         //! ```\n\
         //!\n\
         //! Tags are an enum rather than strings so that a misspelt tag cannot compile and a\n\
         //! `match` over them can be exhaustive. The cost is that a tag Valve adds is unknown\n\
         //! here until this is regenerated — which is why the game generator treats an\n\
         //! unrecognised tag as a failure rather than discarding it.\n\
         //!\n\
         //! Variant names are the display names with non-alphanumerics removed and each word\n\
         //! capitalised; names beginning with a digit take a leading underscore, since Rust\n\
         //! forbids an identifier starting with one.\n\n",
    );

    // Steam's names are Steam's. `LGBTQ` and `RPGMaker` are acronyms it capitalises, and a
    // generated file should read like its source rather than like a style guide.
    out.push_str("#![allow(clippy::upper_case_acronyms, non_camel_case_types)]\n\n");

    let _ = writeln!(
        out,
        "/// A Steam tag. {} of them, as of the last regeneration.",
        tags.len()
    );
    out.push_str("#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]\n");
    out.push_str("pub enum Tag {\n");
    for tag in tags {
        let _ = writeln!(out, "    /// {:?}, tag id {}.", tag.name, tag.tagid);
        let _ = writeln!(out, "    {},", variant(&tag.name));
    }
    out.push_str("}\n\n");

    out.push_str(
        "impl Tag {\n    \
         /// Steam's own id for this tag.\n    \
         #[rustfmt::skip]\n    \
         pub fn id(self) -> u32 {\n        \
         match self {\n",
    );
    for tag in tags {
        let _ = writeln!(
            out,
            "            Self::{} => {},",
            variant(&tag.name),
            tag.tagid
        );
    }
    out.push_str("        }\n    }\n\n");

    out.push_str(
        "    /// The name Steam displays.\n    \
         #[rustfmt::skip]\n    \
         pub fn as_str(self) -> &'static str {\n        \
         match self {\n",
    );
    for tag in tags {
        let _ = writeln!(
            out,
            "            Self::{} => {:?},",
            variant(&tag.name),
            tag.name
        );
    }
    out.push_str("        }\n    }\n}\n\n");

    let _ = writeln!(
        out,
        "/// Every tag, ordered by Steam's id.\n\
         ///\n\
         /// Both `id` and `as_str` are exhaustive `match`es, so a variant cannot exist without\n\
         /// them; this table is what makes the reverse lookups possible.\n\
         #[rustfmt::skip]\n\
         pub const TAGS: &[Tag] = &["
    );
    for tag in tags {
        let _ = writeln!(out, "    Tag::{},", variant(&tag.name));
    }
    out.push_str("];\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_capitalised_and_punctuation_dropped() {
        assert_eq!(variant("Souls-like"), "SoulsLike");
        assert_eq!(variant("Co-op"), "CoOp");
        assert_eq!(variant("Beat 'em up"), "BeatEmUp");
        assert_eq!(variant("Football (Soccer)"), "FootballSoccer");
        assert_eq!(variant("Animation & Modeling"), "AnimationModeling");
        assert_eq!(variant("LGBTQ+"), "LGBTQ");
    }

    #[test]
    fn only_the_first_letter_of_a_word_is_changed() {
        // Lower-casing the rest would turn "Sci-fi" into "Scifi" and "2.5D" into "25d",
        // losing the capitalisation Steam itself uses.
        assert_eq!(variant("Sci-fi"), "SciFi");
        assert_eq!(variant("RPGMaker"), "RPGMaker");
        assert_eq!(variant("2.5D"), "_25D");
    }

    #[test]
    fn a_name_starting_with_a_digit_takes_a_leading_underscore() {
        // Rust forbids an identifier starting with a digit.
        assert_eq!(variant("2D"), "_2D");
        assert_eq!(variant("4X"), "_4X");
        assert_eq!(variant("1990's"), "_1990s");
        assert_eq!(variant("360 Video"), "_360Video");
        assert_eq!(variant("6DOF"), "_6DOF");
    }

    #[test]
    fn a_variant_never_starts_with_a_digit() {
        for name in ["2D", "3D", "4X", "6DOF", "1980s", "2.5D", "360 Video"] {
            let identifier = variant(name);
            assert!(
                !identifier.starts_with(|c: char| c.is_ascii_digit()),
                "{name:?} -> {identifier}"
            );
        }
    }
}
