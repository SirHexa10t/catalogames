//! Building Steam links from a game title.

use catalogames::steam;

/// The query value of a search URL, still percent-encoded.
fn term(title: &str) -> String {
    steam::search_url(title)
        .strip_prefix(steam::SEARCH_BASE)
        .expect("search_url starts with SEARCH_BASE")
        .to_owned()
}

mod the_url {
    use super::*;

    #[test]
    fn points_at_steams_search() {
        assert_eq!(
            steam::search_url("Worldless"),
            "https://store.steampowered.com/search/?term=Worldless"
        );
    }

    #[test]
    fn escapes_spaces() {
        assert_eq!(term("Salt and Sanctuary"), "Salt%20and%20Sanctuary");
    }

    #[test]
    fn escapes_characters_that_would_break_the_query() {
        // Real titles carry every one of these.
        assert_eq!(term("Make & Play"), "Make%20%26%20Play");
        assert_eq!(term("CRPG Pack: Isometric"), "CRPG%20Pack%3A%20Isometric");
        assert_eq!(term("Warhammer 40,000"), "Warhammer%2040%2C000");
        assert_eq!(term("Yes Chef!"), "Yes%20Chef%21");
        assert_eq!(term("Risky's Revenge"), "Risky%27s%20Revenge");
        assert_eq!(term("Mega Man Zero/ZX"), "Mega%20Man%20Zero%2FZX");
    }

    #[test]
    fn escapes_non_ascii_as_utf8_bytes() {
        // Humble's titles contain typographic dashes and quotes, which are
        // multi-byte in UTF-8 and must be escaped byte by byte.
        assert_eq!(
            term("Ready Player One — and Two"),
            "Ready%20Player%20One%20%E2%80%94%20and%20Two"
        );
        assert_eq!(term("Pokémon"), "Pok%C3%A9mon");
    }

    #[test]
    fn leaves_unreserved_characters_alone() {
        // RFC 3986 unreserved set: escaping these would be legal but noisy.
        assert_eq!(term("A-Z_a.z~0"), "A-Z_a.z~0");
    }

    #[test]
    fn handles_an_empty_title_without_panicking() {
        assert_eq!(steam::search_url(""), steam::SEARCH_BASE);
    }

    #[test]
    fn is_built_the_same_way_every_time() {
        assert_eq!(steam::search_url("HAAK"), steam::search_url("HAAK"));
    }
}
