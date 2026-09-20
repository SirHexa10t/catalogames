//! Reading the games index page: which bundles exist and where they live.

use catalogames::Error;
use catalogames::commands::sales::humble;
use serde_json::json;

const INDEX: &str = include_str!("fixtures/games-index.html");
const URL: &str = "https://www.humblebundle.com/games";

/// Wraps a payload the way Humble embeds it, so tests exercise the real
/// extraction path rather than handing JSON straight to the parser.
fn page(block: &str, payload: &str) -> String {
    format!(
        "<!doctype html><html><body>\
         <script id=\"{block}\" type=\"application/json\">{payload}</script>\
         </body></html>"
    )
}

/// An index payload carrying exactly the given products.
fn index_with(products: serde_json::Value) -> String {
    page(
        "landingPage-json-data",
        &json!({ "data": { "games": { "mosaic": [ { "products": products } ] } } }).to_string(),
    )
}

mod a_real_capture {
    use super::*;

    #[test]
    fn yields_one_entry_per_bundle() {
        let entries = humble::parse_index(INDEX, URL).expect("fixture parses");
        assert_eq!(entries.len(), 13);
    }

    #[test]
    fn reads_title_url_and_advertised_count_together() {
        let entries = humble::parse_index(INDEX, URL).expect("fixture parses");

        // Compared as a whole value: a field added to `Entry` shows up here as
        // a compile error rather than slipping past a field-by-field check.
        assert_eq!(
            entries[0],
            humble::Entry {
                title: "Beyond the Metroidverse Bundle".to_owned(),
                url: "https://www.humblebundle.com/games/beyond-metroidverse-bundle".to_owned(),
                advertised_games: Some(7),
                // Humble states this without a zone; it is UTC. See `humble::schema::Product`.
                ends_at: catalogames::clock::Timestamp::parse("2026-09-17T04:00:00"),
            }
        );
    }

    #[test]
    fn makes_every_url_absolute() {
        let entries = humble::parse_index(INDEX, URL).expect("fixture parses");
        assert!(
            entries
                .iter()
                .all(|e| e.url.starts_with("https://www.humblebundle.com/games/")),
            "site-relative link left unresolved: {:?}",
            entries.iter().find(|e| !e.url.starts_with("https://")),
        );
    }

    #[test]
    fn finds_an_advertised_count_for_every_bundle() {
        let entries = humble::parse_index(INDEX, URL).expect("fixture parses");
        let counts: Vec<_> = entries.iter().map(|e| e.advertised_games).collect();
        assert_eq!(
            counts,
            [7, 11, 11, 9, 10, 14, 12, 9, 13, 10, 10, 16, 9].map(Some)
        );
    }

    #[test]
    fn keeps_titles_verbatim_including_punctuation() {
        let entries = humble::parse_index(INDEX, URL).expect("fixture parses");
        let titles: Vec<&str> = entries.iter().map(|e| e.title.as_str()).collect();
        assert!(titles.contains(&"Make & Play Games in Godot"), "{titles:?}");
        assert!(
            titles.contains(&"CRPG Pack: Isometric Immersion"),
            "{titles:?}"
        );
    }
}

mod the_advertised_count {
    use super::*;

    fn count_from(highlights: serde_json::Value) -> Option<usize> {
        let html = index_with(json!([{
            "tile_name": "A Bundle",
            "product_url": "/games/a-bundle",
            "hover_highlights": highlights,
        }]));
        humble::parse_index(&html, URL).expect("parses")[0].advertised_games
    }

    #[test]
    fn is_read_from_a_games_caption() {
        assert_eq!(count_from(json!(["7 games"])), Some(7));
    }

    #[test]
    fn is_found_wherever_it_sits_among_other_captions() {
        assert_eq!(count_from(json!(["US$140 Value", "7 games"])), Some(7));
    }

    #[test]
    fn tolerates_the_singular() {
        assert_eq!(count_from(json!(["1 game"])), Some(1));
    }

    #[test]
    fn is_absent_rather_than_guessed_when_no_caption_says_one() {
        // The count only earns its keep as a cross-check against the tier data,
        // so inventing one would be worse than going without.
        assert_eq!(count_from(json!(["US$140 Value"])), None);
        assert_eq!(count_from(json!([])), None);
        assert_eq!(count_from(json!(["Ends in 3 days"])), None);
    }

    #[test]
    fn is_absent_when_the_caption_counts_something_else() {
        assert_eq!(count_from(json!(["12 ebooks"])), None);
    }
}

mod a_page_that_changed_shape {
    use super::*;

    #[test]
    fn without_the_data_block_says_so() {
        let html = "<!doctype html><html><body><p>nothing here</p></body></html>";
        assert!(matches!(
            humble::parse_index(html, URL),
            Err(Error::MissingDataBlock {
                block: "landingPage-json-data",
                ..
            })
        ));
    }

    #[test]
    fn with_the_block_under_another_id_says_so() {
        let html = page("some-other-json-data", "{}");
        assert!(matches!(
            humble::parse_index(&html, URL),
            Err(Error::MissingDataBlock { .. })
        ));
    }

    #[test]
    fn with_unreadable_json_says_so() {
        let html = page("landingPage-json-data", "{ not json");
        assert!(matches!(
            humble::parse_index(&html, URL),
            Err(Error::Schema { .. })
        ));
    }

    #[test]
    fn listing_no_bundles_is_an_error_not_an_empty_list() {
        // An empty list would be indistinguishable from "Humble is between
        // bundles today", which is not a thing that happens on this page.
        assert!(matches!(
            humble::parse_index(&index_with(json!([])), URL),
            Err(Error::NoBundles { .. })
        ));
    }

    #[test]
    fn dropping_a_load_bearing_field_fails_loudly() {
        // `tile_name` is deliberately not defaulted: a bundle with an empty
        // title would be reported as fact instead of as a schema change.
        let html = index_with(json!([{ "product_url": "/games/a-bundle" }]));
        assert!(matches!(
            humble::parse_index(&html, URL),
            Err(Error::Schema { .. })
        ));
    }

    #[test]
    fn dropping_a_decorative_field_is_tolerated() {
        // The mirror image: captions may come and go without breaking a run.
        let html = index_with(json!([{
            "tile_name": "A Bundle",
            "product_url": "/games/a-bundle",
        }]));
        let entries = humble::parse_index(&html, URL).expect("parses without captions");
        assert_eq!(entries[0].advertised_games, None);
    }

    #[test]
    fn an_already_absolute_url_is_left_alone() {
        let html = index_with(json!([{
            "tile_name": "A Bundle",
            "product_url": "https://elsewhere.example/games/a-bundle",
        }]));
        let entries = humble::parse_index(&html, URL).expect("parses");
        assert_eq!(entries[0].url, "https://elsewhere.example/games/a-bundle");
    }
}

mod deadlines {
    use super::*;
    use catalogames::clock::Timestamp;

    fn ends_at(title: &str) -> Timestamp {
        humble::parse_index(INDEX, URL)
            .expect("fixture parses")
            .into_iter()
            .find(|entry| entry.title == title)
            .unwrap_or_else(|| panic!("no bundle titled {title:?} in the capture"))
            .ends_at
            .unwrap_or_else(|| panic!("{title:?} carries no end date"))
    }

    #[test]
    fn every_bundle_in_the_capture_states_when_it_ends() {
        let entries = humble::parse_index(INDEX, URL).expect("fixture parses");
        let undated: Vec<&str> = entries
            .iter()
            .filter(|entry| entry.ends_at.is_none())
            .map(|entry| entry.title.as_str())
            .collect();
        assert!(undated.is_empty(), "no end date read for {undated:?}");
    }

    #[test]
    fn the_capture_holds_bundles_that_have_already_ended() {
        // Named because they are what keeps the expired path exercised on real data. The
        // capture was taken on 2026-09-11 and these two ended days later, so any test that
        // renders this fixture counts down from a deadline in the past.
        let reference = Timestamp::parse("2026-09-16T00:00:00").expect("a fixed instant");
        for title in [
            "Humble 15th Anniversary - Ready Player One ... and Two",
            "Shantae & Heroic Heroines",
        ] {
            assert!(
                ends_at(title).seconds_from(reference) < 0,
                "{title} was expected to have ended by the reference date"
            );
        }
    }

    #[test]
    fn a_deadline_is_read_as_an_instant_rather_than_a_local_wall_clock() {
        // The value is UTC; see `humble::schema::Product`. The hour itself is deliberately not
        // asserted anywhere, because Humble's changeover follows a US-local rule that shifts
        // by an hour across daylight saving.
        assert_eq!(
            ends_at("Beyond the Metroidverse Bundle"),
            Timestamp::parse("2026-09-17T04:00:00").expect("parses")
        );
    }
}
