//! Plain-text rendering: the one place a game's preview line is decided.
//!
//! Lives in the library rather than the binary because it is pure formatting with no CLI
//! machinery in it, which makes it both testable without a process and reusable by callers that
//! want the same output.
//!
//! Every game line — under a Humble bundle, under a Fanatical bundle, or printed alone from a
//! direct Steam lookup — is built here. A change to what a line says, or how a verdict is
//! coloured, is made once and lands everywhere.
//!
//! Nothing here probes the environment. Whether colour is wanted is the caller's decision,
//! passed in as a [`Palette`]: a library that decided for itself would colour output that was
//! being piped into a file.

use std::fmt::Write as _;

use log::warn;
use table_formatter::{FormatOptions, RowSpacing, format_table};

use crate::clock::Timestamp;
use crate::commands::gamelib;
use crate::inventory::steam as steam_inventory;
use crate::inventory::steam::Identity;
use crate::inventory::steam::{Deck, Os, Rating, Reviews, SteamGame, Vr};
use crate::steam;
use crate::steam::items::{self, Answer, Classified};
use crate::steam::store::GameDetails;
use crate::store_inventory::steam::{delisted, snapshot, tags};
use crate::user_games::crossover;
use crate::user_games::holdings::{Claim, Holdings, Standings};
use crate::{Bundle, Game, Listing, Price};
use crate::{Ladder, Money, Tier};

// ---------------------------------------------------------------------------------------------
// What gets shown
// ---------------------------------------------------------------------------------------------

/// What is known about one game, before it is laid out.
///
/// Built from a bundle's [`Game`] by way of the inventory, or from a live [`GameDetails`]; laid
/// out by [`line`] on its own or by [`listing`] in aligned columns. Every store's games go
/// through this, which is what keeps one game looking the same wherever it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub name: String,
    /// `None` when nothing is known about the game's reviews — no inventory entry, no fetch.
    pub all_time: Option<Reviews>,
    /// `None` when Steam shows no thirty-day row, or when nothing is known at all.
    pub recent: Option<Reviews>,
    /// Named where another store has already given this game away, else empty.
    ///
    /// Its own field rather than spliced into the name, so a caller laying a game out some
    /// other way can put it wherever suits and a comparison can ignore it.
    pub elsewhere: String,
    /// What the reader's own snapshot files say about this game: already owned, already wanted.
    ///
    /// Its own field rather than spliced into the name, for the same reason `elsewhere` is: a
    /// caller laying a game out some other way can put it where that layout wants it, and a
    /// comparison of two previews can ignore it.
    pub standings: Standings,
    /// The game's own store page when its app-id is known, else a search for its title.
    pub url: String,
    /// What is known beyond the reviews. `None` for a game nothing is known about, which is
    /// what makes the two cases visible at a glance: no entry, no detail line.
    pub detail: Option<Detail>,
}

/// The rest of what is known about a game, for the line beneath it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detail {
    /// Release date as Steam prints it.
    pub released: String,
    pub os: Os,
    pub vr: Vr,
    pub deck: Deck,
    /// Player tags, most-voted first.
    pub tags: Vec<String>,
}

/// Whether rendered output may carry colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Palette {
    /// Plain text. The right choice whenever the output is not going to a terminal — a pipe,
    /// a file, another program's stdin.
    #[default]
    Plain,
    /// 24-bit ANSI colour.
    Ansi,
}

/// How many tags a detail line carries.
///
/// Steam publishes about twenty per game, ordered by votes. All twenty would be a paragraph and
/// the tail of them says little — the first few are what the game *is*.
const TAGS_SHOWN: usize = 5;

/// Marks a detail line as belonging to the game above it.
const DETAIL_MARKER: &str = "└ ";

/// Marks a game listed directly in a bundle.
const BULLET: &str = "-";

/// Marks a game that comes inside another entry, rather than on its own.
///
/// An arrow rather than a deeper indent, and joined to the name by a **single** space.
///
/// The single space is what keeps the bullet part of the name cell. Two spaces are how this
/// table separates columns, so a bullet written with two would become a column of its own, be
/// padded to the width of the widest bullet, and push every name — a pack's own title included —
/// to the right of where it belongs. Joined by one space the dashes are never a column, and the
/// arrow's two extra characters are exactly the offset that shows the nesting.
const NESTED_BULLET: &str = "-->";

/// Spaces before every row, which is what sets a bundle's games under its title.
const INDENT: &str = "  ";

/// Grey for detail lines, so the games themselves carry the eye down the list.
const DETAIL_COLOUR: &str = "#808080";

/// Below this many hours left, a bundle's remaining time is marked urgent.
///
/// Fifty rather than forty-eight so that "two days" is already inside the warning: a deadline
/// two days out is one missed evening away from being missed entirely, and the extra couple of
/// hours is what stops a bundle crossing the line unseen overnight.
const URGENT_HOURS: i64 = 50;

/// The same red the lowest review band uses, so the listing has one red and not two.
const URGENT_COLOUR: &str = "#F53030";

/// Below this many hours left, a bundle's remaining time is marked as approaching.
///
/// Five days: long enough to still be a plan rather than a scramble, which is what separates it
/// from [`URGENT_HOURS`].
const SOON_HOURS: i64 = 5 * 24;

/// The orange between the grey and the red.
///
/// Darker than a pure orange so that it stays legible on a white terminal as well as a black
/// one, and far enough from [`URGENT_COLOUR`] that the two are not confused at a glance.
const SOON_COLOUR: &str = "#D7822A";

/// The colour those shared phrases take.
///
/// Green rather than the grey of a detail line: the phrase is not noise to be muted, it says
/// what KIND of bundle this is — one you pick games out of, priced per game — and that is worth
/// seeing at a glance. Mid-toned so it stays legible on a white terminal as well as a black one.
const PHRASE_COLOUR: &str = "#4CA64C";

/// The colour of the hint that a game is held in the committed Epic table.
///
/// The same green the shared wording takes: both are saying "this one is different from its
/// neighbours for a reason you want to know", and a listing with two greens would be saying it
/// twice in two voices. Deliberately NOT [`OWNED_COLOUR`] — that one marks what the reader's own
/// files say, and a guess about somebody else's account must not wear the same badge.
const ELSEWHERE_COLOUR: &str = PHRASE_COLOUR;

/// The colour a whole line takes when the reader's own files say they already own the game.
///
/// **The line, not a badge on it.** Red on the name and the link is read before any of the words
/// are: there is no reason to buy this one, and that should not need reading to notice. The same
/// red [`URGENT_COLOUR`] uses, so the listing still has one red and not two.
///
/// The reviews keep their own band colours through it. They answer a different question — is this
/// game any good — and that answer does not change because you happen to have it.
const OWNED_COLOUR: &str = URGENT_COLOUR;

/// The colour a whole line takes when the reader's own files say they want the game.
///
/// Its own constant even though it starts out equal to [`PHRASE_COLOUR`]: this one answers "do I
/// want this", the phrase colour answers "what kind of bundle is this", and the day one of them
/// moves the other should not have to be found first.
const WANTED_COLOUR: &str = PHRASE_COLOUR;

/// The colour of the `[owned on …]` column itself.
///
/// Grey, and deliberately quieter than the line it annotates. The line's own colour is the
/// verdict; this column is the detail behind it — which stores, and how sure — for the reader who
/// has already seen the colour and wants to know why. Same grey a detail line takes, for the same
/// reason: subordinate to what it sits beside.
const TAG_COLOUR: &str = DETAIL_COLOUR;

/// Marks a store that could only be matched by title rather than by app-id.
///
/// One character, and it goes on the STORE rather than on the bracket, because the claims stack:
/// `[owned on Steam, Epic?]` is one identity and one guess, and a bracket-level marker could not
/// say which was which. See [`crate::user_games::holdings`] for why some claims can only be titles.
const GUESSED: &str = "?";

/// Up to this many hours left, the time remaining is counted in hours rather than days.
///
/// Set above [`URGENT_HOURS`] on purpose: every urgent bundle then states its deadline in the
/// same unit, so "49h left" and "51h left" can be compared at a glance instead of one of them
/// reading "2d 1h".
const HOURS_BEFORE_DAYS: i64 = 72;

/// Stands in for a review column with nothing in it.
///
/// A placeholder rather than a blank, because the column separator is whitespace: an empty cell
/// would merge with the gap beside it and the row would be read as having one column fewer.
/// It is also what a reader needs — "not known", distinct from a bad score.
const UNKNOWN: &str = "-";

impl Detail {
    /// One line: when it came out, what it runs on, what players call it.
    ///
    /// Carries only what is worth saying. VR appears when there is VR to report, and the Deck
    /// rating when Valve has actually given one — "untested" is a fact about Valve rather than
    /// about the game, and printing it down most of a listing would crowd out what does say
    /// something.
    pub fn line(&self) -> String {
        let mut parts = vec![self.released.clone()];

        let platforms = self.os.labels();
        if !platforms.is_empty() {
            parts.push(platforms.join("/"));
        }
        if self.vr != Vr::None {
            parts.push(self.vr.as_str().to_owned());
        }
        if self.deck != Deck::Unknown {
            parts.push(self.deck.as_str().to_owned());
        }
        if !self.tags.is_empty() {
            parts.push(
                self.tags
                    .iter()
                    .take(TAGS_SHOWN)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        parts.join(" · ")
    }
}

impl Preview {
    /// A store's game, as much as the inventory knows about it.
    ///
    /// A game found in [`crate::inventory::steam`] carries its two review verdicts, a link to
    /// its own store page and a detail line. One that is not carries none of them, because
    /// inventing them would be worse than leaving them out — but it still links to its own page
    /// if the store published an app-id, since a search would discard an identity already held.
    pub fn of_game(game: &Game, held: &Holdings, classified: &Classified) -> Self {
        // Both asked once, here, so a game is marked whether or not the inventory knows anything
        // else about it.
        let standings = held.of(game);
        // The committed giveaway table is a hint about ONE account, so it steps aside the moment a
        // real Epic snapshot is in hand: that file is about whoever is running the program, and a
        // guess beside an answer is only ever noise.
        let elsewhere = match held.read_for(gamelib::epic::STORE) {
            true => String::new(),
            false => crossover::owned_on_epic(game)
                .map(|_| crossover::NOTE.to_owned())
                .unwrap_or_default(),
        };
        // The one join, read back into whichever table holds the game. The curated table first,
        // and not because it is older: its band is over the review population this program chose
        // (`purchase_type=all`, key-activated copies counted), which no bulk surface publishes.
        // Where both tables hold a game, the curated one is the right population, not merely the
        // earlier reading.
        let app_id = steam_inventory::app_id_of(game);
        if let Some(entry) = app_id.and_then(steam_inventory::by_app_id) {
            return Self {
                elsewhere,
                standings,
                // The same answer the opener will write, rather than a second one derived from
                // the entry: see `page_of`.
                url: page_of(game, classified),
                ..Self::of_entry(game.title.clone(), entry)
            };
        }
        // Then the snapshot: 188,000 games where the curated table has 415. This is what gives a
        // band to a game the curated table never met — every game a pack delivers, and every
        // Humble row, since Humble publishes no ids and its games resolve by title alone.
        if let Some((found, details)) = app_id
            .and_then(snapshot::by_id)
            .and_then(|found| found.details.map(|details| (found, details)))
        {
            return Self {
                elsewhere,
                standings,
                url: page_of(game, classified),
                ..Self::of_snapshot(game.title.clone(), found.app_id, &details)
            };
        }
        Self {
            name: game.title.clone(),
            all_time: None,
            recent: None,
            elsewhere,
            standings,
            url: page_of(game, classified),
            detail: None,
        }
    }

    /// A snapshot entry, under the name a store gave it.
    ///
    /// What the snapshot lacks, said plainly rather than filled in: `recent`, because the
    /// thirty-day window is in no bulk surface Valve publishes; and VR's DEGREE, because the
    /// snapshot records only whether the store names a headset at all, so a VR-only game reads
    /// here as "VR supported". The band is derived from approval and count exactly as
    /// [`Self::of_entry`] derives it, over the snapshot's population (`purchase_type=steam`).
    fn of_snapshot(name: String, app_id: u32, details: &snapshot::Details) -> Self {
        Self {
            name,
            // Both figures or neither: a percentage over an unknown count says nothing.
            all_time: details
                .approval
                .zip(details.reviews)
                .map(|(approval, count)| Reviews {
                    // The snapshot writes a percentage; anything past 100 would be a corrupt row,
                    // and clamping is the honest reading of one rather than a wrapped byte.
                    approval: u8::try_from(approval.min(100)).unwrap_or(100),
                    count,
                }),
            recent: None,
            elsewhere: String::new(),
            standings: Standings::default(),
            url: steam::app_url(app_id),
            detail: Some(Detail {
                // In the store page's own wording, so a snapshot row reads like the curated row
                // above it. A date that will not parse is shown as it came rather than dressed up.
                released: match details.released {
                    Some(snapshot::Release::Out(iso) | snapshot::Release::Planned(iso)) => {
                        steam::printed_date(iso).unwrap_or_else(|| iso.to_owned())
                    }
                    Some(snapshot::Release::Unannounced) => "coming soon".to_owned(),
                    Some(snapshot::Release::Undated) | None => "?".to_owned(),
                },
                // Unknown draws as a store listing none, which is what the line drew before a
                // cell could be unknown.
                os: details.os.unwrap_or(Os {
                    windows: false,
                    mac: false,
                    linux: false,
                }),
                vr: if details.vr == Some(true) {
                    Vr::Supported
                } else {
                    Vr::None
                },
                deck: details
                    .deck
                    .and_then(|deck| u8::try_from(deck).ok())
                    .and_then(Deck::from_category)
                    .unwrap_or(Deck::Unknown),
                // Strongest first, which is the order the snapshot keeps them in.
                tags: details
                    .tag_ids()
                    .into_iter()
                    .flatten()
                    .filter_map(tags::name)
                    .map(str::to_owned)
                    .collect(),
            }),
        }
    }

    /// An inventory entry, under the name a store gave it.
    fn of_entry(name: String, entry: &SteamGame) -> Self {
        Self {
            name,
            all_time: Some(entry.all_time),
            recent: entry.recent,
            elsewhere: String::new(),
            standings: Standings::default(),
            url: steam::app_url(entry.app_id),
            detail: Some(Detail {
                released: entry.released.to_owned(),
                os: entry.os,
                vr: entry.vr,
                deck: entry.deck,
                tags: entry
                    .tags
                    .iter()
                    .map(|tag| tag.as_str().to_owned())
                    .collect(),
            }),
        }
    }

    /// A game just fetched from Steam.
    pub fn of_details(details: &GameDetails) -> Self {
        Self {
            name: details.name.clone(),
            all_time: Some(details.all_time),
            recent: details.recent,
            elsewhere: crossover::by_title(&details.name)
                .map(|_| crossover::NOTE.to_owned())
                .unwrap_or_default(),
            standings: Standings::default(),
            url: steam::app_url(details.app_id),
            detail: Some(Detail {
                released: details.released.clone(),
                os: details.os,
                vr: details.vr,
                deck: details.deck,
                tags: details.tags.clone(),
            }),
        }
    }

    /// The game line's cells: name, verdicts, link, and what the reader's own files say.
    ///
    /// This is the single place the wording and the colouring are decided.
    ///
    /// The last cell is dropped when it is empty rather than left blank, so a listing where
    /// nothing is owned is byte-identical to one rendered before the column existed.
    ///
    /// **A marked line no longer ends at its link**, which was a documented contract until this
    /// column arrived: the supported parse is now the field that STARTS WITH `https://` rather
    /// than the last field of the line. `tests/holdings.rs` pins that, because the README hands
    /// readers a command that depends on it.
    fn cells(&self, palette: Palette) -> Vec<String> {
        let reviews = match self.all_time {
            Some(all_time) => format!(
                "{} | {}",
                verdict(all_time, Reviews::rating, palette),
                self.recent.map_or_else(
                    || UNKNOWN.to_owned(),
                    |recent| verdict(recent, Reviews::recent_rating, palette)
                )
            ),
            None => UNKNOWN.to_owned(),
        };
        // The note rides inside the NAME cell rather than taking a column of its own, and the
        // reason is the same one the review placeholder exists for: a cell that is empty for
        // most rows cannot be told from the gap beside it, so the row reads as having one
        // column fewer and everything after it stops lining up. A column of dashes on every
        // row would avoid that and say nothing.
        let name = match self.elsewhere.is_empty() {
            true => self.name.clone(),
            false => format!(
                "{} {}",
                self.name,
                paint(&self.elsewhere, ELSEWHERE_COLOUR, palette)
            ),
        };
        // Owned before wanted: having a game settles the question that wanting it asks. One
        // bracket per standing, so a game owned on one store and wishlisted on another — which is
        // an ordinary thing — reads as the two separate facts it is.
        let tags: Vec<String> = [
            ("owned", &self.standings.owned),
            ("wishlisted", &self.standings.wanted),
        ]
        .into_iter()
        .filter(|(_, claims)| !claims.is_empty())
        .map(|(verb, claims)| {
            let stores: Vec<String> = claims.iter().map(named).collect();
            format!("[{verb} on {}]", stores.join(", "))
        })
        .collect();

        // The colour goes on the name and the link, which is the line as a reader scans it, and
        // never on the verdicts. Painted here rather than after alignment so that the padding the
        // table adds lands outside the colour.
        let tone = self.tone();
        let mut cells = vec![
            tinted(&name, tone, palette),
            reviews,
            tinted(&self.url, tone, palette),
        ];
        if !tags.is_empty() {
            cells.push(paint(&tags.join(" "), TAG_COLOUR, palette));
        }
        cells
    }

    /// The colour this whole line takes, if any.
    ///
    /// **Red beats green.** Owning a game answers the question that wanting it asks, so a game on
    /// both a library and a wishlist is one you have — and the red is there to say "no reason to
    /// buy this" without being read.
    fn tone(&self) -> Option<&'static str> {
        if !self.standings.owned.is_empty() {
            Some(OWNED_COLOUR)
        } else if !self.standings.wanted.is_empty() {
            Some(WANTED_COLOUR)
        } else {
            None
        }
    }
}

/// `paint`, for a colour that may not be there — an uncoloured line is the ordinary case.
fn tinted(text: &str, hex: Option<&'static str>, palette: Palette) -> String {
    match hex {
        Some(hex) => paint(text, hex, palette),
        None => text.to_owned(),
    }
}

/// A store claim as a reader sees it: the store, and a `?` when it was only matched by title.
fn named(claim: &Claim) -> String {
    match claim.by_id {
        true => claim.store.to_owned(),
        false => format!("{}{GUESSED}", claim.store),
    }
}

/// The page a game on sale points at: its own where one can be reached, else a search for it.
///
/// **The inventory is asked first, and leaving it out was a real bug.** A store that publishes no
/// app-id — Humble publishes none for anything — can reach its own page only through the
/// inventory, so a version of this that consulted `game.steam_app_id` alone sent every Humble game
/// to a search results page. The listing did not show it, because the listing built a `Preview`,
/// which did ask the inventory: the form showed `…/app/1681600` and the script it wrote opened a
/// search for the title. One answer now, in one place, with a test holding the two together.
///
/// An id the tables could not settle is what `classified` is for — what the item service said it
/// is, once per run ([`crate::steam::items`]). Found, the page is Steam's own path for it: a
/// bundle id a store published as an app id reaches `/bundle/…` instead of bouncing. Asked and
/// claimed by nothing, it is a search: the page would bounce. Never asked — an offline run, a
/// library caller, a request that failed — an unheld id is linked as published, which is what
/// this did before the service was consulted, and is usually right.
#[must_use]
pub fn page_of(game: &Game, classified: &Classified) -> String {
    let search = || steam::search_url(&game.title);
    match steam_inventory::identity_of(game) {
        Identity::App(id) => steam::app_url(id),
        Identity::Unheld(id) => match classified.of(id) {
            Answer::Found(claims) => items::meant(claims, &game.title).url(),
            Answer::Nothing => search(),
            // Never asked: the id as published — unless the ledger saw the store refuse it
            // outright, where the page would bounce. A region-restricted app keeps its page: it is
            // alive, and may well be for sale where the reader is.
            Answer::Unasked => match delisted::get(id).map(|entry| entry.state) {
                Some(delisted::State::Removed) => search(),
                Some(delisted::State::RegionRestricted) | None => steam::app_url(id),
            },
        },
        Identity::Disputed(id) => match classified.of(id) {
            Answer::Found(claims) => items::meant(claims, &game.title).url(),
            Answer::Nothing | Answer::Unasked => search(),
        },
        Identity::Unknown => search(),
    }
}

/// A verdict and the number of reviews behind it, coloured by band.
///
/// Takes the band function rather than the band, because the two review windows are not
/// interchangeable: Steam gates them differently, and 80% over 10 reviews is "Very Positive"
/// in the last thirty days and only "Positive" all-time. Passing the wrong one produces a
/// plausible word in the wrong column, which is the kind of mistake nothing later catches.
///
/// The count carries no thousands separators: this sits in a line meant to be both read and
/// piped, and a separator is one more thing a reader of the output has to strip.
fn verdict(reviews: Reviews, band: fn(Reviews) -> Rating, palette: Palette) -> String {
    let rating = band(reviews);
    paint(
        &format!("{} {}", rating.as_str(), reviews.count),
        rating.gradient_color(),
        palette,
    )
}

/// Wraps `text` in a 24-bit ANSI colour, or returns it untouched under [`Palette::Plain`].
///
/// Truecolor rather than the sixteen named colours, because the palette is a ramp: neighbouring
/// bands differ by a shade, which the named colours cannot express.
/// A bundle's title, followed by when it stops being buyable.
///
/// The deadline is grey like a detail line, because the games are what the eye should follow;
/// the time remaining turns red once it is short, because that is the one part of a listing
/// that is worth interrupting for.
fn heading(bundle: &Bundle, now: Timestamp, palette: Palette) -> String {
    let mut line = marked_phrases(&bundle.title, palette);

    // Parenthesised and bracketed, each with a space inside the mark. Tight against a digit a
    // bracket reads as a leading 1 — `[8h` looks like eighteen hours — and the space is what
    // stops it.
    if let Some(price) = &bundle.price {
        line.push_str("  ");
        line.push_str(&paint(
            &format!("( {} )", price_line(price)),
            DETAIL_COLOUR,
            palette,
        ));
    }
    // No date published is not the same as no deadline, so nothing is claimed either way.
    if let Some(ends_at) = bundle.ends_at {
        let seconds = ends_at.seconds_from(now);
        line.push_str("  ");
        line.push_str(&paint(
            &format!("[ ends {ends_at} ·"),
            DETAIL_COLOUR,
            palette,
        ));
        line.push(' ');
        line.push_str(&paint(
            &describe_remaining(seconds),
            urgency(seconds),
            palette,
        ));
        line.push(' ');
        line.push_str(&paint("]", DETAIL_COLOUR, palette));
    }
    line
}

/// Wording that every bundle of its kind carries, and that therefore distinguishes none of them.
///
/// Matched case-insensitively because the catalogue writes it both ways — "Build your own
/// Bundle" and "Build your Own Bundle" appear in the same listing.
const MARKED_PHRASES: &[&str] = &["Build your own"];

/// Colours the wording a whole product line shares, so the part that names one bundle stands out.
///
/// Marked rather than removed: the title is still the store's, and a listing that quietly
/// rewrote it would be harder to match back against the page it came from.
fn marked_phrases(title: &str, palette: Palette) -> String {
    let lowered = title.to_lowercase();
    let mut out = String::with_capacity(title.len());
    let mut at = 0;
    while at < title.len() {
        let hit = MARKED_PHRASES
            .iter()
            .filter_map(|phrase| {
                let found = lowered[at..].find(&phrase.to_lowercase())? + at;
                Some((found, phrase.len()))
            })
            .min();
        match hit {
            Some((start, length)) => {
                out.push_str(&title[at..start]);
                out.push_str(&paint(
                    &title[start..start + length],
                    PHRASE_COLOUR,
                    palette,
                ));
                at = start + length;
            }
            None => {
                out.push_str(&title[at..]);
                break;
            }
        }
    }
    out
}

/// What the games cost, in the store's own terms.
///
/// A rate names the tier it holds from, because the rate alone hides the commitment: ninety-five
/// cents a game is a different offer at five games than at twenty-five, and the rounded rate does
/// not multiply back to the total either.
fn price_line(price: &Price) -> String {
    match price {
        // An absolute price is the whole story, so nothing is added to it.
        Price::Whole(amount) => amount.to_string(),
        // The "+" says the rate holds from that many games upward, which is what a ladder's
        // top tier offers. Unchanged by the ladder underneath: the folded line stays the concise
        // one, and the rungs are for the line a reader sees on opening the bundle.
        Price::PerGame(ladder) => format!("{}/game at {}+", ladder.each(), ladder.top().games),
    }
}

/// What each next batch of picks costs, up a pick-and-mix ladder:
/// `3: $3.33, then 2: $2.50, then 2: $2.48, then any: $2.85`.
///
/// Fanatical prices a ladder as totals, and its page frames a bigger rung as a saving on the
/// picks already made — "you're saving on what you already picked". Read the other way round,
/// the difference between two rungs, over the picks it adds, is what the NEXT picks cost, which
/// is the figure a buyer deciding whether to add two more games actually needs. The last figure
/// is the top rung's rate over all of its picks: what every pick costs once the ladder is climbed.
/// Checked against the Platinum Collection's own page — 3 for $9.99, 5 for $14.99, 7 for $19.95
/// — which gives exactly the line above.
///
/// Every subtraction is sound by construction: a [`Ladder`] ascends strictly in both count and
/// total, so no step adds zero picks or costs less than nothing.
#[must_use]
pub fn marginal_rates(ladder: &Ladder) -> String {
    let mut said = Vec::with_capacity(ladder.tiers().len() + 1);
    let mut below: Option<&Tier> = None;
    for tier in ladder.tiers() {
        let (picks, hundredths) = match below {
            None => (tier.games, tier.total.hundredths),
            Some(lower) => (
                tier.games - lower.games,
                tier.total.hundredths - lower.total.hundredths,
            ),
        };
        let each = Money::new(hundredths, tier.total.currency.clone())
            .each_of(picks)
            .expect("a ladder's counts ascend strictly, so every step adds a pick");
        said.push(format!("{picks}: {each}"));
        below = Some(tier);
    }
    said.push(format!("any: {}", ladder.each()));
    said.join(", then ")
}

/// The line a reader meets first on opening a bundle: its page, and what each next game costs.
///
/// Two facts the folded heading cannot hold. The page, because a person choosing wants to see
/// the offer as the store presents it, and the heading is already full. And the ladder's
/// marginal rates, which are the honest reading of a pick-and-mix — see [`marginal_rates`]. A
/// bundle sold whole, or one the store published no usable price for, gets the page alone.
///
/// Drawn as a comment row in the form: unselectable, and folded away with the bundle.
#[must_use]
pub fn bundle_note(bundle: &Bundle) -> String {
    match &bundle.price {
        Some(Price::PerGame(ladder)) => {
            format!(
                "# {}  ;  #ofGames:Cost-Each : {}",
                bundle.url,
                marginal_rates(ladder)
            )
        }
        _ => format!("# {}", bundle.url),
    }
}

/// How loudly a deadline is announced.
///
/// Three bands rather than two: red alone makes every deadline either an emergency or invisible,
/// and the week before a bundle ends is exactly when noticing it is still useful.
fn urgency(seconds: i64) -> &'static str {
    const HOUR: i64 = 3_600;
    if seconds < URGENT_HOURS * HOUR {
        URGENT_COLOUR
    } else if seconds < SOON_HOURS * HOUR {
        SOON_COLOUR
    } else {
        DETAIL_COLOUR
    }
}

/// How long is left, in the largest unit that still says something useful.
///
/// One unit, not two: "2d 6h" is more precise than a buyer needs and harder to compare across a
/// list than "2d". Below [`HOURS_BEFORE_DAYS`] the unit is hours, so every bundle near its
/// deadline is measured the same way.
fn describe_remaining(seconds: i64) -> String {
    const HOUR: i64 = 3_600;
    match seconds {
        ..=0 => "ended".to_owned(),
        1..HOUR => format!("{}m left", seconds.div_euclid(60).max(1)),
        s if s < HOURS_BEFORE_DAYS * HOUR => format!("{}h left", s.div_euclid(HOUR)),
        s => format!("{}d left", s.div_euclid(24 * HOUR)),
    }
}

fn paint(text: &str, hex: &str, palette: Palette) -> String {
    let Palette::Ansi = palette else {
        return text.to_owned();
    };
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).unwrap_or(0);
    format!(
        "\x1b[38;2;{};{};{}m{text}\x1b[0m",
        channel(1),
        channel(3),
        channel(5)
    )
}

// ---------------------------------------------------------------------------------------------
// Laying it out
// ---------------------------------------------------------------------------------------------

/// One game, unpadded: name, verdicts and link, with the detail line beneath where there is one.
///
/// The URL is the last field of the *first* line. A detail line is indented and carries no
/// link, so `awk '/https/{print $NF}'` still picks out exactly the links.
pub fn line(preview: &Preview, palette: Palette) -> String {
    let mut out = preview.cells(palette).join("  ");
    if let Some(detail) = &preview.detail {
        let _ = write!(
            out,
            "\n  {}",
            paint(
                &format!("{DETAIL_MARKER}{}", detail.line()),
                DETAIL_COLOUR,
                palette
            )
        );
    }
    out
}

/// Renders each bundle's title followed by its games, one per line, in columns aligned within
/// each bundle.
///
/// The URL is always the last field of a game line, so it cuts out with
/// `awk '/https/{print $NF}'`. That is the only supported way to parse this: game names contain
/// spaces, so field *positions* are not stable and a positional parse will misread any
/// multi-word title.
///
/// Returns an empty string for an empty listing rather than a placeholder, so callers can
/// decide what "nothing on sale" should look like.
pub fn listing(
    listing: &Listing,
    palette: Palette,
    held: &Holdings,
    classified: &Classified,
) -> String {
    listing_at(listing, palette, Timestamp::now(), held, classified)
}

/// [`listing`], with the moment to count down from supplied.
///
/// Public so that a test — or a caller rendering a listing captured earlier — can get the same
/// output twice. [`listing`] reads the clock, which makes it correct and untestable; this one
/// takes `now` as an argument, which makes it both.
pub fn listing_at(
    listing: &Listing,
    palette: Palette,
    now: Timestamp,
    held: &Holdings,
    classified: &Classified,
) -> String {
    let mut out = String::new();
    for (position, bundle) in listing.bundles.iter().enumerate() {
        if position > 0 {
            out.push('\n');
        }
        let _ = writeln!(out, "{}", heading(bundle, now, palette));

        // Each bundle is its own table. Aligning the whole listing at once would look tidier
        // in principle, but a single very long title — the bundled video courses run past
        // seventy characters — would then push every other bundle's links out behind a
        // corridor of spaces. A bundle pays for its own outliers and no one else's.
        let rows = flatten(&bundle.games, held, classified);

        // Every row goes into the table, its bullet included as the first column, so that a
        // nested entry lines up with its siblings instead of being pushed sideways.
        let cells: Vec<String> = rows
            .iter()
            .map(|row| row.cells(palette, &row.lead()))
            .collect();
        let aligned = align(&cells);

        for (row, line) in rows.iter().zip(&aligned) {
            let _ = writeln!(out, "{INDENT}{line}");
            if let Some(detail) = row.preview().and_then(|preview| preview.detail.as_ref()) {
                let _ = writeln!(
                    out,
                    "{INDENT}{}{}",
                    " ".repeat(name_column(row)),
                    paint(
                        &format!("{DETAIL_MARKER}{}", detail.line()),
                        DETAIL_COLOUR,
                        palette
                    )
                );
            }
        }
    }
    out
}

/// A bundle's title line, as the listing prints it: name, price, deadline.
///
/// Public so that a caller presenting a bundle some other way — a form, a menu — heads it with
/// exactly what the listing would, rather than a second wording that has to be kept in step.
#[must_use]
pub fn bundle_heading(bundle: &Bundle, palette: Palette) -> String {
    heading(bundle, Timestamp::now(), palette)
}

/// One tickable line of a bundle — see [`choices`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The line as it should be drawn, already padded into columns with its bundle's others.
    pub line: String,
    /// Whether this is one of the games DELIVERED BY the offer above it, rather than something
    /// the bundle sells on its own.
    ///
    /// Positional, because that is what the caller pairing these back up already relies on: the
    /// games a pack delivers follow it, in order. A pack is still one product bought whole — what
    /// this marks is that each game inside it has a page of its own worth opening or not.
    pub included: bool,
    /// The page this box opens — **already resolved**, so a caller never has to work it out again.
    ///
    /// `None` for a pack, which has no page of its own; what it delivers has one box and one page
    /// each, directly below it.
    ///
    /// Carried rather than re-derived because the alternative is deriving one fact twice: the
    /// listing shows a link and the caller opens one, and those two were once separate
    /// computations that drifted apart — the form showed a game's own page while the script it
    /// wrote opened a search. Handing the answer over with the line it is drawn on is what makes
    /// that unrepresentable rather than merely tested. [`page_of`] remains for a caller holding a
    /// [`Game`] and no offer.
    pub url: Option<String>,
}

/// One aligned line per box a caller should offer, for every thing in the bundle.
///
/// The same traversal [`listing`] prints, so the two can never disagree about what a bundle
/// holds: a pack yields a line for itself and then one per game it delivers. What differs is the
/// marker — the listing writes a bullet whose width shows the nesting, while a form draws its own
/// box and indents the nested ones itself, so these lines carry no bullet at all.
///
/// A nested line's own columns therefore sit two cells right of its siblings', because the
/// indent the form adds shifts the whole row and a fixed-width name column cannot give those
/// rows two fewer cells of padding to compensate. That stagger IS the nesting, and it is the
/// reason the listing marks depth with a wider bullet instead: there, every row starts flush.
#[must_use]
pub fn choices(
    bundle: &Bundle,
    palette: Palette,
    held: &Holdings,
    classified: &Classified,
) -> Vec<Offer> {
    let rows = flatten(&bundle.games, held, classified);
    let lines = align(
        &rows
            .iter()
            .map(|row| row.cells(palette, ""))
            .collect::<Vec<String>>(),
    );
    rows.iter()
        .zip(lines)
        .map(|(row, line)| Offer {
            line,
            included: row.depth() > 0,
            url: row.preview().map(|preview| preview.url.clone()),
        })
        .collect()
}

/// One printed line of a bundle's contents.
enum Row {
    /// A product that is not itself a game, named so that what it delivers has something to sit
    /// under. It gets no reviews and no link: there is no page to link to, and the search link
    /// it used to get was for a phrase no store sells.
    Pack {
        name: String,
        depth: usize,
    },
    Game {
        preview: Preview,
        depth: usize,
    },
}

impl Row {
    fn preview(&self) -> Option<&Preview> {
        match self {
            Self::Game { preview, .. } => Some(preview),
            Self::Pack { .. } => None,
        }
    }

    fn depth(&self) -> usize {
        match self {
            Self::Game { depth, .. } | Self::Pack { depth, .. } => *depth,
        }
    }

    /// What marks this row, and the first column of the table.
    fn bullet(&self) -> &'static str {
        if self.depth() == 0 {
            BULLET
        } else {
            NESTED_BULLET
        }
    }

    /// What precedes this row's name: its bullet and the single space that joins them.
    fn lead(&self) -> String {
        format!("{} ", self.bullet())
    }

    /// The row as one line of cells, ready to be aligned, its name led by `lead`.
    ///
    /// A pack carries its lead and its name and nothing else: it has no reviews, and no store
    /// page to link to.
    ///
    /// The lead is a parameter because the two callers mark a row differently, and for different
    /// reasons. The listing writes a bullet, whose extra width is exactly what shows the nesting
    /// (see [`NESTED_BULLET`]). A form draws a checkbox of its own and indents a nested box
    /// itself, so a bullet there would be a second marker saying the same thing.
    fn cells(&self, palette: Palette, lead: &str) -> String {
        match self {
            Self::Pack { name, .. } => format!("{lead}{name}"),
            Self::Game { preview, .. } => {
                let mut cells = preview.cells(palette);
                cells[0] = format!("{lead}{}", cells[0]);
                cells.join("  ")
            }
        }
    }
}

/// Turns a bundle's games into the lines that will be printed, packs opened out.
fn flatten(games: &[Game], held: &Holdings, classified: &Classified) -> Vec<Row> {
    let mut rows = Vec::with_capacity(games.len());
    for game in games {
        if !game.is_pack() {
            rows.push(Row::Game {
                preview: Preview::of_game(game, held, classified),
                depth: 0,
            });
            continue;
        }
        rows.push(Row::Pack {
            name: game.title.clone(),
            depth: 0,
        });
        for inner in &game.contains {
            rows.push(Row::Game {
                preview: Preview::of_game(inner, held, classified),
                depth: 1,
            });
        }
    }
    rows
}

/// Where a row's own name starts, so its detail line can sit directly under it.
///
/// The bullet is part of the name cell now, so this is simply its width plus the space after it
/// — and it differs between a plain row and a nested one, which is the point.
fn name_column(row: &Row) -> usize {
    row.lead().len()
}

/// Pads the rows into columns.
///
/// Width is measured in terminal cells rather than characters, which is what a hand-rolled
/// `str::chars().count()` gets wrong: a CJK glyph is one character and two cells wide, so a
/// title like "白猫骑士物语" silently pushes its row two cells past every other. Colour codes are
/// excluded from the measurement for the same reason — they occupy no cells at all.
///
/// `reasonable_spacing` keeps one outlier from setting the width for the rest of its bundle,
/// the same way rendering per bundle keeps one bundle's outliers off every other bundle: a few
/// bundled titles run past seventy characters, and padding every row out to them would leave a
/// corridor of blanks before every link.
///
/// On failure the rows come back unpadded rather than not at all. The options here are fixed
/// and valid, so this should not happen — but ragged output a reader can still use beats losing
/// the listing to a layout problem.
fn align(rows: &[String]) -> Vec<String> {
    let options = FormatOptions {
        trim_trailing: true,
        reasonable_spacing: true,
        // Nothing here is a header, and no row must ever be reordered: the order is the
        // store's own, which is curation order.
        header: Some(false),
        space_rows: RowSpacing::Off,
        ..FormatOptions::default()
    };
    match format_table(rows, &options) {
        Ok(table) => table,
        Err(error) => {
            warn!("columns left unaligned: {error}");
            rows.to_vec()
        }
    }
}
