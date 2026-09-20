//! Replacing a file without ever leaving half of one.
//!
//! One writer, because there are now two things that replace a file — a library snapshot and a
//! cached listing — and two copies of "temporary file, then rename" would be two chances to get
//! the awkward parts wrong.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::{Error, Result};

/// Writes `body` to `path` at `mode`, replacing any existing file atomically.
///
/// The temporary file is created in the **same directory** as the target, not in a temp dir: a
/// rename is atomic only within one filesystem, and a home directory on a different mount from
/// `/tmp` is the ordinary layout rather than an exotic one.
///
/// The mode is set as the file is created rather than afterwards. Creating it and then
/// tightening it leaves a window in which the contents are readable by anyone.
pub(crate) fn replace(path: &Path, body: &[u8], mode: u32) -> Result<()> {
    let directory = path.parent().unwrap_or(Path::new("."));
    let failed = |detail: String| Error::Drift {
        subject: path.display().to_string(),
        detail,
    };
    std::fs::create_dir_all(directory).map_err(|source| failed(format!("{source}")))?;

    let temporary = directory.join(format!(".{}.tmp", file_name_of(path)));
    // `create_new` refuses to reuse a file left by an interrupted run, which could be a symlink
    // someone else placed there.
    let _ = std::fs::remove_file(&temporary);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temporary)
        .map_err(|source| {
            failed(format!(
                "could not create {}: {source}",
                temporary.display()
            ))
        })?;

    let written = file
        .write_all(body)
        .and_then(|()| file.sync_all())
        .map_err(|source| failed(format!("{source}")));
    if let Err(error) = written {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    drop(file);

    std::fs::rename(&temporary, path).map_err(|source| {
        let _ = std::fs::remove_file(&temporary);
        failed(format!("could not replace {}: {source}", path.display()))
    })
}

fn file_name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || "file".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}
