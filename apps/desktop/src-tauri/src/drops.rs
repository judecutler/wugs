//! Files dropped onto the window. The webview has `File` objects, not paths,
//! so it streams each file here in chunks; they land in
//! `<project>/cache/drop/<id>/<file name>` and are handed to the same import
//! code the file pickers use. The staged copy is named for the dropped file
//! as far as the file system allows; the name as dropped goes along as the
//! label or title, so they read as they do for a picked file. The cache is
//! disposable: a finished, failed or cancelled drop removes its folder, a
//! drop nobody sent a piece of for ten minutes (the page reloaded) is ended
//! at the next drop, and folders a crash left behind are swept after an hour.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use wugs_model::ulid;

use crate::interchange::{self, Imported};
use crate::session::{self, Session};

pub const DROP_DIR: &str = "drop";
/// Leftovers younger than this may belong to a drop still being written.
const SWEEP_AFTER_SECS: u64 = 3600;
/// A drop with no piece for this long was abandoned (the page reloaded or
/// navigated); the next drop ends it.
const ABANDONED_AFTER: Duration = Duration::from_secs(600);
/// Larger than any recording wugs can import (a WAV tops out under 4 GiB);
/// a bigger declared size is refused before anything is written.
pub const MAX_BYTES: u64 = 8 << 30;

/// Extensions read as recordings (`convert::to_wav` decodes them); a file
/// whose first bytes say RIFF/WAVE is a recording whatever its name.
const RECORDING_EXTENSIONS: &[&str] = &["wav", "wave", "m4a", "mp4", "aac", "mp3", "flac", "ogg", "oga", "aif", "aiff", "aifc"];

struct Staging {
    file: File,
    root: PathBuf,
    dir: PathBuf,
    path: PathBuf,
    name: String,
    size: u64,
    written: u64,
    touched: Instant,
}

/// A dropped file, whole on disk, ready for import.
#[derive(Debug)]
pub struct Staged {
    /// The project's root (canonical) when the drop began.
    pub root: PathBuf,
    /// The drop's own folder; remove it once the import is done.
    pub dir: PathBuf,
    pub path: PathBuf,
    /// The file's name as dropped, for labels and titles; `path` may differ.
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Into the media bin, through the recording import.
    Recording,
    /// Through `interchange::import_file`, which says in words when it cannot read it.
    File,
}

pub fn route(path: &Path) -> Route {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if RECORDING_EXTENSIONS.contains(&ext.as_str()) || crate::convert::is_wav(path) {
        Route::Recording
    } else {
        Route::File
    }
}

/// The dropped file's name as a name this computer can create: whatever a
/// file system refuses is replaced, and Windows device names get a `_`.
pub fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']);
    let (stem, ext) = match cleaned.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() && e.chars().count() <= 16 => (s, Some(e)),
        _ => (cleaned, None),
    };
    // Long project paths plus a long name can pass Windows' path limit.
    let mut stem: String = stem.chars().take(120).collect();
    if stem.is_empty() || stem == "." || stem == ".." {
        stem = "Dropped file".into();
    }
    let device_stem = stem.split('.').next().unwrap_or("").trim_end().to_uppercase();
    let numbered = |prefix: &str| device_stem.strip_prefix(prefix).is_some_and(|rest| rest.chars().count() == 1 && rest.chars().all(|c| c.is_ascii_digit() || "¹²³".contains(c)));
    if ["CON", "PRN", "AUX", "NUL"].contains(&device_stem.as_str()) || numbered("COM") || numbered("LPT") {
        stem.insert(0, '_');
    }
    match ext {
        Some(e) => format!("{stem}.{e}"),
        None => stem,
    }
}

/// Drops being written, by id. Lives outside the `Project`, so a chunk never
/// waits on the session thread.
#[derive(Default)]
pub struct Drops(Mutex<HashMap<String, Staging>>);

impl Drops {
    fn table(&self) -> MutexGuard<'_, HashMap<String, Staging>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Start a drop of `size` bytes named `name` into the project at `root`.
    pub fn begin(&self, root: &Path, name: &str, size: u64) -> Result<String, String> {
        self.begin_at(root, name, size, Instant::now())
    }

    /// `begin` at `now`: drops with no piece since `ABANDONED_AFTER` before it end.
    pub fn begin_at(&self, root: &Path, name: &str, size: u64, now: Instant) -> Result<String, String> {
        if size > MAX_BYTES {
            return Err(format!("{name} is larger than 8 GB, more than any recording or file wugs can import"));
        }
        let root = root.canonicalize().map_err(|e| format!("{}: {e}", root.display()))?;
        let base = root.join("cache").join(DROP_DIR);
        fs::create_dir_all(&base).map_err(|e| format!("{}: {e}", base.display()))?;
        let mut table = self.table();
        let abandoned: Vec<String> = table.iter().filter(|(_, s)| now.saturating_duration_since(s.touched) > ABANDONED_AFTER).map(|(id, _)| id.clone()).collect();
        for id in abandoned {
            discard(table.remove(&id).expect("listed above"));
        }
        sweep(&base, |id| table.contains_key(id));
        let id = ulid().to_string();
        let dir = base.join(&id);
        fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(safe_name(name));
        let file = match File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                fs::remove_dir_all(&dir).ok();
                return Err(format!("{name}: {e}"));
            }
        };
        table.insert(id.clone(), Staging { file, root, dir, path, name: name.to_string(), size, written: 0, touched: now });
        Ok(id)
    }


    /// Append the next piece. A write error or more bytes than announced
    /// ends the drop and removes what was written.
    pub fn chunk(&self, id: &str, bytes: &[u8]) -> Result<(), String> {
        let mut table = self.table();
        let staging = table.get_mut(id).ok_or("that drop is no longer being copied")?;
        let over = staging.written + bytes.len() as u64 > staging.size;
        let result = if over { Err("more bytes arrived than the file holds".to_string()) } else { staging.file.write_all(bytes).map_err(|e| e.to_string()) };
        match result {
            Ok(()) => {
                staging.written += bytes.len() as u64;
                staging.touched = Instant::now();
                Ok(())
            }
            Err(e) => {
                let gone = table.remove(id).expect("present above");
                let name = gone.name.clone();
                discard(gone);
                Err(format!("{name} could not be copied into the project: {e}"))
            }
        }
    }

    /// The whole file arrived: close it and hand it over. A short file is
    /// removed and refused.
    pub fn finish(&self, id: &str) -> Result<Staged, String> {
        let staging = self.table().remove(id).ok_or("that drop is no longer being copied")?;
        let Staging { file, root, dir, path, name, size, written, .. } = staging;
        let synced = file.sync_all();
        drop(file);
        let problem = if written != size {
            Some(format!("{name} arrived incomplete ({written} of {size} bytes)"))
        } else {
            synced.err().map(|e| format!("{name}: {e}"))
        };
        if let Some(problem) = problem {
            fs::remove_dir_all(&dir).ok();
            return Err(problem);
        }
        Ok(Staged { root, dir, path, name })
    }

    /// Stop a drop and remove what was written. Unknown ids are ignored.
    pub fn cancel(&self, id: &str) {
        if let Some(gone) = self.table().remove(id) {
            discard(gone);
        }
    }
}

/// Close the file first: Windows cannot remove a folder holding an open file.
fn discard(gone: Staging) {
    drop(gone.file);
    fs::remove_dir_all(&gone.dir).ok();
}

/// Import a staged text or lexicon file, titled from the name it was dropped
/// with, in the same session-thread call that checks the project is still
/// the one it was dropped into.
pub fn import_file(session: &mut Session, staged: &Staged) -> Result<Imported, String> {
    session::require_root(session, &staged.root)?;
    let path = staged.path.to_str().ok_or_else(|| format!("{} is not a path wugs can read", staged.path.display()))?;
    interchange::import_file_named(session, path, &staged.name)
}

/// Remove drop folders a crash left: not `live`, and nothing in them
/// written for an hour.
pub fn sweep(base: &Path, live: impl Fn(&str) -> bool) {
    let Ok(entries) = fs::read_dir(base) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if live(&name) || !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = e.path();
        // The staged file's own time, since writing to it never touches the
        // folder's; an empty folder goes by its own.
        let files: Vec<PathBuf> = fs::read_dir(&path).into_iter().flatten().flatten().map(|f| f.path()).collect();
        let newest = if files.is_empty() { vec![path.clone()] } else { files }
            .iter()
            .filter_map(|p| p.metadata().and_then(|m| m.modified()).ok())
            .max();
        if newest.and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > SWEEP_AFTER_SECS) {
            fs::remove_dir_all(&path).ok();
        }
    }
}
