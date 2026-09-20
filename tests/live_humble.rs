//! Drift detector: checks this crate against the live site, not a capture.
//!
//! Ignored by default because it makes ~14 network requests and fails whenever
//! Humble is unreachable, which is not a reason to fail a normal test run.
//!
//! **The offline suite cannot replace this.** Fixtures pin the schema as it was
//! on the day it was captured and stay green forever after Humble moves; only
//! this test can tell you the crate has stopped reading the real site. Run it
//! before a release and on a schedule:
//!
//! ```text
//! cargo test --test live_humble -- --ignored --nocapture
//! ```

use catalogames::commands::sales::humble;

#[test]
#[ignore = "makes live network requests to humblebundle.com"]
fn the_live_site_still_parses() {
    let listing = humble::Client::new()
        .expect("client builds")
        .list_bundles()
        .expect("the live games index parses");

    assert!(
        !listing.bundles.is_empty(),
        "the live index yielded no bundles"
    );

    for bundle in &listing.bundles {
        assert!(!bundle.title.is_empty(), "bundle with no title");
        assert!(
            !bundle.games.is_empty(),
            "{:?} came back with no games",
            bundle.title
        );
        println!("{} — {} games", bundle.title, bundle.games.len());
    }

    // The deadline is the one field read from a bare timestamp with no zone marker, so a
    // format change there is silent: every bundle would simply lose its end date. Checked
    // against the live site because that is the only place the change can show up.
    let undated: Vec<&str> = listing
        .bundles
        .iter()
        .filter(|bundle| bundle.ends_at.is_none())
        .map(|bundle| bundle.title.as_str())
        .collect();
    assert!(
        undated.is_empty(),
        "no end date could be read for {undated:?}; the timestamp format has changed"
    );

    // A bundle the site is still selling has not ended yet. The hour itself is deliberately not
    // asserted: Humble's changeover follows a US-local rule and shifts by one across daylight
    // saving, so pinning it would fail twice a year for no reason.
    let now = catalogames::clock::Timestamp::now();
    for bundle in &listing.bundles {
        let ends_at = bundle.ends_at.expect("checked above");
        assert!(
            ends_at.seconds_from(now) > 0,
            "{:?} is on sale but its end date reads {ends_at}, which is in the past — \
             the timestamps are probably no longer UTC",
            bundle.title
        );
    }

    // Prices come from a map keyed separately from the tier list, so a renamed key loses every
    // price silently while the games keep parsing. Only a live run can tell.
    let unpriced: Vec<&str> = listing
        .bundles
        .iter()
        .filter(|bundle| bundle.price.is_none())
        .map(|bundle| bundle.title.as_str())
        .collect();
    assert!(
        unpriced.is_empty(),
        "no price could be read for {unpriced:?}; tier_pricing_data has changed"
    );

    // Every Humble tier is a whole-bundle price. A per-game rate appearing here would mean the
    // parser had started treating cumulative tiers as a pick-and-mix pool.
    for bundle in &listing.bundles {
        assert!(
            matches!(bundle.price, Some(catalogames::Price::Whole(_))),
            "{:?} priced as {:?}, but a Humble tier is bought whole",
            bundle.title,
            bundle.price
        );
    }

    // Every problem here is a real signal: an unreachable page, a tier model
    // that stopped being cumulative, or Humble's own advertised count
    // disagreeing with the tier data. None should be routine.
    assert!(
        listing.is_complete(),
        "live run reported problems:\n{}",
        listing
            .problems
            .iter()
            .map(|p| format!("  {p}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
