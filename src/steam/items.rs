//! What Steam's item service says an id IS: an app, a package, a bundle, or nothing at all.
//!
//! [`crate::inventory::steam::identity_of`] settles a published id only when a table holds it.
//! The rest — an id no table holds, or one a table names as something else — needs Steam, and
//! this asks in ONE request per run for every such id at once, each under every kind it could
//! be: app, package and bundle ids are separate namespaces of the same small integers, and the
//! store that published the number did not say which it meant. Fanatical writes bundle ids into
//! the field it calls `steam_id`: 13009 is `/bundle/13009`, and `/app/13009` bounces to the
//! store front.
//!
//! Fetching and parsing are separate, as everywhere in this crate: [`request_json`] and
//! [`parse`] do no I/O, [`Classified::from_answers`] applies the rules, and [`Classified::fetch`]
//! is the one call that talks to Steam — through [`Client`], so the User-Agent, the pacing and
//! the throttle handling exist once.
//!
//! # What the answers do, measured — fixture `getitems-13009-4278390-620.json`
//!
//! - **A refused probe still answers.** `success: 15` — an `EResult`, never a boolean, so the
//!   test is `== 1` and nothing looser — an empty name, and a `store_url_path` built from the id
//!   alone: `app/0/` for an app, `bundle/620/` for a bundle. The path's SHAPE is therefore no
//!   signal at all: refusals look kind-correct and a success can look kind-wrong (below).
//! - **An id can be several things at once.** 620 is app 620 (Portal 2) AND package 620 — "18
//!   Wheels of Steel American Long Haul", whose path is that app's page, `app/12520/…`. Both
//!   claims are kept, and which one a game gets is decided by NAME: the service returns each
//!   claim's name, and the claim whose name agrees with the store's title is the one the store
//!   meant — the join's own rule, [`same_title`]. Only where no name agrees does precedence
//!   decide, app then bundle then package: a store writing a number into a field called
//!   `steam_id` almost always means the app, and bundles are what stores publish where packages
//!   are internal SKUs. An id claimed by more than one kind is reported ([`Classified::shared`])
//!   rather than resolved in silence: it means a store's feed is ambiguous.
//! - **A package resolves to an app's page**, so the link is the service's path verbatim, never
//!   `/sub/<id>` built from the kind — and re-encoded on the way out, since it is external text
//!   and the opener script's safety rests on every URL being safe by construction.
//! - **`visible` is a second gate.** An item can succeed and be `visible: false` — restricted,
//!   delisted, withdrawn — and its page then behaves like the bounce this exists to remove.
//! - **A removed app answers nothing under any kind.** A class of its own, not a bad id: its
//!   page would bounce, so its link becomes a search. "Asked, and nothing claims it" is kept
//!   apart from "never asked", the distinction the deck report's empty result and the wishlist's
//!   absent key already draw — [`Classified::none`] collapses into neither.
//! - **Answers are matched on `item_type` and `id`, never on position.** An ask the service
//!   leaves out entirely (the sweep has seen it) leaves that id exactly as it was — unasked —
//!   rather than "nothing", and is counted ([`Classified::unanswered`]) so a run can say so.
//! - **The cap is the encoded request's length, and the service answers HTTP 400 rather than
//!   truncating**: 200 asks were fine and 500 refused. Asks, not ids, are what count — an unheld
//!   id spends three. [`BATCH`] sits well under the measured limit, and a 400 splits the chunk
//!   and tries the halves before the run is given up.
//! - **The request names one country.** A title unsold there answers as a refusal, so an app the
//!   store restricts by region is classified as nothing and gets a search link — rare, and the
//!   search then finds it. The sweep, which needs the app itself, asks per country instead.

use std::collections::{BTreeMap, HashMap};

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;

use super::store::Client;
use crate::error::SafeUrl;
use crate::inventory::steam::{Identity, identity_of, same_title};
use crate::{Error, Listing, Result};

const ENDPOINT: &str = "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/";

/// How many asks go into one request: under the measured cap (200 fine, 500 refused) with room
/// for the encoded request to grow, and what the sweep tool has sent since 2026-09-25.
pub const BATCH: usize = 150;

/// What a store path may carry unencoded: the unreserved set and the `/` between its segments.
/// Everything else — a stray quote, a space, a `$` — is encoded, so a path is a URL a shell
/// script can be handed without further thought.
const PATH: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'/');

/// The kinds of thing the store sells under a small-integer id, in the service's own numbering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    App,
    Package,
    Bundle,
}

impl Kind {
    /// The order claims are kept in, and the tiebreak where no name agrees with the title.
    const PRECEDENCE: [Self; 3] = [Self::App, Self::Bundle, Self::Package];

    /// The request key that asks for this kind.
    const fn key(self) -> &'static str {
        match self {
            Self::App => "appid",
            Self::Package => "packageid",
            Self::Bundle => "bundleid",
        }
    }

    /// The service's `item_type` for this kind, and `None` for a number it has not used yet.
    const fn of(item_type: i64) -> Option<Self> {
        match item_type {
            0 => Some(Self::App),
            1 => Some(Self::Package),
            2 => Some(Self::Bundle),
            _ => None,
        }
    }
}

/// One id, asked as one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ask {
    pub id: u32,
    pub kind: Kind,
}

/// One kind's claim on an id, as the service made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub kind: Kind,
    pub name: String,
    /// Steam's own path for it, verbatim:
    /// `bundle/13009/Monster_Hunter_World_Iceborne_Digital_Deluxe`.
    pub path: String,
}

impl Found {
    /// The page, on the store, safe to hand to a shell script: the path is external text and is
    /// encoded here, so the safety does not rest on Steam's slugs staying tame.
    #[must_use]
    pub fn url(&self) -> String {
        format!(
            "{}{}",
            super::STORE_BASE,
            utf8_percent_encode(&self.path, PATH)
        )
    }
}

/// Of several claims on one id, the one a store meant when it published the id beside `title`:
/// the claim whose name agrees with the title, else the first by precedence.
#[must_use]
pub fn meant<'a>(claims: &'a [Found], title: &str) -> &'a Found {
    claims
        .iter()
        .find(|claim| same_title(&claim.name, title))
        .unwrap_or(&claims[0])
}

/// What one run knows about one id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer<'a> {
    /// The service was never asked — no run classified it, the request failed, or the service
    /// left the ask out of its answer.
    Unasked,
    /// Asked under every kind it could be, and nothing claims it.
    Nothing,
    /// Every kind that claims it, in precedence order; never empty. [`meant`] picks one.
    Found(&'a [Found]),
}

/// The answers one run collected, keyed by id.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Classified {
    claims: HashMap<u32, Vec<Found>>,
    unanswered: Vec<u32>,
}

impl Classified {
    /// Nothing asked: what a caller passes when it does not want the network — a test, an offline
    /// run, a library consumer with no Steam traffic to justify. Every id then answers
    /// [`Answer::Unasked`], which is not [`Answer::Nothing`].
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// What this run knows about `id`.
    #[must_use]
    pub fn of(&self, id: u32) -> Answer<'_> {
        match self.claims.get(&id).map(Vec::as_slice) {
            None => Answer::Unasked,
            Some([]) => Answer::Nothing,
            Some(claims) => Answer::Found(claims),
        }
    }

    /// How many ids were asked and answered.
    #[must_use]
    pub fn asked(&self) -> usize {
        self.claims.len()
    }

    /// The ids the service was asked about and left out of its answer, ascending. Each still
    /// answers [`Answer::Unasked`]; this is for a run to say so.
    #[must_use]
    pub fn unanswered(&self) -> &[u32] {
        &self.unanswered
    }

    /// The ids more than one kind claims, ascending, with what claims them: a store feed that
    /// published such a number is ambiguous, and the reader should know a choice was made.
    pub fn shared(&self) -> impl Iterator<Item = (u32, &[Found])> {
        let mut shared: Vec<(u32, &[Found])> = self
            .claims
            .iter()
            .filter(|(_, claims)| claims.len() > 1)
            .map(|(id, claims)| (*id, claims.as_slice()))
            .collect();
        shared.sort_unstable_by_key(|(id, _)| *id);
        shared.into_iter()
    }

    /// Every ask these listings need, each id once under each kind it could be — and nothing for
    /// a game the tables already settled.
    ///
    /// An unheld id is asked three ways. A disputed one is asked as a bundle and a package only:
    /// the one thing the tables know about it is that the app under that number is something
    /// else.
    #[must_use]
    pub fn asks<'a>(listings: impl IntoIterator<Item = &'a Listing>) -> Vec<Ask> {
        let mut asks = std::collections::BTreeSet::new();
        let games = listings
            .into_iter()
            .flat_map(|listing| &listing.bundles)
            .flat_map(|bundle| &bundle.games)
            .flat_map(|game| std::iter::once(game).chain(&game.contains));
        for game in games {
            let (id, kinds): (u32, &[Kind]) = match identity_of(game) {
                Identity::Unheld(id) => (id, &Kind::PRECEDENCE),
                Identity::Disputed(id) => (id, &[Kind::Bundle, Kind::Package]),
                Identity::App(_) | Identity::Unknown => continue,
            };
            asks.extend(kinds.iter().map(|kind| Ask { id, kind: *kind }));
        }
        asks.into_iter().collect()
    }

    /// The rules, applied to what came back. An id answered under EVERY kind it was asked as gets
    /// its claims, in precedence order, or none; an id with an ask the service left out is
    /// recorded as unanswered and stays unasked.
    #[must_use]
    pub fn from_answers(asks: &[Ask], answered: &[Answered]) -> Self {
        let mut kinds_asked: BTreeMap<u32, Vec<Kind>> = BTreeMap::new();
        for ask in asks {
            kinds_asked.entry(ask.id).or_default().push(ask.kind);
        }
        let mut claims = HashMap::new();
        let mut unanswered = Vec::new();
        for (id, kinds) in kinds_asked {
            let answer_as = |kind: Kind| {
                answered
                    .iter()
                    .find(|answer| answer.id == id && Kind::of(answer.item_type) == Some(kind))
            };
            if !kinds.iter().all(|kind| answer_as(*kind).is_some()) {
                unanswered.push(id);
                continue;
            }
            let found: Vec<Found> = Kind::PRECEDENCE
                .into_iter()
                .filter(|kind| kinds.contains(kind))
                .filter_map(|kind| answer_as(kind).and_then(Answered::found))
                .collect();
            claims.insert(id, found);
        }
        Self { claims, unanswered }
    }

    /// Asks Steam — [`BATCH`] asks per request, the client's pause between requests — and applies
    /// [`Self::from_answers`]. No asks, no request.
    ///
    /// # Errors
    ///
    /// Whatever [`Client::get`] reports, and [`Error::Payload`] for an answer that is not the
    /// service's JSON. A caller rendering a listing should turn either into a note and carry on
    /// with [`Self::none`]: the links are then what the stores published, which is what they were
    /// before this existed.
    pub fn fetch(client: &Client, asks: &[Ask]) -> Result<Self> {
        let mut answered = Vec::with_capacity(asks.len());
        for (position, chunk) in asks.chunks(BATCH).enumerate() {
            if position > 0 {
                std::thread::sleep(client.spacing());
            }
            answered.extend(fetch_chunk(client, chunk)?);
        }
        Ok(Self::from_answers(asks, &answered))
    }
}

/// One request, or two halves of it: the cap is on the encoded request's length and the service
/// answers HTTP 400 rather than truncating (200 asks fine, 500 refused), so a refusal of a chunk
/// that can still be split is not yet a failed run.
fn fetch_chunk(client: &Client, chunk: &[Ask]) -> Result<Vec<Answered>> {
    let url = request_url(chunk);
    match client.get(&url, false) {
        Ok(body) => parse(&body, &url),
        Err(Error::Status { status: 400, .. }) if chunk.len() > 1 => {
            let (left, right) = chunk.split_at(chunk.len() / 2);
            let mut answered = fetch_chunk(client, left)?;
            std::thread::sleep(client.spacing());
            answered.extend(fetch_chunk(client, right)?);
            Ok(answered)
        }
        Err(other) => Err(other),
    }
}

/// The request body: each ask as `{"<kind>id": id}`, one country, and no data beyond the item's
/// identity — `data_request: {}` is what keeps a run's one request small.
#[must_use]
pub fn request_json(asks: &[Ask]) -> String {
    let ids: Vec<serde_json::Value> = asks
        .iter()
        .map(|ask| {
            let mut one = serde_json::Map::new();
            one.insert(ask.kind.key().to_owned(), ask.id.into());
            serde_json::Value::Object(one)
        })
        .collect();
    serde_json::json!({
        "ids": ids,
        "context": {"language": "english", "country_code": "US", "steam_realm": 1},
        "data_request": {}
    })
    .to_string()
}

/// The request as a URL: the service takes its JSON in the query string.
#[must_use]
pub fn request_url(asks: &[Ask]) -> String {
    format!(
        "{ENDPOINT}?input_json={}",
        utf8_percent_encode(&request_json(asks), NON_ALPHANUMERIC)
    )
}

/// One answer, as the service printed it. Every field defaults, because a refusal omits most of
/// them and a shape the service adds tomorrow must not turn every answer into an error.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Answered {
    #[serde(default)]
    pub item_type: i64,
    /// The id asked — filled in a refusal too, where `appid` is 0.
    #[serde(default)]
    pub id: u32,
    /// An `EResult`: 1 is success and 15 is the usual refusal. Never read as a boolean.
    #[serde(default)]
    pub success: i64,
    /// Whether the store shows it. Defaults to shown, as the sweep reads it: the field is absent
    /// from some answers, and an absent flag is not a hidden item.
    #[serde(default = "shown")]
    pub visible: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub store_url_path: String,
}

const fn shown() -> bool {
    true
}

impl Answered {
    /// What this answer claims: a page, or `None` for a refusal, a hidden item, or a kind the
    /// service has not used before.
    #[must_use]
    pub fn found(&self) -> Option<Found> {
        (self.success == 1 && self.visible && !self.store_url_path.is_empty())
            .then(|| Kind::of(self.item_type))
            .flatten()
            .map(|kind| Found {
                kind,
                name: self.name.clone(),
                path: self.store_url_path.clone(),
            })
    }
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    response: Body,
}

#[derive(Deserialize, Default)]
struct Body {
    #[serde(default)]
    store_items: Vec<Answered>,
}

/// The answers in one response body. `url` names the request in the error, redacted as every URL
/// in an error is.
///
/// # Errors
///
/// [`Error::Payload`] when the text is not the service's JSON.
pub fn parse(json: &str, url: &str) -> Result<Vec<Answered>> {
    let envelope: Envelope = serde_json::from_str(json).map_err(|source| Error::Payload {
        url: SafeUrl::from(url),
        source,
    })?;
    Ok(envelope.response.store_items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bundle, Game};

    const PROBE: &str = include_str!("../../tests/fixtures/steam/getitems-13009-4278390-620.json");

    fn three_ways(id: u32) -> Vec<Ask> {
        Kind::PRECEDENCE
            .into_iter()
            .map(|kind| Ask { id, kind })
            .collect()
    }

    /// The nine asks the fixture answers, in the order they were made.
    fn probe_asks() -> Vec<Ask> {
        [13_009, 4_278_390, 620]
            .into_iter()
            .flat_map(three_ways)
            .collect()
    }

    fn answered() -> Vec<Answered> {
        parse(PROBE, ENDPOINT).expect("the fixture parses")
    }

    fn probe() -> Classified {
        Classified::from_answers(&probe_asks(), &answered())
    }

    fn game(title: &str, app_id: Option<u32>) -> Game {
        Game {
            title: title.to_owned(),
            machine_name: title.to_lowercase(),
            steam_app_id: app_id,
            contains: Vec::new(),
        }
    }

    fn listing(games: Vec<Game>) -> Listing {
        Listing {
            bundles: vec![Bundle {
                title: "A Bundle".to_owned(),
                url: "https://example.test".to_owned(),
                price: None,
                ends_at: None,
                games,
            }],
            problems: Vec::new(),
        }
    }

    #[test]
    fn every_ask_is_one_id_under_one_key_and_nothing_else_is_requested() {
        let json = request_json(&three_ways(13_009));
        let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
        assert_eq!(
            value["ids"],
            serde_json::json!([{"appid": 13009}, {"bundleid": 13009}, {"packageid": 13009}])
        );
        assert_eq!(
            value["data_request"],
            serde_json::json!({}),
            "identity only: {json}"
        );
        let url = request_url(&three_ways(13_009));
        assert!(
            url.starts_with(ENDPOINT) && url.contains("input_json=%7B"),
            "{url}"
        );
    }

    /// Six of the nine fixture answers are refusals, and a refusal's path is a decoy — built
    /// from the id alone, `app/0/` for an app — so it must never become a page. `success` is an
    /// `EResult`, 15 here, which any truthiness test would read as success.
    #[test]
    fn a_refusal_is_not_a_page_whatever_path_it_carries() {
        let answered = answered();
        assert_eq!(answered.len(), 9);
        let refused: Vec<&Answered> = answered.iter().filter(|a| a.success != 1).collect();
        assert_eq!(refused.len(), 6, "{answered:#?}");
        for answer in refused {
            assert_eq!(answer.success, 15, "an EResult, not a boolean: {answer:?}");
            assert!(
                !answer.store_url_path.is_empty(),
                "the decoy is there: {answer:?}"
            );
            assert_eq!(answer.found(), None, "{answer:?}");
        }
    }

    /// A success the store does not show is no page either: its page behaves like the bounce.
    #[test]
    fn a_hidden_success_is_not_a_page() {
        let hidden = Answered {
            item_type: 0,
            id: 620,
            success: 1,
            visible: false,
            name: "Portal 2".to_owned(),
            store_url_path: "app/620/Portal_2".to_owned(),
        };
        assert_eq!(hidden.found(), None);
        let shown = Answered {
            visible: true,
            ..hidden
        };
        assert_eq!(shown.found().map(|found| found.kind), Some(Kind::App));
    }

    #[test]
    fn a_bundle_id_a_store_published_as_an_app_is_found_as_the_bundle() {
        let classified = probe();
        let Answer::Found(claims) = classified.of(13_009) else {
            panic!("13009 is a bundle: {:?}", classified.of(13_009));
        };
        assert_eq!(claims.len(), 1, "{claims:?}");
        assert_eq!(claims[0].kind, Kind::Bundle);
        assert_eq!(
            claims[0].name,
            "Monster Hunter World: Iceborne Digital Deluxe"
        );
        assert_eq!(
            claims[0].url(),
            "https://store.steampowered.com/bundle/13009/Monster_Hunter_World_Iceborne_Digital_Deluxe"
        );
    }

    /// 620 is claimed as an app AND as a package, and the store's title decides which was meant:
    /// the name the service returned for each claim, compared as the join compares titles.
    #[test]
    fn an_id_two_kinds_claim_goes_to_the_kind_whose_name_agrees_with_the_title() {
        let classified = probe();
        let Answer::Found(claims) = classified.of(620) else {
            panic!("620 is claimed twice");
        };
        assert_eq!(
            claims.iter().map(|claim| claim.kind).collect::<Vec<_>>(),
            [Kind::App, Kind::Package],
            "kept in precedence order"
        );
        assert_eq!(
            meant(claims, "Portal 2").url(),
            "https://store.steampowered.com/app/620/Portal_2"
        );
        assert_eq!(
            meant(claims, "18 Wheels of Steel: American Long Haul").url(),
            "https://store.steampowered.com/app/12520/18_Wheels_of_Steel_American_Long_Haul",
            "the package, by its name — and the page is an app's, as the service says"
        );
        assert_eq!(
            meant(claims, "a title no claim carries").kind,
            Kind::App,
            "precedence is only the tiebreak"
        );
        assert_eq!(
            probe().shared().map(|(id, _)| id).collect::<Vec<_>>(),
            [620],
            "and the collision is reported"
        );
    }

    /// The same, with the answers reversed: precedence is by kind, not by which answer came first.
    #[test]
    fn precedence_is_by_kind_not_by_the_order_answers_arrive_in() {
        let mut reversed = answered();
        reversed.reverse();
        let classified = Classified::from_answers(&probe_asks(), &reversed);
        assert!(
            matches!(classified.of(620), Answer::Found([first, ..]) if first.kind == Kind::App)
        );
        assert!(
            matches!(classified.of(13_009), Answer::Found([only]) if only.kind == Kind::Bundle)
        );
    }

    #[test]
    fn an_id_nothing_claims_is_nothing_and_one_never_asked_is_unasked() {
        assert_eq!(
            probe().of(4_278_390),
            Answer::Nothing,
            "a removed app: refused three ways"
        );
        assert_eq!(probe().of(999_999), Answer::Unasked, "never in the request");
        assert_eq!(
            Classified::none().of(13_009),
            Answer::Unasked,
            "none() asked nothing"
        );
        assert_eq!(probe().asked(), 3);
        assert!(probe().unanswered().is_empty());
    }

    /// The sweep has seen the service leave an ask out of its answer entirely. That is not a
    /// refusal, so the id must not become "nothing" — a search link — on the service's silence.
    #[test]
    fn an_ask_the_service_left_out_leaves_the_id_unasked_and_is_counted() {
        let only_the_app_refusal: Vec<Answered> = answered()
            .into_iter()
            .filter(|answer| answer.id == 13_009 && answer.item_type == 0)
            .collect();
        let classified = Classified::from_answers(&three_ways(13_009), &only_the_app_refusal);
        assert_eq!(classified.of(13_009), Answer::Unasked);
        assert_eq!(classified.unanswered(), [13_009]);
    }

    /// The path is external text on its way into a shell script: encoded, with its slashes kept.
    #[test]
    fn a_path_is_encoded_on_the_way_out_and_keeps_its_slashes() {
        let odd = Found {
            kind: Kind::Bundle,
            name: String::new(),
            path: "bundle/1/It's $5 & \"more\"/".to_owned(),
        };
        assert_eq!(
            odd.url(),
            "https://store.steampowered.com/bundle/1/It%27s%20%245%20%26%20%22more%22/"
        );
    }

    /// What a listing asks for: nothing for a settled game, three kinds for an unheld id, two for
    /// a disputed one, a pack's games as its own, and each ask once however many bundles repeat it.
    #[test]
    fn a_listing_asks_only_about_the_ids_the_tables_could_not_settle() {
        let mut pack = game("A Pack", None);
        pack.contains = vec![game(
            "Monster Hunter World: Iceborne Digital Deluxe",
            Some(13_009),
        )];
        let games = vec![
            game("Portal 2", Some(620)),
            game("Portal 2", None),
            game("a title matching nothing", None),
            game("a title matching nothing", Some(620)),
            game(
                "Monster Hunter World: Iceborne Digital Deluxe",
                Some(13_009),
            ),
            pack,
        ];
        let asks = Classified::asks([&listing(games)]);
        assert_eq!(
            asks,
            vec![
                Ask {
                    id: 620,
                    kind: Kind::Package
                },
                Ask {
                    id: 620,
                    kind: Kind::Bundle
                },
                Ask {
                    id: 13_009,
                    kind: Kind::App
                },
                Ask {
                    id: 13_009,
                    kind: Kind::Package
                },
                Ask {
                    id: 13_009,
                    kind: Kind::Bundle
                },
            ],
            "{asks:?}"
        );
        assert!(Classified::asks([&listing(vec![game("Portal 2", Some(620))])]).is_empty());
    }

    #[test]
    fn a_body_that_is_not_the_services_json_is_a_payload_error() {
        let Err(Error::Payload { .. }) = parse("<html>", ENDPOINT) else {
            panic!("HTML is not an answer");
        };
        assert!(
            parse("{}", ENDPOINT)
                .expect("an empty envelope parses")
                .is_empty()
        );
    }
}
