//! In-app recording: the webview streams PCM16 chunks here and they land in
//! a WAV under `<project>/recordings-pending/`. A pending recording is a
//! durable file, not cache: nothing in this module deletes captured audio
//! except `discard` and `discard_pending`, which the UI calls only after an
//! explicit, confirmed Discard. The open files live in a map outside the
//! `Project`, so a chunk never waits on the session thread.
//!
//! The header is patched to the current length every few seconds of audio,
//! so a crash or power loss leaves a valid WAV missing at most that much.
//! Whatever a crash leaves is repaired and offered back by `recover_list`.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use wugs_model::ulid;

pub const PENDING_DIR: &str = "recordings-pending";
const HEADER_LEN: u64 = 44;
/// RIFF sizes are u32, so a WAV cannot hold more than this much sample data.
const MAX_DATA: u64 = u32::MAX as u64 - 36;
/// Seconds of audio written between header patches (and fsyncs).
const PATCH_EVERY_SECS: u64 = 5;

struct Capture {
    /// `None` once finished: the header is final and the file is closed.
    file: Option<File>,
    path: PathBuf,
    root: PathBuf,
    block_align: u64,
    sample_rate: u32,
    channels: u16,
    data_len: u64,
    unpatched: u64,
}

/// A finished WAV, ready for `session::import_capture`.
#[derive(Debug)]
pub struct Recorded {
    pub path: PathBuf,
    pub root: PathBuf,
}

/// A recording a crash, close or failed import left in `recordings-pending/`.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub id: String,
    pub bytes: u64,
    pub duration_ms: f64,
    /// Milliseconds since the Unix epoch.
    pub modified_ms: u64,
}

pub struct Captures {
    map: Mutex<HashMap<String, Capture>>,
    max_data: u64,
}

impl Default for Captures {
    fn default() -> Captures {
        Captures::with_limit(MAX_DATA)
    }
}

fn header(sample_rate: u32, channels: u16, data_len: u64) -> [u8; HEADER_LEN as usize] {
    let block = channels * 2;
    let data = data_len as u32;
    let mut h = [0u8; HEADER_LEN as usize];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data).to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes());
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    h[28..32].copy_from_slice(&(sample_rate * block as u32).to_le_bytes());
    h[32..34].copy_from_slice(&block.to_le_bytes());
    h[34..36].copy_from_slice(&16u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}

/// Write the sizes for `data_len` bytes of samples and leave the cursor at
/// the end of the data, ready for the next append.
fn patch(file: &mut File, sample_rate: u32, channels: u16, data_len: u64, full_sync: bool) -> std::io::Result<()> {
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header(sample_rate, channels, data_len))?;
    file.seek(SeekFrom::Start(HEADER_LEN + data_len))?;
    if full_sync {
        file.sync_all()
    } else {
        file.sync_data()
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn pending_dir(root: &Path) -> PathBuf {
    root.join(PENDING_DIR)
}

impl Captures {
    /// A capture that refuses data past `max_data` bytes of samples.
    pub fn with_limit(max_data: u64) -> Captures {
        Captures { map: Mutex::default(), max_data }
    }

    fn map(&self) -> Result<MutexGuard<'_, HashMap<String, Capture>>, String> {
        self.map.lock().map_err(|_| "capture state poisoned".to_string())
    }

    /// How many takes are open (recording, or stopped and not yet saved,
    /// kept or discarded). The updater refuses to restart while any is.
    pub fn open_count(&self) -> Result<usize, String> {
        Ok(self.map()?.len())
    }

    /// Open a WAV under `root/recordings-pending/` and return its capture id.
    pub fn begin(&self, root: &Path, sample_rate: u32, channels: u16) -> Result<String, String> {
        if !(8_000..=384_000).contains(&sample_rate) || !(1..=2).contains(&channels) {
            return Err(format!("cannot record {channels} channel(s) at {sample_rate} Hz"));
        }
        let root = root.canonicalize().map_err(|e| format!("resolve {}: {e}", root.display()))?;
        // Held across the file's creation so a recovery scan never sees it unregistered.
        let mut map = self.map()?;
        let dir = pending_dir(&root);
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        let id = ulid();
        let path = dir.join(format!("{id}.wav"));
        let mut file = File::create_new(&path).map_err(|e| format!("create {}: {e}", path.display()))?;
        file.write_all(&header(sample_rate, channels, 0)).map_err(|e| format!("write {}: {e}", path.display()))?;
        let capture = Capture {
            file: Some(file),
            path,
            root,
            block_align: channels as u64 * 2,
            sample_rate,
            channels,
            data_len: 0,
            unpatched: 0,
        };
        map.insert(id.clone(), capture);
        Ok(id)
    }

    /// Append little-endian PCM16 frames. A failure leaves the file a
    /// whole-frame WAV of everything written so far.
    pub fn chunk(&self, id: &str, pcm: &[u8]) -> Result<(), String> {
        let mut map = self.map()?;
        let c = map.get_mut(id).ok_or_else(|| format!("no recording {id} in progress"))?;
        let Some(file) = c.file.as_mut() else {
            return Err(format!("recording {id} is already finished"));
        };
        if pcm.len() as u64 % c.block_align != 0 {
            return Err(format!("chunk of {} bytes is not whole {}-channel frames", pcm.len(), c.channels));
        }
        let room = (self.max_data - c.data_len) / c.block_align * c.block_align;
        let full = pcm.len() as u64 > room;
        let take = &pcm[..pcm.len().min(room as usize)];
        if let Err(e) = file.write_all(take) {
            // A partial write must not leave a torn frame behind.
            file.set_len(HEADER_LEN + c.data_len).ok();
            file.seek(SeekFrom::Start(HEADER_LEN + c.data_len)).ok();
            return Err(format!("write {}: {e}", c.path.display()));
        }
        c.data_len += take.len() as u64;
        c.unpatched += take.len() as u64;
        let due = c.unpatched >= c.sample_rate as u64 * c.block_align * PATCH_EVERY_SECS;
        if due || full {
            patch(file, c.sample_rate, c.channels, c.data_len, false).map_err(|e| format!("sync {}: {e}", c.path.display()))?;
            c.unpatched = 0;
        }
        if full {
            return Err("this recording reached the 4 GiB limit of a WAV file; everything up to here is kept".into());
        }
        Ok(())
    }

    /// Patch the header sizes and fsync. Calling it again returns the same
    /// file, so a failed import can be retried; the capture stays tracked
    /// until `forget` or `discard`.
    pub fn finish(&self, id: &str) -> Result<Recorded, String> {
        let mut map = self.map()?;
        let c = map.get_mut(id).ok_or_else(|| format!("no recording {id}"))?;
        if let Some(file) = c.file.as_mut() {
            patch(file, c.sample_rate, c.channels, c.data_len, true).map_err(|e| format!("finish {}: {e}", c.path.display()))?;
            c.file = None;
        }
        Ok(Recorded { path: c.path.clone(), root: c.root.clone() })
    }

    /// Stop tracking a capture whose audio now lives in the project.
    pub fn forget(&self, id: &str) -> Result<(), String> {
        self.map()?.remove(id);
        Ok(())
    }

    /// The one deletion of captured audio: the user confirmed Discard.
    pub fn discard(&self, id: &str) -> Result<(), String> {
        if let Some(c) = self.map()?.remove(id) {
            drop(c.file);
            fs::remove_file(&c.path).map_err(|e| format!("remove {}: {e}", c.path.display()))?;
        }
        Ok(())
    }

    /// Stop tracking one capture, header patched, file kept: the user chose
    /// to keep it for later. It comes back as a recovered recording.
    pub fn release(&self, id: &str) -> Result<(), String> {
        let mut map = self.map()?;
        if let Some(c) = map.get_mut(id) {
            if let Some(file) = c.file.as_mut() {
                patch(file, c.sample_rate, c.channels, c.data_len, true).map_err(|e| format!("finish {}: {e}", c.path.display()))?;
            }
        }
        map.remove(id);
        Ok(())
    }

    /// Release every tracked capture, files kept. Called when the page that
    /// owned them is gone (a reload).
    pub fn release_all(&self) -> Result<usize, String> {
        let mut map = self.map()?;
        let n = map.len();
        for c in map.values_mut() {
            if let Some(file) = c.file.as_mut() {
                patch(file, c.sample_rate, c.channels, c.data_len, true).ok();
            }
        }
        map.clear();
        Ok(n)
    }

    /// Recordings in `recordings-pending/` that no live capture holds, each
    /// repaired to a valid WAV first. Files that are not recognisably ours
    /// are left untouched and unlisted.
    pub fn recover_list(&self, root: &Path) -> Result<Vec<Pending>, String> {
        let root = root.canonicalize().map_err(|e| format!("resolve {}: {e}", root.display()))?;
        let map = self.map()?;
        let held: HashSet<&Path> = map.values().map(|c| c.path.as_path()).collect();
        let Ok(entries) = fs::read_dir(pending_dir(&root)) else { return Ok(Vec::new()) };
        let mut found = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(id) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".wav")).filter(|id| valid_id(id)) else { continue };
            if held.contains(path.as_path()) {
                continue;
            }
            let Ok(Some(info)) = repair(&path) else { continue };
            if info.data_len == 0 {
                continue;
            }
            let modified_ms = entry.metadata().and_then(|m| m.modified()).ok().and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
            found.push(Pending {
                id: id.to_string(),
                bytes: HEADER_LEN + info.data_len,
                duration_ms: info.data_len as f64 * 1000.0 / (info.sample_rate as u64 * info.block_align) as f64,
                modified_ms,
            });
        }
        found.sort_by(|a, b| a.modified_ms.cmp(&b.modified_ms).then_with(|| a.id.cmp(&b.id)));
        Ok(found)
    }

    /// Repair one recovered recording and hand it to the import path. The
    /// file stays until the import commits.
    pub fn recover_take(&self, root: &Path, id: &str) -> Result<Recorded, String> {
        let (root, path) = self.recovered_path(root, id)?;
        match repair(&path) {
            Ok(Some(_)) => Ok(Recorded { path, root }),
            Ok(None) => Err(format!("{id}.wav is not a recording this app can read")),
            Err(e) => Err(format!("repair {}: {e}", path.display())),
        }
    }

    /// Delete a recovered recording. The caller has the user's confirmation.
    pub fn discard_pending(&self, root: &Path, id: &str) -> Result<(), String> {
        let (_, path) = self.recovered_path(root, id)?;
        fs::remove_file(&path).map_err(|e| format!("remove {}: {e}", path.display()))
    }

    fn recovered_path(&self, root: &Path, id: &str) -> Result<(PathBuf, PathBuf), String> {
        if !valid_id(id) {
            return Err(format!("{id:?} is not a recording id"));
        }
        let root = root.canonicalize().map_err(|e| format!("resolve {}: {e}", root.display()))?;
        let path = pending_dir(&root).join(format!("{id}.wav"));
        if self.map()?.values().any(|c| c.path == path) {
            return Err("that recording is still being taken or saved in this window".into());
        }
        if !path.is_file() {
            return Err(format!("no pending recording {id}"));
        }
        Ok((root, path))
    }
}

struct Repaired {
    sample_rate: u32,
    block_align: u64,
    data_len: u64,
}

/// Make `path` a valid PCM16 WAV: trim a torn trailing frame and write the
/// sizes from the file's length. `None` when the first 44 bytes are not a
/// header this app wrote.
fn repair(path: &Path) -> std::io::Result<Option<Repaired>> {
    let mut file = File::options().read(true).write(true).open(path)?;
    let len = file.metadata()?.len();
    let mut h = [0u8; HEADER_LEN as usize];
    if len < HEADER_LEN || file.read_exact(&mut h).is_err() {
        return Ok(None);
    }
    let u16_at = |o: usize| u16::from_le_bytes([h[o], h[o + 1]]);
    let sample_rate = u32::from_le_bytes([h[24], h[25], h[26], h[27]]);
    let channels = u16_at(22);
    let ours = &h[0..4] == b"RIFF"
        && &h[8..16] == b"WAVEfmt "
        && &h[36..40] == b"data"
        && u16_at(20) == 1
        && u16_at(34) == 16
        && (1..=2).contains(&channels)
        && (8_000..=384_000).contains(&sample_rate);
    if !ours {
        return Ok(None);
    }
    let block_align = channels as u64 * 2;
    let data_len = ((len - HEADER_LEN) / block_align * block_align).min(MAX_DATA / block_align * block_align);
    if len != HEADER_LEN + data_len {
        file.set_len(HEADER_LEN + data_len)?;
    }
    if h != header(sample_rate, channels, data_len) {
        patch(&mut file, sample_rate, channels, data_len, true)?;
    } else {
        file.sync_all()?;
    }
    Ok(Some(Repaired { sample_rate, block_align, data_len }))
}

/// Whether `page` is the bundled UI: `tauri://localhost` (macOS, Linux),
/// `http(s)://tauri.localhost` (Windows), or the dev server in debug builds.
/// Scheme, host and port must all match; a path or query does not matter.
pub fn is_app_origin(page: &tauri::Url, dev_url: Option<&tauri::Url>) -> bool {
    let bundled = match (page.scheme(), page.host_str()) {
        ("tauri", Some("localhost")) => true,
        ("http" | "https", Some("tauri.localhost")) => true,
        _ => false,
    };
    bundled || dev_url.is_some_and(|d| d.origin() == page.origin())
}
