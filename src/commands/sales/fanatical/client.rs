//! The networked half of the Fanatical vendor.

use std::collections::BTreeMap;
use std::time::Duration;

use log::{debug, info};

use super::mcp::{PRODUCTS_PER_CALL, Session};
use super::parse::{self, CATALOGUE_URL, Entry};
use super::schema::Products;
use crate::error::SafeUrl;
use crate::{Error, Listing, Problem, ProblemKind, Result};

/// Identifies this crate honestly rather than imitating a browser.
///
/// Fanatical serves it the same bytes it serves a browser, checked directly. It does render
/// some pages differently for a handful of *named* third-party agents, which this deliberately
/// does not claim to be.
const USER_AGENT: &str = concat!("catalogames/", env!("CARGO_PKG_VERSION"));

/// Region for prices and availability.
///
/// Load-bearing rather than cosmetic: it decides which products resolve at all, so a product
/// missing from one region's answer is a fact about the region and not about the bundle. Pinned
/// so a run is reproducible wherever it happens to be made.
const REGION: &str = "US";

/// What the service calls a bundle of games sold at one price.
///
/// Matched exactly, so that `mystery-bundle` — whose contents are random by design and cannot be
/// listed — is excluded by what the service declares rather than by anything read off its name.
const WHOLE_BUNDLE_TYPE: &str = "game-bundle";

/// Most bundles the search will return in one call, which is the service's own maximum.
const SEARCH_LIMIT: usize = 100;

/// Pause between requests.
///
/// Fanatical rate-limits, and the limit is reached sooner than a burst — a sibling process
/// sharing this machine's address tripped it on a second request. The block arrives as a
/// fifteen-byte non-JSON body, which a tolerant parser would turn into a bundle with no games;
/// every response here is checked for success before it is read.
const REQUEST_SPACING: Duration = Duration::from_millis(1_000);

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Fetches bundle listings from Fanatical.
///
/// ```no_run
/// let listing = catalogames::commands::sales::fanatical::Client::new()?.list_bundles()?;
/// # Ok::<(), catalogames::Error>(())
/// ```
pub struct Client {
    http: reqwest::blocking::Client,
    spacing: Duration,
}

impl Client {
    pub fn new() -> Result<Self> {
        Self::with_spacing(REQUEST_SPACING)
    }

    pub fn with_spacing(spacing: Duration) -> Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            // Measured at 4.8x on the catalogue response, which is the largest thing fetched.
            .gzip(true)
            .build()
            .map_err(|source| Error::Fetch {
                url: CATALOGUE_URL.into(),
                source: source.into(),
            })?;
        Ok(Self { http, spacing })
    }

    /// Lists the current game bundles with the games each offers.
    pub fn list_bundles(&self) -> Result<Listing> {
        info!("fetching bundle catalogue from {CATALOGUE_URL}");
        let catalogue = self.get(CATALOGUE_URL)?;
        let mut entries = parse::parse_catalogue(&catalogue)?;
        info!("{} pick-and-mix bundle(s) listed", entries.len());

        let mut session = Session::open(USER_AGENT, REQUEST_TIMEOUT)?;
        let mut bundles = Vec::with_capacity(entries.len());
        let mut problems = Vec::new();

        // The catalogue carries pick-and-mix bundles and nothing else, so the fixed-price ones
        // are found separately. A failure there costs those bundles and not the run.
        match self.whole_bundle_entries(&mut session, &entries) {
            Ok(extra) => {
                info!("{} fixed-price bundle(s) listed", extra.len());
                entries.extend(extra);
            }
            Err(error) => {
                info!("  fixed-price bundles unavailable: {error}");
                problems.push(Problem {
                    bundle: "fixed-price bundles".to_owned(),
                    kind: ProblemKind::Unavailable(error),
                });
            }
        }

        for (position, entry) in entries.iter().enumerate() {
            if position > 0 {
                std::thread::sleep(self.spacing);
            }
            info!("({}/{}) {}", position + 1, entries.len(), entry.title);

            match self.fetch_bundle(&mut session, entry) {
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

    /// Finds the bundles sold at one fixed price, which the catalogue does not carry.
    ///
    /// One search call lists every game bundle the store sells, pick-and-mix ones included; the
    /// ones already in hand are dropped by slug and the rest are looked up for their price,
    /// their deadline and their contents.
    ///
    /// Mystery bundles fall out on their declared type rather than on their names. Their
    /// contents are random by design, so there is nothing to list, and reading the type is what
    /// keeps that from being a guess about wording.
    fn whole_bundle_entries(&self, session: &mut Session, known: &[Entry]) -> Result<Vec<Entry>> {
        let body = session.call(
            "search_products",
            serde_json::json!({ "type": "bundle", "region": REGION, "limit": SEARCH_LIMIT }),
        )?;
        let found = parse::parse_search(&body)?;

        // A full page is indistinguishable from a truncated one, and the symptom of losing the
        // remainder is bundles quietly not appearing — which nobody reports as a fault.
        if found.len() >= SEARCH_LIMIT {
            return Err(Error::Drift {
                subject: "fanatical bundle search".to_owned(),
                detail: format!(
                    "returned {SEARCH_LIMIT} results, its own limit, so there are probably more \
                     it did not send"
                ),
            });
        }
        debug!("  search listed {} bundle(s)", found.len());

        let wanted: Vec<String> = found
            .iter()
            .filter(|product| product.kind.as_deref() == Some(WHOLE_BUNDLE_TYPE))
            .filter(|product| !known.iter().any(|entry| entry.slug == product.slug))
            .map(|product| product.slug.clone())
            .collect();
        if wanted.is_empty() {
            return Ok(Vec::new());
        }

        std::thread::sleep(self.spacing);
        let records = self.products(session, &wanted)?;
        Ok(parse::whole_bundle_entries(&records))
    }

    fn fetch_bundle(
        &self,
        session: &mut Session,
        entry: &Entry,
    ) -> Result<(crate::Bundle, Vec<Problem>)> {
        let mut detail = self.products(session, &entry.product_slugs)?;
        self.resolve_parents(session, &mut detail)?;
        self.resolve_packs(session, &mut detail)?;
        parse::build_bundle(entry, &detail)
    }

    /// Looks up products by slug, in the batches the service accepts.
    fn products(&self, session: &mut Session, slugs: &[String]) -> Result<Products> {
        let mut merged = Products {
            currency: None,
            products: Vec::with_capacity(slugs.len()),
            not_found: Vec::new(),
        };

        for (position, chunk) in slugs.chunks(PRODUCTS_PER_CALL).enumerate() {
            if position > 0 {
                std::thread::sleep(self.spacing);
            }
            let body = session.call(
                "get_products",
                serde_json::json!({ "slugs": chunk, "region": REGION }),
            )?;
            let mut batch = super::parse::parse_products(&body)?;
            merged.currency = merged.currency.or(batch.currency);
            merged.products.append(&mut batch.products);
            merged.not_found.append(&mut batch.not_found);
        }
        Ok(merged)
    }

    /// Fills in Steam ids for editions and packs by looking up the base product they name.
    ///
    /// An edition is not itself a Steam product, but the game it is built from is. One extra
    /// lookup recovers roughly half of the products that would otherwise carry no id; the rest
    /// are genuine multi-game packs with no single Steam equivalent, and stay empty.
    fn resolve_parents(&self, session: &mut Session, detail: &mut Products) -> Result<()> {
        let wanted = parse::parents_worth_resolving(detail);
        if wanted.is_empty() {
            return Ok(());
        }
        debug!("  resolving {} parent product(s)", wanted.len());

        std::thread::sleep(self.spacing);
        let parents = self.products(session, &wanted)?;
        let ids: BTreeMap<&str, u32> = parents
            .products
            .iter()
            .filter_map(|p| Some((p.slug.as_str(), p.steam_id?)))
            .collect();

        for product in &mut detail.products {
            if product.steam_id.is_some() {
                continue;
            }
            if let Some(parent) = &product.parent
                && let Some(id) = ids.get(parent.slug.as_str())
            {
                debug!("  {} inherits {} from {}", product.slug, id, parent.slug);
                product.steam_id = Some(*id);
            }
        }
        Ok(())
    }

    /// Fills in what a multi-game pack actually delivers.
    ///
    /// A pack is a marketing wrapper with no page of its own: its Steam id is null and it names
    /// no base product, so neither of the other paths reaches it, and the search link we would
    /// otherwise print is for a phrase no store sells. Its record does list what it contains —
    /// by slug and nothing else — so one more lookup turns those slugs into games that each
    /// have their own id and their own page.
    ///
    /// Runs after [`Client::resolve_parents`] so that an edition, which names a base product,
    /// is already resolved and is skipped here rather than being taken apart.
    ///
    /// Only one level deep. A pack listed inside a pack would need another round trip, and no
    /// record seen has one; if that changes, the inner pack simply keeps its name and no link,
    /// which is the same outcome as before this pass existed.
    fn resolve_packs(&self, session: &mut Session, detail: &mut Products) -> Result<()> {
        let wanted = parse::packs_worth_expanding(detail);
        if wanted.is_empty() {
            return Ok(());
        }
        debug!("  expanding {} product(s) named inside packs", wanted.len());

        std::thread::sleep(self.spacing);
        let found = self.products(session, &wanted)?;
        let by_slug: BTreeMap<&str, &super::schema::Product> = found
            .products
            .iter()
            .map(|product| (product.slug.as_str(), product))
            .collect();

        for product in &mut detail.products {
            if product.steam_id.is_some() || product.parent.is_some() {
                continue;
            }
            // `delivered` drops a self-reference: a bundle can list itself among its own
            // contents, and left in it would render as a game inside itself.
            // **A game the second lookup could not describe is still a game the pack sells.**
            // Dropping it collapsed the whole pack to one row: every one of
            // `rock-of-ages-1-3-complete-bundle`'s three slugs came back in `not_found`, so its
            // contents list emptied and it rendered as a bare name — beside
            // `boomer-shooters-furious-4-bundle`, whose four slugs all resolved and which opened
            // out correctly. The pack's own list already carries the store's NAME for each, which
            // is better than the title-from-slug fallback the top level settles for.
            product.contents = parse::delivered(product)
                .map(|contained| match by_slug.get(contained.slug.as_str()) {
                    Some(found) => (*found).clone(),
                    None => super::schema::Product {
                        name: contained.name.clone(),
                        slug: contained.slug.clone(),
                        from_contents_list: true,
                        ..super::schema::Product::unknown()
                    },
                })
                .collect();
            if !product.contents.is_empty() {
                debug!(
                    "  {} delivers {} game(s)",
                    product.slug,
                    product.contents.len()
                );
            }
        }
        Ok(())
    }

    fn get(&self, url: &str) -> Result<String> {
        // Redacted once; see the same construction in `humble::Client::get`.
        let safe = SafeUrl::from(url);
        let fetch_error = |source: reqwest::Error| Error::Fetch {
            url: safe.clone(),
            source: source.into(),
        };
        debug!("GET {url}");
        let response = self.http.get(url).send().map_err(fetch_error)?;

        // Checked before the body is read: a rate-limit reply is a short non-JSON string, and
        // parsing it tolerantly would produce an empty bundle rather than a failure.
        let status = response.status();
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
