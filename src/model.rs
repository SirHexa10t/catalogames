//! Vendor-agnostic types. Nothing here mentions a specific store.

use crate::Error;
use crate::clock::Timestamp;

/// One redeemable entry in a bundle.
///
/// Named `Game` because that is what the stores advertise, though some entries are software,
/// ebooks or multi-game packs. Nothing is filtered out on that basis here: what counts as a
/// game is a question about a particular store's catalogue, so each store's module answers it
/// and this type carries whatever they decide.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Game {
    /// Title as the vendor presents it, e.g. `"Salt and Sanctuary"`.
    pub title: String,
    /// The vendor's stable identifier, e.g. `"salt_and_sanctuary"`. Useful for
    /// matching the same game across stores; vendors rename display titles far
    /// more often than they rename these.
    pub machine_name: String,
    /// Steam application id, where the store publishes one.
    ///
    /// Steam-specific on a store-agnostic type, deliberately. An app id is the nearest thing
    /// PC games have to a shared primary key, and stores treat it as one — Fanatical's own API
    /// accepts it as a *lookup* parameter, not merely as a field it returns. It is identity
    /// rather than vendor trivia, and it is the join key into [`crate::inventory::steam`].
    ///
    /// It earns its place by deleting a class of error: matching a bundle's wording to a game
    /// by NAME once resolved "Ashen" to "Ashen Empires" — a different game, which every
    /// automatic check accepted. An id cannot go wrong that way.
    ///
    /// It can go wrong another way, and this field records what the store SAID rather than what
    /// is true: Fanatical puts Steam bundle ids here, and a bundle id is not an app id. So it is
    /// verified at the point of use — [`crate::inventory::steam::app_id_of`] confirms it against
    /// the tables' own name for it — and never reshaped here, because the store's word is data
    /// and the verdict on it belongs to the reader.
    ///
    /// `None` where the store publishes none: Humble publishes no Steam ids at all, and even a
    /// store that does leaves it empty for multi-game packs that are not one Steam product.
    pub steam_app_id: Option<u32>,

    /// The games this entry actually delivers, when the store sells several under one name.
    ///
    /// Empty for an ordinary game, which is almost every entry. A store sometimes lists a
    /// marketing wrapper — "Lazy Otter Double Pack" — that is not a product anyone can look up:
    /// it has no store page of its own, and searching for the phrase finds nothing. What it
    /// delivers is two real games, each with its own page, and those go here.
    ///
    /// A pack keeps its own `title`, because that is the name the bundle sells it under and the
    /// name a buyer will see in their cart. What it does not keep is a link, since there is
    /// nothing to link to.
    pub contains: Vec<Game>,
}

/// An amount of money, held as hundredths of its currency's major unit.
///
/// An integer rather than a decimal, because a price is exact and a binary float is not: `6.99`
/// has no exact `f64`, and a per-game rate is a division whose result gets compared and printed.
/// The one place a decimal enters is [`Money::from_major`], which the stores that publish
/// decimals go through.
///
/// **Hundredths, not ISO 4217 minor units, and the difference is measured.** Fanatical stores
/// every currency at two decimals whatever that currency's real exponent is: a $6.99 tier is
/// `{"USD": 699, "JPY": 121100}`, and yen have no minor unit at all. Read as ISO that is
/// ¥121,100 for a seven-dollar bundle — seventeen thousand yen to the dollar, absurd — while
/// read as hundredths it is ¥1,211.00, about 173 to the dollar, which is right. A type claiming
/// ISO minor units would render yen a hundred times too large, plausibly, with nothing to catch
/// it. This one claims only what it holds.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Money {
    /// Hundredths of the major unit: cents for USD, pence for GBP.
    pub hundredths: u32,
    /// ISO 4217 code, as the store stated it.
    pub currency: String,
}

impl Money {
    /// Builds an amount from hundredths of the major unit.
    #[must_use]
    pub fn new(hundredths: u32, currency: impl Into<String>) -> Self {
        Self {
            hundredths,
            currency: currency.into(),
        }
    }

    /// Builds an amount from a decimal a store published, e.g. `10.0` for ten dollars.
    ///
    /// Rounded rather than truncated, which is the classic way to lose a cent: `4.35 * 100.0`
    /// is `434.99999999999994` in IEEE 754, and truncating it yields $4.34. Many prices survive
    /// truncation — `7.99`, `5.99` and `23.69` all multiply exactly — which is what makes a test
    /// built only from real prices prove nothing.
    #[must_use]
    pub fn from_major(amount: f64, currency: impl Into<String>) -> Option<Self> {
        Self::from_hundredths(amount * 100.0, currency)
    }

    /// Builds an amount from a count of hundredths a store published as a number.
    ///
    /// Rounded, because the count is not always whole on the wire. Fanatical's catalogue
    /// carries values such as `204.99999999999997` and `805.0000000000001` — 68 of the 861 tier
    /// prices in one capture — which are whole prices that have been through a floating-point
    /// conversion somewhere upstream. The largest distance from a whole number across that
    /// capture was 9e-13, so rounding reads them as the prices they are; truncating would take
    /// a cent off sixty-eight of them.
    ///
    /// `None` for anything that is not a real, non-negative amount. A store that starts
    /// publishing a null, a negative or a NaN should lose its price rather than gain a wrong
    /// one.
    #[must_use]
    pub fn from_hundredths(units: f64, currency: impl Into<String>) -> Option<Self> {
        let rounded = units.round();
        (units.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&rounded))
            .then(|| Self::new(rounded as u32, currency))
    }

    /// This amount split between `count` items, rounded to the nearest minor unit.
    ///
    /// `None` for a count of zero, which is the only way this can fail and is what a store
    /// publishing a tier that grants no games would look like.
    #[must_use]
    pub fn each_of(&self, count: u32) -> Option<Self> {
        (count > 0).then(|| {
            // Rounded half-up in integers: 2369 over 25 is 94.76, which must print as 0.95 and
            // not 0.94. Truncating would understate every per-game rate by up to a cent.
            let each = (u64::from(self.hundredths) + u64::from(count) / 2) / u64::from(count);
            Self::new(each as u32, self.currency.clone())
        })
    }
}

/// Symbols for currencies whose two-decimal encoding has been checked against real data.
///
/// A table rather than a general formatter: the alternative is a locale library for a handful of
/// codes, and a code nobody here has seen is better printed as its code than guessed at.
///
/// **JPY is deliberately absent.** Fanatical quotes it at the same two decimals as everything
/// else, so the magnitude this type holds is right, but yen have no subunit — printing `¥1211.00`
/// would attach a familiar symbol to a precision the currency does not have. Without a symbol it
/// renders `1211.00 JPY`, where the code makes plain that the decimals are this crate's
/// arithmetic and not a claim about yen. A currency joins this table when someone has checked
/// what a store actually encodes for it.
const SYMBOLS: &[(&str, &str)] = &[("USD", "$"), ("GBP", "£"), ("EUR", "€")];

impl std::fmt::Display for Money {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let major = self.hundredths / 100;
        let minor = self.hundredths % 100;
        match SYMBOLS
            .iter()
            .find(|(code, _)| *code == self.currency)
            .map(|(_, symbol)| *symbol)
        {
            Some(symbol) => write!(f, "{symbol}{major}.{minor:02}"),
            None => write!(f, "{major}.{minor:02} {}", self.currency),
        }
    }
}

/// What the games in a bundle cost, in whichever way the store sells them.
///
/// Which shape applies is each store's own question, because tiers do not mean the same thing
/// in both. A Humble tier is a **cumulative partition**: paying the largest tier's price buys
/// all of its games together, and dividing that by the game count would produce an average
/// nobody can actually pay. A Fanatical pick-and-mix tier is a **price point over one shared
/// pool**: paying for twenty-five picks really does cost the stated amount per pick. So a whole
/// price and a per-game rate are different offers, not two renderings of one, and each vendor's
/// parser decides which its own data describes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Price {
    /// One payment unlocks every game listed.
    Whole(Money),
    /// A pick-and-mix ladder — see [`Ladder`] for the shape, and for the one vendor it fits.
    PerGame(Ladder),
}

/// One rung of a pick-and-mix ladder: this many picks, for this total.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tier {
    /// How many games this rung lets the buyer take.
    pub games: u32,
    /// What all of them cost together — the amount PAID, never a retail worth.
    pub total: Money,
}

/// The price points of a pick-and-mix bundle, over one shared pool of games.
///
/// **Fanatical-shaped by construction, and said so because the next reader will reach for it for
/// Humble.** A Fanatical tier is a price point: `{quantity: 5, price: 14.99}` means any five games
/// from the pool for that total, so a rung's rate per pick is a rate a buyer can actually pay. A
/// Humble tier is a cumulative PARTITION — paying the top price buys all of its games together —
/// and a ladder of partitions has no "next N games cost this much" reading at all. Humble prices
/// are [`Price::Whole`]; nothing here describes them.
///
/// Three invariants, enforced at construction and on the way in from a saved capture, so that no
/// consumer has to check them and no position can lie:
///
/// * **never empty** — a ladder with no rung says nothing, and `top` could not answer;
/// * **ascending by count, with no count twice** — sorted here rather than trusted, because the
///   one assumption this project has already been bitten by is a vendor's tier order: Humble's
///   arrived DESCENDING, and "largest" had to be found by count rather than by position;
/// * **ascending by total** — more picks for less money is not a ladder, and the marginal rate
///   between two rungs would come out below nothing.
///
/// `each` and `top` are derived rather than stored, so the rate on the folded line and the rungs
/// behind it can never disagree. `each` is a DISPLAY figure: 2,369 cents over 25 picks rounds to
/// 95, and 95 × 25 is 2,375, so anything that needs the sum must read the rungs, which are the
/// truth.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "Vec<Tier>", into = "Vec<Tier>")]
pub struct Ladder {
    tiers: Vec<Tier>,
}

impl Ladder {
    /// The ladder these rungs form, sorted; `None` when they form none — see the invariants.
    #[must_use]
    pub fn new(mut tiers: Vec<Tier>) -> Option<Self> {
        tiers.sort_by_key(|tier| tier.games);
        let sound = !tiers.is_empty()
            && tiers.iter().all(|tier| tier.games > 0)
            && tiers.windows(2).all(|pair| {
                pair[0].games < pair[1].games
                    && pair[0].total.hundredths < pair[1].total.hundredths
                    && pair[0].total.currency == pair[1].total.currency
            });
        sound.then_some(Self { tiers })
    }

    /// Every rung, fewest picks first.
    #[must_use]
    pub fn tiers(&self) -> &[Tier] {
        &self.tiers
    }

    /// The rung with the most picks — the best rate on offer, and what the folded line shows.
    #[must_use]
    pub fn top(&self) -> &Tier {
        // Non-empty by construction; the constructor is the only way in.
        self.tiers.last().expect("a ladder has at least one rung")
    }

    /// The rate per pick at the top rung, rounded to the cent. A display figure: see the type.
    #[must_use]
    pub fn each(&self) -> Money {
        let top = self.top();
        top.total
            .each_of(top.games)
            .expect("a rung's count is positive by construction")
    }
}

impl TryFrom<Vec<Tier>> for Ladder {
    type Error = String;

    /// The invariants, applied to a saved capture as well: a file written by another version of
    /// this crate is refetched rather than trusted, and one edited by hand is refused the same way.
    fn try_from(tiers: Vec<Tier>) -> Result<Self, Self::Error> {
        Self::new(tiers).ok_or_else(|| {
            "a price ladder must have at least one rung, ascending by count and by total".to_owned()
        })
    }
}

impl From<Ladder> for Vec<Tier> {
    fn from(ladder: Ladder) -> Self {
        ladder.tiers
    }
}

impl Game {
    /// Whether this entry stands for several games rather than being one.
    ///
    /// The one predicate, because two copies of it would drift: the listing uses it to decide
    /// whether to open an entry out, and a chooser uses it to decide whether an entry has a
    /// store page to link to. Both are asking the same question, and a wrapper with no page
    /// answered "no" in one place and "yes" in the other is how a search link for a marketing
    /// name gets back in.
    #[must_use]
    pub fn is_pack(&self) -> bool {
        !self.contains.is_empty()
    }
}

/// A bundle reduced to the contents of its largest tier.
///
/// Vendors sell bundles in price tiers; the largest tier is the one a buyer
/// gets by paying the most, and for every vendor supported so far it contains
/// every game in the smaller tiers as well.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bundle {
    /// Display title, e.g. `"Beyond the Metroidverse Bundle"`.
    pub title: String,
    /// Absolute URL of the bundle's page.
    pub url: String,
    /// What the listed games cost, where the store publishes enough to say.
    ///
    /// `None` means the store published no usable price, never that the bundle is free.
    pub price: Option<Price>,
    /// When the bundle stops being buyable, where the store publishes it.
    ///
    /// `None` means the store published no end date for this bundle, never that it runs
    /// forever — a deadline that could not be read is also `None`, and is warned about where it
    /// is read rather than guessed at here.
    pub ends_at: Option<Timestamp>,
    /// Games in the largest tier, in the order the vendor lists them.
    ///
    /// Vendor order is curation order (featured first), so it is preserved
    /// rather than sorted. Sort it yourself if you need a stable ordering.
    pub games: Vec<Game>,
}

/// The result of one listing run.
///
/// A run reports partial success: one unreachable bundle page does not discard
/// the other twelve. Anything that went wrong lands in `problems` instead of
/// being dropped, so a caller can always tell a complete listing from a
/// degraded one.
#[derive(Debug)]
pub struct Listing {
    pub bundles: Vec<Bundle>,
    pub problems: Vec<Problem>,
}

impl Listing {
    /// True when the run completed with nothing to report.
    pub fn is_complete(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Something that went wrong with one bundle without sinking the whole run.
#[derive(Debug)]
pub struct Problem {
    /// Title of the bundle concerned, as shown on the index page.
    pub bundle: String,
    pub kind: ProblemKind,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ProblemKind {
    /// The bundle's own page could not be fetched or parsed.
    Unavailable(Error),

    /// The store's own count disagrees with what came back.
    ///
    /// Both vendors state a bundle's size in one place and its contents in another, computed
    /// independently. Two sources for one number make a drift detector: while they agree, the
    /// contents are almost certainly being read correctly. They normally do agree, which is
    /// what makes a disagreement worth reporting rather than routine noise.
    GameCountMismatch { advertised: usize, found: usize },

    /// A smaller tier contained something the largest tier did not.
    ///
    /// Tier contents have always been cumulative, which is what makes "the
    /// largest tier" equivalent to "everything in the bundle". If that ever
    /// stops holding, picking the largest tier silently returns a subset — so
    /// the assumption is checked on every run rather than trusted.
    TiersNotCumulative { largest: String, smaller: String },

    /// Tier members with no matching entry in the vendor's item table.
    UnresolvedItems { machine_names: Vec<String> },

    /// The store named a product but published no details for it **in the region asked for**,
    /// so its title was read from its own slug.
    ///
    /// The region matters: availability is regional, and a product a store declines to describe
    /// in one country is often perfectly ordinary in another. A report saying only "unknown"
    /// would invite the conclusion that the product does not exist.
    ///
    /// Distinct from [`ProblemKind::UnresolvedItems`], where the item is dropped entirely. Here
    /// the entry is listed — with a title derived mechanically and a search link rather than a
    /// store page — so the count still matches what the store advertised. Worth reporting
    /// because a derived title is the store's identifier tidied up, not the store's own wording.
    TitleFromSlug { slugs: Vec<String> },

    /// A multi-game pack listed fewer games than it said it holds.
    ///
    /// A pack is the one entry whose games are not its own: it has no store page, and what it
    /// delivers is read from a list that comes with a count beside it. Two numbers for one
    /// fact, like every other check here — while they agree the list arrived whole, and a
    /// disagreement means games are missing from it with nothing else to show for it.
    PackContentsIncomplete {
        pack: String,
        stated: usize,
        listed: usize,
    },
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: ", self.bundle)?;
        match &self.kind {
            ProblemKind::Unavailable(e) => f.write_str(&crate::error::chain(e)),
            ProblemKind::GameCountMismatch { advertised, found } => write!(
                f,
                "the store lists {advertised} games but {found} came back"
            ),
            ProblemKind::TiersNotCumulative { largest, smaller } => write!(
                f,
                "tier {smaller:?} is not contained in the largest tier {largest:?}; \
                 the listed games may be incomplete"
            ),
            ProblemKind::UnresolvedItems { machine_names } => {
                write!(f, "no title found for {}", machine_names.join(", "))
            }
            ProblemKind::TitleFromSlug { slugs } => write!(
                f,
                "no details in this region for {}; titles read from their slugs",
                slugs.join(", ")
            ),
            ProblemKind::PackContentsIncomplete {
                pack,
                stated,
                listed,
            } => write!(
                f,
                "the pack {pack:?} says it holds {stated} games but listed {listed}"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decimal_price_is_rounded_into_hundredths_never_truncated() {
        // Truncation is right for about 95% of two-decimal values, which is the worst possible
        // failure rate: common enough to be real, rare enough to pass any test built from
        // prices that happen to be on hand. These two are the ones that break it.
        assert_eq!(
            Money::from_major(4.35, "USD").expect("valid").hundredths,
            435
        );
        assert_eq!(
            Money::from_major(8.20, "USD").expect("valid").hundredths,
            820
        );
        // And the ones that survive it, so the test still covers the ordinary path.
        assert_eq!(
            Money::from_major(7.99, "USD").expect("valid").hundredths,
            799
        );
        assert_eq!(
            Money::from_major(10.0, "USD").expect("valid").hundredths,
            1000
        );
    }

    #[test]
    fn a_whole_count_published_as_a_float_is_read_as_the_count_it_is() {
        // Fanatical publishes values like these; they are whole prices that have been through
        // a floating-point conversion upstream.
        assert_eq!(
            Money::from_hundredths(204.999_999_999_999_97, "EUR").expect("valid"),
            Money::new(205, "EUR")
        );
        assert_eq!(
            Money::from_hundredths(805.000_000_000_000_1, "GBP").expect("valid"),
            Money::new(805, "GBP")
        );
    }

    #[test]
    fn an_amount_that_is_not_a_real_price_is_refused_rather_than_mangled() {
        // Losing a price is recoverable; showing a wrong one is not.
        assert_eq!(Money::from_major(f64::NAN, "USD"), None);
        assert_eq!(Money::from_major(f64::INFINITY, "USD"), None);
        assert_eq!(Money::from_major(-1.0, "USD"), None);
        assert_eq!(Money::from_major(1e12, "USD"), None);
    }

    #[test]
    fn a_per_item_rate_is_rounded_up_from_a_half_rather_than_cut() {
        // 2369 over 25 is 94.76 and must read as 0.95. Truncating understates every rate.
        let total = Money::new(2369, "USD");
        assert_eq!(total.each_of(25).expect("a rate"), Money::new(95, "USD"));
        assert_eq!(
            Money::new(3499, "USD").each_of(5).expect("a rate"),
            Money::new(700, "USD")
        );
    }

    #[test]
    fn a_rate_is_a_rate_and_does_not_multiply_back_to_the_total() {
        // Pinned so that nobody sums it: 95 x 25 is 2375, six cents above the 2369 actually
        // charged. The rate answers "what does one game cost here", not "what will I pay".
        let total = Money::new(2369, "USD");
        let each = total.each_of(25).expect("a rate");
        assert_ne!(each.hundredths * 25, total.hundredths);
    }

    #[test]
    fn a_tier_that_grants_nothing_has_no_rate() {
        assert_eq!(Money::new(999, "USD").each_of(0), None);
    }

    #[test]
    fn a_price_always_shows_which_currency_it_is_in() {
        // In one real tier, GBP, EUR and USD are all the integer 699 — identical digits, three
        // different prices — so the amount alone can never identify the currency.
        assert_eq!(Money::new(699, "USD").to_string(), "$6.99");
        assert_eq!(Money::new(699, "GBP").to_string(), "£6.99");
        assert_eq!(Money::new(699, "EUR").to_string(), "€6.99");
    }

    #[test]
    fn a_currency_whose_encoding_is_unverified_prints_its_code_not_a_symbol() {
        // Yen especially: Fanatical quotes 121100 for a $6.99 tier, which is ¥1,211 at about
        // 173 to the dollar. The magnitude is right and the decimals are ours, so the code is
        // shown rather than a symbol that would imply yen have subunits.
        assert_eq!(Money::new(121_100, "JPY").to_string(), "1211.00 JPY");
        assert_eq!(Money::new(500, "SEK").to_string(), "5.00 SEK");
    }

    #[test]
    fn a_price_under_a_unit_keeps_its_leading_zero() {
        assert_eq!(Money::new(95, "USD").to_string(), "$0.95");
        assert_eq!(Money::new(5, "USD").to_string(), "$0.05");
        assert_eq!(Money::new(0, "USD").to_string(), "$0.00");
    }
}
