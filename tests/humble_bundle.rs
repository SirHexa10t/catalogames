//! Reading a bundle's own page: which games it contains, and what looks wrong.

use catalogames::commands::sales::humble;
use catalogames::{Error, Game, ProblemKind};
use serde_json::{Value, json};

const BUNDLE: &str = include_str!("fixtures/bundle-page.html");

fn entry(title: &str, advertised: Option<usize>) -> humble::Entry {
    humble::Entry {
        title: title.to_owned(),
        url: format!("https://www.humblebundle.com/games/{title}"),
        advertised_games: advertised,
        ends_at: None,
    }
}

/// Builds a bundle page from tiers and an item table.
///
/// `tiers` maps a tier id to the machine names it sells; `items` maps a machine
/// name to its display title, or to `None` for an item the page names but does
/// not title.
fn bundle_page(tiers: &[(&str, &[&str])], items: &[(&str, Option<&str>)]) -> String {
    let tier_display: Value = tiers
        .iter()
        .map(|(id, names)| {
            (
                (*id).to_owned(),
                json!({ "tier_item_machine_names": names }),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into();
    let tier_items: Value = items
        .iter()
        .map(|(name, title)| ((*name).to_owned(), json!({ "human_name": title })))
        .collect::<serde_json::Map<_, _>>()
        .into();

    let payload = json!({ "bundleData": {
        "tier_display_data": tier_display,
        "tier_item_data": tier_items,
    }});
    format!(
        "<!doctype html><html><body>\
         <script id=\"webpack-bundle-page-data\" type=\"application/json\">{payload}</script>\
         </body></html>"
    )
}

/// Two tiers where the *first* by name is the larger one, so any code that
/// picked a tier by position rather than by size would pick wrongly.
fn misleadingly_ordered_tiers() -> String {
    bundle_page(
        &[
            ("aaa_big", &["one", "two", "three"]),
            ("zzz_small", &["one"]),
        ],
        &[
            ("one", Some("One")),
            ("two", Some("Two")),
            ("three", Some("Three")),
        ],
    )
}

mod a_real_capture {
    use super::*;

    #[test]
    fn lists_the_largest_tier_in_the_order_the_vendor_gives() {
        let entry = entry("Beyond the Metroidverse Bundle", Some(7));
        let (bundle, problems) = humble::parse_bundle(BUNDLE, &entry).expect("fixture parses");

        assert_eq!(bundle.title, entry.title);
        assert_eq!(bundle.url, entry.url);
        assert_eq!(
            bundle.games,
            [
                ("Supraland Six Inches Under", "supralandsixinchesunder"),
                ("HAAK", "haak"),
                (
                    "Monster Boy and the Cursed Kingdom",
                    "monsterboyandthecursedkingdom"
                ),
                ("Worldless", "worldless"),
                ("Hunter X", "hunterx"),
                ("Salt and Sanctuary", "salt_and_sanctuary"),
                ("Salt and Sacrifice", "saltandsacrifice"),
            ]
            .map(|(title, machine_name)| Game {
                title: title.to_owned(),
                machine_name: machine_name.to_owned(),
                // Humble publishes no Steam ids anywhere in its bundle data — checked
                // exhaustively against a live page, not assumed.
                steam_app_id: None,
                contains: Vec::new(),
            })
        );
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn takes_the_largest_tier_not_the_cheapest() {
        // The capture sells a 2-game tier and a 7-game tier.
        let (bundle, _) = humble::parse_bundle(BUNDLE, &entry("Beyond", None)).expect("parses");
        assert_eq!(bundle.games.len(), 7);
    }

    #[test]
    fn leaves_out_items_that_belong_to_no_tier() {
        // The capture's item table also holds the bundle's charity, which is
        // not for sale and must not be listed as a game.
        let (bundle, _) = humble::parse_bundle(BUNDLE, &entry("Beyond", None)).expect("parses");
        assert!(
            !bundle.games.iter().any(|g| g.machine_name == "roomtoread"),
            "charity leaked into the game list: {:?}",
            bundle.games,
        );
    }

    #[test]
    fn keeps_repeated_titles_that_are_distinct_items() {
        // Humble's 15th-anniversary bundle sells two copies of each game — one
        // to keep, one to gift — as machine names suffixed `_duplicate`, and
        // advertises the doubled count. Collapsing them by title would
        // contradict the vendor's own count and lose what the buyer gets, so
        // items are listed as the vendor sells them. `machine_name` is what
        // tells two copies apart.
        let html = bundle_page(
            &[("only", &["mimesis", "mimesis_duplicate"])],
            &[
                ("mimesis", Some("MIMESIS")),
                ("mimesis_duplicate", Some("MIMESIS")),
            ],
        );
        let (bundle, problems) = humble::parse_bundle(&html, &entry("B", Some(2))).expect("parses");

        assert_eq!(bundle.games.len(), 2, "both copies are kept");
        assert_eq!(bundle.games[0].title, bundle.games[1].title);
        assert_ne!(
            bundle.games[0].machine_name, bundle.games[1].machine_name,
            "the copies stay distinguishable"
        );
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn agrees_with_the_count_the_index_advertises() {
        let (bundle, problems) =
            humble::parse_bundle(BUNDLE, &entry("Beyond", Some(7))).expect("parses");
        assert_eq!(bundle.games.len(), 7);
        assert!(problems.is_empty(), "{problems:?}");
    }
}

mod choosing_the_largest_tier {
    use super::*;

    #[test]
    fn goes_by_size_not_by_position() {
        let (bundle, _) =
            humble::parse_bundle(&misleadingly_ordered_tiers(), &entry("B", None)).expect("parses");
        assert_eq!(bundle.games.len(), 3);
    }

    #[test]
    fn is_stable_when_two_tiers_are_the_same_size() {
        // Ties break by tier id, so repeated runs cannot disagree about which
        // tier "the largest" means.
        let html = bundle_page(
            &[("alpha", &["one"]), ("beta", &["two"])],
            &[("one", Some("One")), ("two", Some("Two"))],
        );
        let first = humble::parse_bundle(&html, &entry("B", None))
            .expect("parses")
            .0;
        let again = humble::parse_bundle(&html, &entry("B", None))
            .expect("parses")
            .0;
        assert_eq!(first.games, again.games);
    }

    #[test]
    fn reports_tiers_that_are_not_contained_in_it() {
        // Tiers have always been cumulative, which is the only thing that makes
        // the largest tier mean "the whole bundle". If that stops holding, the
        // run says so instead of quietly returning a subset.
        let html = bundle_page(
            &[("big", &["one", "two"]), ("odd", &["three"])],
            &[
                ("one", Some("One")),
                ("two", Some("Two")),
                ("three", Some("Three")),
            ],
        );
        let (bundle, problems) = humble::parse_bundle(&html, &entry("B", None)).expect("parses");

        assert_eq!(bundle.games.len(), 2);
        assert!(
            problems.iter().any(|p| matches!(
                &p.kind,
                ProblemKind::TiersNotCumulative { largest, smaller } if largest == "big" && smaller == "odd"
            )),
            "{problems:?}"
        );
    }

    #[test]
    fn stays_quiet_when_the_smaller_tier_is_contained_in_it() {
        let (_, problems) =
            humble::parse_bundle(&misleadingly_ordered_tiers(), &entry("B", None)).expect("parses");
        assert!(problems.is_empty(), "{problems:?}");
    }
}

mod cross_checking_the_advertised_count {
    use super::*;

    fn problems_for(advertised: Option<usize>) -> Vec<catalogames::Problem> {
        // Two games in the largest tier.
        let html = bundle_page(
            &[("only", &["one", "two"])],
            &[("one", Some("One")), ("two", Some("Two"))],
        );
        humble::parse_bundle(&html, &entry("B", advertised))
            .expect("parses")
            .1
    }

    #[test]
    fn is_silent_when_the_two_sources_agree() {
        assert!(problems_for(Some(2)).is_empty());
    }

    #[test]
    fn reports_a_disagreement_with_both_numbers() {
        // The index computes its count independently of the tier data, so a
        // disagreement is the earliest sign that the tier model has moved.
        let problems = problems_for(Some(9));
        assert!(
            problems.iter().any(|p| matches!(
                p.kind,
                ProblemKind::GameCountMismatch {
                    advertised: 9,
                    found: 2
                }
            )),
            "{problems:?}"
        );
    }

    #[test]
    fn is_skipped_when_the_index_advertised_nothing() {
        assert!(problems_for(None).is_empty());
    }
}

mod a_page_that_changed_shape {
    use super::*;

    #[test]
    fn without_the_data_block_says_so() {
        let html = "<!doctype html><html><body><p>nothing here</p></body></html>";
        assert!(matches!(
            humble::parse_bundle(html, &entry("B", None)),
            Err(Error::MissingDataBlock {
                block: "webpack-bundle-page-data",
                ..
            })
        ));
    }

    #[test]
    fn using_the_index_pages_block_id_says_so() {
        // The two pages carry their JSON under different ids; confusing them is
        // the likeliest way to misread one for the other.
        let html = "<html><body><script id=\"landingPage-json-data\" \
                    type=\"application/json\">{}</script></body></html>";
        assert!(matches!(
            humble::parse_bundle(html, &entry("B", None)),
            Err(Error::MissingDataBlock { .. })
        ));
    }

    #[test]
    fn declaring_no_tiers_is_an_error_not_an_empty_bundle() {
        assert!(matches!(
            humble::parse_bundle(&bundle_page(&[], &[]), &entry("Empty", None)),
            Err(Error::NoTiers { title }) if title == "Empty"
        ));
    }

    #[test]
    fn a_tier_holding_nothing_is_an_error_not_an_empty_bundle() {
        assert!(matches!(
            humble::parse_bundle(&bundle_page(&[("only", &[])], &[]), &entry("Empty", None)),
            Err(Error::NoGames { title }) if title == "Empty"
        ));
    }

    #[test]
    fn losing_every_title_is_an_error_not_an_empty_bundle() {
        // If `human_name` were renamed, every item would go unresolved. That
        // must fail the bundle rather than report it as having no games.
        let html = bundle_page(
            &[("only", &["one", "two"])],
            &[("one", None), ("two", None)],
        );
        assert!(matches!(
            humble::parse_bundle(&html, &entry("Nameless", None)),
            Err(Error::NoGames { .. })
        ));
    }

    #[test]
    fn losing_one_title_reports_it_and_keeps_the_rest() {
        let html = bundle_page(
            &[("only", &["one", "mystery"])],
            &[("one", Some("One")), ("mystery", None)],
        );
        let (bundle, problems) = humble::parse_bundle(&html, &entry("B", None)).expect("parses");

        assert_eq!(bundle.games.len(), 1, "the named game still comes through");
        assert!(
            problems.iter().any(|p| matches!(
                &p.kind,
                ProblemKind::UnresolvedItems { machine_names } if machine_names == &["mystery"]
            )),
            "{problems:?}"
        );
    }

    #[test]
    fn a_tier_naming_an_item_the_table_omits_reports_it() {
        let html = bundle_page(&[("only", &["one", "ghost"])], &[("one", Some("One"))]);
        let (_, problems) = humble::parse_bundle(&html, &entry("B", None)).expect("parses");
        assert!(
            problems.iter().any(|p| matches!(
                &p.kind,
                ProblemKind::UnresolvedItems { machine_names } if machine_names == &["ghost"]
            )),
            "{problems:?}"
        );
    }

    #[test]
    fn dropping_a_load_bearing_field_fails_loudly() {
        // Defaulting `tier_display_data` to an empty map would turn a rename
        // into a bundle that "successfully" contains no games.
        let html = "<html><body><script id=\"webpack-bundle-page-data\" \
                    type=\"application/json\">{\"bundleData\":{}}</script></body></html>";
        assert!(matches!(
            humble::parse_bundle(html, &entry("B", None)),
            Err(Error::Schema { .. })
        ));
    }
}

mod what_the_bundle_costs {
    use super::*;
    use catalogames::{Money, Price};

    #[test]
    fn the_price_is_what_the_largest_tier_asks_for() {
        // Humble tiers are cumulative: the largest one holds every game the bundle offers, so
        // its price is the least those games can be bought for. That is a whole-bundle price
        // and never a per-game rate — nobody can buy one game out of a Humble bundle, so an
        // average would be a number the store does not offer.
        let (bundle, _) =
            humble::parse_bundle(BUNDLE, &entry("Beyond the Metroidverse Bundle", None))
                .expect("fixture parses");
        assert_eq!(
            bundle.price,
            Some(Price::Whole(Money::new(1000, "USD"))),
            "the capture's largest tier is bt10 at US$10"
        );
    }

    #[test]
    fn a_page_that_publishes_no_prices_still_lists_its_games() {
        // Losing a price is recoverable; losing the games is not. The synthetic page carries no
        // tier_pricing_data at all, which is what a schema change would look like.
        let page = bundle_page(
            &[("a", &["one", "two"])],
            &[("one", Some("One")), ("two", Some("Two"))],
        );
        let (bundle, _) = humble::parse_bundle(&page, &entry("B", None)).expect("parses");
        assert_eq!(bundle.price, None);
        assert_eq!(
            bundle.games.len(),
            2,
            "the games survived the missing price"
        );
    }
}
