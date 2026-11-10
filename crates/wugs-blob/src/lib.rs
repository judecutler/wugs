//! Content-addressed blob store (DATA-MODEL §9).
//!
//! Media, consent scans and keyboard files live under `<project>/blobs/<hex>`,
//! named by their BLAKE3 hash, written once and never modified. Entities store
//! the ref `blake3:<64 lowercase hex>`. Garbage collection is mark-and-sweep:
//! the caller marks from the op log (`Project::blob_refs`) and
//! `BlobStore::gc` sweeps. `blobs/` holds nothing but blobs, since sealing
//! copies it into the bag; the temp dir and the GC lock live in `cache/`.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use wugs_model::BLOB_REF_PREFIX as PREFIX;

/// Validate a blob ref and return its hex digest. Part of the archive format.
pub use wugs_model::parse_blob_ref as parse_ref;

fn is_hex_digest(s: &str) -> bool {
    parse_ref(&format!("{PREFIX}{s}")).is_some()
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GcReport {
    /// blobs in the live set
    pub kept: usize,
    pub swept: usize,
    pub swept_bytes: u64,
    /// unreferenced blobs newer than the grace period
    pub spared_young: usize,
    /// entries in `blobs/` that are not blobs (left alone)
    pub ignored: usize,
    /// stale leftovers removed from `cache/blob-tmp/`
    pub tmp_cleared: usize,
}

pub struct BlobStore {
    dir: PathBuf,
    /// same filesystem as `dir`, so the final rename is atomic
    tmp: PathBuf,
    /// gc holds it exclusively while sweeping; put holds it shared while
    /// it makes its blob present and fresh
    lock: PathBuf,
}

impl BlobStore {
    pub fn open(project_root: &Path) -> io::Result<BlobStore> {
        let dir = project_root.join("blobs");
        let cache = project_root.join("cache");
        let tmp = cache.join("blob-tmp");
        fs::create_dir_all(&dir)?;
        fs::create_dir_all(&tmp)?;
        Ok(BlobStore { dir, tmp, lock: cache.join("blob-gc.lock") })
    }

    fn lock_file(&self) -> io::Result<File> {
        File::options().create(true).truncate(false).write(true).open(&self.lock)
    }

    /// Store the bytes and return their ref. The blob is fresh on return:
    /// commit the ref within the GC grace period, or put again first. Core
    /// intents cannot check that a referenced blob exists.
    pub fn put_bytes(&self, bytes: &[u8]) -> io::Result<String> {
        self.put_reader(&mut &*bytes)
    }

    /// `put_bytes` for a file, streamed. The same grace-period rule applies.
    pub fn put_file(&self, path: &Path) -> io::Result<String> {
        self.put_reader(&mut File::open(path)?)
    }

    fn put_reader(&self, src: &mut dyn Read) -> io::Result<String> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let tmp_path = self.tmp.join(format!(
            "{}-{nanos}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let result = self.stream_into(src, &tmp_path);
        if result.is_err() {
            fs::remove_file(&tmp_path).ok();
        }
        result
    }

    fn stream_into(&self, src: &mut dyn Read, tmp_path: &Path) -> io::Result<String> {
        let mut out = File::options().write(true).create_new(true).open(tmp_path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n])?;
        }
        out.sync_all()?;
        drop(out);
        let hex = hasher.finalize().to_hex().to_string();
        let target = self.dir.join(&hex);
        let lock = self.lock_file()?;
        lock.lock_shared()?;
        // An existing blob keeps its file (same bytes) and gets a fresh
        // mtime so GC spares it until the caller commits the ref. When the
        // refresh fails for any reason the rename puts the bytes in place,
        // and fails itself if the name is taken by something not a file.
        if refresh(&target).is_ok() {
            fs::remove_file(tmp_path)?;
        } else {
            fs::rename(tmp_path, &target)?;
            if let Ok(d) = File::open(&self.dir) {
                d.sync_all().ok();
            }
        }
        drop(lock);
        Ok(format!("{PREFIX}{hex}"))
    }

    /// Path of a stored blob; `None` for a malformed ref or a missing file.
    pub fn path_of(&self, r: &str) -> Option<PathBuf> {
        let p = self.dir.join(parse_ref(r)?);
        p.is_file().then_some(p)
    }

    pub fn has(&self, r: &str) -> bool {
        self.path_of(r).is_some()
    }

    /// Rehash the stored file and compare with its name. `false` for a
    /// corrupted, missing or malformed ref.
    pub fn verify(&self, r: &str) -> io::Result<bool> {
        let Some(p) = self.path_of(r) else { return Ok(false) };
        let hex = parse_ref(r).unwrap_or_default();
        let mut f = match File::open(p) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        };
        let mut hasher = blake3::Hasher::new();
        io::copy(&mut f, &mut hasher)?;
        Ok(hasher.finalize().to_hex().as_str() == hex)
    }

    /// Sweep blobs outside `live` that are older than `grace`, and stale
    /// temp leftovers. Entries that are not blobs are left alone. `live`
    /// must come from the op log (`Project::blob_refs`), never from the
    /// projection, which can trail the log after a crash.
    pub fn gc(&self, live: impl IntoIterator<Item = String>, grace: Duration) -> io::Result<GcReport> {
        let live: HashSet<String> = live
            .into_iter()
            .filter_map(|r| parse_ref(&r).map(str::to_string))
            .collect();
        let lock = self.lock_file()?;
        lock.lock()?;
        let now = SystemTime::now();
        let mut rep = GcReport::default();
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = entry.metadata()?;
            if !meta.is_file() || !is_hex_digest(&name) {
                rep.ignored += 1;
                continue;
            }
            if live.contains(&name) {
                rep.kept += 1;
            } else if is_stale(&meta, now, grace) {
                fs::remove_file(entry.path())?;
                rep.swept += 1;
                rep.swept_bytes += meta.len();
            } else {
                rep.spared_young += 1;
            }
        }
        for entry in fs::read_dir(&self.tmp)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_file() && is_stale(&meta, now, grace) {
                fs::remove_file(entry.path())?;
                rep.tmp_cleared += 1;
            }
        }
        drop(lock);
        Ok(rep)
    }
}

fn refresh(target: &Path) -> io::Result<()> {
    let f = File::open(target)?;
    if !f.metadata()?.is_file() {
        return Err(io::Error::other("not a regular file"));
    }
    f.set_modified(SystemTime::now())
}

/// A future mtime (clock skew) counts as young.
fn is_stale(meta: &fs::Metadata, now: SystemTime, grace: Duration) -> bool {
    meta.modified()
        .ok()
        .and_then(|m| now.duration_since(m).ok())
        .is_some_and(|age| age >= grace)
}
