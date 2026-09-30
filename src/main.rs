//! Thin front-end over the `catalogames` library.

mod terminal_gui;

use std::io::{IsTerminal, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catalogames::clock::Timestamp;
use catalogames::commands::gamelib;
use catalogames::commands::gamelib::config;
use catalogames::commands::sales::cache;
use catalogames::commands::sales::{fanatical, humble};
use catalogames::inventory::steam as steam_inventory;
use catalogames::inventory::steam::{Tag, source};
use catalogames::render::{self, Palette, Preview};
use catalogames::steam;
use catalogames::steam::items::Classified;
use catalogames::user_games::holdings::Holdings;
use catalogames::{Error, Listing, links};
use clap::Parser;
use log::debug;
use terminal_choice::{Chosen, Menu, run_menu};
use terminal_gui::cli::{Cli, Command, Store, Vendor};

/// Listing came back, but something in it needs a human's attention.
const EXIT_DEGRADED: u8 = 2;

fn main() -> ExitCode {
    let Cli { command, verbose } = Cli::parse();
    terminal_gui::logger::install(verbose);

    let outcome = match command {
        Command::Sales {
            stores,
            plain,
            out,
            force,
            account_files,
        } => sales(
            &stores,
            plain,
            out.as_deref(),
            force,
            account_files.as_deref(),
        ),
        Command::Gamelib { store, dir } => snapshot(store, dir.as_deref()),
        Command::AddProjectEntry { game } => add_project_entry(&game),
    };
    outcome.unwrap_or_else(|error| {
        eprintln!("catalogames: {}", catalogames::error::chain(&error));
        ExitCode::FAILURE
    })
}

/// Lists the bundles each named store is selling, or every store when none are named.
///
/// Prefers a saved listing, because fetching every bundle from every store takes minutes and a
/// bundle lasts weeks. What makes a saved one unusable is in [`cache::Capture::stale`]; `force`
/// is the fourth reason, and the only one a person supplies.
///
/// What happens next depends on where the output is going. A terminal gets the picker — the
/// listing as a form, one section per store, a box per thing on sale — because choosing is what
/// the listing is for. A pipe gets the plain listing, since a form cannot be piped and
/// `sales | awk` has to keep working. `--plain` asks for the plain listing either way.
fn sales(
    stores: &[Vendor],
    plain: bool,
    out: Option<&Path>,
    force: bool,
    account_files: Option<&Path>,
) -> catalogames::Result<ExitCode> {
    let wanted: Vec<Vendor> = if stores.is_empty() {
        Vendor::all().to_vec()
    } else {
        stores.to_vec()
    };

    // Read before anything is fetched, so a directory that cannot be read is said so once and at
    // the top rather than after a three-minute wait.
    //
    // Default: where `gamelib` writes. NOT `~/.config` — `gamelib::config::directory` says at
    // length why a snapshot is not configuration — so that taking a snapshot and then listing
    // sales needs no argument on either command.
    let held = match account_files {
        Some(directory) => Holdings::load(directory),
        None => Holdings::load(&config::data_directory()?),
    };

    let Sourced {
        listings,
        notes,
        problems,
    } = match saved(&wanted, force) {
        Some(reloaded) => reloaded,
        None => {
            let read = fetch(&wanted, plain);
            if read.listings.is_empty() {
                return Ok(ExitCode::FAILURE);
            }
            keep(&read.listings);
            read
        }
    };

    // Above the listing, with the other things a reader should know before choosing: where the
    // files were looked for, which were found, and when each was taken. A claim from a snapshot
    // is about the world as it was on that date.
    let mut notes = notes;
    notes.insert(0, held.summary());
    notes.extend(held.problems.iter().cloned());

    // One request to Steam for the ids no table could settle, before anything is drawn, so a
    // bundle id a store published as an app id links to its own page rather than bouncing. Said
    // above the listing like the holdings summary when it fails or leaves a question open: the
    // links are then what the stores published, and a reader should know that is all they are.
    let (classified, asked) = classify(&listings);
    notes.extend(asked);

    if plain || !interactive() {
        for note in &notes {
            eprintln!("catalogames: {note}");
        }
        print(&listings, &held, &classified);
    } else {
        match terminal_gui::picker::pick(listings, palette(), &notes, &held, &classified)? {
            None => eprintln!("catalogames: cancelled — nothing was picked"),
            // An empty pick prints nothing, which is byte-identical to no store having a
            // bundle. Two different outcomes need two messages.
            Some(picked) if picked.is_empty() => eprintln!("catalogames: nothing was ticked"),
            Some(picked) => {
                print(&picked.listings, &held, &classified);
                let path = out.unwrap_or_else(|| Path::new(links::FILE_NAME));
                write_opener(path, &picked.links)?;
                eprintln!(
                    "\ncatalogames: wrote {} — {} page(s), in the order you picked them.\n\
                     Open them with: sh {}",
                    path.display(),
                    picked.links.len(),
                    path.display()
                );
            }
        }
    }

    // Last of everything, so that whatever was asked for comes first and the reasons something
    // is missing come after it. A reader choosing from the form is told the count up front, in
    // the form's own note, so the detail here is the part they come back to.
    report(&problems);

    // Nothing at all is a plain failure; some of it is the degraded result, which is the
    // ordinary outcome when one store is unreachable and the others are not.
    Ok(if problems.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_DEGRADED)
    })
}

/// What went wrong, store by store, after everything else has been written.
fn report(problems: &[(String, Vec<String>)]) {
    let total: usize = problems.iter().map(|(_, found)| found.len()).sum();
    if total == 0 {
        return;
    }
    eprintln!("\ncatalogames: {total} problem(s):");
    for (store, found) in problems {
        for problem in found {
            eprintln!("  {store}: {problem}");
        }
    }
}

/// A line saying how many problems there were, for a reader who is about to choose.
///
/// The detail comes after the results, which means a form would otherwise open over a partial
/// listing with nothing to say so. This is the short version, shown before the choosing.
fn problem_note(problems: &[(String, Vec<String>)]) -> Option<String> {
    let total: usize = problems.iter().map(|(_, found)| found.len()).sum();
    (total > 0).then(|| format!("{total} problem(s) while reading; listed after the results"))
}

/// A saved listing for exactly these stores, if there is a usable one.
///
/// Says on stderr why a saved one was passed over, because "this took three minutes again" is a
/// question a reader will otherwise have to guess at.
/// A set of listings ready to show, and what a reader should know about it.
struct Sourced {
    listings: Vec<(Vendor, Listing)>,
    /// Shown above the form, one line each: where the listing came from, and whether it is
    /// complete.
    notes: Vec<String>,
    /// What went wrong, per store, already rendered. Printed after everything else.
    problems: Vec<(String, Vec<String>)>,
}

fn saved(wanted: &[Vendor], force: bool) -> Option<Sourced> {
    if force {
        return None;
    }
    let capture = cache::newest(&cache::directory().ok()?)?;
    let sites: Vec<&str> = wanted.iter().map(|vendor| vendor.site()).collect();
    if let Some(why) = capture.stale(&sites, Timestamp::now()) {
        eprintln!("catalogames: fetching again — the saved listing is {why}");
        return None;
    }

    // What went wrong when this was read is carried forward, because it is still true of the
    // data being shown. Without it a partial capture would reload looking complete.
    let problems: Vec<(String, Vec<String>)> = capture
        .stores
        .iter()
        .map(|store| (store.site.clone(), store.problems.clone()))
        .collect();
    let notes = [
        // Kept under eighty columns: the form clips every line to the terminal's width, and
        // this is the line that says where the data came from.
        Some(format!(
            "saved listing from {}; rerun with -f to refetch the data",
            capture.captured
        )),
        problem_note(&problems),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Paired back to stores by site, in the order asked for. `stale` has already checked that
    // the two cover the same stores, so this pairs like with like.
    let mut stores = capture.stores;
    stores.sort_by_key(|store| {
        wanted
            .iter()
            .position(|vendor| vendor.site() == store.site)
            .unwrap_or(usize::MAX)
    });
    let listings = wanted
        .iter()
        .zip(stores)
        .map(|(vendor, store)| {
            (
                *vendor,
                Listing {
                    bundles: store.bundles,
                    // Kept as text in the capture and already reported above; an `Error` cannot
                    // be reconstructed from one and nothing here needs to.
                    problems: Vec::new(),
                },
            )
        })
        .collect();
    Some(Sourced {
        listings,
        notes,
        problems,
    })
}

/// Reads every named store, keeping them apart.
///
/// Apart is what lets a problem be reported under the store it came from, and stops one
/// unreachable store from being mistaken for the whole run failing.
fn fetch(wanted: &[Vendor], plain: bool) -> Sourced {
    let mut listings = Vec::new();
    let mut problems: Vec<(String, Vec<String>)> = Vec::new();
    for vendor in wanted {
        // Said on stderr rather than only under `-v`, because nothing at all is printed until
        // every store has answered: without this a slow store looks like a hung command.
        if !plain {
            eprintln!("catalogames: fetching {}...", vendor.site());
        }
        match list(*vendor) {
            Ok(listing) => {
                // Rendered now, while the errors still exist, and printed after the results.
                let found: Vec<String> = listing.problems.iter().map(ToString::to_string).collect();
                if !found.is_empty() {
                    problems.push((vendor.site().to_owned(), found));
                }
                listings.push((*vendor, listing));
            }
            // A store that answered with nothing at all is one problem covering everything it
            // would have offered, which is why it reads as a problem rather than as a silence.
            Err(error) => problems.push((
                vendor.site().to_owned(),
                vec![catalogames::error::chain(&error)],
            )),
        }
    }
    Sourced {
        listings,
        // A fresh read needs no introducing unless something went wrong in it.
        notes: problem_note(&problems).into_iter().collect(),
        problems,
    }
}

/// Saves what was just read, so the next run can offer it at once.
///
/// A failure here is reported and nothing more: the listing is already in hand, and losing the
/// chance to save it costs one slow run rather than this one.
fn keep(listings: &[(Vendor, Listing)]) {
    let stores = listings
        .iter()
        .map(|(vendor, listing)| cache::Store {
            site: vendor.site().to_owned(),
            bundles: listing.bundles.clone(),
            // Rendered now, while the errors still exist: a capture that forgot them would
            // reload as a complete listing when it is a partial one.
            problems: listing.problems.iter().map(ToString::to_string).collect(),
        })
        .collect();
    let saved = cache::directory()
        .and_then(|directory| cache::save(&directory, &cache::Capture::of(stores)));
    match saved {
        Ok(path) => debug!("saved this listing to {}", path.display()),
        Err(error) => eprintln!("catalogames: could not save this listing: {error}"),
    }
}

/// Prints each store's listing, naming the store when there is more than one.
///
/// The header appears only when more than one store is listed, so naming a single store prints
/// exactly what it always did. With several, it says whose bundles follow — which is also how
/// Fanatical's request to be cited when its data is shown is met.
fn print(listings: &[(Vendor, Listing)], held: &Holdings, classified: &Classified) {
    for (position, (vendor, listing)) in listings.iter().enumerate() {
        if listings.len() > 1 {
            // A blank line above every heading, and a second one between sections, so a store's
            // name is never the line straight after another store's last game.
            println!();
            if position > 0 {
                println!();
            }
            println!("[{}]\n", vendor.site());
        }
        std::print!("{}", render::listing(listing, palette(), held, classified));
    }
}

/// What the item service says the ids no table could settle are, and what a reader should be
/// told about it: that Steam could not be asked, that it left some ids unanswered, or that an id
/// is claimed by more than one kind of item and a choice was made by name.
///
/// No unsettled ids, no request. A failure is a note rather than an error, because the links
/// then fall back to what the stores published — which is all they were before this existed.
fn classify(listings: &[(Vendor, Listing)]) -> (Classified, Vec<String>) {
    let asks = Classified::asks(listings.iter().map(|(_, listing)| listing));
    if asks.is_empty() {
        return (Classified::none(), Vec::new());
    }
    let ids = asks
        .iter()
        .map(|ask| ask.id)
        .collect::<std::collections::BTreeSet<u32>>()
        .len();
    let classified =
        match steam::store::Client::new().and_then(|client| Classified::fetch(&client, &asks)) {
            Ok(classified) => classified,
            Err(why) => {
                return (
                    Classified::none(),
                    vec![format!(
                        "Steam could not be asked what {ids} unconfirmed id(s) are ({why}); their \
                     links are as the stores published them"
                    )],
                );
            }
        };
    let mut said = Vec::new();
    if !classified.unanswered().is_empty() {
        let left: Vec<String> = classified.unanswered().iter().map(u32::to_string).collect();
        said.push(format!(
            "Steam left {} of {ids} unconfirmed id(s) unanswered ({}); their links are as the \
             stores published them",
            left.len(),
            left.join(", ")
        ));
    }
    for (id, claims) in classified.shared() {
        let as_what: Vec<String> = claims
            .iter()
            .map(|claim| format!("{:?} {:?}", claim.kind, claim.name))
            .collect();
        said.push(format!(
            "Steam id {id} is claimed as {}; the page chosen is the one named like the game",
            as_what.join(" and ")
        ));
    }
    (classified, said)
}

/// One store's listing. Each store keeps its own handler; this only chooses between them.
fn list(vendor: Vendor) -> catalogames::Result<Listing> {
    match vendor {
        Vendor::Humblebundle => humble::Client::new()?.list_bundles(),
        Vendor::Fanatical => fanatical::Client::new()?.list_bundles(),
    }
}

/// Writes one store's library snapshot, naming each file it wrote on stdout.
///
/// The paths go to stdout and everything else to stderr, so a caller can pipe the output
/// straight into whatever reads the snapshots next.
fn snapshot(store: Store, directory: Option<&Path>) -> catalogames::Result<ExitCode> {
    let directory = snapshot_directory(directory)?;

    // Offered before anything is attempted, and only on a first run: with nothing configured
    // at all, printing instructions and stopping is a worse answer than asking.
    if store == Store::Steam
        && interactive()
        && terminal_gui::setup::untouched(&gamelib::steam::SETTINGS)
    {
        if let Some((path, written)) =
            terminal_gui::setup::credentials(&gamelib::steam::SETTINGS, check_steam)?
        {
            eprintln!(
                "catalogames: wrote {} — {}. Readable only by you.",
                path.display(),
                written.join(" and ")
            );
        }
    }

    let mut report = match store {
        Store::Steam => gamelib::steam::snapshot(&directory),
        Store::Epic => gamelib::epic::snapshot(&directory),
    };

    // The one thing that can be done FOR somebody on the Epic side. Installing the program is
    // theirs; signing it in is a single command, and offering to run it beats printing it.
    //
    // **After the reasons are printed, not before.** Asked first, the question arrives with no
    // context — somebody who does not yet know they are signed out is being offered a fix for a
    // problem they have not been told about. Defaulted to no, because it opens a login page.
    if store == Store::Epic && !report.is_complete() && interactive() && gamelib::epic::installed()
    {
        explain(&report);
        if terminal_choice::prompt_yN("catalogames: run `legendary auth` to sign in now?") {
            match std::process::Command::new(gamelib::epic::PROGRAM)
                .arg("auth")
                .status()
            {
                // Signed in, so ask again rather than making them rerun the command.
                Ok(status) if status.success() => report = gamelib::epic::snapshot(&directory),
                // This is the case the fallback is FOR, and the moment to say so: the sign-in
                // we just ran is the one that crashes on an empty Enter, and a bare exit
                // status leaves somebody staring at a traceback with nothing to do next.
                Ok(status) => eprintln!(
                    "\ncatalogames: legendary auth exited with {status}.\n\n{}\n\n\
                     Then run this again:\n\n    catalogames gamelib epic",
                    gamelib::epic::SIGN_IN_FALLBACK
                ),
                Err(source) => eprintln!("catalogames: could not run legendary auth: {source}"),
            }
        } else {
            // Already said, and saying it twice reads as two separate failures.
            return Ok(ExitCode::FAILURE);
        }
    }
    for path in &report.written {
        println!("{}", path.display());
    }
    if report.is_complete() {
        return Ok(ExitCode::SUCCESS);
    }
    explain(&report);
    // Nothing written is a plain failure; some of it written is the same degraded result a
    // partial bundle listing reports, and for the same reason: the caller's next step differs.
    Ok(if report.written.is_empty() {
        ExitCode::FAILURE
    } else {
        ExitCode::from(EXIT_DEGRADED)
    })
}

/// Builds the inventory entry for one game, ready to paste into the project.
///
/// A link says which store it came from. A bare id does not — every store numbers its own
/// games, and `1202130` is a different game on each — so that case asks rather than assuming
/// the store this program happens to support best.
fn add_project_entry(game: &str) -> catalogames::Result<ExitCode> {
    let store = match Store::of_link(game) {
        Some(store) => store,
        None if looks_like_a_link(game) => {
            eprintln!(
                "catalogames: {game:?} is a link to a store this program does not read.\n\
                 Known stores: {}",
                Store::all()
                    .iter()
                    .map(|store| format!("{} ({})", store.label(), store.hosts()[0]))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            return Ok(ExitCode::FAILURE);
        }
        None => match ask_which_store(game)? {
            Some(store) => store,
            None => return Ok(ExitCode::FAILURE),
        },
    };

    if let Some(why) = store.entry_blocker() {
        eprintln!(
            "catalogames: nothing can be written for {} yet.\n{why}",
            store.label()
        );
        return Ok(ExitCode::FAILURE);
    }
    match store {
        Store::Steam => steam_entry(game),
        // Every other store is refused above by its own blocker, which is why this arm cannot
        // guess: a store that becomes writable needs its own handler here, and the compiler
        // will say so.
        Store::Epic => unreachable!("epic is refused by its entry blocker"),
    }
}

/// Whether the argument is shaped like a link rather than a bare id.
///
/// Only asked once a link has already failed to name a known store, so the two `None` cases can
/// be told apart: an unknown store is a mistake to report, a bare id is a question to ask.
fn looks_like_a_link(text: &str) -> bool {
    text.contains("://") || text.contains('/')
}

/// Asks which store a bare id came from, as a numbered menu.
///
/// Draws on stderr, so stdout still carries nothing but the entry. Returns `None` when the
/// question was not answered — cancelled, or asked where there is no terminal to answer in.
fn ask_which_store(id: &str) -> catalogames::Result<Option<Store>> {
    let mut menu = Menu::new("Which store is that from?")
        .intro(format!(
            "{id} is a bare id, and every store numbers its own games."
        ))
        .intro("Pass a store link instead to skip this question.");
    for store in Store::all() {
        menu = match store.entry_blocker() {
            None => menu.pick(store.label()),
            Some(why) => menu.locked(store.label(), why),
        };
    }

    match run_menu(&mut menu) {
        Ok(Chosen::Picked(at)) => Ok(Some(Store::all()[at])),
        Ok(Chosen::Cancelled) => Ok(None),
        // Not an error worth failing the process over: it means the question cannot be asked
        // here, and the answer can be given on the command line instead.
        Err(source) => Err(Error::Setup {
            detail: format!(
                "{id:?} does not say which store it is from, and the question cannot be asked \
                 here ({source}).\nPass the game's store link instead, for example \
                 https://store.steampowered.com/app/{id} ."
            ),
        }),
    }
}

/// Fetches one Steam game and writes its entry to stdout.
fn steam_entry(game: &str) -> catalogames::Result<ExitCode> {
    let Some(app_id) = steam::app_id_from_url(game) else {
        eprintln!("catalogames: {game:?} is neither an app-id nor a Steam store URL");
        return Ok(ExitCode::FAILURE);
    };

    // Asked before anything is fetched: a duplicate that is caught here is an answer, while one
    // caught after the paste is a failing test in the inventory's own duplicate check.
    if let Some(existing) = steam_inventory::by_app_id(app_id) {
        eprintln!(
            "catalogames: app {app_id} is already in the inventory, as {:?}. Nothing to add.",
            existing.name
        );
        return Ok(ExitCode::SUCCESS);
    }

    let details = steam::store::Client::new()?.fetch(app_id)?;
    // The preview goes to stderr so that stdout is the entry and nothing else, but it is still
    // printed: it is what lets a person see at a glance that the id was the game they meant.
    eprintln!(
        "{}",
        render::line(&Preview::of_details(&details), palette())
    );

    // Fail-closed, exactly as the table generator does: a tag Valve added since `tags.rs` was
    // generated must not become an entry that is silently missing one.
    let tags = details
        .tags
        .iter()
        .map(|name| {
            Tag::from_name(name).ok_or_else(|| Error::Drift {
                subject: format!("app {app_id}"),
                detail: format!(
                    "unknown tag {name:?} — regenerate tags.rs with \
                     `cargo run --release --features tools --bin steam_tags`"
                ),
            })
        })
        .collect::<catalogames::Result<Vec<Tag>>>()?;

    // The capture date rides with the entry. `CAPTURED` is a module-level date and the recent
    // review row is a thirty-day window measured on it, so a row pasted under an older date
    // would make that date wrong for this one row, silently and in the one field that is stale
    // by construction.
    let today = Timestamp::now().date();
    println!("    // captured {today}");
    print!("{}", source::entry(&details, &[], &tags));
    eprintln!(
        "\ncatalogames: paste into src/inventory/steam/regular.rs, and set CAPTURED there to \
         {today} or regenerate the table.\n\
         This entry carries no REVIEW marker: the app-id was given rather than searched for, so \
         there was no title match to be unsure about."
    );
    Ok(ExitCode::SUCCESS)
}

/// Writes the script that opens the picked pages.
///
/// Lives here rather than in the library for the reason the printing does: producing the text is
/// the capability, and deciding where it lands is the command line's business.
///
/// **Never replaces a file it did not write.** The default lands in whatever directory the
/// command was run from, which may well be a repository, and a generated file is one careless
/// run away from overwriting something a person made. The script names itself on its second
/// line; anything without that name is somebody else's and is left alone.
///
/// Written `755`, executable. It holds nothing private — public store pages and titles — and it
/// exists to be run, so making the reader `chmod` it before every run was friction for no gain.
///
/// The earlier reasoning, recorded because it is not wrong, only outweighed: an executable
/// appearing unasked in a working directory is a larger thing to leave behind than a plain file.
/// What settles it is that this one is asked for — it is written only when someone ticked boxes
/// in the form — and that `--out` says where.
///
/// **The permission is set after the write, not only in the open.** `OpenOptions::mode` applies
/// when a file is CREATED, so replacing this program's own earlier output — which is the common
/// case, run after run — would truncate a `644` file and leave it `644` forever.
fn write_opener(path: &Path, picked: &[links::Link]) -> catalogames::Result<()> {
    let refuse = |detail: String| catalogames::Error::Drift {
        subject: path.display().to_string(),
        detail,
    };
    if let Ok(existing) = std::fs::read_to_string(path)
        && existing.lines().nth(1) != Some(links::MARKER)
    {
        return Err(refuse(
            "already exists and was not written by this program, so it has been left alone. \
             Pass --out to write somewhere else."
                .to_owned(),
        ));
    }

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o755)
        .open(path)
        .map_err(|source| refuse(format!("could not be written: {source}")))?;
    file.write_all(links::script(picked).as_bytes())
        .map_err(|source| refuse(format!("{source}")))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|source| refuse(format!("could not be made executable: {source}")))
}

/// Says why each snapshot that was not taken was not taken.
fn explain(report: &gamelib::Report) {
    for failure in &report.failures {
        eprintln!("\ncatalogames: {failure}");
    }
}

/// Where the library and wishlist files go: what `--dir` said, or the data directory.
///
/// Separated out because it is the one decision in this command that is worth being sure of
/// without running it: a snapshot written somewhere unexpected is a file somebody has to go
/// looking for.
fn snapshot_directory(given: Option<&Path>) -> catalogames::Result<PathBuf> {
    match given {
        Some(given) => Ok(given.to_path_buf()),
        None => gamelib::config::data_directory(),
    }
}

/// What to say about a value as it is typed, or nothing while it looks right.
///
/// Shown under the form on every repaint, so a SteamID of the wrong length says so before the
/// file is written rather than after — and the commonest mistake, pasting a custom profile name,
/// is caught at the moment it is pasted.
fn check_steam(setting: &gamelib::config::Setting, typed: &str) -> Result<String, String> {
    let typed = typed.trim();
    if typed.is_empty() || setting.name != "steam_id" {
        return Ok(typed.to_owned());
    }
    // The profile page is what somebody has open, so pasting it counts as answering. What comes
    // back is the number, which is what gets written down.
    gamelib::steam::steam_id_from(typed).ok_or_else(|| {
        format!(
            "{typed:?} is not a SteamID. Paste the number, or the profile page it is on \
             (steamcommunity.com/profiles/...). A custom name (steamcommunity.com/id/...) is \
             not the id and has to be converted first."
        )
    })
}

/// Whether there is both somewhere to draw a form and someone to answer it.
///
/// Two conditions, not one. Colour depends on **stdout**, so `sales | less` goes plain and
/// keeps the pipeline working. A form also depends on **stdin**, because it reads keys — and
/// `sales < /dev/null`, or the same command under cron, has a perfectly good terminal on stdout
/// and no keyboard behind it. That mattered less while the form was opt-in; as the default it
/// is the difference between a plain listing and a command that appears to hang.
fn interactive() -> bool {
    std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
}

/// Colour when a terminal is reading, plain text otherwise.
///
/// Decided here rather than in the library: escape codes written into a pipe or a file are
/// noise in someone else's data, and only the process knows where its output is going.
fn palette() -> Palette {
    if std::io::stdout().is_terminal() {
        Palette::Ansi
    } else {
        Palette::Plain
    }
}

#[cfg(test)]
mod directory_tests {
    use super::*;

    #[test]
    fn an_explicit_directory_is_used_exactly_as_given() {
        assert_eq!(
            snapshot_directory(Some(Path::new("/tmp/somewhere"))).expect("a path"),
            Path::new("/tmp/somewhere")
        );
    }

    #[test]
    fn the_default_is_a_data_directory_and_never_the_bare_home() {
        // These used to land loose in the home directory, where a file called `game_lib_steam`
        // is one that nothing explains. And not beside the credentials either: `~/.config` is
        // for what a person edits.
        let chosen = snapshot_directory(None).expect("a path");
        assert!(
            chosen.ends_with("catalogames"),
            "not in a directory of its own: {chosen:?}"
        );
        assert!(
            !chosen
                .components()
                .any(|part| part.as_os_str() == ".config"),
            "a snapshot is not configuration: {chosen:?}"
        );
    }
}

#[cfg(test)]
mod opener_tests {
    use super::*;
    use catalogames::links::Link;

    fn link() -> Link {
        Link {
            bundle: "A Bundle".to_owned(),
            title: "A Game".to_owned(),
            url: "https://store.steampowered.com/app/1".to_owned(),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "catalogames-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a writable directory");
        path
    }

    #[test]
    fn the_script_is_written_executable_even_when_it_replaces_an_old_one() {
        // It exists to be run, so it arrives runnable. The REPLACEMENT half is the part worth
        // pinning: `OpenOptions::mode` applies only on creation, so a run that overwrites this
        // program's own earlier output would otherwise leave whatever mode that file already had
        // — which is the case someone hits on every run after the first.
        use std::os::unix::fs::PermissionsExt;
        let directory = scratch("write");
        let path = directory.join(links::FILE_NAME);
        let executable = |path: &Path| {
            std::fs::metadata(path)
                .expect("exists")
                .permissions()
                .mode()
                & 0o111
        };

        write_opener(&path, &[link()]).expect("writes");
        assert_ne!(executable(&path), 0, "not runnable on the first write");

        // Hand it back the way a previous version left it, then write again.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        write_opener(&path, &[link(), link()]).expect("replaces");
        assert_ne!(
            executable(&path),
            0,
            "not runnable after replacing an older file"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn its_own_earlier_output_is_replaced() {
        let directory = scratch("replace");
        let path = directory.join(links::FILE_NAME);
        write_opener(&path, &[link()]).expect("writes");
        write_opener(&path, &[link(), link()]).expect("replaces its own file");

        let text = std::fs::read_to_string(&path).expect("reads");
        assert_eq!(text.matches("open_link ").count(), 2);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_file_this_program_did_not_write_is_left_alone() {
        // The default lands in whatever directory the command was run from, which may well be a
        // repository. Recognising its own output by the name on its second line is what stops
        // it eating something a person made.
        let directory = scratch("refuse");
        let path = directory.join(links::FILE_NAME);
        let mine = "#!/bin/sh\n# my own notes\necho hello\n";
        std::fs::write(&path, mine).expect("writes");

        let refused = write_opener(&path, &[link()]).unwrap_err().to_string();
        assert!(refused.contains("not written by this program"), "{refused}");
        assert!(
            refused.contains("--out"),
            "it should say what to do: {refused}"
        );
        assert_eq!(std::fs::read_to_string(&path).expect("still there"), mine);
        let _ = std::fs::remove_dir_all(&directory);
    }
}
