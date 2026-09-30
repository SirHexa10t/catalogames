//! Turning a listing into the text the CLI prints.

use catalogames::clock::Timestamp;
use catalogames::inventory::steam as steam_inventory;
use catalogames::render::Palette;
use catalogames::steam;
use catalogames::steam::items::Classified;
use catalogames::store_inventory::steam::snapshot;
use catalogames::user_games::holdings::Holdings;
use catalogames::{Bundle, Game, Ladder, Listing, Tier, render};

fn game(title: &str) -> Game {
    Game {
        title: title.to_owned(),
        machine_name: title.to_lowercase().replace(' ', "_"),
        steam_app_id: None,
        contains: Vec::new(),
    }
}

fn bundle(title: &str, games: &[&str]) -> Bundle {
    Bundle {
        title: title.to_owned(),
        url: format!("https://www.humblebundle.com/games/{title}"),
        price: None,
        ends_at: None,
        games: games.iter().copied().map(game).collect(),
    }
}

/// A bundle holding games already built, where `bundle` takes titles.
fn bundle_of(games: Vec<Game>) -> Bundle {
    Bundle {
        games,
        ..bundle("A Bundle", &[])
    }
}

fn listing(bundles: Vec<Bundle>) -> Listing {
    Listing {
        bundles,
        problems: Vec::new(),
    }
}

/// A title no store sells, so it cannot be in the inventory however the table changes.
const UNKNOWN: &str = "A Game That Does Not Exist Anywhere";

/// The words on each line, with column padding and links normalised away.
///
/// Content, layout and links are asserted separately: a change to column widths or to how a
/// link is built should fail the one test that pins that thing, not every test that mentions a
/// game.
fn lines(rendered: &str) -> Vec<String> {
    rendered
        .lines()
        .map(|line| match line.find("  https://") {
            Some(at) => &line[..at],
            None => line,
        })
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect()
}

/// The last whitespace-separated field of each line that has one — the documented parse.
fn trailing_urls(rendered: &str) -> Vec<&str> {
    rendered
        .lines()
        .filter_map(|line| line.split_whitespace().next_back())
        .filter(|field| field.starts_with("https://"))
        .collect()
}

mod what_it_says {
    use super::*;

    #[test]
    fn names_each_bundle_then_each_of_its_games() {
        let rendered = render::listing(
            &listing(vec![bundle("A Bundle", &[UNKNOWN])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        // The trailing "-" is the review column with nothing in it.
        assert_eq!(lines(&rendered), ["A Bundle", &format!("- {UNKNOWN} -")]);
    }

    #[test]
    fn a_review_column_with_nothing_in_it_is_a_placeholder_not_a_blank() {
        // Columns are separated by whitespace, so an empty cell would merge with the gap beside
        // it and the row would be read as having one column fewer — which misaligns every row
        // after it. The placeholder also says the true thing: not known, as against a bad score.
        let rendered = render::listing(
            &listing(vec![bundle("B", &[UNKNOWN, "Another Unknown Game"])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        for line in rendered.lines().filter(|l| l.starts_with("  -")) {
            let fields: Vec<&str> = line.split("  ").filter(|f| !f.trim().is_empty()).collect();
            // Name, reviews, link. The bullet belongs to the name cell — joined by a single
            // space — so it is never a column of its own, and the dashes are never padded.
            assert_eq!(fields.len(), 3, "expected three cells in {line:?}");
        }
    }

    #[test]
    fn keeps_bundles_and_games_in_the_order_given() {
        let rendered = render::listing(
            &listing(vec![
                bundle("Second Best", &["Beta Game", "Alpha Game"]),
                bundle("Also Ran", &["Gamma Game"]),
            ]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        let lines = lines(&rendered);
        assert_eq!(lines[0], "Second Best");
        assert!(lines[1].starts_with("- Beta Game"), "{lines:?}");
        assert!(lines[2].starts_with("- Alpha Game"), "{lines:?}");
        assert_eq!(lines[3], "Also Ran");
    }

    #[test]
    fn renders_a_bundle_that_has_no_games_as_just_its_title() {
        assert_eq!(
            lines(&render::listing(
                &listing(vec![bundle("Empty", &[])]),
                Palette::Plain,
                &Holdings::none(),
                &Classified::none(),
            )),
            ["Empty"]
        );
    }

    #[test]
    fn renders_nothing_at_all_for_an_empty_listing() {
        assert_eq!(
            render::listing(
                &listing(Vec::new()),
                Palette::Plain,
                &Holdings::none(),
                &Classified::none()
            ),
            ""
        );
    }
}

mod a_game_the_inventory_knows {
    use super::*;

    /// Skips when the table is empty — it is generated, and a bootstrap build has no rows.
    fn known() -> Option<&'static steam_inventory::SteamGame> {
        steam_inventory::all().next()
    }

    #[test]
    fn links_to_its_own_store_page_rather_than_a_search() {
        let Some(entry) = known() else { return };
        let rendered = render::listing(
            &listing(vec![bundle("B", &[entry.name])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );

        assert!(
            rendered.contains(&steam::app_url(entry.app_id)),
            "{rendered}"
        );
        assert!(
            !rendered.contains(steam::SEARCH_BASE),
            "fell back to a search: {rendered}"
        );
    }

    #[test]
    fn shows_its_all_time_verdict_and_count() {
        let Some(entry) = known() else { return };
        let rendered = render::listing(
            &listing(vec![bundle("B", &[entry.name])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );

        assert!(
            rendered.contains(entry.all_time.rating().as_str()),
            "{rendered}"
        );
        assert!(
            rendered.contains(&entry.all_time.count.to_string()),
            "{rendered}"
        );
    }

    #[test]
    fn shows_the_verdict_derived_from_its_stored_percentage() {
        // The band is not stored; it comes from the approval percentage and the review count,
        // so what renders must be what the translator derives.
        let Some(entry) = known() else { return };
        let rendered = render::listing(
            &listing(vec![bundle("B", &[entry.name])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        let derived =
            steam_inventory::Rating::from_score(entry.all_time.approval, entry.all_time.count);
        assert!(rendered.contains(derived.as_str()), "{rendered}");
    }

    #[test]
    fn reads_the_recent_column_with_the_recent_rule_not_the_all_time_one() {
        // Steam gates the two windows differently, so rendering both through the all-time rule
        // silently under-reports the recent verdict on any game with few recent reviews.
        let Some(entry) = steam_inventory::all()
            .find(|g| g.recent.is_some_and(|r| r.rating() != r.recent_rating()))
        else {
            return;
        };
        let recent = entry.recent.expect("filtered on Some");
        let rendered = render::listing(
            &listing(vec![bundle("B", &[entry.name])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );

        let expected = format!("{} {}", recent.recent_rating().as_str(), recent.count);
        assert!(
            rendered.contains(&expected),
            "expected {expected:?} in:\n{rendered}"
        );
    }

    #[test]
    fn shows_a_recent_verdict_only_when_steam_published_one() {
        let Some(entry) = known() else { return };
        let rendered = render::listing(
            &listing(vec![bundle("B", &[entry.name])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );

        match entry.recent {
            Some(recent) => assert!(rendered.contains(&recent.count.to_string()), "{rendered}"),
            // An absent recent row means Steam showed none, which is not the same fact as a bad
            // score, so it renders as a placeholder rather than a zero.
            None => assert!(rendered.contains(" | -"), "{rendered}"),
        }
    }

    #[test]
    fn is_found_however_the_bundle_spells_it() {
        let Some(entry) = known() else { return };
        let rendered = render::listing(
            &listing(vec![bundle("B", &[&entry.name.to_uppercase()])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(
            rendered.contains(&steam::app_url(entry.app_id)),
            "{rendered}"
        );
    }

    #[test]
    fn is_found_by_any_alias_it_records() {
        let Some(entry) = steam_inventory::all().find(|g| !g.aliases.is_empty()) else {
            return;
        };
        let rendered = render::listing(
            &listing(vec![bundle("B", &[entry.aliases[0]])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(
            rendered.contains(&steam::app_url(entry.app_id)),
            "{rendered}"
        );
    }
}

mod a_game_whose_store_supplied_an_app_id {
    use super::*;

    fn identified(title: &str, app_id: u32) -> Game {
        Game {
            title: title.to_owned(),
            machine_name: title.to_lowercase().replace(' ', "_"),
            steam_app_id: Some(app_id),
            contains: Vec::new(),
        }
    }

    fn bundle_of(games: Vec<Game>) -> Listing {
        listing(vec![Bundle {
            title: "B".to_owned(),
            url: "https://example.test/b".to_owned(),
            price: None,
            ends_at: None,
            games,
        }])
    }

    #[test]
    fn links_straight_to_that_app_even_when_the_inventory_has_no_entry() {
        // The curated table has never carried Counter-Strike; the snapshot has, and names it
        // as the store does. An id a table confirms is an identity, and falling back to a
        // search would discard it.
        assert!(
            steam_inventory::by_app_id(10).is_none(),
            "the test needs an uncurated app"
        );
        let rendered = render::listing(
            &bundle_of(vec![identified("Counter-Strike", 10)]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(rendered.contains(&steam::app_url(10)), "{rendered}");
        assert!(!rendered.contains(steam::SEARCH_BASE), "{rendered}");
    }

    #[test]
    fn an_id_nothing_holds_is_kept_when_the_title_resolves_nothing() {
        // Nothing can confirm or deny an id no table holds — a game newer than the snapshot
        // looks like this, and so does a bundle id — so once the title has resolved nothing
        // the store's word is all there is, and it is linked. Loudly, if it was a bundle id:
        // that page bounces, which is better than a silent search for a title nobody sells.
        let rendered = render::listing(
            &bundle_of(vec![identified(UNKNOWN, 999_999)]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(rendered.contains(&steam::app_url(999_999)), "{rendered}");
        assert!(!rendered.contains(steam::SEARCH_BASE), "{rendered}");
    }

    #[test]
    fn is_preferred_over_matching_the_title_once_a_table_confirms_it() {
        // A title once resolved "Ashen" to "Ashen Empires", so an id the tables CONFIRM wins
        // over any title: the same entry, reached by an id with the store's own edition wording
        // beside it, is that entry and not a search for the wording.
        let Some(entry) = steam_inventory::all().next() else {
            return;
        };
        let worded = identified(&format!("{} - Deluxe Edition", entry.name), entry.app_id);
        let rendered = render::listing(
            &bundle_of(vec![worded]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(
            rendered.contains(&steam::app_url(entry.app_id)),
            "{rendered}"
        );
        assert!(!rendered.contains(steam::SEARCH_BASE), "{rendered}");
    }

    #[test]
    fn an_id_nothing_holds_yields_to_a_title_a_table_does() {
        // The other way round: an id NO table holds might be a bundle id — Fanatical publishes
        // Steam bundle 13009 in the same field — while a title that resolves exactly to a
        // known game is the better lead. The title's game is linked, not the unconfirmed id.
        let Some(entry) = steam_inventory::all().next() else {
            return;
        };
        let rendered = render::listing(
            &bundle_of(vec![identified(entry.name, 999_999)]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(
            rendered.contains(&steam::app_url(entry.app_id)),
            "{rendered}"
        );
        assert!(
            !rendered.contains(&steam::app_url(999_999)),
            "the unconfirmed id lost: {rendered}"
        );
    }
}

mod a_game_whose_id_the_tables_could_not_settle {
    use super::*;
    use catalogames::inventory::steam::Identity;
    use catalogames::steam::items::parse;

    /// The probe recorded in the fixture: 13009 is a bundle only, 4278390 is nothing under any
    /// kind, and 620 is both app 620 and package 620.
    const PROBE: &str = include_str!("fixtures/steam/getitems-13009-4278390-620.json");

    fn identified(title: &str, app_id: u32) -> Game {
        Game {
            title: title.to_owned(),
            machine_name: title.to_lowercase().replace(' ', "_"),
            steam_app_id: Some(app_id),
            contains: Vec::new(),
        }
    }

    /// What a run would have asked for this listing, answered from the fixture — so a disputed
    /// id is asked as a bundle and a package only, exactly as the real run asks it.
    fn asked(listing: &Listing) -> Classified {
        let answered = parse(PROBE, "fixture").expect("the fixture parses");
        Classified::from_answers(&Classified::asks([listing]), &answered)
    }

    /// The bug that started this: Fanatical publishes bundle 13009 in the field it calls an app
    /// id, and `/app/13009` bounces. Unasked, the link is as published; asked, it is the
    /// bundle's own page — in the listing and in the opener alike.
    #[test]
    fn a_bundle_id_published_as_an_app_id_links_to_the_bundle_once_steam_has_been_asked() {
        let title = "Monster Hunter World: Iceborne Digital Deluxe";
        let listing = listing(vec![bundle_of(vec![identified(title, 13_009)])]);
        let game = &listing.bundles[0].games[0];
        assert_eq!(steam_inventory::identity_of(game), Identity::Unheld(13_009));
        assert_eq!(
            render::page_of(game, &Classified::none()),
            steam::app_url(13_009),
            "unasked: as published, which is all there is"
        );
        let bundle_page = "https://store.steampowered.com/bundle/13009/Monster_Hunter_World_Iceborne_Digital_Deluxe";
        let classified = asked(&listing);
        assert_eq!(render::page_of(game, &classified), bundle_page);
        let rendered = render::listing(&listing, Palette::Plain, &Holdings::none(), &classified);
        assert!(rendered.contains(bundle_page), "{rendered}");
        let opened = catalogames::links::of_listing(&listing, &classified);
        assert_eq!(opened[0].url, bundle_page, "the opener agrees: {opened:?}");
    }

    /// An id asked about and claimed by nothing — a removed app — is a search, since its page
    /// would bounce. Unasked, silence is not "nothing": only an id the delisted ledger saw the
    /// store remove falls back to a search, and any other keeps the published link (see the
    /// bundle id above).
    #[test]
    fn an_id_nothing_claims_becomes_a_search_once_steam_or_the_ledger_has_said_so() {
        let removed = catalogames::store_inventory::steam::delisted::get(4_278_390)
            .expect("the test needs an app the ledger saw removed");
        assert_eq!(
            removed.state,
            catalogames::store_inventory::steam::delisted::State::Removed
        );
        let listing = listing(vec![bundle_of(vec![identified(UNKNOWN, 4_278_390)])]);
        let game = &listing.bundles[0].games[0];
        assert_eq!(
            render::page_of(game, &Classified::none()),
            steam::search_url(UNKNOWN),
            "unasked, the ledger's word is the only word"
        );
        assert_eq!(
            render::page_of(game, &asked(&listing)),
            steam::search_url(UNKNOWN)
        );

        // A region-restricted app is alive, and may be for sale where the reader is: unasked, it
        // keeps its page.
        let restricted = super::listing(vec![bundle_of(vec![identified(UNKNOWN, 4_600_150)])]);
        assert_eq!(
            render::page_of(&restricted.bundles[0].games[0], &Classified::none()),
            steam::app_url(4_600_150)
        );
    }

    /// A disputed id — the tables say 620 is Portal 2, the store printed it beside another title
    /// — is asked as a bundle and a package only, and the package that answers has a page: the
    /// app the package delivers, as Steam's own path says. Unasked, it is a search and never
    /// Portal 2.
    #[test]
    fn a_disputed_id_goes_to_whatever_else_claims_it_and_never_to_the_app_the_tables_named() {
        let listing = listing(vec![bundle_of(vec![identified(UNKNOWN, 620)])]);
        let game = &listing.bundles[0].games[0];
        assert_eq!(steam_inventory::identity_of(game), Identity::Disputed(620));
        assert_eq!(
            render::page_of(game, &Classified::none()),
            steam::search_url(UNKNOWN)
        );
        assert_eq!(
            render::page_of(game, &asked(&listing)),
            "https://store.steampowered.com/app/12520/18_Wheels_of_Steel_American_Long_Haul"
        );
    }
}

mod a_game_the_inventory_does_not_know {
    use super::*;

    #[test]
    fn falls_back_to_a_search_link() {
        let rendered = render::listing(
            &listing(vec![bundle("B", &[UNKNOWN])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(rendered.contains(&steam::search_url(UNKNOWN)), "{rendered}");
    }

    #[test]
    fn shows_no_review_verdict_rather_than_an_invented_one() {
        let rendered = render::listing(
            &listing(vec![bundle("B", &[UNKNOWN])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(
            !rendered.contains('|'),
            "invented a verdict column: {rendered}"
        );
        for band in ["Positive", "Negative", "Mixed"] {
            assert!(!rendered.contains(band), "invented a verdict: {rendered}");
        }
    }
}

mod how_it_is_laid_out {
    use super::*;

    #[test]
    fn the_link_is_always_the_last_field() {
        // The documented parse: `awk '{print $NF}'`. Names contain spaces, so positional
        // parsing cannot work and this is the only stable handle on the URL.
        let rendered = render::listing(
            &listing(vec![
                bundle("First", &[UNKNOWN, "Another Unknown Game"]),
                bundle("Second", &["A Third Unknown Game"]),
            ]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert_eq!(trailing_urls(&rendered).len(), 3);
    }

    #[test]
    fn a_bundle_title_line_carries_no_link() {
        let rendered = render::listing(
            &listing(vec![bundle("A Bundle", &[UNKNOWN])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(!rendered.lines().next().unwrap().contains("https://"));
    }

    #[test]
    fn game_lines_are_indented_under_their_bundle() {
        let rendered = render::listing(
            &listing(vec![bundle("A Bundle", &[UNKNOWN])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(
            rendered.lines().nth(1).unwrap().starts_with("  - "),
            "{rendered}"
        );
    }

    #[test]
    fn separates_bundles_with_one_blank_line_and_ends_with_a_newline() {
        let rendered = render::listing(
            &listing(vec![
                bundle("First", &[UNKNOWN]),
                bundle("Second", &[UNKNOWN]),
            ]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert_eq!(rendered.matches("\n\n").count(), 1, "{rendered}");
        assert!(rendered.ends_with('\n'));
        assert!(!rendered.starts_with('\n'));
    }

    #[test]
    fn a_long_title_pads_its_own_bundle_and_no_other() {
        // Each bundle is its own table. One overlong title — the bundled video courses run past
        // seventy characters — would otherwise push every other bundle's links out behind a
        // corridor of spaces, which is a cost the other bundles did nothing to earn.
        let rendered = render::listing(
            &listing(vec![
                bundle("First", &["Short"]),
                bundle("Second", &["A Very Considerably Longer Game Name Indeed"]),
            ]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        let link_columns: Vec<usize> = rendered
            .lines()
            .filter(|line| line.starts_with("  - "))
            .filter_map(|line| line.find("https://"))
            .collect();

        assert_eq!(link_columns.len(), 2);
        assert!(
            link_columns[0] < link_columns[1],
            "the short bundle was padded out to the long one:\n{rendered}"
        );
    }

    #[test]
    fn rows_inside_one_bundle_still_line_up_with_each_other() {
        // Per-bundle, not per-row: within a bundle the columns are still a table.
        let rendered = render::listing(
            &listing(vec![bundle(
                "B",
                &["Short", "A Very Considerably Longer Game Name Indeed"],
            )]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        let link_columns: Vec<usize> = rendered
            .lines()
            .filter(|line| line.starts_with("  - "))
            .filter_map(|line| line.find("https://"))
            .collect();

        assert_eq!(link_columns.len(), 2);
        assert_eq!(
            link_columns[0], link_columns[1],
            "links inside one bundle do not line up:\n{rendered}"
        );
    }
}

mod the_two_review_windows {
    use super::*;
    use catalogames::inventory::steam::Reviews;
    use catalogames::render::Preview;

    #[test]
    fn identical_figures_render_different_bands_by_position() {
        // Steam gates the two windows differently: 80% over 10 reviews is "Positive" all-time
        // and "Very Positive" in the last thirty days. One formatter serves both slots, so a
        // swap during a refactor would be silent — the same plausible words, wrong column.
        // This pins the asymmetry.
        let same = Reviews {
            approval: 80,
            count: 10,
        };
        let preview = Preview {
            standings: Default::default(),
            name: "A Game".to_owned(),
            all_time: Some(same),
            recent: Some(same),
            elsewhere: String::new(),
            url: steam::app_url(1),
            detail: None,
        };
        let line = render::line(&preview, Palette::Plain);
        assert!(line.contains("Positive 10 | Very Positive 10"), "{line}");
    }

    #[test]
    fn a_detail_line_says_nothing_the_reader_cannot_use() {
        use catalogames::inventory::steam::{Deck, Os, Vr};
        use catalogames::render::Detail;

        // "Deck untested" is a fact about Valve, not about the game, and VR-none is the norm;
        // neither earns a place on a line that has to stay readable.
        let quiet = Detail {
            released: "Jun 16, 2022".to_owned(),
            os: Os {
                windows: true,
                mac: false,
                linux: false,
            },
            vr: Vr::None,
            deck: Deck::Unknown,
            tags: vec!["Strategy".to_owned(), "Turn-Based".to_owned()],
        };
        let line = quiet.line();
        assert_eq!(line, "Jun 16, 2022 · Windows · Strategy, Turn-Based");

        // When there IS something to report, it is reported.
        let loud = Detail {
            vr: Vr::Only,
            deck: Deck::Unsupported,
            os: Os {
                windows: true,
                mac: true,
                linux: true,
            },
            ..quiet
        };
        let line = loud.line();
        assert!(line.contains("Windows/Mac/Linux"), "{line}");
        assert!(line.contains("VR only"), "{line}");
        assert!(line.contains("Deck unsupported"), "{line}");
    }

    #[test]
    fn a_detail_line_shows_the_most_voted_tags_and_stops() {
        use catalogames::inventory::steam::{Deck, Os, Vr};
        use catalogames::render::Detail;

        // Steam publishes about twenty, ordered by votes; all twenty would be a paragraph.
        let many: Vec<String> = (1..=20).map(|n| format!("Tag{n}")).collect();
        let detail = Detail {
            released: "x".to_owned(),
            os: Os {
                windows: true,
                mac: false,
                linux: false,
            },
            vr: Vr::None,
            deck: Deck::Unknown,
            tags: many,
        };
        let line = detail.line();
        assert!(line.contains("Tag1, Tag2, Tag3, Tag4, Tag5"), "{line}");
        assert!(!line.contains("Tag6"), "{line}");
    }

    #[test]
    fn every_store_and_a_direct_lookup_print_the_same_detail_for_one_game() {
        // The point of the shared preview: a game looks the same however it was found. A
        // Humble game (no app-id, matched by name) and a Fanatical one (app-id supplied) must
        // produce byte-identical output for the same inventory entry.
        let Some(entry) = steam_inventory::all().find(|g| !g.tags.is_empty()) else {
            return;
        };
        let by_name = Preview::of_game(
            &Game {
                title: entry.name.to_owned(),
                machine_name: String::new(),
                steam_app_id: None,
                contains: Vec::new(),
            },
            &Holdings::none(),
            &Classified::none(),
        );
        // Reached by id under the store's own wording for the same game — an edition suffix,
        // which is what a store actually prints beside an id. A wording no game is called would
        // contradict the id, and the two would rightly not be the same entry.
        let by_app_id = Preview::of_game(
            &Game {
                title: format!("{} - Deluxe Edition", entry.name),
                machine_name: String::new(),
                steam_app_id: Some(entry.app_id),
                contains: Vec::new(),
            },
            &Holdings::none(),
            &Classified::none(),
        );
        assert_eq!(
            by_name.detail, by_app_id.detail,
            "the same entry rendered two different details"
        );
        assert!(by_name.detail.is_some());
    }

    #[test]
    fn a_game_with_no_entry_has_no_detail_line() {
        let preview = Preview::of_game(
            &Game {
                title: UNKNOWN.to_owned(),
                machine_name: String::new(),
                steam_app_id: None,
                contains: Vec::new(),
            },
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(preview.detail.is_none());
        assert_eq!(render::line(&preview, Palette::Plain).lines().count(), 1);
    }

    #[test]
    fn a_single_line_and_a_listing_row_say_the_same_thing() {
        // The listing pads columns; the single line does not. The words must be identical,
        // because they come from the same place.
        let Some(entry) = steam_inventory::all().next() else {
            return;
        };
        let single = render::line(
            &Preview::of_game(
                &Game {
                    title: entry.name.to_owned(),
                    machine_name: String::new(),
                    steam_app_id: Some(entry.app_id),
                    contains: Vec::new(),
                },
                &Holdings::none(),
                &Classified::none(),
            ),
            Palette::Plain,
        );
        let in_listing = render::listing(
            &listing(vec![bundle("B", &[entry.name])]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        let words = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();

        // Both carry the game line and, where there is one, the detail line beneath it. The
        // listing pads columns and prefixes a bullet; the words must otherwise be identical.
        let mut row_words = words(in_listing.lines().nth(1).expect("one game row"));
        row_words.remove(0); // the "-" bullet
        let single_lines: Vec<&str> = single.lines().collect();
        assert_eq!(
            words(single_lines[0]),
            row_words,
            "\n{single}\n{in_listing}"
        );
        if let Some(detail) = single_lines.get(1) {
            let listed = in_listing.lines().nth(2).expect("a detail line");
            assert_eq!(words(detail), words(listed), "\n{single}\n{in_listing}");
        }
    }
}

mod colour {
    use super::*;
    use catalogames::inventory::steam::Reviews;
    use catalogames::render::Preview;

    const ESC: char = '\x1b';

    /// Everything a terminal would not print: the escape sequences themselves.
    fn strip_ansi(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c != ESC {
                out.push(c);
                continue;
            }
            // CSI sequences run to a letter; this output only ever emits `\x1b[…m`.
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        }
        out
    }

    fn a_listing() -> Listing {
        let Some(entry) = steam_inventory::all().find(|g| !g.tags.is_empty()) else {
            return listing(Vec::new());
        };
        listing(vec![bundle("B", &[entry.name, UNKNOWN])])
    }

    #[test]
    fn plain_output_carries_no_escape_codes() {
        // The default, and what anything piped into a file or another program must get.
        let rendered = render::listing(
            &a_listing(),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(!rendered.contains(ESC), "{rendered:?}");
    }

    #[test]
    fn colour_changes_nothing_except_colour() {
        // The strongest statement available: strip the escapes back out and the two must be
        // byte-identical. Alignment, spacing, wording and the link all survive colouring —
        // which is also what stops a coloured cell from throwing the columns out.
        let listing = a_listing();
        let plain = render::listing(
            &listing,
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        let coloured = render::listing(
            &listing,
            Palette::Ansi,
            &Holdings::none(),
            &Classified::none(),
        );
        assert_ne!(plain, coloured, "nothing was coloured at all");
        assert_eq!(plain, strip_ansi(&coloured));
    }

    #[test]
    fn a_verdict_is_painted_in_its_own_bands_shade() {
        let Some(entry) = steam_inventory::all().next() else {
            return;
        };
        let rendered = render::listing(
            &listing(vec![bundle("B", &[entry.name])]),
            Palette::Ansi,
            &Holdings::none(),
            &Classified::none(),
        );
        // Nine bands, nine shades — so the colour alone ranks the game.
        let hex = entry.all_time.rating().gradient_color();
        let rgb = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).unwrap();
        let expected = format!("\x1b[38;2;{};{};{}m", rgb(1), rgb(3), rgb(5));
        assert!(
            rendered.contains(&expected),
            "expected {expected:?} in {rendered:?}"
        );
    }

    #[test]
    fn the_two_review_windows_are_painted_by_their_own_rules() {
        // 80% over 10 reviews is Positive all-time and Very Positive over thirty days, so the
        // two halves of the column must not share a colour. A swap here would be invisible in
        // plain text and wrong in both.
        let same = Reviews {
            approval: 80,
            count: 10,
        };
        let preview = Preview {
            standings: Default::default(),
            name: "A Game".to_owned(),
            all_time: Some(same),
            recent: Some(same),
            elsewhere: String::new(),
            url: steam::app_url(1),
            detail: None,
        };
        let painted = render::line(&preview, Palette::Ansi);
        let shade = |r: catalogames::inventory::steam::Rating| {
            let hex = r.gradient_color();
            let c = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).unwrap();
            format!("\x1b[38;2;{};{};{}m", c(1), c(3), c(5))
        };
        assert!(painted.contains(&shade(same.rating())), "{painted:?}");
        assert!(
            painted.contains(&shade(same.recent_rating())),
            "{painted:?}"
        );
        assert_ne!(shade(same.rating()), shade(same.recent_rating()));
    }

    #[test]
    fn a_detail_line_is_marked_and_greyed() {
        let Some(entry) = steam_inventory::all().find(|g| !g.tags.is_empty()) else {
            return;
        };
        let listed = listing(vec![bundle("B", &[entry.name])]);

        // The marker ties the line to the game above it, in plain text as well as coloured.
        let plain = render::listing(
            &listed,
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        );
        let detail = plain.lines().nth(2).expect("a detail line");
        assert!(detail.trim_start().starts_with('└'), "{plain}");

        // Grey, so the games themselves carry the eye down the list.
        let coloured = render::listing(
            &listed,
            Palette::Ansi,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(coloured.contains("\x1b[38;2;128;128;128m└"), "{coloured:?}");
    }

    #[test]
    fn a_game_with_no_entry_is_left_uncoloured() {
        // Nothing is known about it, so there is no band to colour it by — and a colour would
        // imply one.
        let rendered = render::listing(
            &listing(vec![bundle("B", &[UNKNOWN])]),
            Palette::Ansi,
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(!rendered.contains(ESC), "{rendered:?}");
    }
}

mod when_a_bundle_stops_being_buyable {
    use super::*;

    /// Four in the afternoon on the 16th, against deadlines set relative to it.
    fn now() -> Timestamp {
        Timestamp::parse("2026-09-16T16:00:00").expect("a fixed instant")
    }

    fn with_deadline(deadline: Option<&str>) -> Listing {
        let mut bundle = bundle("A Bundle", &["Game"]);
        bundle.ends_at = deadline.map(|text| Timestamp::parse(text).expect("parses"));
        listing(vec![bundle])
    }

    fn heading(deadline: Option<&str>) -> String {
        render::listing_at(
            &with_deadline(deadline),
            Palette::Plain,
            now(),
            &Holdings::none(),
            &Classified::none(),
        )
        .lines()
        .next()
        .expect("a heading")
        .to_owned()
    }

    #[test]
    fn the_deadline_follows_the_title() {
        let line = heading(Some("2026-09-17T04:00:00"));
        assert!(line.starts_with("A Bundle"), "{line}");
        assert!(line.contains("ends 2026-09-17 04:00 UTC"), "{line}");
        assert!(line.contains("12h left"), "{line}");
    }

    #[test]
    fn a_bundle_whose_store_published_no_deadline_says_nothing_about_one() {
        // Absent is unknown, not "runs forever" — so nothing is claimed either way.
        assert_eq!(heading(None), "A Bundle");
    }

    #[test]
    fn a_deadline_already_past_reads_as_ended_rather_than_as_a_huge_number() {
        // The capture in tests/fixtures/games-index.html holds bundles that ended on
        // 2026-09-12 and 2026-09-15, so this path runs on real data every time the fixture is
        // rendered. Computed in unsigned seconds it would underflow into a plausible-looking
        // "49710 days left" that nobody reads twice.
        for past in [
            "2026-09-15T04:00:00",
            "2026-09-12T04:00:00",
            "1999-01-01T00:00:00",
        ] {
            let line = heading(Some(past));
            assert!(line.contains("ended"), "{past} rendered as {line}");
            assert!(!line.contains("left"), "{past} rendered as {line}");
        }
    }

    #[test]
    fn the_unit_shrinks_as_the_deadline_approaches() {
        // One unit throughout, and hours all the way down to the urgent band, so two bundles
        // near their deadline can be compared without converting anything.
        for (deadline, expected) in [
            ("2026-09-30T16:00:00", "14d left"),
            ("2026-09-19T15:00:00", "71h left"),
            ("2026-09-18T18:00:00", "50h left"),
            ("2026-09-17T04:00:00", "12h left"),
            ("2026-09-16T16:30:00", "30m left"),
        ] {
            let line = heading(Some(deadline));
            assert!(line.contains(expected), "{deadline} rendered as {line}");
        }
    }

    #[test]
    fn only_a_short_deadline_is_coloured_red() {
        // Fifty hours is the threshold, so a deadline just inside it is marked and one just
        // outside it is not. Both still say how long is left.
        let urgent = render::listing_at(
            &with_deadline(Some("2026-09-18T17:00:00")),
            Palette::Ansi,
            now(),
            &Holdings::none(),
            &Classified::none(),
        );
        let calm = render::listing_at(
            &with_deadline(Some("2026-09-18T19:00:00")),
            Palette::Ansi,
            now(),
            &Holdings::none(),
            &Classified::none(),
        );
        const RED: &str = "\x1b[38;2;245;48;48m";
        assert!(urgent.contains(RED), "49h left was not marked: {urgent:?}");
        assert!(!calm.contains(RED), "51h left was marked: {calm:?}");
    }

    #[test]
    fn a_plain_palette_carries_no_escape_codes_at_all() {
        let plain = render::listing_at(
            &with_deadline(Some("2026-09-17T04:00:00")),
            Palette::Plain,
            now(),
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(!plain.contains('\x1b'), "{plain:?}");
    }
}

mod what_a_bundle_costs {
    use super::*;
    use catalogames::{Money, Price};

    fn now() -> Timestamp {
        Timestamp::parse("2026-09-16T16:00:00").expect("a fixed instant")
    }

    fn heading(price: Option<Price>, deadline: Option<&str>) -> String {
        let mut bundle = bundle("A Bundle", &["Game"]);
        bundle.price = price;
        bundle.ends_at = deadline.map(|text| Timestamp::parse(text).expect("parses"));
        render::listing_at(
            &listing(vec![bundle]),
            Palette::Plain,
            now(),
            &Holdings::none(),
            &Classified::none(),
        )
        .lines()
        .next()
        .expect("a heading")
        .to_owned()
    }

    #[test]
    fn a_whole_bundle_price_is_shown_in_spaced_parentheses() {
        // Spaced inside the marks: tight against a digit a bracket reads as a leading 1.
        let line = heading(Some(Price::Whole(Money::new(1000, "USD"))), None);
        assert_eq!(line, "A Bundle  ( $10.00 )");
    }

    #[test]
    fn a_per_game_rate_is_marked_as_a_rate_and_an_absolute_price_is_not() {
        // The two have to be told apart at a glance: one is what the bundle costs, the other is
        // what one game in it costs.
        let rate = heading(
            Some(Price::PerGame(
                Ladder::new(vec![Tier {
                    games: 25,
                    total: Money::new(2369, "USD"),
                }])
                .expect("a rung"),
            )),
            None,
        );
        assert_eq!(rate, "A Bundle  ( $0.95/game at 25+ )");
        assert_eq!(
            heading(Some(Price::Whole(Money::new(95, "USD"))), None),
            "A Bundle  ( $0.95 )"
        );
    }

    #[test]
    fn a_rate_says_from_how_many_games_it_holds() {
        // The rate alone hides the commitment — 95 cents a game is a different offer at five
        // games than at twenty-five — and it does not multiply back to the total either, being
        // rounded to the cent. The "+" says the rate holds from that count upward.
        let line = heading(
            Some(Price::PerGame(
                Ladder::new(vec![Tier {
                    games: 7,
                    total: Money::new(910, "USD"),
                }])
                .expect("a rung"),
            )),
            None,
        );
        assert_eq!(line, "A Bundle  ( $1.30/game at 7+ )");
    }

    #[test]
    fn the_deadline_sits_in_spaced_brackets_after_the_price() {
        let line = heading(
            Some(Price::Whole(Money::new(799, "USD"))),
            Some("2026-09-17T04:00:00"),
        );
        assert_eq!(
            line,
            "A Bundle  ( $7.99 )  [ ends 2026-09-17 04:00 UTC · 12h left ]"
        );
    }

    #[test]
    fn a_bundle_with_no_published_price_says_nothing_about_one() {
        // Absent is unknown, not free.
        assert_eq!(heading(None, None), "A Bundle");
        assert_eq!(
            heading(None, Some("2026-09-17T04:00:00")),
            "A Bundle  [ ends 2026-09-17 04:00 UTC · 12h left ]"
        );
    }

    #[test]
    fn the_amount_carries_its_currency_wherever_it_appears() {
        // Two stores, two currencies possible in one listing, and equal digits in different
        // currencies are different prices.
        assert!(heading(Some(Price::Whole(Money::new(699, "GBP"))), None).contains("£6.99"));
        assert!(heading(Some(Price::Whole(Money::new(699, "SEK"))), None).contains("6.99 SEK"));
    }
}

mod how_loudly_a_deadline_is_announced {
    use super::*;

    fn now() -> Timestamp {
        Timestamp::parse("2026-09-16T16:00:00").expect("a fixed instant")
    }

    const RED: &str = "\x1b[38;2;245;48;48m";
    const ORANGE: &str = "\x1b[38;2;215;130;42m";

    fn coloured(deadline: &str) -> String {
        let mut bundle = bundle("A Bundle", &["Game"]);
        bundle.ends_at = Some(Timestamp::parse(deadline).expect("parses"));
        render::listing_at(
            &listing(vec![bundle]),
            Palette::Ansi,
            now(),
            &Holdings::none(),
            &Classified::none(),
        )
    }

    #[test]
    fn three_bands_separate_an_emergency_from_a_plan_from_a_note() {
        // Red under 50 hours, orange under five days, grey beyond. The boundaries are checked
        // from both sides so that neither threshold can drift unnoticed.
        let cases = [
            ("2026-09-18T17:00:00", Some(RED), "49h"),
            ("2026-09-18T19:00:00", Some(ORANGE), "51h"),
            ("2026-09-21T15:00:00", Some(ORANGE), "just under five days"),
            ("2026-09-21T17:00:00", None, "just over five days"),
            ("2026-10-16T16:00:00", None, "a month"),
        ];
        for (deadline, expected, what) in cases {
            let line = coloured(deadline);
            assert_eq!(
                line.contains(RED),
                expected == Some(RED),
                "{what}: {line:?}"
            );
            assert_eq!(
                line.contains(ORANGE),
                expected == Some(ORANGE),
                "{what}: {line:?}"
            );
        }
    }

    #[test]
    fn a_deadline_already_past_is_as_loud_as_one_about_to_pass() {
        assert!(coloured("2026-09-12T04:00:00").contains(RED));
    }

    #[test]
    fn the_time_left_is_readable_without_any_colour_at_all() {
        // Colour is never the only carrier: piped output, a light terminal and a colourblind
        // reader all get the same words.
        let mut bundle = bundle("A Bundle", &["Game"]);
        bundle.ends_at = Timestamp::parse("2026-09-17T04:00:00");
        let plain = render::listing_at(
            &listing(vec![bundle]),
            Palette::Plain,
            now(),
            &Holdings::none(),
            &Classified::none(),
        );
        assert!(plain.contains("12h left"), "{plain}");
        assert!(!plain.contains('\x1b'), "{plain:?}");
    }
}

mod a_product_that_is_really_several_games {
    use super::*;

    fn packed(name: &str, inner: &[(&str, u32)]) -> Game {
        Game {
            title: name.to_owned(),
            machine_name: name.to_lowercase().replace(' ', "-"),
            // A pack is a marketing wrapper, not a product: no page, no id.
            steam_app_id: None,
            contains: inner
                .iter()
                .map(|(title, app_id)| Game {
                    title: (*title).to_owned(),
                    machine_name: title.to_lowercase().replace(' ', "-"),
                    steam_app_id: Some(*app_id),
                    contains: Vec::new(),
                })
                .collect(),
        }
    }

    fn rendered(games: Vec<Game>) -> Vec<String> {
        let bundle = Bundle {
            title: "A Bundle".to_owned(),
            url: "https://example.test/b".to_owned(),
            price: None,
            ends_at: None,
            games,
        };
        render::listing(
            &listing(vec![bundle]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        )
        .lines()
        .map(str::to_owned)
        .collect()
    }

    #[test]
    fn the_pack_is_named_once_and_its_games_are_listed_beneath_it() {
        let lines = rendered(vec![packed(
            "Lazy Otter Double Pack",
            &[("Slots & Diapers", 4_409_870), ("Idle Chapel", 4_102_010)],
        )]);
        assert!(
            lines[1].trim_end().ends_with("Lazy Otter Double Pack"),
            "{:?}",
            lines[1]
        );
        assert!(lines[1].starts_with("  -"), "{:?}", lines[1]);
        // Each delivered game is a row of its own, arrowed under the pack — and, now that the
        // snapshot knows both, each carries its detail line beneath it, so the arrows are no
        // longer adjacent. The shape is pinned by finding them, not by counting to them.
        let arrows: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.starts_with("  -->"))
            .map(|(at, _)| at)
            .collect();
        assert_eq!(arrows.len(), 2, "{lines:#?}");
        assert_eq!(
            arrows[0], 2,
            "the first game directly under the pack: {lines:#?}"
        );
        for between in arrows[0] + 1..arrows[1] {
            assert!(
                lines[between].contains('└'),
                "only detail lines sit between the games: {lines:#?}"
            );
        }
    }

    /// Where a row's name begins, counted from the start of the line.
    fn name_at(line: &str, name: &str) -> usize {
        line.find(name)
            .unwrap_or_else(|| panic!("{name:?} is not in {line:?}"))
    }

    #[test]
    fn a_nested_name_is_offset_by_its_arrow_and_nothing_else() {
        // The arrow is two characters wider than the plain bullet, and that difference is the
        // whole of the nesting. Every title begins directly after its own bullet, so a pack is
        // never pushed across by the rows underneath it.
        let lines = rendered(vec![
            game("Top Level"),
            packed("A Pack", &[("Inner Game", 7)]),
        ]);
        assert_eq!(name_at(&lines[1], "Top Level"), 4);
        assert_eq!(name_at(&lines[2], "A Pack"), 4);
        assert_eq!(name_at(&lines[3], "Inner Game"), 6, "{lines:#?}");
    }

    #[test]
    fn the_links_line_up_whether_or_not_a_row_is_nested() {
        let lines = rendered(vec![
            game("Top Level"),
            packed("A Pack", &[("Inner Game", 7)]),
        ]);
        assert_eq!(
            name_at(&lines[1], "https://"),
            name_at(&lines[3], "https://"),
            "{lines:#?}"
        );
    }

    #[test]
    fn a_pack_title_is_not_pushed_across_by_the_bullets_beneath_it() {
        // This has bitten twice. Nesting is carried by the bullet, and the bullet is joined to
        // the name by a SINGLE space so it stays part of that cell — two spaces are how this
        // table separates columns, so a bullet written with two becomes a column of its own,
        // gets padded to the width of the widest bullet, and shifts every title including the
        // pack's.
        let lines = rendered(vec![packed("A Pack", &[("Inner", 7)])]);
        assert_eq!(lines[1], "  - A Pack");
        assert!(lines[2].starts_with("  --> Inner"), "{:?}", lines[2]);
    }

    #[test]
    fn the_pack_itself_gets_no_link_because_no_store_page_answers_to_that_name() {
        // This is the whole point of the change: the name is a marketing wrapper, and the
        // search link it used to get found nothing.
        let lines = rendered(vec![packed(
            "Lazy Otter Double Pack",
            &[("Idle Chapel", 4_102_010)],
        )]);
        assert!(!lines[1].contains("http"), "{:?}", lines[1]);
        assert!(!lines[1].contains("search"), "{:?}", lines[1]);
    }

    #[test]
    fn each_contained_game_links_to_its_own_store_page() {
        let lines = rendered(vec![packed(
            "Lazy Otter Double Pack",
            &[("Slots & Diapers", 4_409_870), ("Idle Chapel", 4_102_010)],
        )]);
        // The URL is the last field of a game's row and appears nowhere else — a detail line
        // carries none — so the links are found rather than counted to.
        let linked: Vec<&String> = lines
            .iter()
            .filter(|line| line.contains("https://"))
            .collect();
        assert_eq!(linked.len(), 2, "{lines:#?}");
        assert!(
            linked[0].ends_with("https://store.steampowered.com/app/4409870"),
            "{:?}",
            linked[0]
        );
        assert!(
            linked[1].ends_with("https://store.steampowered.com/app/4102010"),
            "{:?}",
            linked[1]
        );
    }

    /// The case that motivated the snapshot fallback: a two-game pack whose games the curated
    /// table never met. Fanatical's sub-products carry no Steam id, so each is reached by title,
    /// and each now gets the band and detail line the curated table would have given it.
    #[test]
    fn a_packs_games_get_their_reviews_and_details_from_the_snapshot() {
        let games = [("Ziggurat", 308_420), ("Ziggurat 2", 1_159_560)];
        let lines = rendered(vec![packed("Ziggurat 1 & 2 Complete Edition", &games)]);
        let arrows: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.starts_with("  -->"))
            .map(|(at, _)| at)
            .collect();
        assert_eq!(arrows.len(), 2, "{lines:#?}");
        for (at, (_, app_id)) in arrows.into_iter().zip(games) {
            // Asked of the snapshot rather than written down, so a refreshed snapshot does not
            // fail this for the game having gathered more reviews since.
            let reviews = snapshot::by_id(app_id)
                .and_then(|found| found.details)
                .and_then(|details| details.reviews)
                .expect("the snapshot holds the game and its reviews");
            assert!(
                lines[at].contains(&reviews.to_string()),
                "the review count on the row: {:?}",
                lines[at]
            );
            assert!(
                lines.get(at + 1).is_some_and(|next| next.contains('└')),
                "a detail line beneath it: {lines:#?}"
            );
        }
    }

    #[test]
    fn the_documented_way_to_read_links_still_finds_every_one_of_them() {
        // The README promises the link is the last whitespace-separated field of any line
        // carrying one. A pack line carries none, so it must simply not match.
        let lines = rendered(vec![
            packed("A Pack", &[("Inner One", 1), ("Inner Two", 2)]),
            game("Ordinary Game"),
        ]);
        let links: Vec<&str> = lines
            .iter()
            .filter(|line| line.contains("http"))
            .filter_map(|line| line.split_whitespace().next_back())
            .collect();
        assert_eq!(links.len(), 3, "{lines:#?}");
        assert!(
            links.iter().all(|link| link.starts_with("https://")),
            "{links:?}"
        );
    }

    #[test]
    fn an_ordinary_game_keeps_the_plain_bullet_it_always_had() {
        // A bundle with no packs has nothing to line up against, so its rows read as they did
        // before nesting existed.
        let lines = rendered(vec![game("Ordinary Game")]);
        assert!(lines[1].starts_with("  - Ordinary Game"), "{:?}", lines[1]);
        assert!(!lines[1].contains("-->"), "{:?}", lines[1]);
    }

    #[test]
    fn packs_and_plain_games_can_sit_in_one_bundle() {
        let lines = rendered(vec![
            game("First"),
            packed("A Pack", &[("Inner", 7)]),
            game("Last"),
        ]);
        let bullets: Vec<&str> = lines[1..5]
            .iter()
            .map(|line| line.split_whitespace().next().expect("a bullet"))
            .collect();
        assert_eq!(bullets, ["-", "-", "-->", "-"], "{lines:#?}");
    }

    #[test]
    fn a_pack_inside_a_pack_is_opened_one_level_and_no_further() {
        // The type permits any depth and the resolver fills exactly one, so the renderer is
        // pinned to the same one level. Anything deeper would need another round trip that is
        // never made, and would render contents nothing had looked up.
        let mut outer = packed("Outer", &[("Middle", 1)]);
        outer.contains[0].contains = vec![Game {
            title: "Deepest".to_owned(),
            machine_name: "deepest".to_owned(),
            steam_app_id: Some(2),
            contains: Vec::new(),
        }];
        let lines = rendered(vec![outer]);
        assert!(
            !lines.iter().any(|line| line.contains("Deepest")),
            "{lines:#?}"
        );
    }
}

/// [`render::choices`] — the same contents, offered as boxes to tick rather than printed.
mod the_boxes_a_chooser_is_offered {
    use super::*;

    fn packed(name: &str, inner: &[&str]) -> Game {
        Game {
            contains: inner.iter().copied().map(game).collect(),
            ..game(name)
        }
    }

    fn offered(games: Vec<Game>) -> Vec<render::Offer> {
        let bundle = Bundle {
            games,
            ..bundle("A Bundle", &[])
        };
        render::choices(
            &bundle,
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        )
    }

    #[test]
    fn a_box_for_every_thing_on_sale_and_one_more_for_each_game_a_pack_delivers() {
        // A pack is one product, so it gets a box; each game it delivers has a page of its own,
        // and picking pages is what a chooser is for, so each gets a box too.
        let offers = offered(vec![
            game("First"),
            packed("A Pack", &["Inner One", "Inner Two"]),
            game("Last"),
        ]);
        let nested: Vec<bool> = offers.iter().map(|offer| offer.included).collect();
        assert_eq!(nested, [false, false, true, true, false]);
    }

    #[test]
    fn the_lines_are_in_the_order_the_store_listed_them_packs_opened_in_place() {
        let offers = offered(vec![
            game("First"),
            packed("A Pack", &["Inner One", "Inner Two"]),
            game("Last"),
        ]);
        let named: Vec<&str> = offers
            .iter()
            .map(|offer| {
                offer
                    .line
                    .split("  ")
                    .next()
                    .expect("a name before the padding")
            })
            .collect();
        assert_eq!(
            named,
            ["First", "A Pack", "Inner One", "Inner Two", "Last"],
            "curation order, with a pack's games directly under it"
        );
    }

    #[test]
    fn no_line_carries_a_bullet_because_the_form_draws_a_box_of_its_own() {
        // The listing marks depth with a bullet whose width IS the indent. A form draws `[ ] `
        // and indents a nested box itself, so a bullet here would be a second marker for one
        // fact — and the listing's own test pins the bullets, so neither can drift unnoticed.
        let offers = offered(vec![packed("A Pack", &["Inner"]), game("Loose")]);
        for offer in &offers {
            assert!(
                !offer.line.starts_with('-'),
                "{:?} still carries a bullet",
                offer.line
            );
        }
    }

    #[test]
    fn a_pack_names_itself_and_nothing_else() {
        // No reviews and no link: a pack has neither, and the boxes under it name its games,
        // so naming them here as well would say the same thing twice.
        let offers = offered(vec![packed("A Pack", &["Inner One", "Inner Two"])]);
        assert_eq!(offers[0].line.trim_end(), "A Pack");
        assert!(offers[1].line.contains("https://"), "{:?}", offers[1].line);
    }

    #[test]
    fn a_pack_inside_a_pack_is_offered_one_level_deep_exactly_as_it_is_printed() {
        // The listing is pinned to one level because the resolver fills exactly one. A chooser
        // reading deeper would offer a box for something nothing had looked up — and the two
        // must agree about what a bundle holds, or a listing and its form would disagree.
        let mut outer = packed("Outer", &["Middle"]);
        outer.contains[0].contains = vec![game("Deepest")];
        let offers = offered(vec![outer]);
        assert_eq!(offers.len(), 2);
        assert!(
            !offers.iter().any(|offer| offer.line.contains("Deepest")),
            "{:?}",
            offers.iter().map(|o| &o.line).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_lines_are_padded_into_columns_within_the_bundle() {
        // Aligned here rather than by the caller: the caller has one string per box and cannot
        // see the columns inside it, and this is the same `align` the listing uses.
        let offers = offered(vec![game("A"), game("A Much Longer Title")]);
        let links: Vec<Option<usize>> = offers
            .iter()
            .map(|offer| offer.line.find("https://"))
            .collect();
        assert_eq!(
            links[0], links[1],
            "both links should start in the same column: {links:?}"
        );
    }
}

/// What the listing SHOWS and what the opener WRITES have to be the same page.
mod the_link_shown_and_the_link_opened {
    use super::*;

    /// A store that publishes no Steam app-id, which is every Humble game.
    fn unidentified(title: &str) -> Game {
        Game {
            title: title.to_owned(),
            machine_name: title.to_lowercase().replace(' ', "-"),
            steam_app_id: None,
            contains: Vec::new(),
        }
    }

    #[test]
    fn a_game_with_no_published_app_id_still_opens_its_own_page() {
        // The regression this exists for: the listing asked the inventory for the app-id and the
        // opener did not, so the form showed `…/app/1681600` and the script it wrote opened a
        // search for the title. Every Humble game was affected, because Humble publishes no ids.
        let Some(entry) = steam_inventory::all().next() else {
            return;
        };
        let game = unidentified(entry.name);

        let shown = render::Preview::of_game(&game, &Holdings::none(), &Classified::none()).url;
        let opened = render::page_of(&game, &Classified::none());
        let wanted = format!("https://store.steampowered.com/app/{}", entry.app_id);
        assert_eq!(shown, wanted, "the listing must show the game's own page");
        assert_eq!(opened, wanted, "and the opener must write the same one");
    }

    #[test]
    fn every_link_in_the_listing_is_the_link_the_opener_writes() {
        // Held together across a whole listing rather than one game, and over both shapes at
        // once: one the inventory knows under a name, one it knows by nothing at all.
        let Some(entry) = steam_inventory::all().next() else {
            return;
        };
        let listing = listing(vec![bundle_of(vec![
            unidentified(entry.name),
            unidentified(UNKNOWN),
        ])]);

        let printed: Vec<String> = render::listing(
            &listing,
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        )
        .split_whitespace()
        .filter(|field| field.starts_with("https://store.steampowered.com"))
        .map(str::to_owned)
        .collect();
        let opened: Vec<String> = catalogames::links::of_listing(&listing, &Classified::none())
            .into_iter()
            .map(|link| link.url)
            .collect();

        assert_eq!(
            printed, opened,
            "what is shown and what is opened must agree"
        );
        assert!(printed[0].contains("/app/"), "{printed:?}");
        assert!(
            printed[1].contains("/search/"),
            "a game nothing knows: {printed:?}"
        );
    }
}

mod a_stores_own_boilerplate {
    use super::*;

    fn heading(title: &str, palette: Palette) -> String {
        let bundle = bundle(title, &["Game"]);
        render::listing(
            &listing(vec![bundle]),
            palette,
            &Holdings::none(),
            &Classified::none(),
        )
        .lines()
        .next()
        .expect("a heading")
        .to_owned()
    }

    const GREEN: &str = "\x1b[38;2;76;166;76m";

    #[test]
    fn the_wording_every_bundle_of_a_kind_carries_is_coloured() {
        // It says what KIND of bundle this is — one you pick games out of — which is worth
        // seeing at a glance, and the part that names the bundle is what it leaves alone.
        let line = heading("Build your own Titanium Collection", Palette::Ansi);
        assert!(line.starts_with(GREEN), "{line:?}");
        assert!(line.contains("Titanium Collection"), "{line:?}");
        assert!(
            !line.contains(&format!("{GREEN}Titanium")),
            "only the shared wording is coloured: {line:?}"
        );
    }

    #[test]
    fn the_catalogue_writes_it_both_ways_and_both_are_caught() {
        // "Build your own Bundle" and "Build your Own Bundle" appear in one listing.
        for title in ["Build your own Bundle", "Build your Own Bundle"] {
            assert!(heading(title, Palette::Ansi).contains(GREEN), "{title}");
        }
    }

    #[test]
    fn a_title_without_the_wording_is_left_exactly_as_the_store_wrote_it() {
        assert_eq!(
            heading("Beyond the Metroidverse Bundle", Palette::Ansi),
            "Beyond the Metroidverse Bundle"
        );
    }

    #[test]
    fn nothing_is_coloured_when_colour_was_not_asked_for() {
        assert_eq!(
            heading("Build your own Titanium Collection", Palette::Plain),
            "Build your own Titanium Collection"
        );
    }
}

mod a_game_already_owned_somewhere_else {
    use super::*;
    use catalogames::inventory::epic as epic_inventory;
    use catalogames::user_games::crossover;

    /// A game that is in both committed inventories, or nothing to test with.
    fn crossing() -> Option<(&'static str, &'static str)> {
        crossover::all()
            .next()
            .map(|(steam, epic)| (steam.name, epic.title))
    }

    fn line_for(title: &str, app_id: Option<u32>) -> String {
        let mut game = game(title);
        game.steam_app_id = app_id;
        let mut bundle = bundle("A Bundle", &[]);
        bundle.games = vec![game];
        render::listing(
            &listing(vec![bundle]),
            Palette::Plain,
            &Holdings::none(),
            &Classified::none(),
        )
        .lines()
        .nth(1)
        .expect("a game line")
        .to_owned()
    }

    #[test]
    fn the_listing_says_so_beside_the_name() {
        // A bundle offering something already sitting free in an Epic account is not an offer,
        // and somebody scanning to decide what to buy should not have to read a second line.
        let Some((steam_name, _)) = crossing() else {
            return;
        };
        let line = line_for(steam_name, None);
        assert!(line.contains(crossover::NOTE), "{line:?}");
        assert!(line.contains(steam_name), "{line:?}");
    }

    #[test]
    fn a_game_nobody_owns_says_nothing_at_all() {
        let line = line_for("A Game That Is In No Epic Library", None);
        assert!(!line.contains(crossover::NOTE), "{line:?}");
    }

    #[test]
    fn the_note_never_displaces_the_link_from_the_end_of_the_line() {
        // The documented way to read links is the last whitespace-separated field, so the note
        // has to sit before the URL however it is rendered.
        let Some((steam_name, _)) = crossing() else {
            return;
        };
        let line = line_for(steam_name, None);
        let last = line.split_whitespace().next_back().expect("a field");
        assert!(last.starts_with("https://"), "{line:?}");
    }

    #[test]
    fn a_bundles_own_wording_is_matched_when_it_publishes_no_app_id() {
        // Epic's punctuation differs from a bundle's, which is the whole reason the match
        // normalises both sides.
        let Some((_, epic_title)) = crossing() else {
            return;
        };
        assert!(epic_inventory::by_title(epic_title).is_some());
        assert!(line_for(epic_title, None).contains(crossover::NOTE));
    }
}
