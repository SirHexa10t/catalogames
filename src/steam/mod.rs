//! Talking to Steam: links to it, and reading a game's store page.
//!
//! Not under any store's module because it is not any store's concern: a game is the same game
//! whichever bundle sells it. The links live here; fetching what Steam says about an app lives
//! in [`store`], and asking what its item service says an id IS lives in [`items`].

pub mod items;
pub mod store;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

/// Characters left unescaped in a query value: RFC 3986's *unreserved* set.
const UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Where the store's search lands for a title.
pub const SEARCH_BASE: &str = "https://store.steampowered.com/search/?term=";

/// Where a game's own store page lives.
/// The store itself, for a path Steam hands back whole — see [`items::Found::url`].
pub const STORE_BASE: &str = "https://store.steampowered.com/";

pub const APP_BASE: &str = "https://store.steampowered.com/app/";

/// The store page for an app-id.
///
/// Preferred over [`search_url`] wherever an app-id is known: it lands on the game rather than
/// on a list of guesses at it.
///
/// ```
/// assert_eq!(
///     catalogames::steam::app_url(283640),
///     "https://store.steampowered.com/app/283640",
/// );
/// ```
pub fn app_url(app_id: u32) -> String {
    format!("{APP_BASE}{app_id}")
}

/// A date as the store page prints it — `19 Apr, 2011` — from the `YYYY-MM-DD` the snapshot keeps.
///
/// One voice for a listing: the curated table records dates as the store page printed them, and
/// a snapshot row drawn beside a curated one should not switch to ISO. `None` for anything that is
/// not a whole date, so a partial one is shown as it came rather than completed.
#[must_use]
pub fn printed_date(iso: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut parts = iso.splitn(3, '-');
    let year: u16 = parts.next()?.parse().ok()?;
    let month: usize = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    let name = MONTHS.get(month.checked_sub(1)?)?;
    (1..=31)
        .contains(&day)
        .then(|| format!("{day} {name}, {year}"))
}

/// Builds a Steam search link for a game title.
///
/// For a game whose app-id is not known — which is every game Humble sells, since its bundle
/// data carries no Steam id. Where an id *is* known, [`app_url`] is the link to use; a search
/// can only guess, and roughly a fifth of bundle entries are not Steam games at all (discount
/// coupons, video courses, season passes), for which a search that finds nothing is the
/// truthful answer.
///
/// ```
/// assert_eq!(
///     catalogames::steam::search_url("Salt and Sanctuary"),
///     "https://store.steampowered.com/search/?term=Salt%20and%20Sanctuary",
/// );
/// ```
pub fn search_url(title: &str) -> String {
    format!("{SEARCH_BASE}{}", utf8_percent_encode(title, UNRESERVED))
}

/// Reads an app-id out of a Steam store URL, or out of a bare number.
///
/// Accepts what a person pastes: `https://store.steampowered.com/app/1202130`, the same with the
/// title slug and trailing slash Steam appends, `steamcommunity.com/app/1202130`, or just
/// `1202130`. Anything else is `None` rather than a guess — a wrong app-id looks exactly like a
/// right one until someone opens the page.
///
/// ```
/// use catalogames::steam::app_id_from_url;
/// assert_eq!(app_id_from_url("https://store.steampowered.com/app/1202130/"), Some(1202130));
/// assert_eq!(app_id_from_url("1202130"), Some(1202130));
/// assert_eq!(app_id_from_url("https://s.team/a/1202130"), Some(1202130));
/// assert_eq!(app_id_from_url("https://store.steampowered.com/search/?term=x"), None);
/// ```
pub fn app_id_from_url(text: &str) -> Option<u32> {
    let text = text.trim();
    if let Ok(id) = text.parse::<u32>() {
        return Some(id);
    }
    // `/app/N/…` covers the store, the community hub and the age-gate redirect
    // (`/agecheck/app/N/`); `s.team/a/N` is Steam's own short link.
    let (_, after) = text
        .split_once("/app/")
        .or_else(|| text.split_once("s.team/a/"))?;
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    // `/app/` followed by anything but digits is some other page, not an app.
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod printed_date_tests {
    use super::printed_date;

    /// The snapshot's ISO date, in the wording the curated table already uses.
    #[test]
    fn an_iso_date_is_printed_the_way_the_store_page_prints_it() {
        assert_eq!(printed_date("2011-04-19").as_deref(), Some("19 Apr, 2011"));
        assert_eq!(printed_date("1998-11-19").as_deref(), Some("19 Nov, 1998"));
        assert_eq!(printed_date("2026-01-01").as_deref(), Some("1 Jan, 2026"));
    }

    /// A partial or malformed date is refused rather than completed — a snapshot row never holds
    /// one, so meeting one means the file changed, and dressing it up would hide that.
    #[test]
    fn anything_short_of_a_whole_date_is_left_alone() {
        for partial in [
            "2011-04",
            "2011",
            "",
            "2011-13-01",
            "2011-04-32",
            "coming soon",
            "?",
        ] {
            assert_eq!(printed_date(partial), None, "{partial:?}");
        }
    }
}
