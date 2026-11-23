//! Self-contained kit files (`docs/design/BUNDLES.md` §3): a `.wugskit` is a
//! zip holding a kit's `kit.toml`, its `[examples]` folder, and optionally
//! the files of packages it requires under `packages/<type>/<id>/<version>/`,
//! with `files.sha256` listing every other entry in the canonical form
//! (`wugs_package::canonical_file_list`).
//!
//! Opening one treats it as untrusted input: it is extracted into a staging
//! folder, never in place; every entry name is a relative path of safe parts
//! (no `..`, absolute or drive paths, backslashes, Windows device names,
//! symlinks, or two names that differ only in case); the file, each entry,
//! the total and the unpacked-to-packed ratio are capped; and any broken
//! rule refuses the whole file, naming the entry and the rule. Hashes in the
//! file catch damage, not tampering (that is §6's signatures).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use wugs_package::ListedFile;

/// The kit's manifest, at the root of the file.
pub const KIT_FILE: &str = "kit.toml";
/// Every other entry, in canonical file-list form.
pub const LIST_FILE: &str = "files.sha256";
/// Where required packages' files go inside the file.
pub const PACKAGES: &str = "packages";

/// Caps on what a file may unpack to.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The `.wugskit` itself.
    pub file: u64,
    /// One entry, unpacked.
    pub entry: u64,
    /// Every entry together, unpacked.
    pub total: u64,
    /// Unpacked size over packed size, for entries over a megabyte (zip bombs).
    pub ratio: u64,
    pub entries: usize,
}

pub const LIMITS: Limits = Limits { file: 8 << 30, entry: 4 << 30, total: 16 << 30, ratio: 200, entries: 20_000 };

/// A package the file carries, as found in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Included {
    pub kind: String,
    pub id: String,
    pub version: String,
    /// The SHA-256 of its file list, which the kit's requirement pins.
    pub files_sha256: String,
    pub files: usize,
    pub bytes: u64,
}

/// A file opened into its staging folder.
#[derive(Debug)]
pub struct Opened {
    pub kit_toml: String,
    pub kit: wugs_kit::Kit,
    /// The staging folder holding every entry.
    pub staged: PathBuf,
    /// Every entry but `files.sha256`, with size and hash.
    pub files: Vec<ListedFile>,
    /// Required packages the file carries, each matching its pin.
    pub included: Vec<Included>,
    /// Required packages it does not carry (fetched by reference later).
    pub missing: Vec<wugs_kit::Requirement>,
    /// Whether the `[examples]` folder is in the file.
    pub examples: bool,
    /// Whether `files.sha256` was present (and every entry matched it).
    pub listed: bool,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether an entry name is one this format allows: `/`-separated parts as
/// `wugs_kit::valid_bundle_path` allows them, no leading `/`, no `\`.
pub fn valid_entry(name: &str) -> bool {
    !name.starts_with('/') && !name.contains('\\') && !name.contains(':') && wugs_kit::valid_bundle_path(name)
}

fn refuse<T>(entry: &str, rule: &str) -> Result<T, String> {
    Err(format!("This kit file is refused: «{entry}» {rule}. Nothing was installed."))
}

/// Open `path` into `staging` (which must not exist yet; it is created).
/// On any error the staging folder is removed.
pub fn open(path: &Path, staging: &Path, limits: Limits) -> Result<Opened, String> {
    let result = open_inner(path, staging, limits);
    if result.is_err() {
        fs::remove_dir_all(staging).ok();
    }
    result
}

fn open_inner(path: &Path, staging: &Path, limits: Limits) -> Result<Opened, String> {
    let size = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?.len();
    if size > limits.file {
        return Err(format!("This kit file is {} MB, more than the {} MB a kit file may be.", size >> 20, limits.file >> 20));
    }
    let f = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| format!("This is not a kit file (not a readable zip: {e})."))?;
    if zip.len() > limits.entries {
        return Err(format!("This kit file has {} entries, more than the {} allowed.", zip.len(), limits.entries));
    }
    fs::create_dir(staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    let mut seen: HashSet<String> = HashSet::new();
    let mut total = 0u64;
    let mut files: Vec<ListedFile> = Vec::new();
    let mut list_text: Option<String> = None;
    for i in 0..zip.len() {
        let mut e = zip.by_index(i).map_err(|err| format!("This kit file is damaged: {err}."))?;
        let name = e.name().map_err(|err| format!("This kit file has an entry whose name does not read: {err}."))?.to_string();
        if !valid_entry(&name) {
            return refuse(&name, "is not a plain relative path (letters, digits, '.', '-' and '_' in each part, no '..')");
        }
        if e.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return refuse(&name, "is a symbolic link");
        }
        let folded = name.trim_end_matches('/').to_lowercase();
        if !seen.insert(folded) {
            return refuse(&name, "has the same name as another entry apart from case");
        }
        if e.is_dir() {
            fs::create_dir_all(staging.join(name.trim_end_matches('/'))).map_err(|err| err.to_string())?;
            continue;
        }
        let declared = e.size();
        if declared > limits.entry {
            return refuse(&name, "is larger than an entry may be");
        }
        if declared > 1 << 20 && e.compressed_size() > 0 && declared / e.compressed_size() > limits.ratio {
            return refuse(&name, "unpacks to far more than it takes in the file");
        }
        total += declared;
        if total > limits.total {
            return refuse(&name, "takes the file past the most a kit file may unpack to");
        }
        let target = staging.join(&name);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        // Never trust the declared size: read one byte past it to catch a lie.
        let mut out = fs::File::create(&target).map_err(|err| format!("{}: {err}", target.display()))?;
        let mut hasher = Sha256::new();
        let mut written = 0u64;
        let mut buf = vec![0u8; 64 * 1024];
        let mut limited = (&mut e).take(declared + 1);
        loop {
            let n = limited.read(&mut buf).map_err(|err| format!("«{name}» cannot be read: {err}"))?;
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > declared {
                return refuse(&name, "holds more than it says it does");
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|err| err.to_string())?;
        }
        let hash: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if name == LIST_FILE {
            list_text = Some(fs::read_to_string(&target).map_err(|_| format!("{LIST_FILE} is not text"))?);
            continue;
        }
        files.push(ListedFile { path: name, size: written, sha256: hash });
    }
    let kit_toml = fs::read_to_string(staging.join(KIT_FILE)).map_err(|_| format!("This kit file has no {KIT_FILE} at its top."))?;
    let kit = wugs_kit::parse(&kit_toml).map_err(|p| format!("The kit inside does not read: {}", p.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("; ")))?;
    let listed = match &list_text {
        Some(t) => {
            let mut by_path: BTreeMap<&str, &ListedFile> = files.iter().map(|f| (f.path.as_str(), f)).collect();
            for line in t.lines().filter(|l| !l.is_empty()) {
                let mut parts = line.splitn(3, ' ');
                let (Some(hash), Some(size), Some(p)) = (parts.next(), parts.next(), parts.next()) else {
                    return Err(format!("{LIST_FILE} has a line that does not read: {line}"));
                };
                match by_path.remove(p) {
                    Some(f) if f.sha256 == hash && f.size.to_string() == size => {}
                    Some(_) => return refuse(p, "is not what the file's own list says (damaged in transit?)"),
                    None => return refuse(p, "is listed but not in the file"),
                }
            }
            if let Some(p) = by_path.keys().next() {
                return refuse(p, &format!("is in the file but not in {LIST_FILE}"));
            }
            true
        }
        None => false,
    };
    // Required packages the file carries, each checked against its pin.
    let requires = kit.bundle.as_ref().map(|b| b.requires.clone()).unwrap_or_default();
    let mut groups: BTreeMap<(String, String, String), Vec<ListedFile>> = BTreeMap::new();
    for f in &files {
        let Some(rest) = f.path.strip_prefix(&format!("{PACKAGES}/")) else { continue };
        let parts: Vec<&str> = rest.splitn(4, '/').collect();
        if parts.len() < 4 {
            return refuse(&f.path, "is not inside packages/<type>/<id>/<version>/");
        }
        groups.entry((parts[0].into(), parts[1].into(), parts[2].into())).or_default().push(ListedFile { path: parts[3].to_string(), size: f.size, sha256: f.sha256.clone() });
    }
    let mut included = Vec::new();
    for ((kind, id, version), list) in groups {
        let where_ = format!("{PACKAGES}/{kind}/{id}/{version}/");
        let Some(req) = requires.iter().find(|r| r.kind == kind && r.id == id && r.version == version) else {
            return refuse(&where_, "is a package the kit does not require");
        };
        let hash = wugs_package::file_list_sha256(&list);
        if hash != req.files_sha256 {
            return refuse(&where_, "does not match the files_sha256 the kit pins for it");
        }
        included.push(Included { kind, id, version, files_sha256: hash, files: list.len(), bytes: list.iter().map(|f| f.size).sum() });
    }
    let missing = requires.into_iter().filter(|r| !included.iter().any(|i| i.kind == r.kind && i.id == r.id && i.version == r.version)).collect();
    let examples = kit.bundle.as_ref().and_then(|b| b.examples.as_deref()).is_some_and(|ex| {
        let ex = ex.trim_end_matches('/');
        files.iter().any(|f| f.path.starts_with(&format!("{ex}/")))
    });
    Ok(Opened { kit_toml, kit, staged: staging.to_path_buf(), files, included, missing, examples, listed })
}

/// What `write` put in the file.
#[derive(Debug, PartialEq, Eq)]
pub struct Written {
    pub files: usize,
    pub bytes: u64,
    pub packages: usize,
}

/// Every regular file under `dir`, as `(relative path, absolute path)`;
/// a symlink or a name the format refuses stops the write.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let rel = format!("{prefix}{name}");
        let kind = e.file_type().map_err(|err| err.to_string())?;
        if kind.is_symlink() {
            return Err(format!("{} is a symbolic link; a kit file holds only real files", e.path().display()));
        }
        if !valid_entry(&rel) {
            return Err(format!("{rel} has a name a kit file cannot hold (letters, digits, '.', '-' and '_' only)"));
        }
        if kind.is_dir() {
            walk(&e.path(), &format!("{rel}/"), out)?;
        } else {
            out.push((rel, e.path()));
        }
    }
    Ok(())
}

/// Write a `.wugskit` at `out` (which must not exist): `kit_toml`, the
/// folder `examples` (written under the kit's `[examples]` path) when given,
/// and each `(requirement, folder)` package's files under
/// `packages/<type>/<id>/<version>/`, each checked against its pin first.
pub fn write(out: &Path, kit_toml: &str, examples: Option<&Path>, packages: &[(wugs_kit::Requirement, PathBuf)]) -> Result<Written, String> {
    let kit = wugs_kit::parse(kit_toml).map_err(|p| format!("The kit does not read: {}", p.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("; ")))?;
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    if let Some(dir) = examples {
        let at = kit.bundle.as_ref().and_then(|b| b.examples.clone()).ok_or("The kit names no [examples] folder.")?;
        walk(dir, &format!("{}/", at.trim_end_matches('/')), &mut entries)?;
    }
    for (req, dir) in packages {
        let mut files = Vec::new();
        walk(dir, "", &mut files)?;
        let list: Vec<ListedFile> = files
            .iter()
            .map(|(rel, abs)| {
                let bytes = fs::read(abs).map_err(|e| format!("{}: {e}", abs.display()))?;
                Ok(ListedFile { path: rel.clone(), size: bytes.len() as u64, sha256: sha256_hex(&bytes) })
            })
            .collect::<Result<_, String>>()?;
        if wugs_package::file_list_sha256(&list) != req.files_sha256 {
            return Err(format!("The files of {} {} {} on this computer do not match what the kit pins.", req.kind, req.id, req.version));
        }
        let prefix = format!("{PACKAGES}/{}/{}/{}/", req.kind, req.id, req.version);
        entries.extend(files.into_iter().map(|(rel, abs)| (format!("{prefix}{rel}"), abs)));
    }
    let file = fs::OpenOptions::new().write(true).create_new(true).open(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let result = (|| {
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o644);
        let mut listed = vec![ListedFile { path: KIT_FILE.into(), size: kit_toml.len() as u64, sha256: sha256_hex(kit_toml.as_bytes()) }];
        zip.start_file(KIT_FILE, opts).map_err(|e| e.to_string())?;
        zip.write_all(kit_toml.as_bytes()).map_err(|e| e.to_string())?;
        let mut bytes = kit_toml.len() as u64;
        for (rel, abs) in &entries {
            let data = fs::read(abs).map_err(|e| format!("{}: {e}", abs.display()))?;
            zip.start_file(rel.as_str(), opts).map_err(|e| e.to_string())?;
            zip.write_all(&data).map_err(|e| e.to_string())?;
            bytes += data.len() as u64;
            listed.push(ListedFile { path: rel.clone(), size: data.len() as u64, sha256: sha256_hex(&data) });
        }
        zip.start_file(LIST_FILE, opts).map_err(|e| e.to_string())?;
        zip.write_all(wugs_package::canonical_file_list(&listed).as_bytes()).map_err(|e| e.to_string())?;
        zip.finish().map_err(|e| e.to_string())?;
        Ok(Written { files: listed.len(), bytes, packages: packages.len() })
    })();
    if result.is_err() {
        fs::remove_file(out).ok();
    }
    result
}
