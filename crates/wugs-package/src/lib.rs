//! Package manifests (`docs/design/PACKAGES.md`). A package says what it is
//! and where its files live; wugs decides whether it can run it. Models come
//! first: a model package names the capability it serves (the job the app
//! asks for, such as `phone-transcription`) and the runtime adapter that
//! runs it (`onnx-ctc-raw`, …). Adapter settings stay in the `[runtime]`
//! table, which this crate keeps as data for the adapter to read, so a new
//! adapter never needs a new manifest format.
//!
//! Manifests come from other people (the bundled catalogue today, the index
//! later), so everything here is checked: path-safe ids and file names,
//! HTTPS URLs, hex SHA-256 digests, bounded strings.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The largest manifest read, in bytes.
pub const MAX_BYTES: usize = 64 * 1024;
const MAX_TEXT: usize = 2000;

#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    /// Short and path-safe: it names the install folder and is the model
    /// name machine writes record as their provenance.
    pub id: String,
    /// Where the package lives, e.g. `hf:owner/repo` or `github:owner/repo`.
    pub source: String,
    pub version: String,
    pub name: String,
    pub about: String,
    pub license: String,
    pub attribution: String,
    pub homepage: String,
    /// Rough download size for the person deciding to fetch it, when the
    /// files' exact sizes are not all published.
    pub approx_mb: Option<u32>,
    pub model: Model,
    pub files: Vec<File>,
    pub evaluation: Option<Evaluation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub capability: String,
    pub runtime: String,
    /// BCP 47 tags, or `*` for any language.
    pub languages: Vec<String>,
    /// The adapter's own settings, uninterpreted here.
    pub params: toml::Table,
}

#[derive(Clone, Debug, PartialEq)]
pub struct File {
    pub path: String,
    pub url: String,
    /// Lowercase hex, when the publisher gives one.
    pub sha256: Option<String>,
    pub bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    pub benchmark: String,
    /// Error rate per language tag (`*` = overall); lower is better.
    pub per: BTreeMap<String, f64>,
}

impl Manifest {
    /// The download size in MB: exact when every file's size is published,
    /// else the manifest's rough figure.
    pub fn download_mb(&self) -> Option<u32> {
        let exact: Option<u64> = self.files.iter().map(|f| f.bytes).sum();
        match exact {
            Some(b) => Some(b.div_ceil(1_000_000) as u32),
            None => self.approx_mb,
        }
    }

    /// The published error rate for `language`, falling back to the overall one.
    pub fn score(&self, language: Option<&str>) -> Option<f64> {
        let per = &self.evaluation.as_ref()?.per;
        language
            .and_then(|l| per.get(l))
            .or_else(|| per.get("*"))
            .copied()
    }

    /// A runtime setting the adapter needs as a string.
    pub fn param_str(&self, key: &str) -> Result<&str, String> {
        self.model
            .params
            .get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{}: [runtime] needs {key} (a string)", self.id))
    }

    /// A runtime setting that names one of the package's own files. Anything
    /// else (another path, an absolute one) is refused, so a manifest can
    /// never point an adapter outside its install folder.
    pub fn param_file(&self, key: &str) -> Result<&str, String> {
        let name = self.param_str(key)?;
        if self.files.iter().any(|f| f.path == name) {
            Ok(name)
        } else {
            Err(format!(
                "{}: [runtime] {key} = {name:?} is not one of the package's files",
                self.id
            ))
        }
    }

    pub fn param_bool(&self, key: &str) -> Option<bool> {
        self.model.params.get(key).and_then(|v| v.as_bool())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    package: RawPackage,
    model: RawModel,
    #[serde(default)]
    runtime: toml::Table,
    #[serde(default)]
    files: Vec<RawFile>,
    evaluation: Option<RawEvaluation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPackage {
    id: String,
    source: String,
    #[serde(rename = "type")]
    kind: String,
    version: String,
    name: String,
    #[serde(default)]
    about: String,
    license: String,
    #[serde(default)]
    attribution: String,
    #[serde(default)]
    homepage: String,
    approx_mb: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModel {
    capability: String,
    runtime: String,
    #[serde(default)]
    languages: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    path: String,
    url: String,
    sha256: Option<String>,
    bytes: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvaluation {
    benchmark: String,
    per: BTreeMap<String, f64>,
}

/// Parse and check a model manifest. Every problem found is reported, one per line.
pub fn parse(text: &str) -> Result<Manifest, String> {
    if text.len() > MAX_BYTES {
        return Err(format!("a manifest is at most {} kB", MAX_BYTES / 1024));
    }
    let raw: Raw = toml::from_str(text).map_err(|e| e.to_string().trim_end().to_string())?;
    let mut problems = Vec::new();
    let p = &raw.package;
    if p.kind != "model" {
        problems.push(format!(
            "package type {:?} is not one this version reads (only \"model\")",
            p.kind
        ));
    }
    if !slug(&p.id) || windows_reserved(&p.id) {
        problems.push(format!(
            "package id {:?} must be 1 to 64 of a-z, 0-9 and - (and not a name Windows reserves)",
            p.id
        ));
    }
    if !source_ok(&p.source) {
        problems.push(format!(
            "source {:?} must be hf:owner/repo or github:owner/repo",
            p.source
        ));
    }
    for (what, s) in [
        ("version", &p.version),
        ("name", &p.name),
        ("license", &p.license),
    ] {
        if s.trim().is_empty() {
            problems.push(format!("{what} is empty"));
        }
    }
    for (what, s) in [
        ("name", &p.name),
        ("about", &p.about),
        ("attribution", &p.attribution),
        ("homepage", &p.homepage),
        ("license", &p.license),
        ("version", &p.version),
    ] {
        if s.chars().count() > MAX_TEXT {
            problems.push(format!("{what} is longer than {MAX_TEXT} characters"));
        }
    }
    if !p.homepage.is_empty() && !p.homepage.starts_with("https://") {
        problems.push("homepage must be an https:// address".into());
    }
    let m = &raw.model;
    if !slug(&m.capability) {
        problems.push(format!(
            "capability {:?} must be a-z, 0-9 and -",
            m.capability
        ));
    }
    if !slug(&m.runtime) {
        problems.push(format!("runtime {:?} must be a-z, 0-9 and -", m.runtime));
    }
    if m.languages.is_empty() {
        problems.push("languages is empty (use [\"*\"] for any language)".into());
    }
    for l in &m.languages {
        if !(l == "*"
            || (!l.is_empty()
                && l.len() <= 35
                && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')))
        {
            problems.push(format!("language {l:?} is not a BCP 47 tag or *"));
        }
    }
    if raw.files.is_empty() {
        problems.push("a model needs at least one [[files]] entry".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for f in &raw.files {
        if !file_name_ok(&f.path) {
            problems.push(format!(
                "file path {:?} must be a plain file name (no folders, no leading dot)",
                f.path
            ));
        } else if !seen.insert(f.path.to_ascii_lowercase()) {
            problems.push(format!("file {:?} is listed twice", f.path));
        }
        if !f.url.starts_with("https://") || f.url.len() > MAX_TEXT {
            problems.push(format!("{}: url must be an https:// address", f.path));
        }
        if let Some(h) = &f.sha256 {
            if h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
                problems.push(format!("{}: sha256 must be 64 hex digits", f.path));
            }
        }
    }
    if let Some(e) = &raw.evaluation {
        if e.benchmark.trim().is_empty() {
            problems.push("evaluation.benchmark is empty".into());
        }
        for (lang, v) in &e.per {
            if !(v.is_finite() && (0.0..=1.0).contains(v)) {
                problems.push(format!(
                    "evaluation.per[{lang:?}] must be an error rate from 0 to 1"
                ));
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    let Raw {
        package,
        model,
        runtime,
        files,
        evaluation,
    } = raw;
    Ok(Manifest {
        id: package.id,
        source: package.source,
        version: package.version,
        name: package.name,
        about: package.about,
        license: package.license,
        attribution: package.attribution,
        homepage: package.homepage,
        approx_mb: package.approx_mb,
        model: Model {
            capability: model.capability,
            runtime: model.runtime,
            languages: model.languages,
            params: runtime,
        },
        files: files
            .into_iter()
            .map(|f| File {
                path: f.path,
                url: f.url,
                sha256: f.sha256.map(|h| h.to_ascii_lowercase()),
                bytes: f.bytes,
            })
            .collect(),
        evaluation: evaluation.map(|e| Evaluation {
            benchmark: e.benchmark,
            per: e.per,
        }),
    })
}

/// A plain file name that means one file on every OS: no folders, no
/// leading dot, no trailing dot or space (Windows drops them), not a name
/// Windows reserves. Duplicates are compared without case (Windows and macOS).
fn file_name_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && !s.starts_with('.')
        && !s.ends_with('.')
        && !windows_reserved(s)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// 1 to 64 of a-z, 0-9 and `-`, not starting with `-`.
pub fn slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Names Windows refuses as a file or folder name, whatever the extension.
fn windows_reserved(s: &str) -> bool {
    let stem = s.split('.').next().unwrap_or(s).to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

fn source_ok(s: &str) -> bool {
    let Some((host, path)) = s.split_once(':') else {
        return false;
    };
    let Some((owner, repo)) = path.split_once('/') else {
        return false;
    };
    let part = |p: &str| {
        !p.is_empty()
            && p.len() <= 100
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    matches!(host, "hf" | "github") && part(owner) && part(repo)
}

/// One file of a package, as its file list records it: path inside the
/// package (`/`-separated), size in bytes, SHA-256 in lowercase hex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// The package's file list in canonical form (`docs/design/BUNDLES.md` §2):
/// one line per file, `<sha256> <size> <path>`, sorted by path bytes, each
/// ending in LF, UTF-8. This is what the index records per version.
pub fn canonical_file_list(files: &[ListedFile]) -> String {
    let mut sorted: Vec<&ListedFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    sorted.iter().map(|f| format!("{} {} {}\n", f.sha256, f.size, f.path)).collect()
}

/// `files_sha256`: the SHA-256 of the canonical file list, lowercase hex.
/// A bundle's `[[requires]]` pins a package by it.
pub fn file_list_sha256(files: &[ListedFile]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(canonical_file_list(files).as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod file_list_tests {
    use super::*;

    #[test]
    fn the_file_list_hash_matches_an_independent_computation() {
        // Worked out with Python's hashlib, so the format is pinned outside this code.
        let files = vec![
            ListedFile { path: "models/a.onnx".into(), size: 3, sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into() },
            ListedFile { path: "README.md".into(), size: 6, sha256: "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03".into() },
        ];
        assert_eq!(
            canonical_file_list(&files),
            "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03 6 README.md\nba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad 3 models/a.onnx\n"
        );
        assert_eq!(file_list_sha256(&files), "f2d5325d9fbcfa9dab85345f37a40621d5e9b503bec640264f594295a3c96e58");
    }
}
