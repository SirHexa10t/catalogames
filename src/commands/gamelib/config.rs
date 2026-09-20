//! Where the credentials live, and what to tell the operator when they do not.
//!
//! Shared by the store handlers because "read a named secret, or explain how to supply one" is
//! the same job whichever store asks. What each store *needs* is the store's own business and
//! stays in its own module.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use log::debug;

use crate::{Error, Result};

/// The file inside the configuration directory.
pub const FILE: &str = "credentials";

/// Prefix for the environment-variable form of a setting.
const ENV_PREFIX: &str = "CATALOGAMES_";

/// Credentials read from the configuration file, with the environment taking precedence.
///
/// Values are held in memory only for the length of one command; nothing here writes a secret
/// anywhere, and the snapshots the handlers produce carry no credential.
#[derive(Debug, Default)]
pub struct Credentials {
    values: BTreeMap<String, String>,
    /// Where they were read from, for messages. `None` when no file existed.
    source: Option<PathBuf>,
}

impl Credentials {
    /// Reads the configuration file, if there is one.
    ///
    /// A missing file is not an error: every setting can also come from the environment, and a
    /// store that finds nothing produces its own guide, which is far more useful than a generic
    /// "no config file" from here.
    pub fn load() -> Result<Self> {
        let path = directory()?.join(FILE);
        let Ok(text) = fs::read_to_string(&path) else {
            debug!("no credentials file at {}", path.display());
            return Ok(Self::default());
        };
        refuse_if_shared(&path)?;
        debug!("read credentials from {}", path.display());
        Ok(Self {
            values: parse(&text),
            source: Some(path),
        })
    }

    /// The value for `name`, from the environment first and the file second.
    ///
    /// The environment wins so that a caller can override one setting for one run without
    /// editing a file — the usual shape for scripts and containers. It is checked first rather
    /// than used as a fallback because a fallback would silently prefer a stale file.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<String> {
        if let Ok(value) = std::env::var(env_name(name)) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
        self.values.get(name).cloned()
    }

    /// Where the values came from, for a message that has to name it.
    #[must_use]
    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }
}

/// `steam_api_key` becomes `CATALOGAMES_STEAM_API_KEY`.
///
/// A rule rather than a table: a setting added to a store handler gets its environment variable
/// without a second registration site to forget.
fn env_name(setting: &str) -> String {
    format!("{ENV_PREFIX}{}", setting.to_uppercase())
}

/// The configuration directory, respecting `XDG_CONFIG_HOME`.
pub fn directory() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Ok(PathBuf::from(xdg).join(env!("CARGO_PKG_NAME")));
    }
    Ok(home()?.join(".config").join(env!("CARGO_PKG_NAME")))
}

/// Where snapshots go by default: `$XDG_DATA_HOME/catalogames`, else `~/.local/share/catalogames`.
///
/// **Not `~/.config`,** which is where this program's own settings live and where the credentials
/// file is. A snapshot is not configuration: nobody edits it, and deleting it changes nothing
/// about how the program behaves. It is data the program produced and keeps, which is what the
/// data directory is for.
///
/// **Not the home directory either,** which is where these used to land. A file called
/// `game_lib_steam` loose in a home directory is one this program made and nothing says so.
pub fn data_directory() -> Result<PathBuf> {
    let xdg = std::env::var("XDG_DATA_HOME").ok();
    Ok(data_path(xdg.as_deref(), &home()?))
}

/// The rule itself, over a named environment, so it can be checked without one.
fn data_path(xdg: Option<&str>, home: &Path) -> PathBuf {
    // An empty-but-set variable is not an answer: joining onto it yields a path at the root of
    // the filesystem, which is the one place this must never write.
    match xdg.filter(|value| !value.is_empty()) {
        Some(value) => PathBuf::from(value).join(env!("CARGO_PKG_NAME")),
        None => home
            .join(".local")
            .join("share")
            .join(env!("CARGO_PKG_NAME")),
    }
}

/// The account holder's home directory.
pub fn home() -> Result<PathBuf> {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => Ok(PathBuf::from(home)),
        _ => Err(Error::Setup {
            detail: "HOME is not set, so there is no default directory to write to. \
                     Pass an explicit one with --dir."
                .to_owned(),
        }),
    }
}

/// Refuses a credentials file that anyone but its owner can read.
///
/// Checked rather than fixed. Silently tightening a file the operator chose to leave open would
/// hide the fact that whatever already read it has had its chance — and on a shared machine,
/// "it has been world-readable until now" is the part worth knowing.
fn refuse_if_shared(path: &Path) -> Result<()> {
    let mode = fs::metadata(path)
        .map_err(|source| Error::Setup {
            detail: format!("could not read {}: {source}", path.display()),
        })?
        .permissions()
        .mode();
    if mode & 0o077 == 0 {
        return Ok(());
    }
    Err(Error::Setup {
        detail: format!(
            "{} is readable by other accounts on this machine (mode {:04o}).\n\
             Tighten it before using it:\n\n    chmod 600 {}\n",
            path.display(),
            mode & 0o7777,
            path.display()
        ),
    })
}

/// Reads `name = value` lines, ignoring blanks and `#` comments.
///
/// Hand-parsed rather than TOML: the file holds a handful of flat strings, and a parser
/// dependency would arrive with a format that invites structure this file should not have.
/// Quotes around a value are stripped, because an operator who copied one in meant the
/// contents, not the quotes.
fn parse(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| {
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            (name.trim().to_lowercase(), value.to_owned())
        })
        .collect()
}

/// Removes this crate's own settings from a child process's environment.
///
/// A spawned program inherits everything, including a Steam Web API key that happens to be in
/// the environment — handing a credential to a tool that has no use for it and may log its own
/// environment. Every variable named with this crate's prefix is stripped, rather than a list of
/// known names, so a setting added later cannot be forgotten here.
pub fn deny_inherited_settings(command: &mut std::process::Command) {
    deny_from(command, std::env::vars().map(|(name, _)| name));
}

/// The rule itself, over a named environment, so it can be checked without one.
fn deny_from(command: &mut std::process::Command, names: impl IntoIterator<Item = String>) {
    for name in names {
        if name.starts_with(ENV_PREFIX) {
            debug!("not passing {name} to the child process");
            command.env_remove(name);
        }
    }
}

/// One thing the account holder has to supply, and everything needed to ask them for it.
///
/// A table rather than four parallel lists, because the printed guide and the form that collects
/// these must ask for exactly the same things. Two descriptions of one setting drift, and the
/// drift shows up as instructions that no longer match the questions.
#[derive(Debug, Clone, Copy)]
pub struct Setting {
    /// The name it takes in the file: `steam_id`.
    pub name: &'static str,
    /// What to call it when asking a person: `SteamID`.
    pub label: &'static str,
    /// What to show in its place in a written-out example.
    pub example: &'static str,
    /// Where to get it, in prose, for whoever has not got one yet.
    pub guidance: &'static str,
}

/// Writes the credentials file, replacing whatever was there.
///
/// Owner-only and created that way rather than tightened afterwards, and the directory 0700, so
/// there is no moment at which either is readable by anyone else. Atomic, so an interrupted
/// write cannot leave half a key behind.
///
/// Returns where it went, because a program that writes a file on someone's behalf owes them
/// the path.
pub fn save(values: &[(&str, String)]) -> Result<PathBuf> {
    let directory = directory()?;
    let failed = |detail: String| Error::Drift {
        subject: directory.display().to_string(),
        detail,
    };
    std::fs::create_dir_all(&directory).map_err(|source| failed(format!("{source}")))?;
    std::fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|source| failed(format!("could not tighten it: {source}")))?;

    let body: String = values
        .iter()
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(name, value)| format!("{name} = {}\n", value.trim()))
        .collect();
    let path = directory.join(FILE);
    crate::write::replace(&path, body.as_bytes(), 0o600)?;
    Ok(path)
}

/// How to create the credentials file, ready to paste.
///
/// Written as "create this file with these contents" rather than a shell command holding the
/// secret, so the key never reaches the shell history.
#[must_use]
pub fn how_to_write(settings: &[Setting]) -> String {
    let path = directory().map_or_else(
        |_| format!("~/.config/{}/{FILE}", env!("CARGO_PKG_NAME")),
        |dir| dir.join(FILE).display().to_string(),
    );
    let lines: String = settings
        .iter()
        .map(|setting| format!("    {} = {}\n", setting.name, setting.example))
        .collect();
    let directory = directory().map_or_else(
        |_| format!("~/.config/{}", env!("CARGO_PKG_NAME")),
        |dir| dir.display().to_string(),
    );
    format!(
        "Create this file, readable only by you:\n\n    {path}\n\n\
         with these lines:\n\n{lines}\n\
         If the directory is not there yet, and to leave both readable only by you:\n\n\
         \x20   mkdir -p {directory} && chmod 700 {directory}\n\
         \x20   chmod 600 {path}\n\n\
         Do not paste the key into a shell command — it would be kept in your shell history.\n\
         Every setting can also be given as an environment variable instead \
         ({ENV_PREFIX}STEAM_API_KEY and so on), which keeps it off disk but exposes it to \
         every program this one starts."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_go_to_a_data_directory_and_never_beside_the_settings() {
        // `~/.config` holds what a person edits; a snapshot is data this program made, and
        // nobody edits it. The home directory is wrong for a different reason: a file called
        // `game_lib_steam` loose in one is a file nothing explains.
        let home = Path::new("/home/someone");
        assert_eq!(
            data_path(None, home),
            Path::new("/home/someone/.local/share/catalogames")
        );
        assert_eq!(
            data_path(Some("/data"), home),
            Path::new("/data/catalogames")
        );
    }

    #[test]
    fn an_empty_but_set_variable_is_not_an_answer() {
        // Joining onto it would put the files at the root of the filesystem.
        assert_eq!(
            data_path(Some(""), Path::new("/home/someone")),
            Path::new("/home/someone/.local/share/catalogames")
        );
    }

    #[test]
    fn a_setting_becomes_a_prefixed_upper_case_variable() {
        assert_eq!(env_name("steam_api_key"), "CATALOGAMES_STEAM_API_KEY");
        assert_eq!(env_name("steam_id"), "CATALOGAMES_STEAM_ID");
    }

    #[test]
    fn names_and_values_are_read_from_plain_lines() {
        let values = parse("steam_api_key = ABC123\nsteam_id=7656119\n");
        assert_eq!(values["steam_api_key"], "ABC123");
        assert_eq!(values["steam_id"], "7656119");
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let values = parse("# a note\n\n  # indented note\nsteam_id = 7\n");
        assert_eq!(values.len(), 1);
        assert_eq!(values["steam_id"], "7");
    }

    #[test]
    fn quotes_an_operator_copied_in_are_not_part_of_the_value() {
        assert_eq!(parse("steam_api_key = \"ABC\"")["steam_api_key"], "ABC");
    }

    #[test]
    fn a_name_is_matched_whatever_case_it_was_written_in() {
        assert_eq!(parse("Steam_API_Key = ABC")["steam_api_key"], "ABC");
    }

    #[test]
    fn a_value_containing_an_equals_sign_survives_intact() {
        // Base64-ish secrets end in `=`; splitting on the last one would truncate them.
        assert_eq!(parse("token = a=b=c")["token"], "a=b=c");
    }

    #[test]
    fn a_line_without_an_equals_sign_is_skipped_rather_than_guessed_at() {
        assert!(parse("steam_api_key\n").is_empty());
    }

    #[test]
    fn a_world_readable_file_is_refused_and_says_how_to_fix_it() {
        let directory =
            std::env::temp_dir().join(format!("catalogames-perm-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("creates");
        let path = directory.join("credentials");
        fs::write(&path, "steam_id = 7\n").expect("writes");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmods");

        let failure = refuse_if_shared(&path).unwrap_err().to_string();
        assert!(failure.contains("readable by other accounts"), "{failure}");
        assert!(failure.contains("chmod 600"), "{failure}");

        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("chmods");
        assert!(refuse_if_shared(&path).is_ok());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_guide_never_puts_a_secret_on_a_command_line() {
        let guide = how_to_write(&[Setting {
            name: "steam_api_key",
            label: "Web API key",
            example: "<your key>",
            guidance: "",
        }]);
        assert!(guide.contains("Create this file"), "{guide}");
        assert!(guide.contains("chmod 600"), "{guide}");
        assert!(
            !guide.contains("export "),
            "the guide suggests putting a key in the shell: {guide}"
        );
    }

    #[test]
    fn every_setting_of_ours_is_kept_from_a_child_process() {
        // Stripped by prefix rather than by a list of names, so a setting added later cannot
        // be forgotten here — including one that does not exist yet.
        let mut command = std::process::Command::new("true");
        deny_from(
            &mut command,
            [
                "CATALOGAMES_STEAM_API_KEY".to_owned(),
                "CATALOGAMES_SOMETHING_NOT_INVENTED_YET".to_owned(),
                "PATH".to_owned(),
                "HOME".to_owned(),
            ],
        );

        // Sorted, because which names are stripped is the contract and the order they come
        // back in is not.
        let mut removed: Vec<String> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        removed.sort();
        assert_eq!(
            removed,
            [
                "CATALOGAMES_SOMETHING_NOT_INVENTED_YET",
                "CATALOGAMES_STEAM_API_KEY"
            ]
        );
    }

    #[test]
    fn a_child_keeps_the_environment_it_actually_needs() {
        // Legendary needs HOME to find its own configuration and PATH to run at all; stripping
        // the whole environment instead of ours would break it.
        let mut command = std::process::Command::new("true");
        deny_from(&mut command, ["PATH".to_owned(), "HOME".to_owned()]);
        assert_eq!(
            command.get_envs().count(),
            0,
            "an unrelated variable was touched"
        );
    }
}
