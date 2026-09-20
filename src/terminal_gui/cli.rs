//! Command-line surface.
//!
//! Lives in the binary, not the library: embedding projects call the library
//! functions directly and should not inherit clap.

use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "catalogames",
    version,
    about = "Track game bundle deals and sort out your games library."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Print what the program is doing. Repeat for more detail: `-vv`.
    ///
    /// Narration goes to stderr, so stdout stays a clean listing.
    #[arg(short, long, action = ArgAction::Count, global = true)]
    pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List the bundles a store is currently selling, with their games.
    #[command(about = sales_about())]
    Sales {
        /// Stores to list. Every store when none are named.
        #[arg(value_name = "STORE")]
        stores: Vec<Vendor>,

        /// Print the listing instead of offering it as a form to choose from.
        ///
        /// On a terminal the listing is a form by default: each bundle a section that folds,
        /// each thing it sells a checkbox, and what you tick becomes a shell script that opens
        /// those pages. Output that is not a terminal is plain already, so this is for asking
        /// for plain output on one.
        #[arg(long)]
        plain: bool,

        /// Where to write the script that opens the pages you tick.
        ///
        /// Defaults to `catalogames-open.sh` in the current directory. A file that this
        /// program did not write is never replaced.
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,

        /// Fetch again instead of using a saved listing.
        ///
        /// A saved listing is used when it is under a day old, covers the stores asked for, and
        /// holds no bundle that has since ended. This overrides all of that.
        #[arg(short = 'f', long)]
        force: bool,

        /// Where your own library and wishlist files are.
        ///
        /// A game on sale that one of those files already names is marked: `[owned on Steam]`,
        /// or `[wishlisted on Steam]`, naming every store that says so. A store the file matched
        /// by title rather than by app-id is marked with a `?`, because only Steam publishes the
        /// app-ids every one of these sale sites keys on.
        ///
        /// Defaults to where `gamelib` writes them, so a plain `gamelib steam` and a plain
        /// `sales` work together with nothing passed to either. The listing says which files it
        /// found and where it looked, so a wrong directory cannot pass for owning nothing.
        #[arg(long, value_name = "DIR")]
        account_files: Option<PathBuf>,
    },

    /// Snapshot the games you own on a store, into files you keep.
    #[command(about = choices::<Store>("Snapshot the games you own on a store, into files you keep"))]
    Gamelib {
        /// Store to snapshot.
        store: Store,

        /// Directory to write the library and wishlist files into.
        ///
        /// Defaults to `$XDG_DATA_HOME/catalogames`, else `~/.local/share/catalogames`. Not
        /// `~/.config`, which holds settings you edit; a snapshot is data this program made.
        #[arg(short = 'd', long, value_name = "PATH")]
        dir: Option<PathBuf>,
    },

    /// Build the project's inventory entry for one game, ready to paste.
    ///
    /// Takes a store link or a bare id, e.g.
    /// `https://store.steampowered.com/app/1202130` or `1202130`. A link says which store it
    /// came from; a bare id does not, so that case asks.
    ///
    /// Named with underscores because that is what it was asked to be called; the hyphenated
    /// spelling clap would otherwise use is accepted as an alias.
    #[command(name = "add_project_entry", alias = "add-project-entry")]
    AddProjectEntry {
        /// A store link, or a bare game id.
        game: String,
    },
}

/// The `sales` summary, naming every store it accepts.
fn sales_about() -> String {
    choices::<Vendor>(
        "List the bundles stores are currently selling, with their games. \
         Names one or more stores; lists every one when given none",
    )
}

/// A one-line summary with the accepted values appended.
///
/// Built from the enum rather than written out, so a store added to one cannot be missing from
/// its help text — the list and the parser are the same thing.
fn choices<T: ValueEnum>(summary: &str) -> String {
    let names: Vec<String> = T::value_variants()
        .iter()
        .filter_map(clap::ValueEnum::to_possible_value)
        .map(|value| value.get_name().to_owned())
        .collect();
    format!("{summary} [{}]", names.join(", "))
}

/// Stores that can be listed.
///
/// A plain enum rather than a trait: with two stores there is nothing worth abstracting over
/// yet, and clap turns this into both the argument parser and the help text. Adding a store
/// means adding a variant and its match arm — the compiler names every place that needs
/// touching, and [`sales_about`] picks the name up on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Vendor {
    /// humblebundle.com
    Humblebundle,
    /// fanatical.com
    Fanatical,
}

impl Vendor {
    /// Every store, in the order the help text lists them.
    ///
    /// What `sales` uses when it is given no stores. Derived from the enum, so a store added
    /// later is listed by default without a second place to remember.
    #[must_use]
    pub fn all() -> &'static [Self] {
        Self::value_variants()
    }

    /// The store's own name, as it writes it.
    ///
    /// Distinct from [`Vendor::site`], which is the domain: one heads a section of a form for a
    /// person to read, the other cites where the data came from.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Humblebundle => "HumbleBundle",
            Self::Fanatical => "Fanatical",
        }
    }

    /// The site the bundles come from.
    ///
    /// Printed as a section header when a run covers more than one store, which is both what
    /// tells a reader whose bundle they are looking at and how Fanatical's request to be cited
    /// when its data is displayed is met.
    #[must_use]
    pub fn site(self) -> &'static str {
        match self {
            Self::Humblebundle => "humblebundle.com",
            Self::Fanatical => "fanatical.com",
        }
    }
}

/// Stores whose library can be snapshotted.
///
/// Deliberately not [`Vendor`]: that one lists stores that *sell bundles*, and these are stores
/// that *hold an account's games*. The two sets overlap in neither direction today, and merging
/// them would offer every combination on the command line while only some of them exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Store {
    /// store.steampowered.com — owned games and wishlist
    Steam,
    /// store.epicgames.com — owned games, read through legendary
    Epic,
}

impl Store {
    /// Every store, in the order the help text lists them.
    #[must_use]
    pub fn all() -> &'static [Self] {
        Self::value_variants()
    }

    /// How the store is named to a person.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Steam => "Steam",
            Self::Epic => "Epic Games Store",
        }
    }

    /// Hosts whose links belong to this store.
    ///
    /// Matched against a link's host rather than looked for anywhere in the string: a path can
    /// contain anything, and `https://elsewhere.example/store.steampowered.com` is not Steam.
    #[must_use]
    pub fn hosts(self) -> &'static [&'static str] {
        match self {
            Self::Steam => &["store.steampowered.com", "steamcommunity.com", "s.team"],
            Self::Epic => &["store.epicgames.com", "epicgames.com"],
        }
    }

    /// Why this store cannot take a new inventory entry yet, if it cannot.
    ///
    /// One fact, read in two places: it dims the option in the store menu, and it is the
    /// explanation given when a link for that store is passed directly.
    #[must_use]
    pub fn entry_blocker(self) -> Option<&'static str> {
        match self {
            Self::Steam => None,
            Self::Epic => Some(
                "the Epic inventory is still a placeholder — it has no entry format to write, \
                 because what Epic publishes about a game is not yet known",
            ),
        }
    }

    /// Which store a link belongs to, or `None` when the text names no host this program knows.
    ///
    /// `None` covers two different cases on purpose, and the caller tells them apart: a bare id
    /// has no host at all and is worth asking about, while a link to a store nobody here
    /// supports is worth refusing.
    #[must_use]
    pub fn of_link(text: &str) -> Option<Self> {
        let host = host_of(text)?;
        Self::all().iter().copied().find(|store| {
            store
                .hosts()
                .iter()
                .any(|known| host == *known || host.ends_with(&format!(".{known}")))
        })
    }
}

/// The host part of a URL, lower-cased. `None` when the text is not shaped like one.
///
/// Hand-written rather than parsed with a crate: this reads one argument a person typed, and
/// the only question asked of it is which of a handful of known hosts it names.
fn host_of(text: &str) -> Option<String> {
    let after_scheme = text.split_once("://").map_or(text, |(_, rest)| rest);
    let authority = after_scheme.split(['/', '?', '#']).next()?;
    // Userinfo, then the port; whatever is left is the host.
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.rsplit_once(':').map_or(host, |(h, port)| {
        if port.chars().all(|c| c.is_ascii_digit()) {
            h
        } else {
            host
        }
    });
    // A host has a dot and no spaces; a bare app-id has neither, which is how the two are told
    // apart without guessing at what an id looks like.
    (host.contains('.') && !host.contains(' ')).then(|| host.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_link_is_recognised_by_its_host() {
        for (link, store) in [
            ("https://store.steampowered.com/app/1202130", Store::Steam),
            (
                "https://store.steampowered.com/agecheck/app/283640/",
                Store::Steam,
            ),
            ("http://s.team/a/1202130", Store::Steam),
            ("https://steamcommunity.com/app/1202130", Store::Steam),
            ("https://store.epicgames.com/en-US/p/control", Store::Epic),
            ("https://www.epicgames.com/store/p/control", Store::Epic),
        ] {
            assert_eq!(Store::of_link(link), Some(store), "{link}");
        }
    }

    #[test]
    fn a_host_is_never_read_out_of_a_path() {
        // The whole reason the host is extracted rather than searched for.
        assert_eq!(
            Store::of_link("https://elsewhere.example/store.steampowered.com/app/1"),
            None
        );
        assert_eq!(Store::of_link("https://notsteampowered.com/app/1"), None);
    }

    #[test]
    fn a_bare_id_names_no_store_at_all() {
        // Distinct from an unknown link: this is the case worth asking a question about.
        assert_eq!(host_of("1202130"), None);
        assert_eq!(Store::of_link("1202130"), None);
    }

    #[test]
    fn a_link_to_a_store_nobody_here_supports_is_not_mistaken_for_one_that_is() {
        assert_eq!(
            host_of("https://gog.com/game/control"),
            Some("gog.com".to_owned())
        );
        assert_eq!(Store::of_link("https://gog.com/game/control"), None);
    }

    #[test]
    fn a_port_or_a_user_in_the_authority_does_not_hide_the_host() {
        assert_eq!(
            host_of("https://user@store.steampowered.com:443/app/1"),
            Some("store.steampowered.com".to_owned())
        );
    }

    #[test]
    fn every_store_the_help_lists_is_one_the_menu_can_offer() {
        // Both lists come from the same enum, so this pins that they stay the same length
        // rather than that either is correct on its own.
        assert_eq!(Store::all().len(), Store::value_variants().len());
        assert!(Store::all().iter().all(|store| !store.label().is_empty()));
        assert!(Store::all().iter().all(|store| !store.hosts().is_empty()));
    }

    #[test]
    fn a_store_with_no_entry_format_says_why_rather_than_being_hidden() {
        assert_eq!(Store::Steam.entry_blocker(), None);
        let why = Store::Epic
            .entry_blocker()
            .expect("epic has no entry format yet");
        assert!(why.contains("placeholder"), "{why}");
    }
}
