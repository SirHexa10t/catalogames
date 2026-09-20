//! Reading what Steam says about one app, from captured responses.

use catalogames::Error;
use catalogames::inventory::steam::{Deck, Rating, Vr};
use catalogames::render::{self, Palette, Preview};
use catalogames::steam::store::AppKind;
use catalogames::steam::store::{self, GameDetails};
use catalogames::steam::{self, app_id_from_url};

const APP: u32 = 1202130;
const APP_DETAILS: &str = include_str!("fixtures/steam/1202130-appdetails.json");
const APP_REVIEWS: &str = include_str!("fixtures/steam/1202130-appreviews.json");
const DECK: &str = include_str!("fixtures/steam/1202130-deck.json");
const PAGE: &str = include_str!("fixtures/steam/1202130-page.html");
const AGE_GATE: &str = include_str!("fixtures/steam/agecheck-283640.html");

fn details() -> store::AppDetails {
    store::parse_app_details(APP_DETAILS, APP)
        .expect("fixture parses")
        .expect("the app exists")
}

fn assembled() -> GameDetails {
    store::assemble(
        details(),
        store::parse_store_page(PAGE, APP).expect("page parses"),
        store::parse_reviews(APP_REVIEWS, APP).expect("reviews parse"),
        store::parse_deck(DECK, APP).expect("deck parses"),
    )
}

mod app_details {
    use super::*;

    #[test]
    fn names_and_types_the_app_and_dates_it_the_way_steam_prints() {
        let d = details();
        assert_eq!(d.app_id, APP);
        assert_eq!(d.name, "Starship Troopers: Terran Command");
        assert_eq!(d.kind, AppKind::Game);
        assert_eq!(d.released.as_deref(), Some("Jun 16, 2022"));
    }

    #[test]
    fn reads_the_platforms() {
        let d = details();
        assert!(d.os.windows);
        assert!(!d.os.mac);
        assert!(!d.os.linux);
    }

    #[test]
    fn keeps_category_ids_beside_their_descriptions() {
        // Descriptions are localized and ids are not; both are kept so a caller can match on
        // the id and display the description.
        let d = details();
        assert!(!d.categories.is_empty());
        assert!(
            d.categories
                .iter()
                .any(|(id, name)| *id == 2 && name == "Single-player"),
            "{:?}",
            d.categories
        );
    }

    #[test]
    fn strips_prices_from_package_options() {
        let d = details();
        assert!(!d.package_options.is_empty());
        assert!(
            d.package_options.iter().all(|o| !o.contains('$')),
            "{:?}",
            d.package_options
        );
    }

    #[test]
    fn a_type_this_crate_has_not_seen_is_carried_not_assumed_to_be_a_game() {
        let odd = format!(
            r#"{{"{APP}":{{"success":true,"data":{{"name":"X","type":"hardware","platforms":{{"windows":true}}}}}}}}"#
        );
        let d = store::parse_app_details(&odd, APP)
            .expect("parses")
            .expect("exists");
        assert_eq!(d.kind, AppKind::Other("hardware".to_owned()));
    }

    #[test]
    fn an_unknown_app_is_none_not_an_error() {
        let missing = r#"{"999999999":{"success":false}}"#;
        assert!(
            store::parse_app_details(missing, 999_999_999)
                .expect("parses")
                .is_none()
        );
    }

    #[test]
    fn a_response_without_platforms_means_the_filter_list_is_wrong_and_fails_loudly() {
        // Steam ignores an unrecognised filter name silently; the only way to notice is that a
        // field the list should have delivered is missing.
        let hollow =
            format!(r#"{{"{APP}":{{"success":true,"data":{{"name":"X","type":"game"}}}}}}"#);
        assert!(matches!(
            store::parse_app_details(&hollow, APP),
            Err(Error::Drift { .. })
        ));
    }

    #[test]
    fn malformed_json_is_a_payload_error() {
        assert!(matches!(
            store::parse_app_details("{ nope", APP),
            Err(Error::Payload { .. })
        ));
    }
}

mod reviews {
    use super::*;

    #[test]
    fn stores_a_floored_percentage_and_the_total_count() {
        // 10,510 of 12,032 is 87.35%, which floors to 87.
        let r = store::parse_reviews(APP_REVIEWS, APP).expect("parses");
        assert_eq!(r.count, 12_032);
        assert_eq!(r.approval, 87);
    }

    #[test]
    fn the_derived_band_agrees_with_valves_own_index() {
        // The fixture carries review_score 8; the parser would have refused it otherwise.
        let r = store::parse_reviews(APP_REVIEWS, APP).expect("parses");
        assert_eq!(r.rating(), Rating::VeryPositive);
        assert_eq!(Rating::from_valve_score(8), Some(Rating::VeryPositive));
    }

    #[test]
    fn a_band_that_no_longer_matches_the_thresholds_is_refused() {
        // 87% over 12,032 derives Very Positive; a response claiming Mixed means the thresholds
        // moved, and a table row must not be written from it.
        let moved =
            r#"{"query_summary":{"total_reviews":12032,"total_positive":10510,"review_score":5}}"#;
        let failure = store::parse_reviews(moved, APP).unwrap_err();
        assert!(matches!(failure, Error::Drift { .. }), "{failure:?}");
        assert!(
            failure.to_string().contains("thresholds have moved"),
            "{failure}"
        );
    }

    #[test]
    fn a_summary_missing_its_totals_is_refused_rather_than_zeroed() {
        let empty = r#"{"query_summary":{"num_reviews":0}}"#;
        assert!(matches!(
            store::parse_reviews(empty, APP),
            Err(Error::Drift { .. })
        ));
    }
}

mod deck {
    use super::*;

    #[test]
    fn reads_valves_verdict() {
        assert_eq!(
            store::parse_deck(DECK, APP).expect("parses"),
            Deck::Playable
        );
    }

    #[test]
    fn a_report_for_no_app_is_an_error_not_unknown() {
        // Steam answers success:1 with an empty ARRAY for an app-id that does not exist.
        // "No report" and a genuine Unknown rating are different facts.
        let nothing = r#"{"success":1,"results":[]}"#;
        assert!(matches!(
            store::parse_deck(nothing, 0),
            Err(Error::Drift { .. })
        ));
    }

    #[test]
    fn a_real_app_valve_has_not_rated_reports_an_explicit_zero_which_is_unknown() {
        let unrated = r#"{"success":1,"results":{"appid":1,"resolved_category":0}}"#;
        assert_eq!(
            store::parse_deck(unrated, 1).expect("parses"),
            Deck::Unknown
        );
    }

    #[test]
    fn a_report_missing_its_category_is_an_error_not_unknown() {
        // Unknown has a defined meaning — "Valve has not rated it" — and a renamed field must
        // not be allowed to wear it.
        let renamed = r#"{"success":1,"results":{"appid":1,"category":2}}"#;
        assert!(matches!(
            store::parse_deck(renamed, 1),
            Err(Error::Drift { .. })
        ));
    }

    #[test]
    fn a_category_valve_does_not_publish_is_refused() {
        let odd = r#"{"success":1,"results":{"resolved_category":7}}"#;
        assert!(matches!(
            store::parse_deck(odd, 1),
            Err(Error::Drift { .. })
        ));
    }
}

mod the_store_page {
    use super::*;

    #[test]
    fn yields_every_tag_by_name_in_steams_order() {
        let page = store::parse_store_page(PAGE, APP).expect("parses");
        assert!(page.tags.len() >= 10, "{:?}", page.tags);
        assert_eq!(
            page.tags.first().map(String::as_str),
            Some("Strategy"),
            "{:?}",
            page.tags
        );
    }

    #[test]
    fn every_tag_on_a_real_page_is_one_the_inventory_enum_knows() {
        // The library returns strings so a new Valve tag cannot break a runtime fetch; the
        // generator maps them to the enum and fails closed. This checks the two agree today.
        use catalogames::inventory::steam::Tag;
        let page = store::parse_store_page(PAGE, APP).expect("parses");
        for tag in &page.tags {
            assert!(Tag::from_name(tag).is_some(), "{tag:?} is not in tags.rs");
        }
    }

    #[test]
    fn reads_the_thirty_day_row_when_steam_shows_one() {
        let page = store::parse_store_page(PAGE, APP).expect("parses");
        let recent = page.recent.expect("this capture has a recent row");
        assert!(recent.count > 0);
        assert!(recent.approval <= 100);
    }

    #[test]
    fn steams_actual_age_gate_page_is_diagnosed_as_exactly_that() {
        // A real capture of what an ungated fetch of a mature title returns: HTTP 200, no tags,
        // no review rows. Parsing it as a game would yield a thinner record that looks complete.
        let failure = store::parse_store_page(AGE_GATE, 283640).unwrap_err();
        assert!(matches!(failure, Error::Drift { .. }), "{failure:?}");
        assert!(
            failure.to_string().contains("age gate not cleared"),
            "{failure}"
        );
    }

    #[test]
    fn a_page_with_no_tag_data_and_no_gate_is_a_layout_change() {
        let odd = "<html><body><div id='userReviews'></div></body></html>";
        let failure = store::parse_store_page(odd, APP).unwrap_err();
        assert!(
            failure.to_string().contains("layout has changed"),
            "{failure}"
        );
    }
}

mod assembling {
    use super::*;

    #[test]
    fn combines_the_four_sources_into_one_record() {
        let g = assembled();
        assert_eq!(g.app_id, APP);
        assert_eq!(g.name, "Starship Troopers: Terran Command");
        assert_eq!(g.kind, AppKind::Game);
        assert_eq!(g.released, "Jun 16, 2022");
        assert_eq!(g.all_time.count, 12_032);
        assert_eq!(g.deck, Deck::Playable);
        assert_eq!(g.vr, Vr::None);
        assert!(g.os.windows && !g.os.linux);
        assert!(
            g.features.iter().any(|f| f == "Single-player"),
            "{:?}",
            g.features
        );
        assert!(!g.tags.is_empty());
    }

    #[test]
    fn a_missing_release_date_is_named_rather_than_blank() {
        let mut d = details();
        d.released = None;
        let g = store::assemble(
            d,
            store::parse_store_page(PAGE, APP).expect("parses"),
            store::parse_reviews(APP_REVIEWS, APP).expect("parses"),
            Deck::Unknown,
        );
        assert_eq!(g.released, "Unknown");
    }
}

mod the_preview_line {
    use super::*;

    #[test]
    fn a_fetched_game_prints_the_same_shape_as_a_bundle_line() {
        let printed = render::line(&Preview::of_details(&assembled()), Palette::Plain);
        let game_line = printed.lines().next().expect("a game line");
        assert!(
            game_line.starts_with("Starship Troopers: Terran Command  "),
            "{printed}"
        );
        assert!(game_line.contains("Very Positive 12032 |"), "{printed}");
        assert!(game_line.ends_with(&steam::app_url(APP)), "{printed}");
    }

    #[test]
    fn the_url_is_the_last_field_of_the_game_line() {
        // A detail line follows and carries no link, so `awk '/https/{print $NF}'` still picks
        // out exactly the links — but the URL is no longer the last field of the whole output.
        let printed = render::line(&Preview::of_details(&assembled()), Palette::Plain);
        let game_line = printed.lines().next().expect("a game line");
        assert_eq!(
            game_line.split_whitespace().next_back(),
            Some(steam::app_url(APP).as_str())
        );
        assert!(
            !printed
                .lines()
                .nth(1)
                .expect("a detail line")
                .contains("https"),
            "{printed}"
        );
    }

    #[test]
    fn a_fetched_game_carries_its_detail_line() {
        // Everything the entry knows beyond the reviews, through the same `Detail` that every
        // store's games are rendered with.
        let printed = render::line(&Preview::of_details(&assembled()), Palette::Plain);
        let detail = printed.lines().nth(1).expect("a detail line");
        assert!(
            detail.trim_start().starts_with('└'),
            "marked as belonging to the game above: {printed}"
        );
        assert!(
            detail.starts_with("  "),
            "indented under its game: {printed}"
        );
        assert!(detail.contains("Jun 16, 2022"), "{printed}");
        assert!(detail.contains("Windows"), "{printed}");
        assert!(detail.contains("Deck playable"), "{printed}");
        assert!(detail.contains("Strategy"), "{printed}");
    }
}

mod app_ids_from_what_people_paste {
    use super::*;

    #[test]
    fn a_bare_number_is_an_app_id() {
        assert_eq!(app_id_from_url("1202130"), Some(1202130));
        assert_eq!(app_id_from_url("  1202130 "), Some(1202130));
    }

    #[test]
    fn a_store_url_with_or_without_its_slug_is_an_app_id() {
        assert_eq!(
            app_id_from_url("https://store.steampowered.com/app/1202130"),
            Some(1202130)
        );
        assert_eq!(
            app_id_from_url("https://store.steampowered.com/app/1202130/"),
            Some(1202130)
        );
        assert_eq!(
            app_id_from_url(
                "https://store.steampowered.com/app/1202130/Starship_Troopers_Terran_Command/?l=english"
            ),
            Some(1202130)
        );
        assert_eq!(
            app_id_from_url("https://steamcommunity.com/app/1202130/discussions/"),
            Some(1202130)
        );
    }

    #[test]
    fn the_age_gates_own_redirect_url_and_steams_short_link_are_app_ids_too() {
        // The gate rewrites the effective URL; a client detecting it by URL will hand this in.
        assert_eq!(
            app_id_from_url("https://store.steampowered.com/agecheck/app/283640/"),
            Some(283640)
        );
        assert_eq!(app_id_from_url("https://s.team/a/283640"), Some(283640));
    }

    #[test]
    fn anything_else_is_refused_rather_than_guessed() {
        assert_eq!(
            app_id_from_url("https://store.steampowered.com/search/?term=starship"),
            None
        );
        assert_eq!(app_id_from_url("https://store.steampowered.com/app/"), None);
        assert_eq!(app_id_from_url("starship troopers"), None);
        assert_eq!(app_id_from_url(""), None);
    }
}
