//! BagIt 1.0 (RFC 8493) writing and verification, SHA-256 only.
//!
//! Choices within the RFC: tag files use LF line endings and UTF-8; manifest
//! paths use `/` and percent-encode only what §2.1.3 requires (CR, LF, `%`);
//! manifest lines are sorted by path so two seals of one state differ only
//! in what changed.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use crate::sha256::Sha256;

pub const MANIFEST: &str = "manifest-sha256.txt";
pub const TAG_MANIFEST: &str = "tagmanifest-sha256.txt";
const BAGIT_TXT: &str = "BagIt-Version: 1.0\nTag-File-Character-Encoding: UTF-8\n";

/// A bag being written into `root`. Payload paths are relative to `data/`.
pub struct BagWriter {
    root: PathBuf,
    /// bag-relative path -> (sha256 hex, bytes)
    payload: BTreeMap<String, (String, u64)>,
}

impl BagWriter {
    pub fn new(root: &Path) -> Result<BagWriter, String> {
        fs::create_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
        Ok(BagWriter { root: root.to_path_buf(), payload: BTreeMap::new() })
    }

    pub fn put(&mut self, rel: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.payload_path(rel)?;
        write_synced(&path, bytes)?;
        let mut h = Sha256::default();
        h.update(bytes);
        self.payload.insert(format!("data/{rel}"), (h.finish_hex(), bytes.len() as u64));
        Ok(())
    }

    /// Streamed, so media never has to fit in memory.
    pub fn copy(&mut self, rel: &str, src: &Path) -> Result<(), String> {
        let path = self.payload_path(rel)?;
        let mut input = File::open(src).map_err(|e| format!("{}: {e}", src.display()))?;
        let mut output = File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let (hex, bytes) = hash_stream(&mut input, Some(&mut output))?;
        output.sync_all().map_err(|e| format!("{}: {e}", path.display()))?;
        self.payload.insert(format!("data/{rel}"), (hex, bytes));
        Ok(())
    }

    /// `rel` comes partly from synced data (text ids), so it must stay a
    /// plain relative path: every `/`-separated part a single normal name.
    fn payload_path(&self, rel: &str) -> Result<PathBuf, String> {
        let plain = |seg: &str| {
            let mut c = Path::new(seg).components();
            !seg.is_empty()
                && !seg.contains(['\\', '\0'])
                && matches!(c.next(), Some(Component::Normal(n)) if n == std::ffi::OsStr::new(seg))
                && c.next().is_none()
        };
        if !rel.split('/').all(plain) {
            return Err(format!("{rel:?}: not a plain relative payload path"));
        }
        let path = self.root.join("data").join(rel);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        Ok(path)
    }

    pub fn payload_files(&self) -> usize {
        self.payload.len()
    }

    pub fn payload_bytes(&self) -> u64 {
        self.payload.values().map(|(_, n)| n).sum()
    }

    /// Write bagit.txt, bag-info.txt (Payload-Oxum appended), the payload
    /// manifest, and the tag manifest over the other three.
    pub fn finish(self, info: &[(&str, String)]) -> Result<(), String> {
        let mut bag_info = String::new();
        for (k, v) in info {
            bag_info.push_str(&format!("{k}: {}\n", fold(v)));
        }
        bag_info.push_str(&format!("Payload-Oxum: {}.{}\n", self.payload_bytes(), self.payload_files()));
        let manifest = manifest_text(self.payload.iter().map(|(p, (h, _))| (p.as_str(), h.as_str())));

        let mut tags = BTreeMap::new();
        for (name, body) in [("bagit.txt", BAGIT_TXT), ("bag-info.txt", &bag_info), (MANIFEST, &manifest)] {
            write_synced(&self.root.join(name), body.as_bytes())?;
            tags.insert(name.to_string(), crate::sha256::hex(body.as_bytes()));
        }
        let tag_manifest = manifest_text(tags.iter().map(|(p, h)| (p.as_str(), h.as_str())));
        write_synced(&self.root.join(TAG_MANIFEST), tag_manifest.as_bytes())
    }
}

/// RFC 8493 §2.2.2: a value continues onto the next line only after
/// whitespace, so an embedded line break must never start a new element.
fn fold(v: &str) -> String {
    v.replace("\r\n", "\n").replace('\r', "\n").replace('\n', "\n ")
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let err = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut f = File::create(path).map_err(err)?;
    f.write_all(bytes).map_err(err)?;
    f.sync_all().map_err(err)
}

/// fsync `dir` and every directory below it, so the entries written into
/// them survive a crash before the bag is renamed into place.
pub fn sync_tree(dir: &Path) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            sync_tree(&entry.path())?;
        }
    }
    sync_dir(dir)
}

pub fn sync_dir(dir: &Path) -> Result<(), String> {
    // std cannot open a directory for fsync on Windows.
    if cfg!(unix) {
        File::open(dir).and_then(|d| d.sync_all()).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    Ok(())
}

fn manifest_text<'a>(entries: impl Iterator<Item = (&'a str, &'a str)>) -> String {
    entries.map(|(path, hash)| format!("{hash}  {}\n", encode_path(path))).collect()
}

fn encode_path(p: &str) -> String {
    p.replace('%', "%25").replace('\r', "%0D").replace('\n', "%0A")
}

fn decode_path(p: &str) -> String {
    p.replace("%0D", "\r").replace("%0d", "\r").replace("%0A", "\n").replace("%0a", "\n").replace("%25", "%")
}

fn hash_stream(input: &mut impl Read, mut copy_to: Option<&mut File>) -> Result<(String, u64), String> {
    let mut h = Sha256::default();
    let mut buf = vec![0u8; 1 << 16];
    let mut total = 0u64;
    loop {
        let n = input.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        if let Some(out) = copy_to.as_mut() {
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
        total += n as u64;
    }
    if let Some(out) = copy_to {
        out.sync_all().map_err(|e| e.to_string())?;
    }
    Ok((h.finish_hex(), total))
}

fn hash_file(path: &Path) -> Result<(String, u64), String> {
    let mut f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    hash_stream(&mut f, None)
}

fn read_tag(bag: &Path, name: &str) -> Result<String, String> {
    fs::read_to_string(bag.join(name)).map_err(|e| format!("{name}: {e}"))
}

/// Parse a manifest into bag-relative path -> hex digest.
fn parse_manifest(name: &str, text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (hash, path) = line
            .split_once([' ', '\t'])
            .ok_or_else(|| format!("{name} line {}: no path", i + 1))?;
        let path = decode_path(path.trim_start_matches([' ', '\t']));
        if path.starts_with('/') || path.split('/').any(|seg| seg == "..") {
            return Err(format!("{name} line {}: path escapes the bag: {path}", i + 1));
        }
        out.insert(path, hash.to_ascii_lowercase());
    }
    Ok(out)
}

/// Every regular file under `dir`, as `/`-joined paths relative to `base`.
/// Symlinks are never followed; returns how many were skipped.
pub fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) -> Result<usize, String> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    let mut symlinks = 0;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        if kind.is_symlink() {
            symlinks += 1;
        } else if kind.is_dir() {
            symlinks += walk(base, &path, out)?;
        } else {
            let rel = path.strip_prefix(base).map_err(|e| e.to_string())?;
            let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            out.push(parts.join("/"));
        }
    }
    Ok(symlinks)
}

/// Check a bag: declaration, every payload file listed and matching its
/// digest, no listed file missing, Payload-Oxum, and the tag manifest.
/// The error names the first file that fails.
pub fn verify(bag: &Path) -> Result<(), String> {
    let decl = read_tag(bag, "bagit.txt")?;
    if !decl.lines().any(|l| l.trim() == "BagIt-Version: 1.0") {
        return Err("bagit.txt: not a BagIt 1.0 bag".into());
    }

    let manifest = parse_manifest(MANIFEST, &read_tag(bag, MANIFEST)?)?;
    let mut on_disk = Vec::new();
    if walk(bag, &bag.join("data"), &mut on_disk)? > 0 {
        return Err("data/: contains a symlink; a bag holds only regular files".into());
    }
    on_disk.sort();
    for path in &on_disk {
        if !manifest.contains_key(path) {
            return Err(format!("{path}: payload file not in {MANIFEST}"));
        }
    }
    let (mut octets, mut streams) = (0u64, 0usize);
    for (path, want) in &manifest {
        if !path.starts_with("data/") {
            return Err(format!("{MANIFEST}: {path} is not under data/"));
        }
        let full = bag.join(path);
        if !full.is_file() {
            return Err(format!("{path}: listed in {MANIFEST} but missing"));
        }
        let (got, n) = hash_file(&full)?;
        if &got != want {
            return Err(format!("{path}: sha256 mismatch (manifest {want}, file {got})"));
        }
        octets += n;
        streams += 1;
    }

    let info = read_tag(bag, "bag-info.txt")?;
    if let Some(oxum) = info.lines().find_map(|l| l.strip_prefix("Payload-Oxum:")) {
        let want = format!("{octets}.{streams}");
        if oxum.trim() != want {
            return Err(format!("bag-info.txt: Payload-Oxum {} but payload is {want}", oxum.trim()));
        }
    }

    let tags = parse_manifest(TAG_MANIFEST, &read_tag(bag, TAG_MANIFEST)?)?;
    for required in ["bagit.txt", "bag-info.txt", MANIFEST] {
        if !tags.contains_key(required) {
            return Err(format!("{TAG_MANIFEST}: does not list {required}"));
        }
    }
    for (path, want) in &tags {
        let (got, _) = hash_file(&bag.join(path)).map_err(|e| format!("{path}: {e}"))?;
        if &got != want {
            return Err(format!("{path}: sha256 mismatch (tag manifest {want}, file {got})"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_paths_must_be_plain_relative_names() {
        let root = std::env::temp_dir().join(format!("wugs-seal-unit-{}", wugs_model::ulid()));
        fs::create_dir(&root).unwrap();
        let mut bag = BagWriter::new(&root.join("bag")).unwrap();
        for rel in ["../x", "a/../../x", "/abs", "a//b", "./a", "a/.", "", "a\\..\\x", "a/"] {
            assert!(bag.put(rel, b"x").is_err(), "{rel:?} accepted");
        }
        bag.put("exports/elan/txt_ok.eaf", b"x").unwrap();
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1, "nothing written beside the bag");
        assert_eq!(bag.payload_files(), 1);
        fs::remove_dir_all(&root).unwrap();
    }
}
