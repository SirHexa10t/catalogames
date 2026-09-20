//! Games as Steam presents them, as data: which app-id a title is, when it came out, what its
//! players make of it, and what they tag it. One module per category — [`regular`] for games
//! on sale today, and room for `delisted` and its kind alongside — so a consumer asks for the
//! category it means rather than filtering a single undifferentiated pile.
//!
//! The entries are generated, not typed: `steam_lookup` resolves them from the titles a store
//! is bundling and prints the table, which a human reads as a diff before it lands. What the
//! generator cannot decide it marks `// REVIEW:` rather than guessing — see [`regular`].
//!
//! House rules for entries, learned the hard way:
//! - **"All-time" means `language=all` and `purchase_type=all`.** Steam's review API defaults
//!   `purchase_type` to `steam`, which drops key-activated copies — 20% of the reviews on a
//!   popular title. For a catalogue about bundles, key-activated copies are exactly the
//!   population that matters, so both parameters are always passed explicitly.
//! - **That figure therefore does not match the store page, on purpose.** The page displays the
//!   `purchase_type=steam` population, so an entry here reads higher than the page it came
//!   from — by 8% on one measured title, 20% on another. The divergence is the point; do not
//!   "fix" it toward the page.
//! - **Off-topic reviews are excluded**, which is Steam's default. That inherits Valve's own
//!   editorial call on review-bombing rather than making one here. It is a separate choice from
//!   the one above and rests on its own reasoning, not on matching any displayed number. How
//!   much it moves a count has not been measured on a review-bombed title, which is the only
//!   place it would show.
//! - **"Recent" is a 30-day window and therefore always stale.** It is a snapshot taken on the
//!   module's `captured` date and drifts from that day onward. Read it as "this is what the
//!   last 30 days looked like *then*", never as current.
//! - **Ratings are stored as an approval percentage, floored.** Steam's band is derived from
//!   it rather than stored, so the two can never disagree. Flooring is not a detail: checked
//!   against 137 real titles, flooring reproduces Steam's own band 137/137 while rounding gets
//!   6 wrong — every one of them at a boundary, where a title on 79.93% rounds up to 80 and
//!   changes band.
//! - **A tag list is never empty.** Steam age-gates mature titles, and a gated page parses into
//!   zero tags — indistinguishable from a game nobody tagged. The tests below refuse an empty
//!   list so that failure cannot land quietly.
//! - **One app-id per entry, and it is the base game.** `type == "game"` rules out DLC, demos
//!   and soundtracks, but not a GOTY edition or a re-release of the same title under a
//!   different id. Those the generator flags for a human.
//! - **Editions are aliases, not entries.** A "Game of the Year Edition" is usually not its own
//!   app — Steam sells it as a bundle over the base game's app-id. Storing it separately would
//!   duplicate every review count and tag under a second id that does not exist. The bundle's
//!   wording goes in [`SteamGame::aliases`] instead, and [`by_name`] finds the entry by either.
//! - **Compatibility is what Steam publishes, not what a game can do.** `Deck::Unknown` means
//!   Valve has not rated it, not that it fails; an absent feature means Steam does not list it.

pub mod regular;
pub mod source;
pub mod tags;

pub use tags::Tag;

/// Steam's verdict on a game's reviews — the nine bands it displays, and the case where it
/// declines to give one.
///
/// A closed enum rather than a string: the bands are Valve's, they change about never, and a
/// typo in a generated table should not compile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rating {
    OverwhelminglyPositive,
    VeryPositive,
    Positive,
    MostlyPositive,
    Mixed,
    MostlyNegative,
    Negative,
    VeryNegative,
    OverwhelminglyNegative,
    /// Too few reviews for Steam to name a band; it shows a bare count instead.
    TooFewReviews,
}

/// Every band, in Valve's own order from best to worst.
///
/// Iterated by the round-trip test, so a band added to [`Rating`] and forgotten here is caught.
pub const RATINGS: &[Rating] = &[
    Rating::OverwhelminglyPositive,
    Rating::VeryPositive,
    Rating::Positive,
    Rating::MostlyPositive,
    Rating::Mixed,
    Rating::MostlyNegative,
    Rating::Negative,
    Rating::VeryNegative,
    Rating::OverwhelminglyNegative,
    Rating::TooFewReviews,
];

/// One row of Steam's verdict table: the lowest approval and review count that reach a band.
pub struct Band {
    /// Minimum approval percentage, inclusive.
    pub min_approval: u8,
    /// Minimum number of reviews, inclusive.
    pub min_reviews: u32,
    pub rating: Rating,
}

/// Steam's bands, best first — the first row a score reaches is the verdict.
///
/// Two things about this table are easy to get wrong. The count is **load-bearing, not a
/// tie-break**: a game on 99% with 201 reviews is "Very Positive", not "Overwhelmingly
/// Positive", because the top band needs 500 reviews. A translator taking only a percentage
/// therefore cannot reproduce Steam's system at all.
///
/// And the order matters: rows are tried top to bottom, so `Positive` sits below `VeryPositive`
/// at the same approval and catches the titles that miss its review count.
///
/// Flooring the percentage before it reaches here is exact rather than approximate, because
/// every threshold is a whole number: a title on 79.98% floors to 79 and lands in the same band
/// as the exact ratio would. Rounding does not have that property — 79.98% rounds to 80 and
/// changes band, which is measurably wrong against Steam.
///
/// The 70, 80 and 95 boundaries are observed from both sides. **The 20 and 40 boundaries, the
/// 10/50/500 review counts, and the whole negative half are inference** — no sample sat on
/// those thresholds, and negatively-reviewed games with enough reviews are rare. Valve
/// documents none of it.
pub const BANDS: &[Band] = &[
    Band {
        min_approval: 95,
        min_reviews: 500,
        rating: Rating::OverwhelminglyPositive,
    },
    Band {
        min_approval: 80,
        min_reviews: 50,
        rating: Rating::VeryPositive,
    },
    Band {
        min_approval: 80,
        min_reviews: 10,
        rating: Rating::Positive,
    },
    Band {
        min_approval: 70,
        min_reviews: 10,
        rating: Rating::MostlyPositive,
    },
    Band {
        min_approval: 40,
        min_reviews: 10,
        rating: Rating::Mixed,
    },
    Band {
        min_approval: 20,
        min_reviews: 10,
        rating: Rating::MostlyNegative,
    },
    Band {
        min_approval: 0,
        min_reviews: 500,
        rating: Rating::OverwhelminglyNegative,
    },
    Band {
        min_approval: 0,
        min_reviews: 50,
        rating: Rating::VeryNegative,
    },
    Band {
        min_approval: 0,
        min_reviews: 10,
        rating: Rating::Negative,
    },
];

/// Fewest reviews Steam will assign any band to.
const MINIMUM_REVIEWS: u32 = 10;

/// Fewest reviews the top band needs in the thirty-day window.
///
/// **Inferred, not measured**, and revised once already. Across 131 real thirty-day rows the
/// highest "Very Positive" at 95%+ had 85 reviews and the lowest "Overwhelmingly Positive" had
/// 291, so the threshold lies somewhere in `(85, 291]` and cannot be pinned from observation.
///
/// 100 is chosen from inside that bracket because it is where the rest of the scheme points:
/// this window reaches "Very Positive" at 10 reviews where the all-time rule needs 50, a
/// fivefold reduction, and a fivefold reduction of the all-time top band's 500 is 100.
///
/// An earlier value of 50 came from a smaller sample whose highest 95%+ "Very Positive" had
/// only 40 reviews. It was the lowest value consistent with *that* sample, and seven titles
/// later proved it wrong — which is the argument for the cross-check in the fetch rather than
/// for trusting any number written here.
pub const RECENT_TOP_BAND_REVIEWS: u32 = 100;

/// The bracket is what is actually established; the value inside it is a choice.
///
/// Checked at compile time rather than in a test, because an edit that leaves the measured
/// range is wrong whatever number it picks, and finding that out at build time is cheaper than
/// finding it out from a failed regeneration.
const _: () = assert!(
    RECENT_TOP_BAND_REVIEWS > 85 && RECENT_TOP_BAND_REVIEWS <= 291,
    "RECENT_TOP_BAND_REVIEWS contradicts a measured thirty-day row: the highest Very Positive \
     at 95%+ had 85 reviews and the lowest Overwhelmingly Positive had 291"
);

impl Rating {
    /// Steam's verdict for an approval percentage over a number of reviews.
    ///
    /// Below ten reviews Steam declines to give one, which is [`Rating::TooFewReviews`] rather
    /// than a guess at what the handful of reviews mean.
    pub fn from_score(approval: u8, count: u32) -> Self {
        Self::scan(approval, count, |band| band.min_reviews)
    }

    /// Steam's verdict for the **last thirty days**, which is not the same function.
    ///
    /// The percentage bands are identical, but the review-count gates are not — measured across
    /// 86 real titles:
    ///
    /// - 80% over **10** reviews reads "Very Positive" here, where the all-time rule needs 50
    ///   and would say "Positive". [`Rating::Positive`] is therefore unreachable in this window.
    /// - 95% over **371** reviews reads "Overwhelmingly Positive", where the all-time rule needs
    ///   500 and would say "Very Positive".
    ///
    /// Which makes sense: a thirty-day window rarely holds hundreds of reviews, and gating it
    /// like the all-time figure would collapse almost every game into one band.
    ///
    /// The top band's gate is **not pinned** — see [`RECENT_TOP_BAND_REVIEWS`] for the bracket
    /// it was measured into and why the value inside it was chosen. Every entry the fetch
    /// writes has its derived band checked against the one Steam printed, so a wrong value
    /// there fails a run rather than filling a table with verdicts Steam disagrees with.
    pub fn from_recent_score(approval: u8, count: u32) -> Self {
        Self::scan(approval, count, |band| match band.rating {
            Self::OverwhelminglyPositive | Self::OverwhelminglyNegative => RECENT_TOP_BAND_REVIEWS,
            _ => MINIMUM_REVIEWS,
        })
    }

    /// First band whose approval and review-count gates a score clears.
    fn scan(approval: u8, count: u32, gate: impl Fn(&Band) -> u32) -> Self {
        BANDS
            .iter()
            .find(|band| approval >= band.min_approval && count >= gate(band))
            .map_or(Self::TooFewReviews, |band| band.rating)
    }

    /// The wording Steam itself prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OverwhelminglyPositive => "Overwhelmingly Positive",
            Self::VeryPositive => "Very Positive",
            Self::Positive => "Positive",
            Self::MostlyPositive => "Mostly Positive",
            Self::Mixed => "Mixed",
            Self::MostlyNegative => "Mostly Negative",
            Self::Negative => "Negative",
            Self::VeryNegative => "Very Negative",
            Self::OverwhelminglyNegative => "Overwhelmingly Negative",
            Self::TooFewReviews => "Too Few Reviews",
        }
    }

    /// Reads a band back from Steam's wording. `None` for anything unrecognised, so a new band
    /// surfaces as a generator failure rather than as a silently wrong entry.
    pub fn from_steam(text: &str) -> Option<Self> {
        RATINGS
            .iter()
            .copied()
            .find(|rating| rating.as_str().eq_ignore_ascii_case(text.trim()))
    }

    /// Valve's own band index, from the `review_score` field of a review summary.
    ///
    /// Worth having even though [`from_score`](Self::from_score) can derive the band: it is
    /// Valve's answer rather than ours, which makes it the instrument for checking that the
    /// thresholds in [`BANDS`] still match what Steam actually does.
    ///
    /// Indices 1-3 (the three negative bands) are Valve's published numbering but have not been
    /// observed in the wild — negatively-reviewed games with enough reviews are rare — so they
    /// are inference, like the negative half of [`BANDS`].
    pub fn from_valve_score(score: u8) -> Option<Self> {
        Some(match score {
            0 => Self::TooFewReviews,
            1 => Self::OverwhelminglyNegative,
            2 => Self::VeryNegative,
            3 => Self::Negative,
            4 => Self::MostlyNegative,
            5 => Self::Mixed,
            6 => Self::MostlyPositive,
            7 => Self::Positive,
            8 => Self::VeryPositive,
            9 => Self::OverwhelminglyPositive,
            _ => return None,
        })
    }

    /// The colour Steam prints this verdict in, as a CSS hex value.
    ///
    /// **Three colours and a grey, not nine.** Steam does not shade the bands individually:
    /// "Positive", "Very Positive" and "Overwhelmingly Positive" are the same blue. Giving each
    /// band its own colour would be inventing a palette, not reproducing Steam's.
    ///
    /// Negative has no class of its own — Steam renders it with an empty modifier and lets it
    /// fall through to the base rule, which is why the orange below is the *unqualified*
    /// `.game_review_summary` colour.
    ///
    /// Taken from the app-page stylesheet (`store.css`) as observed on 2026-09-11. Valve
    /// publishes none of this: the values are read off a versioned stylesheet, they differ in
    /// other contexts (the listing bar renders Mixed as `#a8926a`), and nothing in the API
    /// exposes them. Treat them as a snapshot rather than a contract. They also assume Steam's
    /// dark background — there is no light variant, and this blue on white reads poorly.
    pub fn color(self) -> &'static str {
        match self {
            Self::OverwhelminglyPositive
            | Self::VeryPositive
            | Self::Positive
            | Self::MostlyPositive => "#66C0F4",
            Self::Mixed => "#B9A074",
            Self::MostlyNegative
            | Self::Negative
            | Self::VeryNegative
            | Self::OverwhelminglyNegative => "#c85e2d",
            Self::TooFewReviews => "#929396",
        }
    }

    /// A colour per band, for a display that wants to rank at a glance.
    ///
    /// Nine distinct shades where [`Rating::color`] gives Steam's three — because Steam's
    /// palette cannot tell "Positive" from "Overwhelmingly Positive", which is most of what a
    /// reader scanning a list wants to know. It keeps Steam's *shape*, though: blue above
    /// Mixed, Steam's own tan for Mixed, red below. Within each family the band nearest Mixed
    /// is the dullest and the extreme is the most vivid, so distance from the middle reads as
    /// intensity in both directions.
    ///
    /// Two of these are Steam's own values — `#66C0F4` for Very Positive, `#C85E2D` for
    /// Negative — with the rest ramped away from them. **This palette is ours, not Steam's**;
    /// [`Rating::color`] is the faithful one.
    pub fn gradient_color(self) -> &'static str {
        match self {
            Self::OverwhelminglyPositive => "#6FF0E8",
            Self::VeryPositive => "#66C0F4",
            Self::Positive => "#4A93C9",
            Self::MostlyPositive => "#2E6A96",
            Self::Mixed => "#B9A074",
            Self::MostlyNegative => "#B06A45",
            Self::Negative => "#C85E2D",
            Self::VeryNegative => "#DC4526",
            Self::OverwhelminglyNegative => "#F53030",
            Self::TooFewReviews => "#929396",
        }
    }

    /// Whether the band is a positive verdict — the three positive bands and no others.
    pub fn is_positive(self) -> bool {
        matches!(
            self,
            Self::OverwhelminglyPositive
                | Self::VeryPositive
                | Self::Positive
                | Self::MostlyPositive
        )
    }
}

/// Which operating systems a game ships for.
///
/// Steam's own three; there is no fourth, and a game with none of them set is one Steam has
/// delisted from every platform.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Os {
    pub windows: bool,
    pub mac: bool,
    pub linux: bool,
}

impl Os {
    /// Whether the game runs on any platform at all.
    pub fn any(self) -> bool {
        self.windows || self.mac || self.linux
    }

    /// Short labels for the platforms supported, in Steam's own order.
    pub fn labels(self) -> Vec<&'static str> {
        [
            (self.windows, "Windows"),
            (self.mac, "Mac"),
            (self.linux, "Linux"),
        ]
        .into_iter()
        .filter_map(|(supported, label)| supported.then_some(label))
        .collect()
    }
}

/// How a game relates to virtual reality.
///
/// Three states rather than a boolean: a VR-only title cannot be played on a monitor, which is
/// the difference between "I can play this" and "I cannot" for anyone without a headset.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Vr {
    /// No VR support.
    None,
    /// Playable either way.
    Supported,
    /// Requires a headset.
    Only,
}

impl Vr {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "no VR",
            Self::Supported => "VR supported",
            Self::Only => "VR only",
        }
    }
}

/// Valve's Steam Deck compatibility rating.
///
/// The four states Valve publishes. `Unknown` is the common case and means only that Valve has
/// not tested the title — never that it fails.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Deck {
    Unknown,
    Unsupported,
    Playable,
    Verified,
}

/// Every Deck state, paired with the number Valve's compatibility report uses.
///
/// The numbers are Valve's, confirmed against five titles: a VR-only game reports 1, a
/// keyboard-heavy strategy game 2, and fully controller-native games 3.
pub const DECK_STATES: &[(u8, Deck)] = &[
    (0, Deck::Unknown),
    (1, Deck::Unsupported),
    (2, Deck::Playable),
    (3, Deck::Verified),
];

impl Deck {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "Deck untested",
            Self::Unsupported => "Deck unsupported",
            Self::Playable => "Deck playable",
            Self::Verified => "Deck verified",
        }
    }

    /// Reads a state from Valve's compatibility category number. Anything unrecognised is
    /// `None`, so a new category surfaces as a generator failure rather than a wrong badge.
    pub fn from_category(category: u8) -> Option<Self> {
        DECK_STATES
            .iter()
            .find(|(n, _)| *n == category)
            .map(|(_, state)| *state)
    }
}

/// One review summary: how many reviews, and what share of them are positive.
///
/// The verdict Steam prints is *derived* from these two numbers by [`Reviews::rating`] rather
/// than stored beside them, so a stored band cannot drift out of step with the figures it is
/// supposed to summarise.
/// The same type serves both windows, but the windows are not symmetric in what Steam
/// publishes: the all-time figure comes from the reviews API with Valve's own integer band
/// beside it (used as a cross-check when fetched), while the thirty-day figure exists only on
/// the store page, as a percentage and a count with no integer band. Neither is stored here —
/// only the two numbers — which is what lets one type carry both. Pick the verdict for the
/// window you hold: [`Reviews::rating`] for all-time, [`Reviews::recent_rating`] for the last
/// thirty days.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reviews {
    /// Share of reviews that are positive, as a whole percent, **floored**.
    ///
    /// Floored rather than rounded: a title on 79.93% rounds to 80 and would cross into a
    /// higher band than Steam gives it. See the house rules in [`super`].
    pub approval: u8,
    /// How many reviews the percentage is computed over.
    pub count: u32,
}

impl Reviews {
    /// The verdict Steam prints for these figures over a game's whole history.
    ///
    /// For a thirty-day summary use [`Reviews::recent_rating`] — Steam gates the two windows
    /// differently and this one would under-report.
    pub fn rating(self) -> Rating {
        Rating::from_score(self.approval, self.count)
    }

    /// The verdict Steam prints for these figures over the last thirty days.
    pub fn recent_rating(self) -> Rating {
        Rating::from_recent_score(self.approval, self.count)
    }
}

/// One game on Steam.
///
/// `#[non_exhaustive]`: only the generated table constructs these, and the field list grows as
/// more of a store page turns out to be worth keeping. Marking it means that growth never
/// breaks a consumer, which construction outside this crate would otherwise make impossible.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub struct SteamGame {
    /// Steam's application id — the stable identity, and what `store.steampowered.com/app/<id>`
    /// takes.
    pub app_id: u32,
    /// Title as Steam's English store prints it, which is not always what a bundle calls it.
    pub name: &'static str,
    /// Other wordings this same product is sold under — typically edition names a store uses
    /// for what Steam sells as a bundle over this app-id. [`by_name`] matches these too.
    pub aliases: &'static [&'static str],
    /// Release date exactly as the English store renders it, e.g. `"May 17, 2016"`.
    ///
    /// Kept as Steam's own string rather than normalised to ISO-8601: the endpoint returns a
    /// localized display string with no machine-readable form behind it, and titles that have
    /// not shipped say things like `"Coming soon"` that no date type can hold.
    pub released: &'static str,
    /// The last 30 days as of the module's `captured` date. `None` when Steam showed no recent
    /// row — a game nobody has reviewed lately.
    pub recent: Option<Reviews>,
    /// Every review, all languages, bought-or-key-activated. See the house rules.
    pub all_time: Reviews,
    /// Player tags, most-voted first, as Steam orders them.
    pub tags: &'static [Tag],
    /// Operating systems the game ships for.
    pub os: Os,
    /// Whether a headset is optional, required, or irrelevant.
    pub vr: Vr,
    /// Valve's Steam Deck verdict.
    pub deck: Deck,
    /// What Steam lists the game as doing — single-player, co-op, achievements, cloud saves,
    /// controller support and the rest of the store page's feature column, in Steam's order.
    pub features: &'static [&'static str],
}

/// One category of games, and when its entries were captured.
///
/// Modules are listed in [`MODULES`] rather than discovered: that table is the single
/// registration site, so a module nobody added there cannot be silently absent from [`all`].
pub struct Module {
    /// Category name, matching the module's own name.
    pub name: &'static str,
    /// ISO-8601 date the entries were generated. A lower bound on freshness, not a claim that
    /// nothing has changed since.
    pub captured: &'static str,
    pub games: &'static [SteamGame],
}

/// Every category in this inventory. Adding a module means adding a row here.
pub const MODULES: &[Module] = &[Module {
    name: "regular",
    captured: regular::CAPTURED,
    games: regular::REGULAR,
}];

/// Every game in every category.
pub fn all() -> impl Iterator<Item = &'static SteamGame> {
    MODULES.iter().flat_map(|module| module.games.iter())
}

impl Tag {
    /// The tag with this Steam id.
    pub fn from_id(id: u32) -> Option<Self> {
        tags::TAGS.iter().copied().find(|tag| tag.id() == id)
    }

    /// The tag Steam displays under this name, compared case-insensitively.
    ///
    /// `None` for anything unrecognised — which is how a tag Valve has added since the enum was
    /// generated surfaces, rather than being silently dropped from an entry.
    /// Both sides are trimmed: Valve's own tag list contains names with trailing whitespace
    /// (`"Dystopian "`, tag 5030), so trimming only the caller's string would fail to match the
    /// very tags that need it.
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim();
        tags::TAGS
            .iter()
            .copied()
            .find(|tag| tag.as_str().trim().eq_ignore_ascii_case(name))
    }
}

/// Reduces a title to what is worth comparing across stores.
///
/// Case, spacing and punctuation all differ between what a bundle prints and what Steam prints
/// for the same game, and none of those differences mean a different game. Public because the
/// generator matches with it too: a second normalisation that merely *ought* to agree with this
/// one would eventually disagree, and it would disagree exactly where it decides which app-id a
/// title gets.
pub fn comparable(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The inventory entry a store's game refers to.
///
/// The app-id is tried first and the title only as a fallback, because they are not equally
/// trustworthy: an id is an identity, while a title is a guess that once resolved "Ashen" to
/// "Ashen Empires" — a different game that every automatic check accepted.
///
/// **An id the store published is authoritative, and there is deliberately no fallback from it.**
/// Falling through to the title when the inventory happens not to hold that id would reintroduce
/// exactly the error the id exists to prevent: a near-miss title silently attaching the wrong
/// entry.
///
/// One place, because this is the join every part of the program makes — what a store is selling,
/// against what Steam knows about it. A second traversal would eventually resolve the same game
/// to a different entry than the reviews and the store link on its own line already used.
#[must_use]
pub fn of_game(game: &crate::Game) -> Option<&'static SteamGame> {
    match game.steam_app_id {
        Some(app_id) => by_app_id(app_id),
        None => by_name(&game.title),
    }
}

/// Every name this entry goes by: its own first, then its aliases.
///
/// One place, because more than one caller needs the same set — [`by_name`] matches a store's
/// wording against it, and a cross-store lookup has to try each spelling another store might
/// print. Two traversals of the same two fields would eventually visit different ones.
pub fn names(entry: &SteamGame) -> impl Iterator<Item = &'static str> {
    // Both fields are already `'static`, so they are copied out before the iterator is built:
    // that way the entry only has to be borrowed long enough to read them, and a caller holding
    // a shorter-lived reference to a table entry can still ask.
    let (name, aliases) = (entry.name, entry.aliases);
    std::iter::once(name).chain(aliases.iter().copied())
}

/// The game a store's wording refers to, matched on the entry's name or any of its aliases.
///
/// `None` when nothing matches — which is the common case for a store selling something that is
/// not on Steam at all.
pub fn by_name(title: &str) -> Option<&'static SteamGame> {
    let wanted = comparable(title);
    all().find(|game| {
        comparable(game.name) == wanted
            || game.aliases.iter().any(|alias| comparable(alias) == wanted)
    })
}

/// The game with this app-id, in whichever category holds it.
pub fn by_app_id(app_id: u32) -> Option<&'static SteamGame> {
    all().find(|game| game.app_id == app_id)
}

/// Every game carrying `tag`.
pub fn tagged(tag: Tag) -> impl Iterator<Item = &'static SteamGame> {
    all().filter(move |game| game.tags.contains(&tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The house rules, held mechanically: nothing empty, no app-id twice, every tag list
    /// populated, every count consistent with its band.
    #[test]
    fn every_entry_is_well_formed_and_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for module in MODULES {
            for game in module.games {
                let id = game.app_id;
                assert!(id > 0, "{} has no app-id", game.name);
                assert!(!game.name.is_empty(), "{id} has no name");
                assert!(!game.released.is_empty(), "{id} has no release date");
                assert!(seen.insert(id), "app-id {id} is listed twice");
                assert!(
                    !game.tags.is_empty(),
                    "{id} ({}) has no tags — an age-gated page parses this way, \
                     so an empty list is a scrape failure, not a fact",
                    game.name
                );
                let mut seen_tags = std::collections::BTreeSet::new();
                for tag in game.tags {
                    assert!(seen_tags.insert(tag), "{id} lists {tag:?} twice");
                }
            }
        }
    }

    /// A band of `TooFewReviews` and a large count contradict each other; so does any other
    /// band with no reviews behind it.
    #[test]
    fn every_rating_agrees_with_its_count() {
        for game in all() {
            for (which, reviews) in [("all-time", Some(game.all_time)), ("recent", game.recent)] {
                let Some(reviews) = reviews else { continue };
                if reviews.rating() == Rating::TooFewReviews {
                    assert!(
                        reviews.count < 10,
                        "{}: {which} says too few reviews but counts {}",
                        game.name,
                        reviews.count
                    );
                } else {
                    assert!(
                        reviews.count > 0,
                        "{}: {which} band with no reviews",
                        game.name
                    );
                }
            }
        }
    }

    /// Recent reviews are a subset of all reviews — a 30-day window cannot exceed all time.
    #[test]
    fn recent_reviews_never_outnumber_all_reviews() {
        for game in all() {
            if let Some(recent) = game.recent {
                assert!(
                    recent.count <= game.all_time.count,
                    "{}: {} recent exceeds {} all-time",
                    game.name,
                    recent.count,
                    game.all_time.count
                );
            }
        }
    }

    /// Entries are sorted by app-id so regeneration produces a reviewable diff rather than a
    /// reshuffled file.
    #[test]
    fn every_module_is_sorted_by_app_id() {
        for module in MODULES {
            let ids: Vec<u32> = module.games.iter().map(|game| game.app_id).collect();
            let mut sorted = ids.clone();
            sorted.sort_unstable();
            assert_eq!(ids, sorted, "{} is not sorted by app-id", module.name);
        }
    }

    /// Every module states when it was captured, in a shape a reader can compare.
    #[test]
    fn every_module_records_an_iso_capture_date() {
        for module in MODULES {
            let date = module.captured;
            assert_eq!(
                date.len(),
                10,
                "{}: {date:?} is not YYYY-MM-DD",
                module.name
            );
            let parts: Vec<&str> = date.split('-').collect();
            assert!(
                parts.len() == 3 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())),
                "{}: {date:?} is not YYYY-MM-DD",
                module.name
            );
        }
    }

    #[test]
    fn a_modules_name_matches_the_module_it_lists() {
        // MODULES is the one registration site; a row naming a module it does not carry would
        // make `all()` quietly wrong.
        assert!(MODULES.iter().any(|m| m.name == "regular"));
    }

    /// Every band round-trips through Steam's own wording.
    #[test]
    fn every_rating_reads_back_from_its_steam_wording() {
        for &rating in RATINGS {
            assert_eq!(Rating::from_steam(rating.as_str()), Some(rating));
        }
        assert_eq!(
            Rating::from_steam("very positive"),
            Some(Rating::VeryPositive)
        );
        assert_eq!(Rating::from_steam("  Mixed  "), Some(Rating::Mixed));
    }

    /// Wording Steam does not use must not resolve to a band.
    #[test]
    fn unknown_wording_is_rejected_rather_than_guessed() {
        assert_eq!(Rating::from_steam("Somewhat Positive"), None);
        assert_eq!(Rating::from_steam(""), None);
        assert_eq!(Rating::from_steam("Positive-ish"), None);
    }

    #[test]
    fn ratings_lists_every_variant_exactly_once() {
        let mut seen = std::collections::BTreeSet::new();
        for rating in RATINGS {
            assert!(seen.insert(rating.as_str()), "{rating:?} listed twice");
        }
        assert_eq!(seen.len(), RATINGS.len());
    }

    /// Compatibility fields must describe a real product: something Steam sells runs somewhere.
    #[test]
    fn every_game_runs_on_at_least_one_operating_system() {
        for game in all() {
            assert!(
                game.os.any(),
                "{} ({}) ships for no platform",
                game.name,
                game.app_id
            );
        }
    }

    /// An alias exists to catch a *different* wording; one equal to the name is dead weight, and
    /// one shared between entries would make `by_name` return whichever came first.
    #[test]
    fn aliases_are_distinct_from_their_name_and_from_each_other() {
        let mut seen = std::collections::BTreeSet::new();
        for game in all() {
            seen.insert(comparable(game.name));
        }
        for game in all() {
            for alias in game.aliases {
                assert!(!alias.trim().is_empty(), "{} has a blank alias", game.name);
                assert_ne!(
                    comparable(alias),
                    comparable(game.name),
                    "{}: alias {alias:?} only repeats the name",
                    game.name
                );
                assert!(
                    !seen.contains(&comparable(alias)),
                    "{}: alias {alias:?} is another entry's name",
                    game.name
                );
            }
        }
    }

    /// Every name and alias in the whole table, taken together, must be unique.
    ///
    /// [`by_name`] returns the first row that matches, so a wording claimed by two rows would
    /// silently attach a bundle's game to whichever happened to come first — and which one that
    /// is would change with the sort order.
    #[test]
    fn no_wording_is_claimed_by_two_entries() {
        let mut claimed: std::collections::BTreeMap<String, u32> =
            std::collections::BTreeMap::new();
        for game in all() {
            for wording in std::iter::once(game.name).chain(game.aliases.iter().copied()) {
                if let Some(other) = claimed.insert(comparable(wording), game.app_id) {
                    assert_eq!(
                        other, game.app_id,
                        "{wording:?} is claimed by both app {other} and app {}",
                        game.app_id
                    );
                }
            }
        }
    }

    /// Every entry must find itself through the same lookup a caller uses.
    ///
    /// This is the test that catches a normalisation change: the table is generated, and if the
    /// shared [`comparable`] ever stops agreeing with what the generator matched on, rows go on
    /// looking perfectly well-formed while silently becoming unreachable.
    #[test]
    fn every_entry_finds_its_own_row() {
        for game in all() {
            assert_eq!(
                by_name(game.name).map(|found| found.app_id),
                Some(game.app_id),
                "{:?} (app {}) cannot be found by its own name",
                game.name,
                game.app_id
            );
            for alias in game.aliases {
                assert_eq!(
                    by_name(alias).map(|found| found.app_id),
                    Some(game.app_id),
                    "{:?} cannot be found by its own alias {alias:?}",
                    game.name
                );
            }
        }
    }

    /// The count is load-bearing, not a tie-break.
    #[test]
    fn the_top_band_needs_reviews_as_well_as_approval() {
        // Measured: a real title on 99% with 201 reviews is "Very Positive"; another on 95%
        // with 12,375 is "Overwhelmingly Positive". Percentage alone cannot tell them apart.
        assert_eq!(Rating::from_score(99, 201), Rating::VeryPositive);
        assert_eq!(
            Rating::from_score(95, 12_375),
            Rating::OverwhelminglyPositive
        );
        assert_eq!(Rating::from_score(95, 499), Rating::VeryPositive);
        assert_eq!(Rating::from_score(95, 500), Rating::OverwhelminglyPositive);
    }

    #[test]
    fn the_same_approval_reads_differently_at_different_review_counts() {
        assert_eq!(Rating::from_score(85, 9), Rating::TooFewReviews);
        assert_eq!(Rating::from_score(85, 10), Rating::Positive);
        assert_eq!(Rating::from_score(85, 49), Rating::Positive);
        assert_eq!(Rating::from_score(85, 50), Rating::VeryPositive);
    }

    #[test]
    fn each_positive_band_starts_exactly_where_steam_says() {
        // Boundaries confirmed against real titles: 79.93% is Mostly Positive and 80.25% is
        // Very Positive; 68.83% is Mixed and 70.48% is Mostly Positive.
        assert_eq!(Rating::from_score(79, 1_000), Rating::MostlyPositive);
        assert_eq!(Rating::from_score(80, 1_000), Rating::VeryPositive);
        assert_eq!(Rating::from_score(69, 1_000), Rating::Mixed);
        assert_eq!(Rating::from_score(70, 1_000), Rating::MostlyPositive);
    }

    #[test]
    fn the_negative_bands_mirror_the_positive_ones() {
        assert_eq!(Rating::from_score(39, 1_000), Rating::MostlyNegative);
        assert_eq!(
            Rating::from_score(19, 1_000),
            Rating::OverwhelminglyNegative
        );
        assert_eq!(Rating::from_score(19, 100), Rating::VeryNegative);
        assert_eq!(Rating::from_score(19, 10), Rating::Negative);
        assert_eq!(Rating::from_score(0, 10), Rating::Negative);
    }

    #[test]
    fn too_few_reviews_is_the_answer_below_ten_whatever_the_score() {
        for approval in [0, 50, 100] {
            for count in [0, 1, 9] {
                assert_eq!(
                    Rating::from_score(approval, count),
                    Rating::TooFewReviews,
                    "{approval}% over {count} reviews"
                );
            }
        }
    }

    #[test]
    fn the_band_table_is_ordered_so_the_first_match_is_the_best_one() {
        // Rows are tried top to bottom, so a later row must never be reachable before an
        // earlier one that also matches — which means approval thresholds never increase.
        let mut previous = u8::MAX;
        for band in BANDS {
            assert!(
                band.min_approval <= previous,
                "{:?} raises the approval threshold",
                band.rating
            );
            previous = band.min_approval;
        }
    }

    #[test]
    fn every_band_in_the_table_is_reachable() {
        // A row shadowed by an earlier one would be dead weight that nothing could produce.
        for band in BANDS {
            assert_eq!(
                Rating::from_score(band.min_approval, band.min_reviews),
                band.rating,
                "{:?} is shadowed by an earlier row",
                band.rating
            );
        }
    }

    #[test]
    fn a_stored_percentage_reproduces_the_verdict_it_came_from() {
        let reviews = Reviews {
            approval: 89,
            count: 21_898,
        };
        assert_eq!(reviews.rating(), Rating::VeryPositive);
        assert_eq!(reviews.rating().as_str(), "Very Positive");
    }

    #[test]
    fn every_tag_in_the_table_round_trips_through_its_id_and_name() {
        for game in all() {
            for tag in game.tags {
                assert_eq!(Tag::from_id(tag.id()), Some(*tag));
                assert_eq!(Tag::from_name(tag.as_str()), Some(*tag));
            }
        }
    }

    /// Valve's own band index must agree with the table we derive from.
    /// The thirty-day window is gated differently, and these are the measured cases.
    #[test]
    fn the_recent_window_reaches_higher_bands_on_fewer_reviews() {
        // All-time would call both of these something lower.
        assert_eq!(Rating::from_recent_score(80, 10), Rating::VeryPositive);
        assert_eq!(Rating::from_score(80, 10), Rating::Positive);

        assert_eq!(
            Rating::from_recent_score(95, 371),
            Rating::OverwhelminglyPositive
        );
        assert_eq!(Rating::from_score(95, 371), Rating::VeryPositive);
    }

    #[test]
    fn the_recent_window_still_gates_its_top_band() {
        // Real rows, both sides of the bracket: 95%+ on 85 reviews is Very Positive, on 291 is
        // Overwhelmingly Positive. These are the two observations the threshold sits between.
        assert_eq!(Rating::from_recent_score(100, 16), Rating::VeryPositive);
        assert_eq!(Rating::from_recent_score(96, 85), Rating::VeryPositive);
        assert_eq!(
            Rating::from_recent_score(95, 291),
            Rating::OverwhelminglyPositive
        );
    }

    #[test]
    fn plain_positive_cannot_occur_in_the_recent_window() {
        // It sits below Very Positive at the same approval and is reached only by the all-time
        // review-count gate, which this window does not apply.
        for approval in 80..=100 {
            for count in [10, 49, 50, 100] {
                assert_ne!(
                    Rating::from_recent_score(approval, count),
                    Rating::Positive,
                    "{approval}% over {count}"
                );
            }
        }
    }

    #[test]
    fn both_windows_agree_below_ten_reviews() {
        assert_eq!(Rating::from_recent_score(100, 9), Rating::TooFewReviews);
        assert_eq!(Rating::from_score(100, 9), Rating::TooFewReviews);
    }

    #[test]
    fn both_windows_share_the_percentage_boundaries() {
        // Only the count gates differ; the bands themselves are the same.
        for approval in [0, 19, 20, 39, 40, 69, 70, 79] {
            assert_eq!(
                Rating::from_recent_score(approval, 1_000),
                Rating::from_score(approval, 1_000),
                "{approval}%"
            );
        }
    }

    #[test]
    fn valve_band_indices_map_onto_the_bands_we_derive() {
        assert_eq!(
            Rating::from_valve_score(9),
            Some(Rating::OverwhelminglyPositive)
        );
        assert_eq!(Rating::from_valve_score(8), Some(Rating::VeryPositive));
        assert_eq!(Rating::from_valve_score(7), Some(Rating::Positive));
        assert_eq!(Rating::from_valve_score(6), Some(Rating::MostlyPositive));
        assert_eq!(Rating::from_valve_score(5), Some(Rating::Mixed));
        assert_eq!(Rating::from_valve_score(4), Some(Rating::MostlyNegative));
        assert_eq!(Rating::from_valve_score(0), Some(Rating::TooFewReviews));
        // An index Valve does not publish must not become a band.
        assert_eq!(Rating::from_valve_score(10), None);
        assert_eq!(Rating::from_valve_score(255), None);
    }

    #[test]
    fn every_band_has_a_colour_and_the_positive_ones_share_it() {
        // Steam does not shade the bands individually; giving each its own colour would be
        // inventing a palette rather than reproducing one.
        let blue = Rating::VeryPositive.color();
        for band in [
            Rating::OverwhelminglyPositive,
            Rating::VeryPositive,
            Rating::Positive,
            Rating::MostlyPositive,
        ] {
            assert_eq!(
                band.color(),
                blue,
                "{band:?} should share the positive colour"
            );
        }
        for band in [
            Rating::MostlyNegative,
            Rating::Negative,
            Rating::VeryNegative,
            Rating::OverwhelminglyNegative,
        ] {
            assert_eq!(band.color(), Rating::Negative.color());
        }
        assert_ne!(Rating::Mixed.color(), blue);
        assert_ne!(Rating::Mixed.color(), Rating::Negative.color());
    }

    #[test]
    fn every_colour_is_a_css_hex_value() {
        for &rating in RATINGS {
            for colour in [rating.color(), rating.gradient_color()] {
                assert!(colour.starts_with('#'), "{rating:?}: {colour}");
                assert_eq!(colour.len(), 7, "{rating:?}: {colour}");
                assert!(
                    colour[1..].chars().all(|c| c.is_ascii_hexdigit()),
                    "{rating:?}: {colour}"
                );
            }
        }
    }

    #[test]
    fn the_gradient_gives_every_band_its_own_shade() {
        // The whole point of it: Steam's palette cannot tell Positive from Overwhelmingly
        // Positive, and a reader scanning a list wants exactly that distinction.
        let mut seen = std::collections::BTreeSet::new();
        for &rating in RATINGS {
            assert!(
                seen.insert(rating.gradient_color()),
                "{rating:?} repeats a shade"
            );
        }
        assert_eq!(seen.len(), RATINGS.len());
    }

    #[test]
    fn the_gradient_keeps_steams_shape_blue_above_mixed_and_red_below() {
        // Blue is blue-dominant, red is red-dominant; Mixed is Steam's own tan. Checked on the
        // channels rather than by eye, so a future edit cannot quietly cross the families.
        let channels = |hex: &str| {
            (
                u8::from_str_radix(&hex[1..3], 16).unwrap(),
                u8::from_str_radix(&hex[3..5], 16).unwrap(),
                u8::from_str_radix(&hex[5..7], 16).unwrap(),
            )
        };
        for band in [
            Rating::OverwhelminglyPositive,
            Rating::VeryPositive,
            Rating::Positive,
            Rating::MostlyPositive,
        ] {
            let (r, _, b) = channels(band.gradient_color());
            assert!(b > r, "{band:?} is not blue-dominant");
        }
        for band in [
            Rating::MostlyNegative,
            Rating::Negative,
            Rating::VeryNegative,
            Rating::OverwhelminglyNegative,
        ] {
            let (r, _, b) = channels(band.gradient_color());
            assert!(r > b, "{band:?} is not red-dominant");
        }
        assert_eq!(
            Rating::Mixed.gradient_color(),
            Rating::Mixed.color(),
            "Mixed is Steam's tan"
        );
    }

    #[test]
    fn the_extremes_are_more_vivid_than_the_bands_beside_mixed() {
        // Distance from the middle reads as intensity, in both directions.
        //
        // Measured as chroma — the spread between the strongest and weakest channel — not as
        // lightness. A vivid red is high in one channel and low in the others, so summing
        // channels ranks a muted brown *above* it and would pass this test while the colours
        // said the opposite.
        let chroma = |hex: &str| {
            let channels: Vec<u32> = (1..7)
                .step_by(2)
                .map(|i| u32::from(u8::from_str_radix(&hex[i..i + 2], 16).unwrap()))
                .collect();
            channels.iter().max().unwrap() - channels.iter().min().unwrap()
        };
        assert!(
            chroma(Rating::OverwhelminglyPositive.gradient_color())
                > chroma(Rating::MostlyPositive.gradient_color()),
            "the top positive band should be more vivid than the one beside Mixed"
        );
        assert!(
            chroma(Rating::OverwhelminglyNegative.gradient_color())
                > chroma(Rating::MostlyNegative.gradient_color()),
            "the bottom negative band should be more vivid than the one beside Mixed"
        );
    }

    #[test]
    fn every_deck_category_valve_publishes_maps_to_a_state() {
        for &(number, state) in DECK_STATES {
            assert_eq!(Deck::from_category(number), Some(state));
        }
        // A category Valve does not use must not silently become a badge.
        assert_eq!(Deck::from_category(4), None);
        assert_eq!(Deck::from_category(255), None);
    }

    #[test]
    fn deck_states_are_listed_once_each_and_ordered_by_category() {
        let numbers: Vec<u8> = DECK_STATES.iter().map(|(n, _)| *n).collect();
        let mut sorted = numbers.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(numbers, sorted);
    }

    #[test]
    fn platform_labels_name_only_supported_systems() {
        let all_three = Os {
            windows: true,
            mac: true,
            linux: true,
        };
        assert_eq!(all_three.labels(), ["Windows", "Mac", "Linux"]);

        let windows_only = Os {
            windows: true,
            mac: false,
            linux: false,
        };
        assert_eq!(windows_only.labels(), ["Windows"]);
        assert!(windows_only.any());

        let nothing = Os {
            windows: false,
            mac: false,
            linux: false,
        };
        assert!(nothing.labels().is_empty());
        assert!(!nothing.any());
    }

    #[test]
    fn compatibility_states_all_describe_themselves() {
        for state in [Vr::None, Vr::Supported, Vr::Only] {
            assert!(!state.as_str().is_empty());
        }
        for &(_, state) in DECK_STATES {
            assert!(!state.as_str().is_empty());
        }
        // The three VR states must read differently, or the distinction is invisible.
        assert_ne!(Vr::Supported.as_str(), Vr::Only.as_str());
    }

    #[test]
    fn titles_compare_across_case_spacing_and_punctuation() {
        assert_eq!(
            comparable("Salt and Sanctuary"),
            comparable("salt  and  sanctuary")
        );
        assert_eq!(
            comparable("Warhammer 40,000: Rogue Trader"),
            comparable("Warhammer 40000 Rogue Trader")
        );
        // Neighbouring titles in a series must not collapse into each other.
        assert_ne!(comparable("Torchlight II"), comparable("Torchlight III"));
        assert_ne!(
            comparable("Salt and Sanctuary"),
            comparable("Salt and Sacrifice")
        );
    }

    #[test]
    fn a_game_is_found_by_its_name_however_it_is_written() {
        let Some(game) = all().next() else { return };
        assert_eq!(by_name(game.name).map(|g| g.app_id), Some(game.app_id));
        assert_eq!(
            by_name(&game.name.to_uppercase()).map(|g| g.app_id),
            Some(game.app_id)
        );
        assert_eq!(
            by_name(&game.name.to_lowercase()).map(|g| g.app_id),
            Some(game.app_id)
        );
    }

    #[test]
    fn a_game_is_found_by_any_alias_it_records() {
        let Some(game) = all().find(|g| !g.aliases.is_empty()) else {
            return;
        };
        for alias in game.aliases {
            assert_eq!(
                by_name(alias).map(|g| g.app_id),
                Some(game.app_id),
                "alias {alias:?} did not find {}",
                game.name
            );
        }
    }

    #[test]
    fn a_title_the_table_does_not_hold_is_not_guessed_at() {
        assert!(by_name("Some Game That Is Not In This Table At All").is_none());
        assert!(by_name("").is_none());
    }

    #[test]
    fn lookups_find_what_the_table_holds() {
        let Some(first) = all().next() else { return };
        assert_eq!(by_app_id(first.app_id).map(|g| g.name), Some(first.name));
        assert_eq!(by_app_id(0), None);

        let tag = first.tags[0];
        assert!(tagged(tag).any(|g| g.app_id == first.app_id));
    }
}
