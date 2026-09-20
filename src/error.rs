//! Error type shared by every vendor, and the two wrappers that keep secrets out of it.

use std::fmt;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Query parameters whose value is a credential, lower-case.
///
/// Steam's Web API takes its key as `?key=…` because Valve accepts it nowhere else — not a
/// header, not a body field — so the one thing that must never be printed is carried by the one
/// thing errors and logs print by default. The rest of the list is the usual company that
/// parameter keeps; costless to cover, and each one is a leak the day some other store is added.
const SECRET_PARAMETERS: &[&str] = &[
    "key",
    "api_key",
    "apikey",
    "access_token",
    "token",
    "auth",
    "authorization",
    "password",
    "passwd",
    "secret",
    "sessionid",
    "session_id",
    "signature",
    "sig",
];

/// What replaces a secret's value.
const REDACTION: &str = "REDACTED";

/// A URL that cannot print a credential, whatever it was built from.
///
/// The redaction happens when the value is created, not when it is shown, so there is no path
/// that reaches the secret afterwards: [`fmt::Display`] and [`fmt::Debug`] both read the same
/// already-cleaned string. That matters because an error is shown both ways — `Display` by the
/// binary, `Debug` by `unwrap`, by a panic message, and by `main() -> Result<_, _>`.
///
/// This is a newtype rather than a redacting helper function because a helper has to be
/// remembered. Here the type of [`Error::Fetch`]'s `url` field is not `String`, so a site that
/// passes a raw one does not compile, and the next variant someone adds inherits the rule for
/// free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeUrl(String);

impl SafeUrl {
    /// The redacted text. There is deliberately no accessor for the original.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SafeUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for SafeUrl {
    fn from(url: &str) -> Self {
        Self(redact(url))
    }
}

impl From<String> for SafeUrl {
    fn from(url: String) -> Self {
        Self(redact(&url))
    }
}

impl From<&String> for SafeUrl {
    fn from(url: &String) -> Self {
        Self(redact(url))
    }
}

/// Replaces every credential in `url` with `REDACTED`, leaving the rest legible.
///
/// Legibility is the point: an error saying which endpoint failed, with which parameters, is
/// most of the diagnosis. Blanking the whole URL would be safer still and useless.
///
/// Deliberately hand-written rather than parsed with a URL crate. This runs on the failure path,
/// where the input may be exactly the malformed string that caused the failure, and a parser
/// that rejects it would leave the caller printing the raw URL as a fallback — the one outcome
/// this must not have.
#[must_use]
pub fn redact(url: &str) -> String {
    let mut output = String::with_capacity(url.len());

    // Userinfo first: `https://user:secret@host` predates query parameters as a way to leak a
    // password, and some tools still emit it.
    let (authority_end, rest_start) = match url.find("://") {
        Some(scheme) => {
            let after = scheme + 3;
            let end = url[after..]
                .find(['/', '?', '#'])
                .map_or(url.len(), |i| after + i);
            (end, after)
        }
        None => (0, 0),
    };
    if authority_end > rest_start
        && let Some(at) = url[rest_start..authority_end].rfind('@')
    {
        let at = rest_start + at;
        let user_end = url[rest_start..at].find(':').map_or(at, |i| rest_start + i);
        output.push_str(&url[..user_end]);
        if user_end < at {
            output.push(':');
            output.push_str(REDACTION);
        }
        output.push_str(&url[at..]);
        return redact_query(&output);
    }

    redact_query(url)
}

/// The query-parameter half of [`redact`], split out so userinfo redaction can reuse it.
fn redact_query(url: &str) -> String {
    let Some(query_start) = url.find('?') else {
        return url.to_owned();
    };
    // A fragment is not part of the query, and splitting on `&` through one would mangle it.
    let query_end = url[query_start..]
        .find('#')
        .map_or(url.len(), |i| query_start + i);

    let mut output = String::with_capacity(url.len());
    output.push_str(&url[..=query_start]);
    for (index, pair) in url[query_start + 1..query_end].split('&').enumerate() {
        if index > 0 {
            output.push('&');
        }
        match pair.split_once('=') {
            Some((name, _)) if is_secret(name) => {
                output.push_str(name);
                output.push('=');
                output.push_str(REDACTION);
            }
            _ => output.push_str(pair),
        }
    }
    output.push_str(&url[query_end..]);
    output
}

/// Whether a query parameter's value is a credential.
fn is_secret(name: &str) -> bool {
    SECRET_PARAMETERS
        .iter()
        .any(|secret| name.eq_ignore_ascii_case(secret))
}

/// A transport failure with the URL taken out of it.
///
/// [`reqwest::Error`] prints the URL it was trying to reach, and that message is reached through
/// `source()`, which the binary walks and prints. Redacting [`Error`]'s own `url` field would
/// therefore have accomplished nothing on its own: the same URL, unredacted, arrives one link
/// down the chain.
///
/// The only way to build one strips the URL outright rather than redacting it, because
/// [`Error::Fetch`] already names the endpoint in its own message and a second copy adds
/// nothing but a second chance to leak.
#[derive(Debug)]
pub struct Transport(reqwest::Error);

impl From<reqwest::Error> for Transport {
    fn from(source: reqwest::Error) -> Self {
        Self(source.without_url())
    }
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for Transport {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

/// Flattens an error and the errors behind it into one line.
///
/// `Display` on an error prints only its own message by design, and the cause is usually the
/// half that says what to do about it. Lives here so that the binary, [`crate::Problem`] and
/// [`crate::commands::gamelib::Failure`] all render a chain the same way rather than each keeping its own
/// loop — three copies of six lines that would drift apart the first time one was improved.
#[must_use]
pub fn chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        message.push_str(": ");
        message.push_str(&inner.to_string());
        source = inner.source();
    }
    message
}

/// Everything that can go wrong while collecting bundle listings.
///
/// The variants separate *transport* problems (the network, an HTTP status)
/// from *schema* problems (the vendor changed their page). That split matters
/// to callers: a transport failure is worth retrying, a schema failure is not
/// and means this crate needs updating.
///
/// Every URL a variant carries is a [`SafeUrl`], and every transport cause a [`Transport`].
/// Neither can hold a credential, so no variant can print one — by construction rather than by
/// each author's care.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("could not reach {url}")]
    Fetch {
        url: SafeUrl,
        #[source]
        source: Transport,
    },

    #[error("{url} returned HTTP {status}")]
    Status { url: SafeUrl, status: u16 },

    #[error("no <script id=\"{block}\"> element in {url}; the page layout has changed")]
    MissingDataBlock { block: &'static str, url: SafeUrl },

    #[error("the JSON in <script id=\"{block}\"> from {url} no longer matches the expected schema")]
    Schema {
        block: &'static str,
        url: SafeUrl,
        #[source]
        source: serde_json::Error,
    },

    #[error("{url} listed no bundles at all; the page layout has changed")]
    NoBundles { url: SafeUrl },

    #[error("the JSON from {url} does not match the expected schema")]
    Payload {
        url: SafeUrl,
        #[source]
        source: serde_json::Error,
    },

    /// An app-id Steam does not know — `appdetails` answered `success: false`.
    #[error("Steam has no app {app_id}")]
    NoSuchApp { app_id: u32 },

    /// Well-formed data that contradicts something this crate relies on.
    ///
    /// Distinct from [`Error::Payload`] (malformed) and [`Error::Protocol`] (wrong envelope):
    /// the answer parsed, and what it says is that the world moved — a review band no longer
    /// where the thresholds put it, a tag Valve added since the enum was generated, a report in
    /// a shape not seen before. Surfaced rather than tolerated, because every one of these,
    /// tolerated, turns into a plausible wrong entry that nothing flags afterwards.
    #[error("{subject}: {detail}")]
    Drift { subject: String, detail: String },

    /// The service asked for a pause. The one transport failure whose retry timing is known.
    #[error("{url} rate-limited this client (Retry-After: {})", retry_after.as_deref().unwrap_or("unspecified"))]
    RateLimited {
        url: SafeUrl,
        retry_after: Option<String>,
    },

    /// The operator has to provide or change something before this can work.
    ///
    /// Not a failure of the world but a message to a person: a missing API key, a profile set
    /// to private, a helper program that is not installed. `detail` is written to be read and
    /// acted on, so it may run to several lines and name exact files and commands.
    ///
    /// Separate from [`Error::Drift`], which reports that reality disagrees with this crate and
    /// is nobody's to fix from the outside.
    #[error("{detail}")]
    Setup { detail: String },

    /// A service answered, but not in the shape its protocol requires.
    ///
    /// Distinct from [`Error::Payload`]: the payload was never reached, because the envelope
    /// carrying it was wrong — a JSON-RPC error, a missing session, an unframed response.
    #[error("{service}: {detail}")]
    Protocol {
        service: &'static str,
        detail: String,
    },

    /// A bundle page parsed but declared no tiers.
    ///
    /// Treated as an error rather than an empty result on purpose: an empty
    /// list is indistinguishable from a real bundle that happens to be empty,
    /// so silently returning one would let a schema change reach callers
    /// disguised as a fact. See `NoGames` for the same reasoning.
    #[error("bundle {title:?} declares no tiers; the page layout has changed")]
    NoTiers { title: String },

    #[error("bundle {title:?} resolved to zero games; the page layout has changed")]
    NoGames { title: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0123456789ABCDEF0123456789ABCDEF";

    #[test]
    fn a_steam_web_api_key_is_removed() {
        let url = format!(
            "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?key={KEY}&steamid=7"
        );
        let safe = redact(&url);
        assert!(!safe.contains(KEY), "{safe}");
        assert!(safe.contains("key=REDACTED"), "{safe}");
    }

    #[test]
    fn everything_but_the_secret_survives() {
        // A redacted URL that no longer says which endpoint failed would be safe and useless.
        let safe = redact(&format!(
            "https://api.steampowered.com/IWishlistService/GetWishlist/v1/?key={KEY}&steamid=7&format=json"
        ));
        assert_eq!(
            safe,
            "https://api.steampowered.com/IWishlistService/GetWishlist/v1/?key=REDACTED&steamid=7&format=json"
        );
    }

    #[test]
    fn the_parameter_name_is_matched_whatever_its_case() {
        assert!(!redact(&format!("https://x.test/?Key={KEY}")).contains(KEY));
        assert!(!redact(&format!("https://x.test/?API_KEY={KEY}")).contains(KEY));
        assert!(!redact(&format!("https://x.test/?access_token={KEY}")).contains(KEY));
    }

    #[test]
    fn a_parameter_merely_containing_a_secret_name_is_left_alone() {
        // `keywords` is not `key`; blanking it would lose information for no gain.
        let safe = redact("https://x.test/?keywords=portal&monkey=1");
        assert_eq!(safe, "https://x.test/?keywords=portal&monkey=1");
    }

    #[test]
    fn a_password_in_the_authority_is_removed() {
        let safe = redact(&format!("https://user:{KEY}@example.test/path?a=1"));
        assert!(!safe.contains(KEY), "{safe}");
        assert_eq!(safe, "https://user:REDACTED@example.test/path?a=1");
    }

    #[test]
    fn an_at_sign_in_the_path_is_not_mistaken_for_a_password() {
        let url = "https://example.test/users/a@b.test?x=1";
        assert_eq!(redact(url), url);
    }

    #[test]
    fn a_fragment_is_left_intact() {
        let safe = redact(&format!("https://x.test/?key={KEY}#section&two"));
        assert_eq!(safe, "https://x.test/?key=REDACTED#section&two");
    }

    #[test]
    fn urls_without_a_query_pass_through_unchanged() {
        for url in [
            "https://store.steampowered.com/app/1202130",
            "https://x.test/?",
            "not a url at all",
            "",
        ] {
            assert_eq!(redact(url), url, "changed {url:?}");
        }
    }

    #[test]
    fn a_valueless_parameter_is_left_alone() {
        assert_eq!(redact("https://x.test/?key&b=2"), "https://x.test/?key&b=2");
    }

    /// The property the whole module exists for, checked on every variant that carries a URL.
    ///
    /// Written as a list of built values rather than a loop over variant names because Rust
    /// cannot enumerate variants: what keeps it honest is that a new variant carrying a
    /// [`SafeUrl`] is already redacted by its field's type, so this test cannot silently fall
    /// behind — it re-checks the guarantee rather than being the guarantee.
    #[test]
    fn no_variant_can_print_a_key_however_it_is_formatted() {
        let url = format!("https://api.steampowered.com/x/v1/?key={KEY}&steamid=7");
        let errors = vec![
            Error::Status {
                url: url.clone().into(),
                status: 401,
            },
            Error::MissingDataBlock {
                block: "b",
                url: url.clone().into(),
            },
            Error::NoBundles {
                url: url.clone().into(),
            },
            Error::Payload {
                url: url.clone().into(),
                source: serde_json::from_str::<u8>("x").unwrap_err(),
            },
            Error::Schema {
                block: "b",
                url: url.clone().into(),
                source: serde_json::from_str::<u8>("x").unwrap_err(),
            },
            Error::RateLimited {
                url: url.clone().into(),
                retry_after: Some("30".to_owned()),
            },
        ];

        for error in &errors {
            assert!(!format!("{error}").contains(KEY), "Display leaked: {error}");
            assert!(
                !format!("{error:?}").contains(KEY),
                "Debug leaked: {error:?}"
            );
            let mut source = std::error::Error::source(error);
            while let Some(inner) = source {
                assert!(!format!("{inner}").contains(KEY), "source leaked: {inner}");
                source = inner.source();
            }
        }
    }
}
