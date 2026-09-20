//! Steam's own Web API — the sanctioned way to read an account's library and wishlist.
//!
//! Its own handler, sharing nothing with [`super::epic`] but the snapshot shape: Steam publishes
//! a documented, key-gated HTTP API and Epic publishes none, so the two have no step in common.
//!
//! # Why a Web API key and not a password
//!
//! Valve issues a key that reads public profile data and nothing else. It cannot buy, cannot
//! change the account, and can be revoked from the page that issued it. Nothing here asks for a
//! Steam password, and nothing here would accept one.

use std::path::{Path, PathBuf};
use std::time::Duration;

use log::{debug, info};

use super::{Entry, Failure, Kind, Provenance, Report, Snapshot, config};
use crate::error::SafeUrl;
use crate::{Error, Result};

/// Name this store goes by in file names and provenance.
pub const STORE: &str = "steam";

/// Settings this handler reads, in the order they are asked for.
///
/// Public so that the command line can put the same questions to a person that the printed
/// guide writes out, from one table rather than two.
pub const SETTINGS: [config::Setting; 2] = [
    config::Setting {
        name: "steam_id",
        label: "SteamID",
        example: "<your 17-digit SteamID>",
        guidance: "The 17 digits at the end of a profile URL such as \
                   https://steamcommunity.com/profiles/76561197960287930 . If your profile shows \
                   a custom name instead, turn on Steam > View > Settings > Interface > \
                   \"Display Steam URL address bar\" to see the number.",
    },
    config::Setting {
        name: "steam_api_key",
        label: "Web API key",
        example: "<your Web API key>",
        guidance: "Register one at https://steamcommunity.com/dev/apikey . It reads public \
                   profile data only: it cannot buy anything, cannot change the account, and can \
                   be revoked from that same page at any time. The wishlist alone needs no key.",
    },
];

const ID_SETTING: &str = SETTINGS[0].name;
const KEY_SETTING: &str = SETTINGS[1].name;

/// The SteamID in `text`, whether it was given as the number or as the profile page it is on.
///
/// Nobody has their SteamID to hand; they have the page open. So a pasted
/// `https://steamcommunity.com/profiles/76561197960287930` is as good an answer as the digits,
/// and both are accepted wherever one is asked for.
///
/// A **vanity** URL — `steamcommunity.com/id/<name>` — is deliberately not converted. The name is
/// not the id and turning one into the other takes a request to Valve, so it is refused with an
/// explanation rather than guessed at.
#[must_use]
pub fn steam_id_from(text: &str) -> Option<String> {
    let text = text.trim();
    if is_steam_id(text) {
        return Some(text.to_owned());
    }
    // The host is required, not just the path: `/profiles/` on its own says nothing about
    // whose profile, and a number lifted out of an unrelated address is a guess.
    let after = text.split_once("steamcommunity.com/profiles/")?.1;
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    is_steam_id(&digits).then_some(digits)
}

const API_BASE: &str = "https://api.steampowered.com";

/// Where Valve issues a Web API key.
const KEY_PAGE: &str = "https://steamcommunity.com/dev/apikey";

/// Where the profile's visibility is set.
const PRIVACY_PAGE: &str = "https://steamcommunity.com/my/edit/settings";

/// A whole library or wishlist arrives in one response, so this only has to cover one slow read.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Identifies this client honestly, as Valve's terms of use ask.
const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (+",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);

/// Reads one account's library and wishlist.
///
/// Holds no session and refreshes nothing: each method is one request, and the credentials live
/// only as long as the value does.
pub struct Client {
    http: reqwest::blocking::Client,
    steam_id: String,
    /// `None` until a key is configured. Only the library needs one.
    key: Option<String>,
}

impl Client {
    /// Builds a client from the configured credentials.
    ///
    /// Fails with a guide rather than a diagnosis when the SteamID is missing, because at that
    /// point nothing has gone wrong — the operator simply has not told us whose library to read.
    pub fn new() -> Result<Self> {
        let credentials = config::Credentials::load()?;
        let Some(given) = credentials.get(ID_SETTING) else {
            return Err(Error::Setup {
                detail: guide("needs to know whose library to read"),
            });
        };
        let Some(steam_id) = steam_id_from(&given) else {
            return Err(Error::Setup {
                detail: format!(
                    "{ID_SETTING} is {given:?}, which is not a SteamID.\n\
                     A SteamID is 17 digits and starts with 7656119 — the number at the end of \
                     a profile URL like https://steamcommunity.com/profiles/76561197960287930 .\n\
                     A custom profile name (steamcommunity.com/id/<name>) is not one, and has \
                     to be converted."
                ),
            });
        };

        // Redirects are refused rather than followed. Valve's retired wishlist endpoint answers
        // a request it no longer serves with a 200 and a megabyte of store homepage; a client
        // that follows redirects reads that as success and writes an empty wishlist over a good
        // one. Refusing turns the same event into an HTTP status this can report.
        let http = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|source| Error::Fetch {
                url: API_BASE.into(),
                source: source.into(),
            })?;

        Ok(Self {
            http,
            steam_id,
            key: credentials.get(KEY_SETTING),
        })
    }

    /// Snapshots the games the account owns.
    ///
    /// Free games that have been played are included: owning one is indistinguishable from
    /// owning a paid one for anything this tool does with the list.
    pub fn library(&self) -> Result<Snapshot> {
        let Some(key) = self.key.as_deref() else {
            return Err(Error::Setup {
                detail: guide("needs a Web API key to read the owned-games list"),
            });
        };
        let response = self.get(&owned_games_url(key, &self.steam_id))?;
        parse_library(&response, &self.steam_id)
    }

    /// Snapshots the account's wishlist.
    ///
    /// Needs no API key — Valve serves this one on the SteamID alone, for any profile whose
    /// games are public.
    pub fn wishlist(&self) -> Result<Snapshot> {
        let response = self.get(&wishlist_url(&self.steam_id))?;
        parse_wishlist(&response, &self.steam_id)
    }

    /// The one place a URL carrying a key is handled.
    ///
    /// Everything that can print — the log line, every error — is built from `safe`, and the
    /// transport error has its own copy of the URL stripped on the way in. That is why the key
    /// may travel as a query parameter at all: Valve accepts it nowhere else.
    fn get(&self, url: &str) -> Result<String> {
        let safe = SafeUrl::from(url);
        debug!("GET {safe}");
        let response = self.http.get(url).send().map_err(|source| Error::Fetch {
            url: safe.clone(),
            source: source.into(),
        })?;

        let status = response.status();
        debug!("  HTTP {status}");
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited {
                url: safe.clone(),
                retry_after: response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
            });
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(Error::Setup {
                detail: format!(
                    "Steam refused the request with HTTP {status}.\n\
                     The usual cause is a Web API key that has been revoked or mistyped. \
                     Check it at {KEY_PAGE} .",
                ),
            });
        }
        if !status.is_success() {
            return Err(Error::Status {
                url: safe.clone(),
                status: status.as_u16(),
            });
        }

        // Checked because a retired endpoint here does not 404: it answers 200 with a store
        // page. Without this, that HTML would reach the JSON parser and be reported as a schema
        // change in data Steam never sent.
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        if !content_type.starts_with("application/json") {
            return Err(Error::Drift {
                subject: safe.to_string(),
                detail: format!(
                    "answered with {content_type:?} instead of JSON; \
                     this endpoint is no longer serving the API"
                ),
            });
        }

        let body = response.text().map_err(|source| Error::Fetch {
            url: safe,
            source: source.into(),
        })?;
        debug!("  {} bytes", body.len());
        Ok(body)
    }
}

/// Writes both Steam snapshots into `directory`.
///
/// Each is attempted whatever the other did. A public library beside a private wishlist is an
/// ordinary configuration, and losing the library snapshot over it would be a poor trade.
pub fn snapshot(directory: &Path) -> Report {
    let client = match Client::new() {
        Ok(client) => client,
        // Nothing can be read at all, so the failure belongs to the store rather than to one
        // snapshot — reported once instead of repeating the same guide twice.
        Err(reason) => {
            return Report {
                written: Vec::new(),
                failures: vec![Failure { what: None, reason }],
            };
        }
    };

    let mut report = Report::default();
    report.take(Kind::Library, directory, || client.library());
    report.take(Kind::Wishlist, directory, || client.wishlist());
    report
}

/// Writes just the owned-games snapshot.
pub fn library(directory: &Path) -> Result<PathBuf> {
    Client::new()?.library()?.write(directory)
}

/// Writes just the wishlist snapshot.
pub fn wishlist(directory: &Path) -> Result<PathBuf> {
    Client::new()?.wishlist()?.write(directory)
}

/// Reads a `GetOwnedGames` response into a snapshot.
///
/// Takes text and does no I/O, like every other parser in this crate, so a caller with its own
/// HTTP stack can use it without [`Client`].
pub fn parse_library(json: &str, steam_id: &str) -> Result<Snapshot> {
    let response = decode(json, &owned_games_url("", steam_id))?;

    // An account whose game details are private answers with an empty object rather than an
    // error, so "no games key" is the wire's way of saying "not allowed to look" — never a
    // library with nothing in it. Writing that as an empty snapshot would erase a good one.
    let Some(games) = response["response"]["games"].as_array() else {
        return Err(Error::Setup {
            detail: private_profile("owned games"),
        });
    };

    let entries: Vec<Entry> = games.iter().filter_map(owned_entry).collect();
    info!("steam library: {} games", entries.len());
    Ok(Snapshot::new(
        STORE,
        Kind::Library,
        Provenance {
            // Built with an empty key so that the recorded URL takes the redacted shape even
            // here, where no key was ever in hand. A snapshot is a file the operator may pass on.
            source_url: redacted(&owned_games_url("", steam_id)),
            account: Some(steam_id.to_owned()),
            reported_count: response["response"]["game_count"]
                .as_u64()
                .and_then(|count| u32::try_from(count).ok()),
            read_with: None,
        },
        entries,
    ))
}

/// Reads a `GetWishlist` response into a snapshot.
pub fn parse_wishlist(json: &str, steam_id: &str) -> Result<Snapshot> {
    let url = wishlist_url(steam_id);
    let response = decode(json, &url)?;

    // Same rule as the library, and the reason it is stated twice: an absent `items` is a
    // refusal, an `items: []` is a genuinely empty wishlist, and only the second may be
    // written. Collapsing them would let a privacy change quietly empty the file.
    let Some(items) = response["response"]["items"].as_array() else {
        return Err(Error::Setup {
            detail: private_profile("wishlist"),
        });
    };

    let entries: Vec<Entry> = items.iter().filter_map(wishlist_entry).collect();
    info!("steam wishlist: {} games", entries.len());
    Ok(Snapshot::new(
        STORE,
        Kind::Wishlist,
        Provenance {
            source_url: redacted(&url),
            account: Some(steam_id.to_owned()),
            reported_count: None,
            read_with: None,
        },
        entries,
    ))
}

/// Parses one of Steam's JSON responses, naming the endpoint if it will not parse.
fn decode(json: &str, url: &str) -> Result<serde_json::Value> {
    serde_json::from_str(json).map_err(|source| Error::Payload {
        url: redacted(url).into(),
        source,
    })
}

fn owned_games_url(key: &str, steam_id: &str) -> String {
    format!(
        "{API_BASE}/IPlayerService/GetOwnedGames/v1/\
         ?key={key}&steamid={steam_id}&include_appinfo=1&include_played_free_games=1&format=json"
    )
}

fn wishlist_url(steam_id: &str) -> String {
    format!("{API_BASE}/IWishlistService/GetWishlist/v1/?steamid={steam_id}")
}

/// One row of `GetOwnedGames`.
///
/// A row with no `appid` is dropped rather than guessed at: the id is the only field that makes
/// an entry useful, and a row without one cannot be matched to anything.
fn owned_entry(game: &serde_json::Value) -> Option<Entry> {
    let app_id = u32::try_from(game["appid"].as_u64()?).ok()?;
    Some(Entry {
        id: app_id.to_string(),
        app_id: Some(app_id),
        name: game["name"].as_str().map(str::to_owned),
        playtime_minutes: game["playtime_forever"]
            .as_u64()
            .and_then(|minutes| u32::try_from(minutes).ok()),
        priority: None,
    })
}

/// One row of `GetWishlist`, which carries an id and a rank and no title.
fn wishlist_entry(item: &serde_json::Value) -> Option<Entry> {
    let app_id = u32::try_from(item["appid"].as_u64()?).ok()?;
    Some(Entry {
        id: app_id.to_string(),
        app_id: Some(app_id),
        name: None,
        playtime_minutes: None,
        priority: item["priority"]
            .as_u64()
            .and_then(|rank| u32::try_from(rank).ok()),
    })
}

/// A URL with its key removed, safe to record in a file the operator may pass on.
fn redacted(url: &str) -> String {
    SafeUrl::from(url).as_str().to_owned()
}

/// Whether a string is shaped like a 64-bit SteamID.
///
/// Individual accounts occupy a block that begins at 76561197960265728, so every personal
/// SteamID is 17 digits starting `7656119`. Checked because the commonest mistake is pasting a
/// custom profile name, and Steam answers that with an empty response rather than an error —
/// indistinguishable, without this, from a private profile.
fn is_steam_id(value: &str) -> bool {
    value.len() == 17 && value.bytes().all(|b| b.is_ascii_digit()) && value.starts_with("7656119")
}

/// The full setup guide, introduced by what this particular run was missing.
fn guide(missing: &str) -> String {
    format!(
        "gamelib steam {missing}.\n\n\
         1. Your SteamID — the 17 digits at the end of a profile URL such as\n   \
            https://steamcommunity.com/profiles/76561197960287930 .\n   \
            If your profile shows a custom name instead, turn on\n   \
            Steam > View > Settings > Interface > \"Display Steam URL address bar\"\n   \
            to see the number.\n\n\
         2. A Steam Web API key, registered at\n   {KEY_PAGE}\n   \
            It reads public profile data only: it cannot buy anything, cannot change\n   \
            the account, and can be revoked from that same page at any time.\n   \
            The wishlist alone needs no key — a SteamID is enough for that one.\n\n\
         {}\n",
        config::how_to_write(&SETTINGS)
    )
}

/// What to say when Steam declines to show something.
fn private_profile(what: &str) -> String {
    format!(
        "Steam returned no {what} for this account.\n\
         It answers that way for a profile whose game details are not public, so this is \
         almost certainly a privacy setting rather than an empty {what}.\n\n\
         Set \"Game details\" to Public at\n    {PRIVACY_PAGE}\n\
         run this command again, and set it back afterwards — the snapshot is a file on \
         your own disk and never needs the setting again.\n\n\
         If the profile is already public, check that {ID_SETTING} is the right account."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_page_is_as_good_an_answer_as_the_number() {
        // Nobody has their SteamID to hand; they have the page open.
        for text in [
            "76561197960287930",
            "  76561197960287930  ",
            "https://steamcommunity.com/profiles/76561197960287930",
            "https://steamcommunity.com/profiles/76561197960287930/",
            "steamcommunity.com/profiles/76561197960287930/games/?tab=all",
            "https://steamcommunity.com/profiles/76561197960287930?snr=1_2_3",
        ] {
            assert_eq!(
                steam_id_from(text).as_deref(),
                Some("76561197960287930"),
                "{text}"
            );
        }
    }

    #[test]
    fn a_vanity_name_is_refused_rather_than_guessed_at() {
        // The name is not the id, and turning one into the other takes a request to Valve.
        for text in [
            "https://steamcommunity.com/id/gabelogannewell",
            "https://steamcommunity.com/id/gabelogannewell/",
            "gabelogannewell",
        ] {
            assert_eq!(steam_id_from(text), None, "{text}");
        }
    }

    #[test]
    fn a_url_that_only_looks_right_is_refused() {
        for text in [
            // The digits after `/profiles/` should be an id, and these are not.
            "https://steamcommunity.com/profiles/12345678901234567",
            "https://steamcommunity.com/profiles/",
            "https://steamcommunity.com/profiles/7656119",
            // And a number lifted out of an unrelated address is a guess, not an answer.
            "https://example.test/profiles/76561197960287930",
        ] {
            assert_eq!(steam_id_from(text), None, "{text}");
        }
    }

    #[test]
    fn a_profile_url_number_is_recognised_and_a_custom_name_is_not() {
        assert!(is_steam_id("76561197960287930"));
        assert!(
            !is_steam_id("gabelogannewell"),
            "a custom name is not an id"
        );
        assert!(!is_steam_id("7656119796028793"), "16 digits is too few");
        assert!(!is_steam_id("765611979602879301"), "18 digits is too many");
        assert!(!is_steam_id("12345678901234567"), "wrong account block");
        assert!(!is_steam_id(""));
    }

    #[test]
    fn an_owned_game_keeps_its_id_name_and_playtime() {
        let row = serde_json::json!({
            "appid": 1202130,
            "name": "Starfield",
            "playtime_forever": 431,
            "img_icon_url": "ignored"
        });
        let entry = owned_entry(&row).expect("a complete row");
        assert_eq!(entry.id, "1202130");
        assert_eq!(entry.app_id, Some(1_202_130));
        assert_eq!(entry.name.as_deref(), Some("Starfield"));
        assert_eq!(entry.playtime_minutes, Some(431));
        assert_eq!(entry.priority, None);
    }

    #[test]
    fn a_game_without_an_app_id_is_dropped_rather_than_invented() {
        assert!(owned_entry(&serde_json::json!({ "name": "Nameless" })).is_none());
    }

    #[test]
    fn an_unplayed_game_is_kept_with_a_playtime_of_zero() {
        // Zero is a fact; treating it as missing would lose the difference between
        // "owned and never started" and "playtime withheld".
        let entry =
            owned_entry(&serde_json::json!({ "appid": 10, "playtime_forever": 0 })).expect("a row");
        assert_eq!(entry.playtime_minutes, Some(0));
    }

    #[test]
    fn withheld_playtime_stays_absent_rather_than_becoming_zero() {
        let entry = owned_entry(&serde_json::json!({ "appid": 10 })).expect("a row");
        assert_eq!(entry.playtime_minutes, None);
    }

    #[test]
    fn a_wishlist_row_keeps_its_rank() {
        let entry =
            wishlist_entry(&serde_json::json!({ "appid": 570, "priority": 3 })).expect("a row");
        assert_eq!(entry.app_id, Some(570));
        assert_eq!(entry.priority, Some(3));
        assert_eq!(entry.name, None, "this endpoint publishes no titles");
    }

    #[test]
    fn the_key_never_reaches_the_recorded_provenance() {
        let key = "0123456789ABCDEF0123456789ABCDEF";
        let recorded = redacted(&format!(
            "{API_BASE}/IPlayerService/GetOwnedGames/v1/?key={key}&steamid=76561197960287930"
        ));
        assert!(!recorded.contains(key), "{recorded}");
        assert!(recorded.contains("steamid=76561197960287930"), "{recorded}");
    }

    #[test]
    fn the_guide_names_both_things_the_operator_has_to_fetch() {
        let text = guide("needs a Web API key");
        assert!(text.contains(KEY_PAGE), "{text}");
        assert!(text.contains("SteamID"), "{text}");
        assert!(text.contains("steam_api_key"), "{text}");
        assert!(text.contains("steam_id"), "{text}");
    }

    #[test]
    fn the_privacy_message_says_the_change_can_be_undone() {
        let text = private_profile("owned games");
        assert!(text.contains(PRIVACY_PAGE), "{text}");
        assert!(text.contains("set it back afterwards"), "{text}");
    }
}
