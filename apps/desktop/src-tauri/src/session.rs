//! Everything the desktop shell can do to a project, as plain functions over
//! the open session. The tauri commands in `lib.rs` only forward to these.

use std::fs::{self, File, TryLockError};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use wugs_auth::{pair_host, Body, DeviceKey, PairError, PairingCode, PublicKey, Roster, RosterFile, RosterState, SignedRecord, Standing};
use wugs_blob::BlobStore;
use wugs_core::{CommitResult, Engines, Project, SegmentCheck, SegmentState, SCOPE_CONFIG};
use wugs_model::Value;
use wugs_peaks::{Params, Peaks};
use wugs_net::{default_policy, ClientReport, Role, SessionOutcome, Warning, Watchdog};
use wugs_sync::SyncPolicy;
use wugs_text::LoroMerger;
use wugs_unicode::NfcNormalizer;

use crate::capture::Recorded;
use crate::identity::fingerprint;

pub type Session = Option<OpenProject>;

/// A project this process has open, with the lock that keeps any other
/// process from opening it too: two writers on one folder would append
/// to one device's log and sign two records on one roster chain.
pub struct OpenProject {
    project: Project,
    /// Peak pyramids already loaded this session, newest first. Two entries
    /// cover a stereo A/B workflow; a .wpk reload is cheap anyway.
    peaks: Vec<(String, Rc<Peaks>)>,
    _lock: File,
}

const PEAKS_KEPT: usize = 2;

impl std::ops::Deref for OpenProject {
    type Target = Project;
    fn deref(&self) -> &Project {
        &self.project
    }
}

impl std::ops::DerefMut for OpenProject {
    fn deref_mut(&mut self) -> &mut Project {
        &mut self.project
    }
}

/// Inside the project folder, not `cache/`, so it is taken before anything
/// in the folder is read or written. The OS drops it when the process
/// exits, crash included, so a stale file never locks anyone out.
const LOCK_FILE: &str = ".wugs-lock";

fn lock_folder(path: &str) -> Result<File, String> {
    fs::create_dir_all(path).map_err(|e| format!("create {path}: {e}"))?;
    let lock_path = Path::new(path).join(LOCK_FILE);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|e| format!("open {}: {e}", lock_path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(format!(
            "{path} is open in another wugs window or process; close it there first (two open copies would fork this device's history)"
        )),
        Err(TryLockError::Error(e)) => Err(format!("lock {}: {e}", lock_path.display())),
    }
}

/// Reopening the folder this session already holds must not trip over its
/// own lock, so that project is closed first.
fn release_if_open(session: &mut Session, path: &str) {
    let same = |p: &OpenProject| match (p.root.canonicalize(), Path::new(path).canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if session.as_ref().is_some_and(same) {
        *session = None;
    }
}

/// Marks the shell-made default tier, so it is never confused with an
/// imported tier that happens to be called "utterance".
const UTTERANCE_ORIGIN: &str = "wugs:utterance";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub project_id: String,
    pub path: String,
    pub texts: usize,
    pub conflicts: i64,
    /// Set when this folder is a copy of another one. Warn-only; the shell
    /// offers the two fixes.
    pub relocation: Option<RelocationInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelocationInfo {
    /// Where the folder was last recorded.
    pub from: String,
    /// That path still holds this project, so this folder is a copy.
    pub original_still_present: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRow {
    pub id: String,
    pub title: String,
    pub kind: Option<String>,
    pub segments: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TierRow {
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
    /// What the tier is for (`transcription`, `translation`, `note`, …); core never reads it.
    pub role: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentRow {
    pub id: String,
    pub tier: String,
    pub start_ms: Option<f64>,
    pub end_ms: Option<f64>,
    pub content: String,
    pub pos: String,
    /// Set while the transcription is still the one a phone model wrote.
    pub machine: Option<MachineView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineView {
    pub model: String,
    pub confidence: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDetail {
    pub id: String,
    pub title: String,
    pub tiers: Vec<TierRow>,
    pub segments: Vec<SegmentRow>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    pub content: String,
    pub findings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedSegment {
    pub id: String,
    pub findings: Vec<String>,
    /// The segments as the edit left them (the new one; for a split, both
    /// parts, first then second), so a timeline undo can check nothing
    /// changed since before it writes.
    pub states: Vec<SegmentStateView>,
}

/// What a timeline undo would delete or overwrite on a segment
/// (`wugs_core::SegmentState`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentStateView {
    pub id: String,
    pub deleted: bool,
    pub start_ms: Option<f64>,
    pub end_ms: Option<f64>,
    pub content: String,
    pub words: usize,
}

impl From<SegmentState> for SegmentStateView {
    fn from(s: SegmentState) -> Self {
        SegmentStateView { id: s.id, deleted: s.deleted, start_ms: s.start, end_ms: s.end, content: s.content, words: s.words }
    }
}

/// The parts of a segment's state an undo depends on; an absent part is
/// not checked (`wugs_core::SegmentCheck`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentCheckView {
    pub id: String,
    pub deleted: Option<bool>,
    pub start_ms: Option<f64>,
    pub end_ms: Option<f64>,
    pub content: Option<String>,
    pub words: Option<usize>,
}

fn checks(expect: &[SegmentCheckView]) -> Vec<SegmentCheck> {
    expect
        .iter()
        .map(|c| SegmentCheck { id: c.id.clone(), deleted: c.deleted, start: c.start_ms, end: c.end_ms, content: c.content.clone(), words: c.words })
        .collect()
}

pub(crate) fn states(p: &Project, ids: &[String]) -> Result<Vec<SegmentStateView>, String> {
    Ok(p.segment_states(ids)?.into_iter().map(Into::into).collect())
}

/// Findings and the touched segments' states after the edit (and, for a
/// merge, before it: what its undo puts back).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentsEdit {
    pub findings: Vec<String>,
    pub states: Vec<SegmentStateView>,
    pub before: Vec<SegmentStateView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedSegments {
    pub deleted: Vec<String>,
    /// Edited since the checks were taken, so left alone.
    pub kept: Vec<String>,
    pub findings: Vec<String>,
}

pub(crate) fn finding_lines(res: &CommitResult) -> Vec<String> {
    res.findings.iter().map(|f| format!("{}: {}", f.code, f.message)).collect()
}

/// The engines every project opens with, on every path (open, create, join):
/// collaborative text, and NFC for vernacular text at entry.
fn engines() -> Engines {
    Engines { merger: Some(Box::new(LoroMerger)), normalizer: Some(Box::new(NfcNormalizer)) }
}

fn open(session: &mut Session, path: &str, researcher: &str) -> Result<ProjectInfo, String> {
    if researcher.trim().is_empty() {
        return Err("a researcher id is required".into());
    }
    release_if_open(session, path);
    let lock = lock_folder(path)?;
    let project = Project::open_with(path, researcher, engines())?;
    info(session.insert(OpenProject { project, peaks: Vec::new(), _lock: lock }), path)
}

pub fn open_project(session: &mut Session, path: &str, researcher: &str) -> Result<ProjectInfo, String> {
    if !Path::new(path).join("project.json").is_file() {
        return Err(format!("no wugs project at {path} (project.json not found)"));
    }
    open(session, path, researcher)
}

pub fn create_project(session: &mut Session, path: &str, researcher: &str) -> Result<ProjectInfo, String> {
    if Path::new(path).join("project.json").exists() {
        return Err(format!("{path} already holds a wugs project"));
    }
    open(session, path, researcher)
}

/// Canonical Windows paths carry a `\\?\` prefix a person should not see.
pub fn display_path(path: &str) -> String {
    match path.strip_prefix(r"\\?\UNC\") {
        Some(share) => format!(r"\\{share}"),
        None => path.strip_prefix(r"\\?\").unwrap_or(path).to_string(),
    }
}

fn info(p: &Project, path: &str) -> Result<ProjectInfo, String> {
    Ok(ProjectInfo {
        project_id: p.project_id.clone(),
        path: path.to_string(),
        texts: p.proj.entities_of_kind("text")?.len(),
        conflicts: p.proj.open_conflict_count()?,
        relocation: p.relocation().map(|r| RelocationInfo { from: display_path(&r.stored_path), original_still_present: r.original_still_present }),
    })
}

pub(crate) fn project(session: &mut Session) -> Result<&mut Project, String> {
    session.as_mut().map(|o| &mut o.project).ok_or_else(|| "no project is open".to_string())
}

pub fn list_texts(session: &mut Session) -> Result<Vec<TextRow>, String> {
    let p = project(session)?;
    let mut rows = Vec::new();
    for id in p.proj.entities_of_kind("text")? {
        rows.push(TextRow {
            title: p.proj.field_str(&id, "title")?.unwrap_or_default(),
            kind: p.proj.field_str(&id, "kind")?,
            segments: p.proj.children_by_ref("text", &id, "segment")?.len(),
            id,
        });
    }
    Ok(rows)
}

pub fn get_text(session: &mut Session, text_id: &str) -> Result<TextDetail, String> {
    let p = project(session)?;
    if p.proj.entity_kind(text_id)?.as_deref() != Some("text") || p.proj.is_tombstoned(text_id)? {
        return Err(format!("no text {text_id}"));
    }
    let mut segments = Vec::new();
    let mut tier_ids: Vec<String> = Vec::new();
    for id in p.proj.ordered_children("text", text_id, "segment")? {
        let tier = p.proj.field_str(&id, "tier")?.unwrap_or_default();
        if !tier.is_empty() && !tier_ids.contains(&tier) {
            tier_ids.push(tier.clone());
        }
        segments.push(SegmentRow {
            tier,
            start_ms: p.proj.field_num(&id, "start")?,
            end_ms: p.proj.field_num(&id, "end")?,
            content: p.proj.field_str(&id, "content")?.unwrap_or_default(),
            pos: p.proj.field_str(&id, "pos")?.unwrap_or_default(),
            machine: p.machine_mark(&id)?.map(|m| MachineView { model: m.model, confidence: m.confidence }),
            id,
        });
    }
    // The lanes: tiers this text's segments use, plus every tier that
    // belongs to it: project-wide ones (the shell default, "Add track")
    // and tiers imported with this text (x.elan.text). A tier without
    // segments must still show, or nothing could be drawn on it.
    for id in p.proj.entities_of_kind("tier")? {
        if tier_ids.contains(&id) {
            continue;
        }
        match p.proj.field_str(&id, "x.elan.text")? {
            None => tier_ids.push(id),
            Some(t) if t == text_id => tier_ids.push(id),
            Some(_) => {}
        }
    }
    // Imported tiers keep their ELAN order; tiers without one follow, in
    // creation (id) order.
    let mut keyed = Vec::with_capacity(tier_ids.len());
    for id in tier_ids {
        let order = p.proj.field_num(&id, "x.elan.order")?.unwrap_or(f64::MAX);
        keyed.push((order, id));
    }
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut tiers = Vec::new();
    for (_, id) in keyed {
        tiers.push(TierRow {
            name: p.proj.field_str(&id, "name")?.unwrap_or_else(|| id.clone()),
            kind: p.proj.field_str(&id, "kind")?,
            role: p.proj.field_str(&id, "role")?,
            language: p.proj.field_str(&id, "language")?,
            id,
        });
    }
    Ok(TextDetail {
        id: text_id.to_string(),
        title: p.proj.field_str(text_id, "title")?.unwrap_or_default(),
        tiers,
        segments,
    })
}

pub fn create_text(session: &mut Session, title: &str) -> Result<String, String> {
    project(session)?.create_text(title)
}

pub fn add_segment(
    session: &mut Session,
    text_id: &str,
    tier_id: Option<&str>,
    start_ms: Option<f64>,
    end_ms: Option<f64>,
    content: &str,
) -> Result<AddedSegment, String> {
    let p = project(session)?;
    let tier = match tier_id {
        Some(t) => t.to_string(),
        None => utterance_tier(p)?,
    };
    let index = next_pos_index(p, text_id)?;
    let (id, res) = p.add_segment(text_id, &tier, index, start_ms, end_ms, content)?;
    let states = states(p, std::slice::from_ref(&id))?;
    Ok(AddedSegment { id, findings: finding_lines(&res), states })
}

/// Retime one or both edges. An omitted edge is not written, so a drag of
/// one handle cannot revert a concurrent edit of the other.
pub fn set_segment_times(
    session: &mut Session,
    text_id: &str,
    segment_id: &str,
    start_ms: Option<f64>,
    end_ms: Option<f64>,
) -> Result<Vec<String>, String> {
    let p = project(session)?;
    let stored = |field| p.proj.field_num(segment_id, field).map(|v| v.unwrap_or(0.0));
    let (lo, hi) = (start_ms.map_or_else(|| stored("start"), Ok)?, end_ms.map_or_else(|| stored("end"), Ok)?);
    if !(lo.is_finite() && hi.is_finite() && lo >= 0.0 && hi >= lo) {
        return Err(format!("bad segment times [{lo}..{hi}]"));
    }
    let res = p.set_segment_times(text_id, segment_id, start_ms, end_ms)?;
    Ok(finding_lines(&res))
}

pub fn delete_segment(session: &mut Session, text_id: &str, segment_id: &str) -> Result<Vec<String>, String> {
    let res = project(session)?.delete_segment(text_id, segment_id)?;
    Ok(finding_lines(&res))
}

/// `set_segment_times` for a timeline undo or redo: refused, writing
/// nothing, when `expect` no longer holds (the segment was edited since).
/// The check and the write run in one call on the session thread, so no
/// other command can land between them.
pub fn set_segment_times_checked(
    session: &mut Session,
    text_id: &str,
    segment_id: &str,
    start_ms: Option<f64>,
    end_ms: Option<f64>,
    expect: &[SegmentCheckView],
) -> Result<Vec<String>, String> {
    project(session)?.require_unchanged(&checks(expect))?;
    set_segment_times(session, text_id, segment_id, start_ms, end_ms)
}

/// `edit_segment_content` for a timeline undo or redo, checked like
/// [`set_segment_times_checked`].
pub fn edit_segment_content_checked(session: &mut Session, text_id: &str, segment_id: &str, content: &str, expect: &[SegmentCheckView]) -> Result<EditResult, String> {
    project(session)?.require_unchanged(&checks(expect))?;
    edit_segment_content(session, text_id, segment_id, content)
}

/// Bring deleted segments back in one txn (the timeline's undo of a
/// delete, redo of a draw or Auto-segment). Nothing is lost by a restore.
pub fn restore_segments(session: &mut Session, text_id: &str, segment_ids: &[String]) -> Result<SegmentsEdit, String> {
    let p = project(session)?;
    let res = p.restore_segments(text_id, segment_ids)?;
    Ok(SegmentsEdit { findings: finding_lines(&res), states: states(p, segment_ids)?, before: Vec::new() })
}

/// Delete segments in one txn (the timeline's undo of a draw or an
/// Auto-segment, redo of a delete), only those `expect` still holds for.
/// With `keep_changed`, the changed ones are kept and the rest deleted;
/// without, any change refuses the whole step.
pub fn delete_segments(session: &mut Session, text_id: &str, expect: &[SegmentCheckView], keep_changed: bool) -> Result<DeletedSegments, String> {
    let p = project(session)?;
    let checks = checks(expect);
    if !keep_changed {
        p.require_unchanged(&checks)?;
    }
    let (deleted, kept, res) = p.delete_unchanged_segments(text_id, &checks)?;
    Ok(DeletedSegments { deleted, kept, findings: finding_lines(&res) })
}

/// Cut a segment at `at_ms`; the transcription stays on the first part.
pub fn split_segment(session: &mut Session, text_id: &str, segment_id: &str, at_ms: f64) -> Result<AddedSegment, String> {
    let p = project(session)?;
    let (id, res) = p.split_segment(text_id, segment_id, at_ms.round())?;
    let states = states(p, &[segment_id.to_string(), id.clone()])?;
    Ok(AddedSegment { id, findings: finding_lines(&res), states })
}

/// Join a segment with the next one on its tier. With `expect` (the undo
/// of a split, the redo of a merge), refused when it no longer holds.
/// States: first, then second (deleted).
pub fn merge_segments(session: &mut Session, text_id: &str, first_id: &str, second_id: &str, expect: &[SegmentCheckView]) -> Result<SegmentsEdit, String> {
    let p = project(session)?;
    p.require_unchanged(&checks(expect))?;
    let pair = [first_id.to_string(), second_id.to_string()];
    let before = states(p, &pair)?;
    let res = p.merge_segments(text_id, first_id, second_id)?;
    Ok(SegmentsEdit { findings: finding_lines(&res), states: states(p, &pair)?, before })
}

/// Take a merge back (or redo a split) in one txn, refused when `expect`
/// no longer holds. States: first, then second.
pub fn unmerge_segments(
    session: &mut Session,
    text_id: &str,
    first_id: &str,
    second_id: &str,
    first_end_ms: f64,
    first_content: Option<&str>,
    expect: &[SegmentCheckView],
) -> Result<SegmentsEdit, String> {
    let p = project(session)?;
    p.require_unchanged(&checks(expect))?;
    let res = p.unmerge_segments(text_id, first_id, second_id, first_end_ms, first_content)?;
    Ok(SegmentsEdit { findings: finding_lines(&res), states: states(p, &[first_id.to_string(), second_id.to_string()])?, before: Vec::new() })
}

/// The "Add track" templates (TONGUES-UI): each is a default name and a tier
/// role over the time-aligned kind. There is no "gloss" template: glosses live
/// on morphs, under the words of a transcription (GLOSSING-MODEL), so a gloss
/// track would hold nothing the Gloss panel reads. The symbolic ELAN kinds
/// stay data-only for now.
pub const TIER_TEMPLATES: [(&str, Option<&str>); 4] = [
    ("transcription", Some("transcription")),
    ("translation", Some("translation")),
    ("notes", Some("note")),
    ("custom", None),
];

pub fn add_tier(session: &mut Session, template: &str, name: Option<&str>) -> Result<TierRow, String> {
    let Some((_, role)) = TIER_TEMPLATES.iter().find(|(t, _)| *t == template) else {
        let known: Vec<&str> = TIER_TEMPLATES.iter().map(|(t, _)| *t).collect();
        return Err(format!("unknown track template {template:?}; one of {known:?}"));
    };
    let name = name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or(template);
    let p = project(session)?;
    let (id, _) = p.add_tier(name, "time-aligned", *role, None)?;
    Ok(TierRow { id, name: name.to_string(), kind: Some("time-aligned".to_string()), role: role.map(str::to_string), language: None })
}

/// One past the largest existing pos, so a new segment sorts last even when
/// earlier ones were deleted or imported with their own numbering. The live
/// count alone can collide with a taken pos. Deleted segments count too, so
/// restoring one never brings back a pos a newer segment took.
pub(crate) fn next_pos_index(p: &Project, text_id: &str) -> Result<usize, String> {
    let mut next = 0usize;
    for seg in p.proj.children_by_ref_with_deleted("text", text_id, "segment")? {
        let pos = p.proj.field_str(&seg, "pos")?.unwrap_or_default();
        if let Ok(i) = usize::from_str_radix(&pos, 16) {
            next = next.max(i + 1);
        } else {
            next = next.max(1);
        }
    }
    Ok(next)
}

pub(crate) fn utterance_tier(p: &mut Project) -> Result<String, String> {
    for id in p.proj.entities_of_kind("tier")? {
        if p.proj.field_str(&id, "origin")?.as_deref() == Some(UTTERANCE_ORIGIN) {
            return Ok(id);
        }
    }
    let mut t = p.intent("create-tier");
    let id = t.create(
        "tier",
        "tir",
        SCOPE_CONFIG,
        vec![
            ("name", Value::str("utterance")),
            ("kind", Value::str("time-aligned")),
            ("origin", Value::str(UTTERANCE_ORIGIN)),
        ],
    );
    t.commit()?;
    Ok(id)
}

pub fn edit_segment_content(
    session: &mut Session,
    text_id: &str,
    segment_id: &str,
    content: &str,
) -> Result<EditResult, String> {
    let p = project(session)?;
    let res = p.edit_content(text_id, segment_id, content)?;
    Ok(EditResult {
        content: p.proj.field_str(segment_id, "content")?.unwrap_or_default(),
        findings: res.findings.iter().map(|f| format!("{}: {}", f.code, f.message)).collect(),
    })
}

pub fn conflict_count(session: &mut Session) -> Result<i64, String> {
    project(session)?.proj.open_conflict_count()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRow {
    pub id: String,
    pub label: String,
    pub kind: Option<String>,
    /// `None` means unset, which is open (DECISIONS 2026-11-08).
    pub access: Option<String>,
    pub has_media: bool,
}

pub fn list_sources(session: &mut Session) -> Result<Vec<SourceRow>, String> {
    let p = project(session)?;
    let mut rows = Vec::new();
    for id in p.proj.entities_of_kind("source")? {
        rows.push(SourceRow {
            label: p.proj.field_str(&id, "label")?.unwrap_or_default(),
            kind: p.proj.field_str(&id, "kind")?,
            access: p.proj.field_str(&id, "access")?,
            // A whole-file blob on the source, or any media track (how
            // audio and video attach, DATA-MODEL §3).
            has_media: p.proj.field_str(&id, "blob")?.is_some()
                || !p.proj.children_by_ref("source", &id, "media-track")?.is_empty(),
            id,
        });
    }
    Ok(rows)
}

// ------------------------------------------------------------ media, peaks

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeaksInfo {
    pub frames: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: f64,
    /// Absolute path of the media file, for the shell's <audio> element
    /// (served over the asset protocol).
    pub media_path: String,
}

/// One `Peaks::query` answer, JSON-shaped for the canvas.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeaksSlice {
    pub bucket_frames: u64,
    pub first_frame: u64,
    pub mins: Vec<i16>,
    pub maxs: Vec<i16>,
}

/// The canvas asks for strips of a few thousand pixels; anything much
/// larger is a bug upstream, not a drawing request.
const MAX_QUERY_PIXELS: usize = 16 * 1024;

fn duration_ms(info: &wugs_peaks::SourceInfo) -> f64 {
    if info.sample_rate == 0 {
        return 0.0;
    }
    info.frames as f64 * 1000.0 / info.sample_rate as f64
}

/// Import a WAV file: blob store, source + audio track intents, source
/// duration, and the peak pyramid computed and cached while the file is hot.
pub fn import_media(session: &mut Session, path: &str) -> Result<SourceRow, String> {
    let open = session.as_mut().ok_or_else(|| "no project is open".to_string())?;
    let file = Path::new(path);
    let label = file
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{path} has no usable file name"))?
        .to_string();
    import_wav(open, file, &label)
}

/// Import a WAV under another name: a compressed recording decoded by
/// `convert::to_wav` keeps its own file name as the source's label.
/// `expected_root`: the project the recording was meant for (a dropped
/// file); checked in this same call, so a project opened meanwhile gets
/// nothing.
pub fn import_media_named(session: &mut Session, wav: &Path, label: &str, expected_root: Option<&Path>) -> Result<SourceRow, String> {
    if let Some(root) = expected_root {
        require_root(session, root)?;
    }
    let open = session.as_mut().ok_or_else(|| "no project is open".to_string())?;
    import_wav(open, wav, label)
}

/// A finished in-app recording becomes a source named `label`. The pending
/// file goes only after the import's intents commit; any failure leaves it
/// in `recordings-pending/` for a retry or a Recovered recording.
pub fn import_capture(session: &mut Session, done: Recorded, label: &str) -> Result<SourceRow, String> {
    let open = session.as_mut().ok_or_else(|| format!("no project is open; the recording is kept at {}", done.path.display()))?;
    if open.project.root.canonicalize().ok().as_deref() != Some(done.root.as_path()) {
        return Err(format!("the project changed during the recording; it is kept at {}", done.path.display()));
    }
    let label = if label.trim().is_empty() { "Recording" } else { label.trim() };
    let row = import_wav(open, &done.path, label).map_err(|e| format!("{e}; the recording is kept at {}", done.path.display()))?;
    fs::remove_file(&done.path).ok();
    Ok(row)
}

pub fn project_root(session: &mut Session) -> Result<PathBuf, String> {
    session.as_ref().map(|p| p.root.clone()).ok_or_else(|| "no project is open".to_string())
}

/// Refused when another project than `expected` is open now: work begun in
/// one project, such as a dropped file, must not land in another.
pub fn require_root(session: &mut Session, expected: &Path) -> Result<(), String> {
    let root = project_root(session)?;
    if root == expected {
        return Ok(());
    }
    match (root.canonicalize(), expected.canonicalize()) {
        (Ok(now), Ok(then)) if now == then => Ok(()),
        (Ok(_), Ok(_)) => Err("another project was opened after the files were dropped; nothing was imported".into()),
        // Some virtual drives cannot resolve a path: say what really happened.
        _ => Err(format!("wugs could not tell whether {} is still the open project; nothing was imported", expected.display())),
    }
}

fn import_wav(open: &mut OpenProject, file: &Path, label: &str) -> Result<SourceRow, String> {
    let label = label.to_string();
    let peaks = Peaks::compute_path(file, Params::default(), None).map_err(|e| match e {
        wugs_peaks::Error::NotWav(m) => format!("{label} is not a WAV file wugs can read ({m})"),
        wugs_peaks::Error::Unsupported(m) => format!("{label}: unsupported WAV ({m})"),
        e => format!("{label}: {e}"),
    })?;
    let info = peaks.info();
    if info.frames == 0 {
        return Err(format!("{label} holds no audio frames"));
    }

    let p = &mut open.project;
    let store = BlobStore::open(&p.root).map_err(|e| format!("open blob store: {e}"))?;
    let blob = store.put_file(file).map_err(|e| format!("store {label}: {e}"))?;
    let hex = wugs_blob::parse_ref(&blob).ok_or("blob store returned a malformed ref")?.to_string();
    peaks.save(&p.root.join("cache"), &hex).map_err(|e| format!("save peak cache: {e}"))?;

    let source = p.add_source(&label, None, "audio", None)?;
    // Duration feeds validator #7 (segments inside the recording).
    let mut t = p.intent("set-duration");
    t.set("source", &source, &source, "duration", Value::Num(duration_ms(&info)))?;
    t.commit()?;
    p.add_media_track(&source, &blob, "audio", &label, 0)?;

    remember_peaks(open, hex, Rc::new(peaks));
    let p = &open.project;
    Ok(SourceRow {
        label: p.proj.field_str(&source, "label")?.unwrap_or(label),
        kind: p.proj.field_str(&source, "kind")?,
        access: p.proj.field_str(&source, "access")?,
        has_media: true,
        id: source,
    })
}

/// The audio blob behind a source: its first audio track's blob (lowest
/// track id, so every device picks the same one), else the source's own.
pub(crate) fn audio_blob(p: &Project, source_id: &str) -> Result<String, String> {
    if p.proj.entity_kind(source_id)?.as_deref() != Some("source") || p.proj.is_tombstoned(source_id)? {
        return Err(format!("no source {source_id}"));
    }
    let mut tracks = p.proj.children_by_ref("source", source_id, "media-track")?;
    tracks.sort();
    for track in tracks {
        if p.proj.field_str(&track, "kind")?.as_deref() == Some("audio") {
            if let Some(blob) = p.proj.field_str(&track, "blob")? {
                return Ok(blob);
            }
        }
    }
    p.proj
        .field_str(source_id, "blob")?
        .ok_or_else(|| "this source has no audio file attached".to_string())
}

/// The pyramid for a source's audio, from the session cache, the `.wpk`
/// under `cache/peaks/`, or a fresh compute (saved for next time), in that
/// order. Returns the pyramid with the media file's path.
fn ensure_peaks(open: &mut OpenProject, source_id: &str) -> Result<(Rc<Peaks>, PathBuf), String> {
    let p = &open.project;
    let blob = audio_blob(p, source_id)?;
    let hex = wugs_blob::parse_ref(&blob).ok_or_else(|| format!("malformed blob ref {blob:?}"))?.to_string();
    // The bare path, not BlobStore::path_of, which demands the file: with a
    // .wpk in cache the waveform still draws when the media file is gone.
    let media = p.root.join("blobs").join(&hex);

    if let Some(i) = open.peaks.iter().position(|(k, _)| *k == hex) {
        let hit = open.peaks.remove(i);
        let peaks = hit.1.clone();
        open.peaks.insert(0, hit);
        return Ok((peaks, media));
    }
    let cache_dir = p.root.join("cache");
    let peaks = match Peaks::load(&cache_dir, &hex) {
        Ok(Some(peaks)) => peaks,
        // No cache, or a torn/stale one: recompute from the blob. The blob
        // must exist for that (it always does unless the folder was pruned
        // by hand).
        Ok(None) | Err(_) => {
            let peaks = Peaks::compute_path(&media, Params::default(), None)
                .map_err(|e| format!("compute waveform of {}: {e}", media.display()))?;
            peaks.save(&cache_dir, &hex).map_err(|e| format!("save peak cache: {e}"))?;
            peaks
        }
    };
    let peaks = Rc::new(peaks);
    remember_peaks(open, hex, peaks.clone());
    Ok((peaks, media))
}

fn remember_peaks(open: &mut OpenProject, hex: String, peaks: Rc<Peaks>) {
    open.peaks.retain(|(k, _)| *k != hex);
    open.peaks.insert(0, (hex, peaks));
    open.peaks.truncate(PEAKS_KEPT);
}

pub fn peaks_info(session: &mut Session, source_id: &str) -> Result<PeaksInfo, String> {
    let open = session.as_mut().ok_or_else(|| "no project is open".to_string())?;
    let (peaks, media) = ensure_peaks(open, source_id)?;
    let info = peaks.info();
    Ok(PeaksInfo {
        frames: info.frames,
        sample_rate: info.sample_rate,
        channels: info.channels,
        duration_ms: duration_ms(&info),
        media_path: media.to_string_lossy().into_owned(),
    })
}

pub fn peaks_query(
    session: &mut Session,
    source_id: &str,
    channel: u16,
    start_frame: u64,
    end_frame: u64,
    pixels: usize,
) -> Result<PeaksSlice, String> {
    let open = session.as_mut().ok_or_else(|| "no project is open".to_string())?;
    let (peaks, _) = ensure_peaks(open, source_id)?;
    let info = peaks.info();
    if channel >= info.channels {
        return Err(format!("channel {channel} of a {}-channel file", info.channels));
    }
    if end_frame < start_frame || pixels == 0 || pixels > MAX_QUERY_PIXELS {
        return Err(format!("bad peaks query [{start_frame}..{end_frame}] over {pixels}px"));
    }
    let view = peaks.query(channel as usize, start_frame..end_frame, pixels);
    let (mins, maxs) = view.buckets.iter().copied().unzip();
    Ok(PeaksSlice {
        bucket_frames: view.bucket_frames,
        first_frame: view.first_frame,
        mins,
        maxs,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRow {
    pub target: String,
    pub kind: String,
    pub field: String,
    pub snippet: String,
    /// The text a segment hit belongs to, so the shell can open it.
    pub text_id: Option<String>,
    pub text_title: Option<String>,
    /// The segment's tier and start, so the shell can select and label it.
    pub tier_id: Option<String>,
    pub tier_name: Option<String>,
    pub start_ms: Option<f64>,
}

pub const SEARCH_LIMIT: usize = 50;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchScope {
    All,
    Texts,
    Lexicon,
}

impl SearchScope {
    fn kinds(self) -> &'static [&'static str] {
        match self {
            SearchScope::All => &[],
            SearchScope::Texts => &["text", "segment"],
            SearchScope::Lexicon => &["lexeme", "sense"],
        }
    }
}

pub fn search(session: &mut Session, query: &str, scope: SearchScope) -> Result<Vec<SearchRow>, String> {
    let p = project(session)?;
    let mut rows = Vec::new();
    for hit in p.proj.search(query, scope.kinds(), SEARCH_LIMIT)? {
        // A sense whose entry was deleted (say, on another device while this
        // one added the sense) is not part of the lexicon any more.
        if hit.kind == "sense" {
            match p.proj.field_str(&hit.target, "lexeme")? {
                Some(lex) if !p.proj.is_tombstoned(&lex)? => {}
                _ => continue,
            }
        }
        let (text_id, tier_id) = match hit.kind.as_str() {
            "segment" => (p.proj.field_str(&hit.target, "text")?, p.proj.field_str(&hit.target, "tier")?),
            "text" => (Some(hit.target.clone()), None),
            _ => (None, None),
        };
        let name = |id: &Option<String>, field: &str| match id {
            Some(id) => p.proj.field_str(id, field),
            None => Ok(None),
        };
        let start_ms = if hit.kind == "segment" { p.proj.field_num(&hit.target, "start")? } else { None };
        rows.push(SearchRow {
            text_title: name(&text_id, "title")?,
            tier_name: name(&tier_id, "name")?,
            target: hit.target,
            kind: hit.kind,
            field: hit.field,
            snippet: hit.snippet,
            text_id,
            tier_id,
            start_ms,
        });
    }
    Ok(rows)
}

// ------------------------------------------------------------- sync auth
//
// Every roster write (founding, enrolment, revocation, the attest and merge
// a sync does) runs here on the session thread: two writers holding their
// own copy of the roster would sign two records on one chain, and a forked
// chain fails closed. Only waiting on a socket happens off the thread.

/// How long a pairing code stays valid.
pub const PAIR_TTL: Duration = Duration::from_secs(120);
const PAIR_IO_TIMEOUT: Duration = Duration::from_secs(30);
/// The whole pairing exchange, from connect to enrolment. It runs on the
/// session thread, so a stalling peer must not hold it longer.
pub const PAIR_DEADLINE: Duration = Duration::from_secs(60);
/// How long an accepted connection may stay silent before it is dropped,
/// never having reached the session thread.
pub const FIRST_BYTE_WAIT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StandingView {
    Member,
    Guest,
    Revoked,
    Unknown,
}

impl From<Standing> for StandingView {
    fn from(s: Standing) -> Self {
        match s {
            Standing::Member => StandingView::Member,
            Standing::Guest => StandingView::Guest,
            Standing::Revoked => StandingView::Revoked,
            Standing::Unknown => StandingView::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRow {
    pub key: String,
    pub fingerprint: String,
    pub label: String,
    pub standing: StandingView,
    pub guest: bool,
    /// The key's record chain forked (the key is in two hands) or its
    /// revocation is paradoxical; it fails closed until a person looks.
    pub contested: bool,
    pub this_device: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicesView {
    /// The project has a roster; `false` until this device founds one or joins.
    pub paired: bool,
    pub devices: Vec<DeviceRow>,
    /// Set when this folder's device id is attested to another key too:
    /// the folder is probably a copy of another machine's.
    pub copy_warning: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairOffer {
    pub digits: String,
    pub port: u16,
    pub expires_in: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinResult {
    pub project: ProjectInfo,
    pub standing: StandingView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub peer_label: Option<String>,
    pub peer_guest: bool,
    pub pulled_ops: usize,
    pub answered: usize,
    pub txns_held: usize,
    pub conflicts: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServedSession {
    pub peer_label: Option<String>,
    /// "converged", "peer left", or why the session was refused or failed.
    pub outcome: String,
    pub pulled_ops: usize,
    pub warnings: Vec<String>,
}

fn device_row(st: &RosterState, me: &PublicKey, key: &PublicKey) -> DeviceRow {
    let dev = st.devices.get(key);
    DeviceRow {
        key: key.to_hex(),
        fingerprint: fingerprint(key),
        label: dev.map(|d| d.label.clone()).unwrap_or_default(),
        standing: st.standing(key).into(),
        guest: dev.is_some_and(|d| d.guest),
        contested: st.contested.contains(key),
        this_device: key == me,
    }
}

fn load_roster(p: &Project) -> Result<(RosterFile, Roster), String> {
    let file = RosterFile::in_project(&p.root);
    if !file.exists() {
        return Err("this project has no device roster yet; show a pairing code to found it".into());
    }
    let roster = file.load(&p.project_id)?;
    Ok((file, roster))
}

/// Whether this device is on the project's roster as a guest. A project
/// with no roster, or a device without a key, has no guests.
pub fn is_guest(session: &mut Session, key: Option<&DeviceKey>) -> Result<bool, String> {
    let p = project(session)?;
    let (Some(key), true) = (key, RosterFile::in_project(&p.root).exists()) else { return Ok(false) };
    let (_, roster) = load_roster(p)?;
    Ok(matches!(roster.state().standing(&key.public()), Standing::Guest))
}

/// A project made before sync auth, or never paired, has no roster: this
/// device founds it. Either way this device must then be a member.
fn found_roster(p: &Project, key: &DeviceKey, label: &str) -> Result<(), String> {
    let file = RosterFile::in_project(&p.root);
    if !file.exists() {
        file.create(&Roster::genesis(&p.project_id, key, label))?;
    }
    match file.load(&p.project_id)?.state().standing(&key.public()) {
        Standing::Member => Ok(()),
        Standing::Guest => Err("guest devices cannot pair new devices or serve sync".into()),
        Standing::Revoked => Err("this device has been marked lost".into()),
        Standing::Unknown => Err("this device is not on the project's roster".into()),
    }
}

pub fn devices_list(session: &mut Session, key: &DeviceKey) -> Result<DevicesView, String> {
    let p = project(session)?;
    let file = RosterFile::in_project(&p.root);
    if !file.exists() {
        return Ok(DevicesView { paired: false, devices: Vec::new(), copy_warning: None });
    }
    let st = file.load(&p.project_id)?.state();
    let me = key.public();
    let devices = st.devices.keys().map(|k| device_row(&st, &me, k)).collect();
    let copy_warning = st
        .dvc_claimed_by_other(p.device(), &me)
        .map(|other| Warning::OwnDeviceIdClaimed { dvc: p.device().to_string(), other }.to_string());
    Ok(DevicesView { paired: true, devices, copy_warning })
}

pub fn device_mark_lost(session: &mut Session, key: &DeviceKey, target: &str) -> Result<DevicesView, String> {
    let target = PublicKey::from_hex(target)?;
    if target == key.public() {
        return Err("mark this device lost from another member's device".into());
    }
    let p = project(session)?;
    let (file, mut roster) = load_roster(p)?;
    let rec = roster.revoke(key, target)?;
    file.append(&[rec])?;
    devices_list(session, key)
}

/// The host's half of a pairing in flight: its code and the port the
/// joiner connects to. Moves to the waiting thread, then back to the
/// session thread for `pair_finish`.
pub struct HostPairing {
    project_id: String,
    listener: TcpListener,
    code: PairingCode,
    deadline: Instant,
    guest: bool,
}

/// Show a pairing code. `label` names this device if it founds the roster;
/// `guest` is what the new device will be allowed to do.
pub fn pair_start(session: &mut Session, key: &DeviceKey, label: &str, guest: bool) -> Result<(PairOffer, HostPairing), String> {
    let p = project(session)?;
    found_roster(p, key, label)?;
    let listener = TcpListener::bind(("0.0.0.0", 0)).map_err(|e| format!("listen for pairing: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let code = PairingCode::generate(PAIR_TTL);
    let offer = PairOffer { digits: code.digits().to_string(), port, expires_in: PAIR_TTL.as_secs() };
    let pairing = HostPairing { project_id: p.project_id.clone(), listener, code, deadline: Instant::now() + PAIR_TTL, guest };
    Ok((offer, pairing))
}

impl HostPairing {
    /// Wait for the joiner until the code expires. Runs off the session
    /// thread; polling keeps the wait bounded by the code's lifetime. A
    /// connection that sends nothing is dropped here and the wait goes on,
    /// so a silent stranger can neither hold the session thread nor burn
    /// the code.
    pub fn wait(&self) -> Result<TcpStream, String> {
        self.listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        loop {
            match self.listener.accept() {
                Ok((tcp, _)) => {
                    // Some platforms hand out accepted sockets in the listener's mode.
                    tcp.set_nonblocking(false).map_err(|e| e.to_string())?;
                    if wugs_net::wait_for_first_byte(&tcp, FIRST_BYTE_WAIT).is_ok() {
                        return Ok(tcp);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= self.deadline {
                        return Err(PairError::Expired.to_string());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => return Err(format!("accept the joining device: {e}")),
            }
        }
    }
}

/// Run the pairing exchange with the joiner that connected and enrol it.
/// `pair_host` persists the enrolment before the joiner hears of it.
pub fn pair_finish(session: &mut Session, key: &DeviceKey, mut pairing: HostPairing, mut tcp: TcpStream) -> Result<DeviceRow, String> {
    let p = project(session)?;
    if p.project_id != pairing.project_id {
        return Err("the project changed while the pairing code was shown; show a new code".into());
    }
    let watchdog = pair_io(&tcp)?;
    let (file, mut roster) = load_roster(p)?;
    let rec = pair_host(&mut tcp, &mut pairing.code, key, &mut roster, pairing.guest, &mut |r| file.append(std::slice::from_ref(r)))
        .map_err(|e| pair_failure(&watchdog, e))?;
    let Body::Enrol { key: joined, .. } = &rec.record().body else {
        return Err("pairing produced a record that is not an enrolment".into());
    };
    Ok(device_row(&roster.state(), &key.public(), joined))
}

/// Per-read limits, and the wall clock over the whole exchange.
fn pair_io(tcp: &TcpStream) -> Result<Watchdog, String> {
    tcp.set_read_timeout(Some(PAIR_IO_TIMEOUT)).map_err(|e| e.to_string())?;
    tcp.set_write_timeout(Some(PAIR_IO_TIMEOUT)).map_err(|e| e.to_string())?;
    Watchdog::arm(tcp, PAIR_DEADLINE)
}

fn pair_failure(watchdog: &Watchdog, e: PairError) -> String {
    if watchdog.fired() {
        format!("the other device took longer than {} s to pair, so the connection was closed", PAIR_DEADLINE.as_secs())
    } else {
        e.to_string()
    }
}

fn resolve(addr: &str) -> Result<SocketAddr, String> {
    addr.trim()
        .to_socket_addrs()
        .map_err(|e| format!("{addr:?} is not an address (host:port): {e}"))?
        .next()
        .ok_or_else(|| format!("{addr} resolves to no address"))
}

/// Join a project another device is showing a code for, into a new folder
/// at `path`, and open it. Whether the device is a guest is the host's
/// choice; the result says what it chose.
pub fn pair_join(
    session: &mut Session,
    key: &DeviceKey,
    path: &str,
    researcher: &str,
    addr: &str,
    code: &str,
    label: &str,
) -> Result<JoinResult, String> {
    if researcher.trim().is_empty() {
        return Err("a researcher id is required".into());
    }
    let lock = lock_folder(path)?;
    let (project_id, standing) = match earlier_join(path, key)? {
        Some(earlier) => earlier,
        None => {
            if Path::new(path).join("project.json").exists() {
                return Err(format!("{path} already holds a wugs project; join into an empty folder"));
            }
            let addr = resolve(addr)?;
            let mut tcp = TcpStream::connect_timeout(&addr, PAIR_IO_TIMEOUT).map_err(|e| format!("connect {addr}: {e}"))?;
            let watchdog = pair_io(&tcp)?;
            let joined = wugs_auth::pair_join(&mut tcp, code, key, label).map_err(|e| pair_failure(&watchdog, e))?;
            RosterFile::in_project(path).create(&joined.roster)?;
            (joined.project_id, joined.standing)
        }
    };
    let project = if Path::new(path).join("project.json").exists() {
        Project::open_with(path, researcher, engines())?
    } else {
        Project::open_joined_with(path, researcher, &project_id, engines())?
    };
    if project.project_id != project_id {
        return Err(format!("{path} holds project {} but its roster is for {project_id}; join into an empty folder", project.project_id));
    }
    let project = info(session.insert(OpenProject { project, peaks: Vec::new(), _lock: lock }), path)?;
    Ok(JoinResult { project, standing: standing.into() })
}

/// A join into `path` that got as far as writing the roster and then
/// stopped (a crash, or the folder could not be opened). The host enrolled
/// this device already, and pairing again would enrol it twice, so the
/// join resumes from the roster without touching the network. Which step
/// stopped it does not matter: a folder whose roster enrols this device
/// is opened as the project that roster names.
fn earlier_join(path: &str, key: &DeviceKey) -> Result<Option<(String, Standing)>, String> {
    let file = RosterFile::in_project(path);
    if !file.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(file.path()).map_err(|e| format!("read {}: {e}", file.path().display()))?;
    let genesis = text.lines().find(|l| !l.trim().is_empty()).ok_or("the folder's roster is empty; join into an empty folder")?;
    let project_id = SignedRecord::from_line(genesis)?.record().project.clone();
    match file.load(&project_id)?.state().standing(&key.public()) {
        s @ (Standing::Member | Standing::Guest) => Ok(Some((project_id, s))),
        _ => Err(format!("{path} holds a roster that does not enrol this device; join into an empty folder")),
    }
}

fn sync_result(r: ClientReport) -> SyncResult {
    SyncResult {
        peer_label: r.peer.as_ref().map(|p| p.label.clone()),
        peer_guest: r.peer.as_ref().is_some_and(|p| p.role == Role::Guest),
        pulled_ops: r.pulled.ops,
        answered: r.answered,
        txns_held: r.pulled.ingest.txns_held,
        conflicts: r.pulled.ingest.conflicts,
        warnings: r.warnings.iter().map(Warning::to_string).collect(),
    }
}

/// One authenticated sync with the device at `addr`, both directions.
pub fn sync_now(session: &mut Session, key: &DeviceKey, addr: &str) -> Result<SyncResult, String> {
    let addr = resolve(addr)?;
    let p = project(session)?;
    wugs_net::sync_with(p, addr, key, &default_policy).map(sync_result)
}

/// Unauthenticated sync, for development only: anyone at `addr` gets everything.
pub fn sync_now_open(session: &mut Session, addr: &str) -> Result<SyncResult, String> {
    let addr = resolve(addr)?;
    let p = project(session)?;
    wugs_net::sync_open(p, addr, &SyncPolicy::allow_all()).map(sync_result)
}

/// The port this device serves sync on, bound for the open project.
pub struct SyncListener {
    pub project_id: String,
    pub device_id: String,
    pub listener: TcpListener,
}

pub fn sync_listen(session: &mut Session, key: &DeviceKey, label: &str) -> Result<SyncListener, String> {
    let p = project(session)?;
    found_roster(p, key, label)?;
    let listener = TcpListener::bind(("0.0.0.0", 0)).map_err(|e| format!("listen for sync: {e}"))?;
    Ok(SyncListener { project_id: p.project_id.clone(), device_id: p.device().to_string(), listener })
}

/// Serve one sync session on a connection the listener thread accepted.
/// `serve_stream` bounds it by `wugs_net::SESSION_DEADLINE`, and an
/// unauthenticated peer by `HANDSHAKE_DEADLINE`.
pub fn serve_connection(session: &mut Session, key: &DeviceKey, project_id: &str, conn: TcpStream) -> Result<ServedSession, String> {
    let p = project(session)?;
    if p.project_id != project_id {
        return Err("a sync connection arrived for a project that is no longer open".into());
    }
    let s = wugs_net::serve_stream(p, conn, key, &default_policy);
    let (outcome, pulled_ops) = match s.outcome {
        SessionOutcome::Converged(t) => ("converged".to_string(), t.ops),
        SessionOutcome::PeerLeft => ("the other device left before the session finished".to_string(), 0),
        SessionOutcome::Refused(r) => (format!("refused: {r}"), 0),
        SessionOutcome::Failed(m) => (format!("failed: {m}"), 0),
    };
    Ok(ServedSession {
        peer_label: s.peer.map(|p| p.label),
        outcome,
        pulled_ops,
        warnings: s.warnings.iter().map(Warning::to_string).collect(),
    })
}

/// Give a copied project folder its own device id (`Project::mint_device_id`).
/// The next sync attests the new id to this device's key.
pub fn mint_device_identity(session: &mut Session, key: &DeviceKey) -> Result<DevicesView, String> {
    project(session)?.mint_device_id()?;
    devices_list(session, key)
}

/// "I moved this folder": keep the device id, record the new path.
pub fn relocation_accept_move(session: &mut Session) -> Result<(), String> {
    project(session)?.relocation_accept_move();
    Ok(())
}
