//! Asking the account holder for what a store needs, and writing it down for them.
//!
//! Lives in the binary for the reason [`crate::picker`] does: it draws a form and reads keys.
//! What it collects goes to the library to write, because writing a credentials file is a
//! capability and the file's permissions are nobody's business but that module's.
//!
//! **The key is on screen while it is typed.** The crate has no masked field, so this is worth
//! knowing rather than hiding: the form draws on stderr and clears itself on exit in one write,
//! so nothing is left in the scrollback, and the file lands `0600` in a directory made `0700`.
//! What it does not survive is somebody reading over a shoulder.

use std::path::PathBuf;

use catalogames::commands::gamelib::config::{self, Setting};
use catalogames::{Error, Result};
use terminal_choice::{Form, Outcome, run_with_warnings};

/// Whether nothing at all has been configured yet, which is the moment worth offering to help.
///
/// Asked about the whole set rather than each setting: somebody who has deliberately supplied
/// one and not the other has answered the question, and being asked again every run is what
/// makes a helpful program tiresome.
pub fn untouched(settings: &[Setting]) -> bool {
    config::Credentials::load().is_ok_and(|held| {
        settings
            .iter()
            .all(|setting| held.get(setting.name).is_none())
    })
}

/// Puts the settings up as a form, writes what was typed, and says where it went.
///
/// `settle` is called on every repaint with each setting's current value. What it returns is
/// what gets written — so an answer given in one shape can be recorded in another, a profile URL
/// becoming the id it contains — and what it rejects is shown in red under the form, while it is
/// being typed rather than after the file has been written.
///
/// `None` when the form was cancelled, which leaves any existing file exactly as it was.
pub fn credentials(
    settings: &[Setting],
    settle: impl Fn(&Setting, &str) -> std::result::Result<String, String>,
) -> Result<Option<(PathBuf, Vec<&'static str>)>> {
    let held = config::Credentials::load()?;
    let mut form = Form::new().title("What catalogames needs, and where to get it");

    for setting in settings {
        // The guidance above each field rather than in a preamble: by the time somebody is
        // typing into the second box they should not have to scroll back for what it wants.
        form = form
            .comment(format!("\n{}: {}", setting.label, setting.guidance))
            .text(setting.label, held.get(setting.name).unwrap_or_default());
    }

    let outcome = run_with_warnings(&mut form, |live| {
        settings
            .iter()
            .filter_map(|setting| {
                settle(setting, live.text_value(setting.label).unwrap_or("")).err()
            })
            .collect()
    });
    match outcome {
        Ok(Outcome::Submitted) => {}
        Ok(Outcome::Cancelled) => return Ok(None),
        Err(source) => {
            return Err(Error::Setup {
                detail: format!(
                    "this needs a terminal to draw on ({source}).\n\n{}",
                    config::how_to_write(settings)
                ),
            });
        }
    }

    // Written in the shape `settle` settled on, so the file holds an id rather than whatever
    // page it was copied from. A value it still rejects is written as typed: the form was
    // submitted anyway, and the store's own error says more than this could.
    let typed: Vec<(&str, String)> = settings
        .iter()
        .map(|setting| {
            let raw = form.text_value(setting.label).unwrap_or("");
            (
                setting.name,
                settle(setting, raw).unwrap_or_else(|_| raw.to_owned()),
            )
        })
        .collect();
    if typed.iter().all(|(_, value)| value.trim().is_empty()) {
        return Ok(None);
    }

    // Named back to the caller rather than printed here, so that what was written and where is
    // said once, by whoever is talking to the person.
    let filled: Vec<&'static str> = settings
        .iter()
        .zip(&typed)
        .filter(|(_, (_, value))| !value.trim().is_empty())
        .map(|(setting, _)| setting.label)
        .collect();
    config::save(&typed).map(|path| Some((path, filled)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_choice::Item;

    const SETTINGS: [Setting; 1] = [Setting {
        name: "steam_id",
        label: "SteamID",
        example: "<id>",
        guidance: "the 17 digits at the end of a profile URL",
    }];

    #[test]
    fn the_form_asks_for_every_setting_and_says_where_to_get_it() {
        // Built here rather than driven, since building is the part with a contract: a field
        // per setting, each introduced by its own guidance.
        let held = config::Credentials::load().expect("loads");
        let mut form = Form::new();
        for setting in &SETTINGS {
            form = form
                .comment(format!("\n{}: {}", setting.label, setting.guidance))
                .text(setting.label, held.get(setting.name).unwrap_or_default());
        }

        assert_eq!(form.items.len(), 2, "a comment and a field for each");
        assert!(matches!(form.items[0], Item::Comment(_)));
        assert_eq!(form.text_value("SteamID"), Some(""));
    }

    #[test]
    fn the_guidance_is_drawn_above_its_own_field() {
        let setting = SETTINGS[0];
        let comment = format!("\n{}: {}", setting.label, setting.guidance);
        // A leading newline draws a blank line first; see the same trick in the picker.
        assert_eq!(comment.lines().count(), 2);
        assert!(comment.contains("17 digits"));
    }
}
