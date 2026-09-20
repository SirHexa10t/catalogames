//! The Epic Games Store library, read through Legendary.
//!
//! Its own handler, sharing nothing with [`super::steam`] but the snapshot shape. Epic publishes
//! no API for reading your own library, so there is no request to make: this runs an external
//! program that already implements Epic's login and asks it what the account owns.
//!
//! # Why an external program
//!
//! Reaching Epic directly would mean re-implementing their authentication — holding the
//! account's credentials, refreshing tokens, and tracking an undocumented protocol that changes
//! without notice. Legendary already does that, is widely used, and keeps its own session where
//! the account holder can see and revoke it. Shelling out keeps every credential out of this
//! program entirely.
//!
//! # What this handler will not do
//!
//! It never reads Legendary's configuration or its cached manifests. Those are Legendary's
//! private files, their format is not a contract, and a snapshot built from them would be a
//! guess about another project's internals. Only the documented `--json` output is read.
//!
//! Legendary is a *runtime* dependency: not compiled in, not downloaded by a build, expected on
//! `PATH` when this subcommand runs and checked for before it is used. It is declared under
//! `[[package.metadata.runtime-dependencies]]` in `Cargo.toml` so that a project packaging this
//! one can prepare the environment without reading the source.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use log::{debug, info};

use super::{Entry, Failure, Kind, Provenance, Report, Snapshot, config};
use crate::{Error, Result};

/// Name this store goes by in file names and provenance.
pub const STORE: &str = "epic";

/// The program that does the reading.
pub const PROGRAM: &str = "legendary";

/// What to do when the interactive sign-in does not finish, which it may not.
///
/// Three routes, worst last, because the one everybody meets first is the one with a stopwatch
/// on it. All measured against 0.21.1.
///
/// The **copy-and-paste** route hands over an OAuth authorization code, which Epic makes
/// single-use and expires within minutes. Miss that window, or spend the code on an attempt that
/// got far enough to send it, and Epic answers `authorization_code_not_found` — one message for
/// three different mistakes, which is what makes it hard to act on.
///
/// Its prompt is also fragile: it reads what was typed and then indexes the first character
/// without checking that anything was, so a bare Enter raises `IndexError` and takes the command
/// down. That crash happens before the code is sent, leaving the code unused — but only for a
/// prompt that was genuinely empty.
///
/// The other two routes have no stopwatch. A **session id** is exchanged for a fresh code by
/// Legendary itself, and an **embedded browser** does the whole login with nothing to copy at
/// all; that one needs `pywebview` plus GTK or Qt Python bindings, which a desktop Linux
/// generally has or can install from its own packages.
pub const SIGN_IN_FALLBACK: &str = "If that prompt crashed, or Epic answered \
     \"authorization_code_not_found\", the code was empty, already spent or expired — they last \
     minutes and work once. Any of these gets past it.\n\n\
     Sign in with no code to copy, which is the route without a stopwatch on it:\n\n\
     \x20   pip install 'legendary-gl[webview-gtk]'   # or [webview] with Qt bindings\n\
     \x20   legendary auth\n\n\
     Or hand over a session id, which Legendary exchanges for a fresh code itself. Open\n\
     https://www.epicgames.com/id/api/redirect while signed in to Epic, and pass the \"sid\":\n\n\
     \x20   legendary auth --sid <sid>\n\n\
     Or take a fresh code from https://legendary.gl/epiclogin and use it straight away. Pass \
     the \"authorizationCode\" value, quoted, not the whole URL:\n\n\
     \x20   legendary auth --code '<authorizationCode>'";

/// Where it comes from and how to sign it in.
const PROJECT_PAGE: &str = "https://github.com/derrod/legendary";

/// Writes the Epic snapshot into `directory`.
///
/// Epic publishes one thing here — the owned library — so the report holds at most one entry.
/// It is still a [`Report`] so that every store answers a `gamelib` run in the same shape.
pub fn snapshot(directory: &Path) -> Report {
    // Whether Legendary can be used at all is a store-wide question, answered before any
    // snapshot is attempted. Asking it here is what keeps its setup guide from being reported
    // as though the library in particular had failed.
    let version = match version() {
        Ok(version) => version,
        Err(reason) => {
            return Report {
                written: Vec::new(),
                failures: vec![Failure { what: None, reason }],
            };
        }
    };

    let mut report = Report::default();
    report.take(Kind::Library, directory, || library_from(version));
    report
}

/// Writes the owned-games snapshot, returning the file it wrote.
pub fn library(directory: &Path) -> Result<PathBuf> {
    read_library()?.write(directory)
}

/// Asks Legendary what the account owns.
pub fn read_library() -> Result<Snapshot> {
    library_from(version()?)
}

/// The reading itself, once Legendary is known to be present.
fn library_from(version: String) -> Result<Snapshot> {
    info!("reading the Epic library with {version}");

    // Asked before the list, so that being signed out reads as a sentence rather than a
    // traceback from inside Legendary.
    let status = status()?;
    let Some(account) = status.account else {
        return Err(Error::Setup {
            detail: format!(
                "{PROGRAM} is installed but nobody is signed in to it.\n\
                 Sign in once with:\n\n\
                 \x20   {PROGRAM} auth\n\n\
                 That opens Epic's own login page and leaves a token in {PROGRAM}'s own \
                 configuration. This program never sees an Epic password.\n\n\
                 Once it reports an account, run this again — the token is kept, so there is \
                 nothing else to pass in.\n\n\
                 {SIGN_IN_FALLBACK}"
            ),
        });
    };

    let output = run(&["list", "--json"])?;
    let entries = parse_list(&output)?;
    info!("epic library: {} game(s) for {account}", entries.len());

    Ok(Snapshot::new(
        STORE,
        Kind::Library,
        Provenance {
            source_url: format!("{PROGRAM} list --json"),
            account: Some(account),
            // Counted by Legendary separately from the list it prints, so the two numbers
            // together say whether the read came back whole.
            reported_count: status.games_available,
            read_with: Some(version),
        },
        entries,
    ))
}

/// Turns `legendary list --json` output into snapshot entries.
///
/// Separate from the process call and public, so the shape Legendary publishes can be tested —
/// and so a caller holding that JSON from anywhere else can use it without this crate spawning
/// anything.
///
/// Only `app_name` and `app_title` are read. Everything else Legendary prints — asset
/// manifests, base URLs, metadata, the nested `dlcs` list — is Legendary's business, and reading
/// it would tie this crate to fields it has no use for. Leaving `dlcs` unread is also what keeps
/// downloadable content out of a list that is meant to count games.
pub fn parse_list(json: &str) -> Result<Vec<Entry>> {
    let listed: serde_json::Value =
        serde_json::from_str(json.trim()).map_err(|source| Error::Payload {
            url: format!("{PROGRAM} list --json").as_str().into(),
            source,
        })?;
    let Some(games) = listed.as_array() else {
        return Err(Error::Drift {
            subject: format!("{PROGRAM} list --json"),
            detail: format!(
                "printed a {} where a list of games was expected",
                kind_of(&listed)
            ),
        });
    };

    let entries: Vec<Entry> = games.iter().filter_map(entry).collect();

    // A named game count is the one cross-check available here: Legendary either lists games or
    // it does not, and silently writing an empty library over a good one is the failure this
    // whole module is arranged to avoid.
    if entries.is_empty() && !games.is_empty() {
        return Err(Error::Drift {
            subject: format!("{PROGRAM} list --json"),
            detail: format!(
                "listed {} entries, none of which carried an app_name; \
                 the output format has changed",
                games.len()
            ),
        });
    }
    Ok(entries)
}

/// One game from Legendary's list. `None` for a row with no identifier to key on.
fn entry(game: &serde_json::Value) -> Option<Entry> {
    let app_name = game["app_name"].as_str()?;
    Some(Entry {
        id: app_name.to_owned(),
        // Epic issues its own identifiers and publishes no Steam app-id, so there is nothing
        // honest to put here. A title-based guess is exactly the mistake `steam_app_id` exists
        // to prevent.
        app_id: None,
        name: game["app_title"].as_str().map(str::to_owned),
        playtime_minutes: None,
        priority: None,
    })
}

/// What Legendary says about itself: who is signed in, and how much they own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// The signed-in account, or `None` when nobody is.
    pub account: Option<String>,
    /// How many games Legendary says the account has, when it says.
    pub games_available: Option<u32>,
}

/// What Legendary reports before anything is asked of it.
///
/// Asked first, and the reason is the difference between a sentence and a stack trace. Running
/// `list` while signed out does not fail politely in the version measured, 0.21.1: it raises an
/// unhandled `ValueError: No saved credentials` and prints a full Python traceback, which is
/// what a first run would otherwise show. `status` answers the same question cleanly, exits
/// zero, and is quick.
///
/// It also answers two things worth recording. The account name is the only place Legendary
/// publishes it, and `games_available` is counted separately from the list itself — two numbers
/// for one fact, the same free check every other reader here uses.
pub fn status() -> Result<Status> {
    // Its own stderr is swallowed rather than shown: this is a question asked on the way past,
    // and a locale warning is not something anybody needs to read.
    let output = spawn(&["status", "--json"])
        .stderr(Stdio::null())
        .output()
        .map_err(|source| Error::Setup {
            detail: format!("could not run {PROGRAM}: {source}\n\n{}", guide()),
        })?;
    parse_status(&String::from_utf8_lossy(&output.stdout))
}

/// Reads a `legendary status --json` answer.
///
/// Separate from running it, like every other parser here, so the shape Legendary publishes can
/// be tested without a process.
pub fn parse_status(json: &str) -> Result<Status> {
    let reported: serde_json::Value =
        serde_json::from_str(json.trim()).map_err(|source| Error::Payload {
            url: format!("{PROGRAM} status --json").as_str().into(),
            source,
        })?;

    Ok(Status {
        // Signed out, Legendary answers with the placeholder "<not logged in>" rather than
        // omitting the field. Angle brackets are its way of writing "no value", so they are
        // what is tested for — a real account name cannot start with one.
        account: reported["account"]
            .as_str()
            .filter(|name| !name.starts_with('<'))
            .map(str::to_owned),
        games_available: reported["games_available"]
            .as_u64()
            .and_then(|count| u32::try_from(count).ok()),
    })
}

/// Whether Legendary is installed, for a caller deciding what to offer.
///
/// Distinguishes the two ways this can fail, which want different answers: a program that is not
/// installed cannot be signed in to, and one that is installed but not signed in needs only that.
#[must_use]
pub fn installed() -> bool {
    version().is_ok()
}

/// Legendary's version string, and the check that it is installed at all.
fn version() -> Result<String> {
    let output = Command::new(PROGRAM)
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|source| match source.kind() {
            std::io::ErrorKind::NotFound => Error::Setup { detail: guide() },
            _ => Error::Setup {
                detail: format!("could not run {PROGRAM}: {source}\n\n{}", guide()),
            },
        })?;

    let reported = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if reported.is_empty() {
        // No version claim is recorded rather than a made-up one: the snapshot says what read
        // it, and "an unnamed version of legendary" is the truthful answer here.
        debug!("{PROGRAM} --version printed nothing");
        return Ok(PROGRAM.to_owned());
    }
    debug!("{reported}");

    // No minimum version is enforced. `list --json` has been part of Legendary throughout, and
    // a floor picked without a release that actually breaks it would be a number nobody could
    // trace. A genuine incompatibility surfaces as a parse failure naming what came back.
    Ok(reported)
}

/// Runs Legendary and returns its standard output.
///
/// Its standard error is left attached to the terminal rather than captured. Reading a library
/// takes Legendary several seconds of network work that it narrates there, and a captured
/// stream would turn that into a silent wait with no way to tell progress from a hang. It also
/// means Legendary's own diagnosis of a failure — an expired login, most often — reaches the
/// operator in Legendary's words rather than paraphrased here.
fn run(arguments: &[&str]) -> Result<String> {
    let output = spawn(arguments).output().map_err(|source| Error::Setup {
        detail: format!("could not run {PROGRAM}: {source}\n\n{}", guide()),
    })?;

    if !output.status.success() {
        return Err(Error::Setup {
            detail: format!(
                "{PROGRAM} {} exited with {} — its own message is above.\n\
                 The usual cause is a login that has expired or was never made. Sign in with:\n\n\
                 \x20   {PROGRAM} auth\n",
                arguments.join(" "),
                output.status
            ),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// What to tell an operator who has not set Legendary up.
fn guide() -> String {
    format!(
        "gamelib epic reads your Epic library with {PROGRAM}, an open-source Epic Games Store \
         client, because Epic publishes no API of their own for it.\n\n\
         Install it:\n\n\
         \x20   pipx install legendary-gl\n\
         \x20   (or: pip install --user legendary-gl)\n\n\
         Then sign in once:\n\n\
         \x20   {PROGRAM} auth\n\n\
         That opens Epic's own login page and leaves a token in {PROGRAM}'s configuration. \
         This program never sees an Epic password, never stores an Epic token, and never reads \
         {PROGRAM}'s files — it runs `{PROGRAM} list --json` and reads what that prints.\n\n\
         {SIGN_IN_FALLBACK}\n\n\
         {PROGRAM}: {PROJECT_PAGE}"
    )
}

/// One place a child process is built, so every one is denied this crate's own settings.
fn spawn(arguments: &[&str]) -> Command {
    debug!("running {PROGRAM} {}", arguments.join(" "));
    let mut command = Command::new(PROGRAM);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    config::deny_inherited_settings(&mut command);
    command
}

/// Names a JSON value's type for an error message.
fn kind_of(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "list",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped as Legendary prints it: every field of its game model, plus the nested DLC list.
    const REAL_SHAPE: &str = r#"[
      {
        "app_name": "Fortnite",
        "app_title": "Fortnite",
        "app_version": "",
        "asset_infos": {"Windows": {"app_name": "Fortnite"}},
        "base_urls": [],
        "metadata": {"id": "abc", "title": "Fortnite"},
        "sidecar": null,
        "achievements": null,
        "dlcs": [{"app_name": "FortniteDLC", "app_title": "Some DLC"}]
      },
      {
        "app_name": "Mangrove",
        "app_title": "Control",
        "dlcs": []
      }
    ]"#;

    #[test]
    fn a_game_keeps_its_epic_identifier_and_title() {
        let entries = parse_list(REAL_SHAPE).expect("parses");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "Fortnite");
        assert_eq!(entries[1].id, "Mangrove");
        assert_eq!(entries[1].name.as_deref(), Some("Control"));
    }

    #[test]
    fn downloadable_content_is_not_counted_as_a_game() {
        // Legendary nests DLC inside its owner; reading only the top level is what excludes it.
        let entries = parse_list(REAL_SHAPE).expect("parses");
        assert!(
            !entries.iter().any(|e| e.id == "FortniteDLC"),
            "a DLC entry reached the library snapshot"
        );
    }

    #[test]
    fn no_epic_game_claims_a_steam_app_id() {
        // Epic ids are not Steam ids, and matching the two by title is the error the
        // `steam_app_id` field exists to prevent.
        let entries = parse_list(REAL_SHAPE).expect("parses");
        assert!(entries.iter().all(|e| e.app_id.is_none()));
    }

    #[test]
    fn an_empty_library_is_accepted_as_written() {
        assert!(parse_list("[]").expect("parses").is_empty());
        assert!(parse_list("  []  \n").expect("parses").is_empty());
    }

    #[test]
    fn a_list_of_rows_that_name_no_game_is_a_format_change_not_an_empty_library() {
        let failure = parse_list(r#"[{"appName": "NewSpelling"}]"#).unwrap_err();
        let message = failure.to_string();
        assert!(message.contains("app_name"), "{message}");
        assert!(message.contains("format has changed"), "{message}");
    }

    #[test]
    fn output_that_is_not_a_list_is_reported_by_what_it_actually_was() {
        let failure = parse_list(r#"{"games": []}"#).unwrap_err().to_string();
        assert!(failure.contains("object"), "{failure}");
        assert!(failure.contains("list of games"), "{failure}");
    }

    #[test]
    fn output_that_is_not_json_at_all_is_reported_as_such() {
        let failure = parse_list("legendary: command failed").unwrap_err();
        assert!(matches!(failure, Error::Payload { .. }), "{failure}");
    }

    /// Verbatim from `legendary status --json` on 0.21.1 with nobody signed in.
    const SIGNED_OUT: &str = r#"{"account": "<not logged in>", "games_available": 0,
        "games_installed": 0, "egl_sync_enabled": false, "config_directory": "/home/x/.config/legendary"}"#;

    #[test]
    fn being_signed_out_is_recognised_rather_than_read_as_an_account() {
        // Legendary answers with a placeholder rather than omitting the field, and running
        // `list` in that state raises an unhandled Python error with a full traceback — which
        // is what asking this question first exists to avoid showing anybody.
        let status = parse_status(SIGNED_OUT).expect("parses");
        assert_eq!(status.account, None);
        assert_eq!(status.games_available, Some(0));
    }

    #[test]
    fn a_signed_in_account_is_carried_into_the_snapshot() {
        let status = parse_status(r#"{"account": "someone@example.test", "games_available": 42}"#)
            .expect("parses");
        assert_eq!(status.account.as_deref(), Some("someone@example.test"));
        assert_eq!(status.games_available, Some(42));
    }

    #[test]
    fn a_status_missing_its_fields_yields_nothing_rather_than_a_guess() {
        let status = parse_status("{}").expect("parses");
        assert_eq!(status.account, None);
        assert_eq!(status.games_available, None);
    }

    #[test]
    fn the_guidance_offers_every_way_past_a_failed_sign_in() {
        // Epic answers the same "not found" whether the code was empty, already spent or
        // simply old, so the guidance cannot diagnose it — it has to offer every route, and
        // name the time limit, because that is the part a reader cannot see.
        for route in ["--code", "--sid", "webview"] {
            assert!(
                SIGN_IN_FALLBACK.contains(route),
                "{route}: {SIGN_IN_FALLBACK}"
            );
        }
        assert!(SIGN_IN_FALLBACK.contains("work once"), "{SIGN_IN_FALLBACK}");
        assert!(
            guide().contains("--code"),
            "the install guide carries it too"
        );
    }

    #[test]
    fn the_guide_names_the_program_how_to_install_it_and_how_to_sign_in() {
        let text = guide();
        assert!(text.contains("legendary-gl"), "{text}");
        assert!(text.contains("legendary auth"), "{text}");
        assert!(text.contains(PROJECT_PAGE), "{text}");
        assert!(
            text.contains("never sees an Epic password"),
            "the guide should say what it does not do: {text}"
        );
    }
}
