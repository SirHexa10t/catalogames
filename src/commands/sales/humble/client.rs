//! The networked half of the Humble vendor.

use log::{debug, info};
use std::time::Duration;

use super::parse::{self, Entry, INDEX_URL};
use crate::error::SafeUrl;
use crate::{Error, Listing, Problem, ProblemKind, Result};

/// Identifies this crate honestly rather than imitating a browser.
///
/// Humble serves the page to this string exactly as it serves it to a browser
/// (checked against the live site), so there is nothing to gain from pretending
/// to be one.
const USER_AGENT: &str = concat!("catalogames/", env!("CARGO_PKG_VERSION"));

/// Pause between the per-bundle page fetches.
///
/// A listing run costs `1 + N` requests — N was 13 when this was written. At
/// this spacing the run stays slower than a person clicking through the same
/// pages, and still finishes in well under a minute. Fetching the bundle pages
/// concurrently would be faster and is deliberately not done.
const REQUEST_SPACING: Duration = Duration::from_millis(500);

/// How long a single request may take before it is abandoned.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Fetches bundle listings from Humble Bundle.
///
/// ```no_run
/// let listing = catalogames::commands::sales::humble::Client::new()?.list_bundles()?;
/// for bundle in &listing.bundles {
///     println!("{} ({} games)", bundle.title, bundle.games.len());
/// }
/// # Ok::<(), catalogames::Error>(())
/// ```
#[derive(Debug)]
pub struct Client {
    http: reqwest::blocking::Client,
    spacing: Duration,
}

impl Client {
    /// Builds a client with the default pacing.
    pub fn new() -> Result<Self> {
        Self::with_spacing(REQUEST_SPACING)
    }

    /// Builds a client that waits `spacing` between per-bundle requests.
    ///
    /// Worth raising if you poll on a schedule; lowering it is asking to be
    /// rate-limited.
    pub fn with_spacing(spacing: Duration) -> Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|source| Error::Fetch {
                url: INDEX_URL.into(),
                source: source.into(),
            })?;
        Ok(Self { http, spacing })
    }

    /// Lists every current game bundle with the contents of its largest tier.
    ///
    /// Fails only when the index itself cannot be read. A bundle whose own page
    /// fails is recorded in [`Listing::problems`] and the rest still return, so
    /// one bad page does not cost the caller the whole listing.
    pub fn list_bundles(&self) -> Result<Listing> {
        info!("fetching bundle index from {INDEX_URL}");
        let index = self.get(INDEX_URL)?;
        let entries = parse::parse_index(&index, INDEX_URL)?;
        info!("index lists {} bundles", entries.len());

        let mut bundles = Vec::with_capacity(entries.len());
        let mut problems = Vec::new();

        for (position, entry) in entries.iter().enumerate() {
            if position > 0 {
                debug!("pausing {:?} before the next request", self.spacing);
                std::thread::sleep(self.spacing);
            }
            info!("({}/{}) {}", position + 1, entries.len(), entry.title);
            match self.fetch_bundle(entry) {
                Ok((bundle, found)) => {
                    bundles.push(bundle);
                    problems.extend(found);
                }
                Err(error) => {
                    info!("  unavailable: {error}");
                    problems.push(Problem {
                        bundle: entry.title.clone(),
                        kind: ProblemKind::Unavailable(error),
                    });
                }
            }
        }
        info!(
            "read {} of {} bundles, {} problem(s)",
            bundles.len(),
            entries.len(),
            problems.len()
        );
        Ok(Listing { bundles, problems })
    }

    fn fetch_bundle(&self, entry: &Entry) -> Result<(crate::Bundle, Vec<Problem>)> {
        let html = self.get(&entry.url)?;
        parse::parse_bundle(&html, entry)
    }

    fn get(&self, url: &str) -> Result<String> {
        // Redacted once, and every message below is built from it: a URL reaches a log line
        // or an error only through `safe`, so a credential in a query parameter has no way out.
        let safe = SafeUrl::from(url);
        let fetch_error = |source: reqwest::Error| Error::Fetch {
            url: safe.clone(),
            source: source.into(),
        };

        debug!("GET {safe}");
        let response = self.http.get(url).send().map_err(fetch_error)?;
        let status = response.status();
        debug!("  HTTP {status}");
        if !status.is_success() {
            return Err(Error::Status {
                url: safe.clone(),
                status: status.as_u16(),
            });
        }
        let body = response.text().map_err(fetch_error)?;
        debug!("  {} bytes", body.len());
        Ok(body)
    }
}
