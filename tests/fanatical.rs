//! Reading Fanatical's catalogue and its bundle contents.

use catalogames::commands::sales::fanatical;
use catalogames::{Error, ProblemKind};

const CATALOGUE: &str = include_str!("fixtures/fanatical/all-en.json");
const PRODUCTS: &str = include_str!("fixtures/fanatical/mcp-get-products.sse");

/// Wraps a payload the way the MCP server frames one: JSON-RPC inside a server-sent event,
/// whose text content is itself a JSON document.
fn mcp_response(payload: &str) -> String {
    let envelope = serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "result": { "content": [ { "type": "text", "text": payload } ] }
    });
    format!("event: message\nid: abc\ndata: {envelope}\n\n")
}

fn entry(title: &str, slugs: &[&str]) -> fanatical::Entry {
    fanatical::Entry {
        title: title.to_owned(),
        slug: "a-bundle".to_owned(),
        url: fanatical::bundle_url("a-bundle"),
        product_slugs: slugs.iter().map(|s| (*s).to_owned()).collect(),
        smallest_pick: Some(2),
        price: None,
        ends_at: None,
    }
}

mod the_catalogue {
    use super::*;

    #[test]
    fn keeps_only_bundles_of_games() {
        // The listing mixes games with book, elearning, software and comic bundles. The
        // fixture holds five entries, three of them games.
        let entries = fanatical::parse_catalogue(CATALOGUE).expect("fixture parses");
        assert_eq!(
            entries.len(),
            3,
            "{:?}",
            entries.iter().map(|e| &e.title).collect::<Vec<_>>()
        );
    }

    #[test]
    fn names_each_bundle_and_links_to_its_page() {
        let entries = fanatical::parse_catalogue(CATALOGUE).expect("fixture parses");
        let target = entries
            .iter()
            .find(|e| e.slug == "fanatical-favorites-build-your-own-bundle")
            .expect("the target bundle is in the fixture");

        assert!(
            target.title.contains("Fanatical Favorites"),
            "{}",
            target.title
        );
        assert_eq!(
            target.url,
            "https://www.fanatical.com/en/pick-and-mix/fanatical-favorites-build-your-own-bundle"
        );
        assert!(!target.product_slugs.is_empty());
    }

    #[test]
    fn reads_the_smallest_number_of_games_a_tier_lets_you_pick() {
        // Fanatical's tiers are price points over one shared pool, not nested content sets:
        // "pick 2 for £6.99, 3 for £9.99, 5 for £14.99".
        let entries = fanatical::parse_catalogue(CATALOGUE).expect("fixture parses");
        let target = entries
            .iter()
            .find(|e| e.slug == "fanatical-favorites-build-your-own-bundle")
            .expect("present");
        assert_eq!(target.smallest_pick, Some(2));
    }

    #[test]
    fn a_catalogue_of_no_game_bundles_is_an_error_not_an_empty_list() {
        let only_books = r#"{"pickandmix":[{"name":"B","slug":"b","type":"book-bundle","products":[],"tiers":[]}]}"#;
        assert!(matches!(
            fanatical::parse_catalogue(only_books),
            Err(Error::NoBundles { .. })
        ));
    }

    #[test]
    fn an_unfamiliar_category_is_excluded_rather_than_assumed_to_be_games() {
        // "bundle" is the unmarked default, so the filter names it positively. A deny-list
        // would admit any category Fanatical adds later and look like it was working.
        let future = r#"{"pickandmix":[{"name":"A","slug":"a","type":"audio-bundle","products":[],"tiers":[]}]}"#;
        assert!(matches!(
            fanatical::parse_catalogue(future),
            Err(Error::NoBundles { .. })
        ));
    }

    #[test]
    fn a_payload_that_changed_shape_fails_loudly() {
        assert!(matches!(
            fanatical::parse_catalogue("{ not json"),
            Err(Error::Payload { .. })
        ));
        assert!(matches!(
            fanatical::parse_catalogue("{}"),
            Err(Error::Payload { .. })
        ));
    }
}

mod a_real_mcp_response {
    use super::*;

    #[test]
    fn yields_a_game_per_product_with_its_steam_id() {
        let entry = entry(
            "A Bundle",
            &["art-of-rally", "the-witness", "everspace-ultimate-edition"],
        );
        let (bundle, _) = fanatical::parse_bundle(&entry, PRODUCTS).expect("fixture parses");

        assert_eq!(bundle.games.len(), 3);
        let rally = bundle
            .games
            .iter()
            .find(|g| g.machine_name == "art-of-rally")
            .expect("present");
        assert_eq!(rally.steam_app_id, Some(550320));
        assert_eq!(rally.title, "art of rally");

        let witness = bundle
            .games
            .iter()
            .find(|g| g.machine_name == "the-witness")
            .expect("present");
        assert_eq!(witness.steam_app_id, Some(210970));
    }

    #[test]
    fn leaves_an_edition_without_an_id_rather_than_guessing_one() {
        // "Everspace Ultimate Edition" is not itself a Steam product. Its sibling `steam.type`
        // still reads "app", so only the null id distinguishes it — the client resolves this
        // one through the parent it names, but the parser must not invent anything.
        let entry = entry(
            "A Bundle",
            &["art-of-rally", "the-witness", "everspace-ultimate-edition"],
        );
        let (bundle, _) = fanatical::parse_bundle(&entry, PRODUCTS).expect("parses");

        let edition = bundle
            .games
            .iter()
            .find(|g| g.machine_name == "everspace-ultimate-edition")
            .expect("present");
        assert_eq!(edition.steam_app_id, None);
    }

    #[test]
    fn is_silent_when_the_two_counts_agree() {
        let entry = entry(
            "A Bundle",
            &["art-of-rally", "the-witness", "everspace-ultimate-edition"],
        );
        let (_, problems) = fanatical::parse_bundle(&entry, PRODUCTS).expect("parses");
        assert!(problems.is_empty(), "{problems:?}");
    }
}

mod cross_checking_the_pool_size {
    use super::*;

    fn two_products() -> String {
        mcp_response(
            r#"{"products":[
                {"name":"One","slug":"one","steam_id":1},
                {"name":"Two","slug":"two","steam_id":2}],
              "not_found":[]}"#,
        )
    }

    #[test]
    fn reports_a_listing_that_disagrees_with_what_came_back() {
        // The two numbers are computed independently and normally agree, so a disagreement is
        // a signal rather than routine noise.
        let entry = entry("A Bundle", &["one", "two", "three"]);
        let (bundle, problems) = fanatical::parse_bundle(&entry, &two_products()).expect("parses");

        assert_eq!(bundle.games.len(), 2);
        assert!(
            problems.iter().any(|p| matches!(
                p.kind,
                ProblemKind::GameCountMismatch {
                    advertised: 3,
                    found: 2
                }
            )),
            "{problems:?}"
        );
    }

    #[test]
    fn a_product_the_region_cannot_resolve_is_still_listed_from_its_own_slug() {
        // Availability is regional, so the service names what it could not describe. The store
        // still sells the game, and its slug still reads as a title, so it is listed with a
        // search link rather than vanishing from a bundle that contains it.
        let payload = mcp_response(
            r#"{"products":[{"name":"One","slug":"one","steam_id":1}],
                "not_found":["those-who-rule"]}"#,
        );
        let entry = entry("A Bundle", &["one", "those-who-rule"]);
        let (bundle, problems) = fanatical::parse_bundle(&entry, &payload).expect("parses");

        let titles: Vec<&str> = bundle.games.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["One", "Those Who Rule"]);
        assert_eq!(bundle.games[1].steam_app_id, None, "nothing was invented");
        assert!(
            problems.iter().any(|p| matches!(
                &p.kind,
                ProblemKind::TitleFromSlug { slugs } if slugs == &["those-who-rule"]
            )),
            "a derived title has to be reported as derived: {problems:?}"
        );
    }

    #[test]
    fn the_games_come_back_in_the_order_the_listing_gave_them() {
        // Curation order is the store's, and it is the listing that states it — the detail
        // answers may arrive in any order at all.
        let payload = mcp_response(
            r#"{"products":[{"name":"Third","slug":"c","steam_id":3},
                            {"name":"First","slug":"a","steam_id":1},
                            {"name":"Second","slug":"b","steam_id":2}],
                "not_found":[]}"#,
        );
        let (bundle, _) = fanatical::parse_bundle(&entry("A Bundle", &["a", "b", "c"]), &payload)
            .expect("parses");
        let titles: Vec<&str> = bundle.games.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["First", "Second", "Third"]);
    }

    #[test]
    fn a_bundle_that_resolved_to_nothing_is_an_error_not_an_empty_bundle() {
        // A listing naming no products at all: nothing to derive a title from either, so this
        // stays the loud failure it was.
        let empty = mcp_response(r#"{"products":[],"not_found":[]}"#);
        assert!(matches!(
            fanatical::parse_bundle(&entry("Empty", &[]), &empty),
            Err(Error::NoGames { .. })
        ));
    }

    #[test]
    fn a_slug_the_service_neither_answered_nor_refused_still_trips_the_count_check() {
        // Distinct from a refusal: the service said nothing about this slug at all, which is a
        // protocol oddity rather than a regional gap, so there is no title to derive and the
        // pool-size check is what reports it.
        let payload = mcp_response(
            r#"{"products":[{"name":"One","slug":"one","steam_id":1}],"not_found":[]}"#,
        );
        let entry = entry("A Bundle", &["one", "silently-dropped"]);
        let (bundle, problems) = fanatical::parse_bundle(&entry, &payload).expect("parses");

        assert_eq!(bundle.games.len(), 1);
        assert!(
            problems.iter().any(|p| matches!(
                p.kind,
                ProblemKind::GameCountMismatch {
                    advertised: 2,
                    found: 1
                }
            )),
            "{problems:?}"
        );
    }
}

mod the_mcp_framing {
    use super::*;

    #[test]
    fn reads_the_payload_out_of_a_server_sent_event() {
        let response =
            mcp_response(r#"{"products":[{"name":"X","slug":"x","steam_id":7}],"not_found":[]}"#);
        let (bundle, _) = fanatical::parse_bundle(&entry("B", &["x"]), &response).expect("parses");
        assert_eq!(bundle.games[0].steam_app_id, Some(7));
    }

    #[test]
    fn takes_the_last_frame_when_a_stream_carries_several() {
        // A stream may carry earlier messages; the answer to the request just made is the last.
        let first = serde_json::json!({"jsonrpc":"2.0","method":"notifications/progress"});
        let payload = r#"{"products":[{"name":"X","slug":"x","steam_id":7}],"not_found":[]}"#;
        let stream = format!("data: {first}\n\n{}", mcp_response(payload));
        assert!(fanatical::parse_bundle(&entry("B", &["x"]), &stream).is_ok());
    }

    #[test]
    fn a_json_rpc_error_is_reported_as_one() {
        let err = serde_json::json!({
            "jsonrpc":"2.0","id":1,
            "error":{"code":-32602,"message":"Invalid params"}
        });
        let response = format!("event: message\ndata: {err}\n\n");
        let failure = fanatical::parse_bundle(&entry("B", &["x"]), &response).unwrap_err();
        assert!(matches!(failure, Error::Protocol { .. }), "{failure:?}");
        assert!(failure.to_string().contains("Invalid params"), "{failure}");
    }

    #[test]
    fn a_response_with_no_event_frame_is_reported_rather_than_parsed() {
        let failure = fanatical::parse_bundle(&entry("B", &["x"]), "Request blocked").unwrap_err();
        assert!(matches!(failure, Error::Protocol { .. }), "{failure:?}");
    }

    #[test]
    fn a_result_carrying_no_text_content_is_reported() {
        let odd = serde_json::json!({"jsonrpc":"2.0","id":1,"result":{"content":[]}});
        let response = format!("data: {odd}\n\n");
        assert!(matches!(
            fanatical::parse_bundle(&entry("B", &["x"]), &response),
            Err(Error::Protocol { .. })
        ));
    }
}
