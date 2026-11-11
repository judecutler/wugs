//! wugs-core: the Project facade — the ONLY write path into a wugs project.
//!
//! All edits are domain intents that emit primitive ops sharing a txn id
//! (DATA-MODEL.md §2.2). Local commits: validators warn-only. Incoming sync:
//! quarantine per txn inside a SAVEPOINT; any Hold finding rolls the txn back
//! and records it (DATA-MODEL.md §6).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use wugs_model::{
    base64, json, new_id, parse_blob_ref, scan_blob_refs, Clock, Hlc, Normalizer, Op, TextMerger, Value, SCHEMA_VERSION,
};
use wugs_oplog::LogDir;
use wugs_projection::{ApplyStats, Projection};
pub use wugs_projection::{HiddenEdit, OpenConflict, Version};
use wugs_validate::{default_validators, has_hold, run_all, Validator};

pub mod assist;
pub mod conflicts;
mod elicitation;
mod gloss;
mod lexicon;
pub mod people;
mod segments;
pub mod settings;
pub use wugs_validate::{Finding, Severity};
pub use gloss::{lcs_pairs, AppliedToUnanalysed, LexiconMatch, MorphSpec, Retokenized, Suggestion};
pub use segments::{SegmentCheck, SegmentState, MACHINE_SEGMENT_INTENT};
pub use lexicon::{domain_id, DomainRow, ImportedDomains};
pub use elicitation::{concept_id, concept_list_id, ConceptRow, ImportedConcepts};

/// Fixed project-level scopes (DATA-MODEL.md §4). Source/text scopes use the
/// source's or text's own id.
pub const SCOPE_LEXICON: &str = "lexicon";
pub const SCOPE_CONFIG: &str = "project";
pub const SCOPE_PEOPLE: &str = "people";

pub struct Project {
    pub root: PathBuf,
    text_peer: u64,
    pub log: LogDir,
    pub proj: Projection,
    pub project_id: String,
    pub actor: String,
    clock: Clock,
    validators: Vec<Box<dyn Validator>>,
    relocation: Option<Relocation>,
    caught_up: usize,
}

/// `cache/` is not the directory `cache/instance.json` recorded: the folder was
/// copied (two copies writing one device id fork that device's log). Detected
/// at open, before any sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relocation {
    pub stored_path: String,
    /// The recorded path still holds a project with this id: this folder is
    /// a copy, not a move.
    pub original_still_present: bool,
}

#[derive(Debug)]
pub struct CommitResult {
    pub txn: String,
    pub ops: usize,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Default)]
pub struct IngestReport {
    pub txns_applied: usize,
    pub txns_held: usize,
    pub ops_applied: usize,
    /// ops dropped because this device already applied them (idempotent re-offer)
    pub ops_skipped_dupes: usize,
    /// Conflict rows this call opened (DATA-MODEL §5.3): a row that was
    /// already open before the call is not counted again, and a row closed
    /// by this call is not subtracted. `Projection::count("conflicts")` is
    /// the number open now.
    pub conflicts: usize,
    /// previously-held txns that applied during this call's retry pass
    pub txns_retried: usize,
    /// txns still held when this call returned: (txn, reason)
    pub held: Vec<(String, String)>,
}

/// The edge engines a project runs with, supplied at open (`Project::open_with`).
#[derive(Default)]
pub struct Engines {
    /// Collaborative text (DATA-MODEL §2.3). None = text-edit fields take the
    /// spec's LWW degradation path.
    pub merger: Option<Box<dyn TextMerger>>,
    /// NFC at entry (PRD §7) for vernacular text written through intents.
    /// Replay applies ops verbatim, because the log is the truth as written;
    /// the projection still compares through it, so older NFD text matches.
    pub normalizer: Option<Box<dyn Normalizer>>,
}

enum TxnOutcome {
    Applied(ApplyStats),
    Held(String),
}

impl Project {
    /// Open (or initialize) a project directory with no engines: text-edit
    /// fields take the LWW degradation path and text is stored as typed.
    /// `actor` is the person id of the researcher at the keyboard.
    pub fn open(root: impl Into<PathBuf>, actor: &str) -> Result<Project, String> {
        Project::open_with(root, actor, Engines::default())
    }

    /// Open with engines, installed before any replay so a rebuilt cache
    /// reflects merged text and NFC-folded lookup keys. An app opens every
    /// project this way, so no open path can forget one.
    pub fn open_with(root: impl Into<PathBuf>, actor: &str, engines: Engines) -> Result<Project, String> {
        Project::open_inner(root.into(), actor, engines)
    }

    /// Join an existing project from another device: the directory must be
    /// fresh, and it adopts the given project id so sync peers recognize it
    /// (the wire protocol refuses to merge two different projects).
    pub fn open_joined(root: impl Into<PathBuf>, actor: &str, project_id: &str) -> Result<Project, String> {
        Project::open_joined_with(root, actor, project_id, Engines::default())
    }

    /// `open_joined` with engines, as `open_with`.
    pub fn open_joined_with(root: impl Into<PathBuf>, actor: &str, project_id: &str, engines: Engines) -> Result<Project, String> {
        Project::open_joined_inner(root.into(), actor, project_id, engines)
    }

    fn open_joined_inner(root: PathBuf, actor: &str, project_id: &str, engines: Engines) -> Result<Project, String> {
        if root.join("project.json").exists() {
            return Err(format!("{} already holds a project", root.display()));
        }
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let v = Value::obj(vec![
            ("project_id", Value::str(project_id)),
            ("schema", Value::Num(SCHEMA_VERSION as f64)),
            ("format", Value::str("wugs-oplog/0.1")),
        ]);
        fs::write(root.join("project.json"), v.to_json()).map_err(|e| e.to_string())?;
        Project::open_inner(root, actor, engines)
    }

    fn open_inner(root: PathBuf, actor: &str, engines: Engines) -> Result<Project, String> {
        fs::create_dir_all(root.join("cache")).map_err(|e| e.to_string())?;
        fs::create_dir_all(root.join("blobs")).map_err(|e| e.to_string())?;

        let project_id = load_or_init_project_json(&root)?;
        // Device identity is local-only (cache/ is never synced), so a fresh
        // machine mints a fresh device id — exactly what we want.
        let device = load_or_init_device_id(&root)?;

        let log = LogDir::new(&root);
        let relocation = check_instance(&root, &project_id);
        let db_path = root.join("cache").join("projection.sqlite");
        let fresh = !db_path.exists();
        let mut proj = Projection::open(db_path.to_str().ok_or("non-utf8 path")?, engines.normalizer)?;
        // An old-schema cache was wiped: it must be rebuilt like a fresh one.
        let fresh = fresh || proj.wiped;
        proj.merger = engines.merger;

        let clock = Clock::new(device);
        let text_peer = text_hash(&format!("{}:{}", clock.device, wugs_model::ulid()));
        let mut project = Project {
            root,
            text_peer,
            log,
            proj,
            project_id,
            actor: actor.to_string(),
            clock,
            validators: default_validators(),
            relocation,
            caught_up: 0,
        };
        if fresh {
            project.replay()?;
            project.reload_quarantine()?;
        } else {
            project.caught_up = project.catch_up()?;
        }
        // fold the newest known timestamp into the clock so we issue after it
        if let Some(maxh) = project.proj.max_hlc()? {
            if let Some(h) = Hlc::parse(&maxh) {
                project.clock.absorb(&h);
            }
        }
        Ok(project)
    }

    /// Rebuild the projection from the log (the only repair path).
    pub fn replay(&mut self) -> Result<ApplyStats, String> {
        self.apply_log(true)
    }

    /// Apply every log op the cache lacks, in log order, in one transaction:
    /// a crash between a commit's log append and its savepoint release
    /// leaves ops in the log that the cache never saw. It is the replay run
    /// on a warm cache from where the last replay or catch-up stopped
    /// reading each file (`Projection::log_ends`), so it reads what the
    /// last session wrote, not the whole log; `Projection::apply` skips the
    /// applied ops among them and re-parks pending ones as a no-op. Unlike a
    /// rebuild it does not refuse a corrupt line: the cache already holds
    /// what it held, and opening must not get worse because the log did.
    fn catch_up(&mut self) -> Result<usize, String> {
        let stats = self.apply_log(false)?;
        // A crash inside a held txn's retry leaves its ops logged and its
        // hold recorded; the retry releases holds whose ops all applied.
        if stats.applied > 0 && self.proj.count("quarantine")? > 0 {
            self.retry_held()?;
        }
        Ok(stats.applied)
    }

    /// A rebuild reads every file whole; a catch-up reads from the recorded ends.
    fn apply_log(&mut self, rebuild: bool) -> Result<ApplyStats, String> {
        let starts = if rebuild { Default::default() } else { self.proj.log_ends()? };
        let mut stream = self.log.stream_from(&starts).map_err(|e| e.to_string())?;
        let mut stats = ApplyStats::default();
        self.proj.begin()?;
        let mut applied = Ok(());
        loop {
            let op = match stream.next_op() {
                Ok(Some(op)) => op,
                Ok(None) => break,
                Err(e) => {
                    applied = Err(e.to_string());
                    break;
                }
            };
            if let Err(e) = self.proj.apply(&op, &mut stats) {
                applied = Err(e);
                break;
            }
            if let Some(h) = Hlc::parse(&op.hlc) {
                self.clock.absorb(&h);
            }
        }
        if applied.is_ok() && rebuild && stream.corrupt_lines > 0 {
            applied = Err(format!(
                "log corruption: {} bad non-tail lines — refusing to replay silently",
                stream.corrupt_lines
            ));
        }
        if applied.is_ok() {
            applied = stream.ends().try_for_each(|(file, end)| self.proj.set_log_end(file, end));
        }
        if let Err(e) = applied {
            self.proj.rollback()?;
            return Err(e);
        }
        self.proj.commit()?;
        Ok(stats)
    }

    /// Ops the log held that the cache lacked when this project opened,
    /// applied then (`catch_up`). 0 after a clean close or a rebuild.
    pub fn caught_up(&self) -> usize {
        self.caught_up
    }

    /// Start a domain intent. All edits happen through this.
    pub fn intent(&mut self, name: &str) -> Txn<'_> {
        Txn {
            txn_id: new_id("txn"),
            intent: name.to_string(),
            ops: Vec::new(),
            project: self,
            session: None,
            resolving: false,
        }
    }

    /// Ingest ops from a peer (already scope-filtered by the sync layer).
    /// Groups by txn; each txn applies inside a savepoint, validators run on
    /// the would-be state, Hold findings roll it back into quarantine.
    pub fn ingest(&mut self, mut ops: Vec<Op>) -> Result<IngestReport, String> {
        ops.sort_by(|a, b| (a.hlc.as_str(), a.id.as_str()).cmp(&(b.hlc.as_str(), b.id.as_str())));
        // group into txns, preserving first-op order
        let mut order: Vec<String> = Vec::new();
        let mut groups: BTreeMap<String, Vec<Op>> = BTreeMap::new();
        for op in ops {
            if !groups.contains_key(&op.txn) {
                order.push(op.txn.clone());
            }
            groups.entry(op.txn.clone()).or_default().push(op);
        }

        let mut report = IngestReport::default();
        let mut held_now: BTreeMap<String, String> = BTreeMap::new();
        for txn_id in order {
            let mut txn_ops = groups.remove(&txn_id).unwrap();
            // Idempotence at the ingest boundary: an op this device already
            // applied must not be re-applied or re-mirrored into the log.
            // (A HELD txn's ops were never applied, so a re-offer still
            // re-validates — that path is untouched.)
            let before = txn_ops.len();
            let mut keep = Vec::with_capacity(before);
            for op in txn_ops.drain(..) {
                // A parked op is already in the log and will apply when its
                // target arrives; re-accepting it would append a duplicate
                // log line on every sync.
                if self.proj.already_applied(&op.id)? || self.proj.is_pending(&op.id)? {
                    report.ops_skipped_dupes += 1;
                } else {
                    keep.push(op);
                }
            }
            let txn_ops = keep;
            if txn_ops.is_empty() {
                continue;
            }
            match self.try_txn(&txn_ops)? {
                TxnOutcome::Applied(stats) => {
                    report.txns_applied += 1;
                    report.ops_applied += stats.applied;
                    report.conflicts += stats.conflicts;
                }
                TxnOutcome::Held(reason) => {
                    self.hold(&txn_id, &txn_ops, &reason)?;
                    held_now.insert(txn_id, reason);
                }
            }
        }
        // A txn held earlier may only have been waiting for ops that arrived
        // in this batch (or this batch's holds may chain), so retry held
        // txns until none makes progress. Summaries count held ops, so a
        // peer never re-sends them; this local retry is the only way they
        // get another chance (DATA-MODEL §6).
        report.txns_retried = self.retry_held_inner(&mut report, &mut held_now)?;
        report.txns_held = held_now.len();
        report.held = held_now.into_iter().collect();
        Ok(report)
    }

    /// Re-attempt every quarantined txn until a full pass holds them all.
    /// Returns how many applied. Also public for a UI "retry" action.
    pub fn retry_held(&mut self) -> Result<usize, String> {
        let mut report = IngestReport::default();
        let mut held_now = BTreeMap::new();
        self.retry_held_inner(&mut report, &mut held_now)
    }

    fn retry_held_inner(
        &mut self,
        report: &mut IngestReport,
        held_now: &mut BTreeMap<String, String>,
    ) -> Result<usize, String> {
        let mut retried = 0usize;
        loop {
            let mut progressed = false;
            for (txn_id, mut ops) in self.proj.held_txns(None)? {
                let mut keep = Vec::with_capacity(ops.len());
                for op in ops.drain(..) {
                    if !self.proj.already_applied(&op.id)? {
                        keep.push(op);
                    }
                }
                if keep.is_empty() {
                    // every op arrived some other way; nothing left to hold
                    self.proj.quarantine_release(&txn_id)?;
                    self.unspill(&txn_id);
                    held_now.remove(&txn_id);
                    progressed = true;
                    continue;
                }
                match self.try_txn(&keep)? {
                    TxnOutcome::Applied(stats) => {
                        self.proj.quarantine_release(&txn_id)?;
                        self.unspill(&txn_id);
                        held_now.remove(&txn_id);
                        report.txns_applied += 1;
                        report.ops_applied += stats.applied;
                        report.conflicts += stats.conflicts;
                        retried += 1;
                        progressed = true;
                    }
                    TxnOutcome::Held(reason) => {
                        self.quarantine(&txn_id, &reason)?;
                        if let Some(r) = held_now.get_mut(&txn_id) {
                            *r = reason;
                        }
                    }
                }
            }
            if !progressed {
                return Ok(retried);
            }
        }
    }

    /// Apply one txn atomically: validators on the would-be state, the log
    /// written before the savepoint is released (the projection must never
    /// commit state the log lacks; a log ahead of the projection heals on
    /// rebuild, the reverse cannot).
    fn try_txn(&mut self, txn_ops: &[Op]) -> Result<TxnOutcome, String> {
        // A txn from a clock more than a day ahead is held before anything
        // applies: committing it and then failing to observe its clock would
        // leave it in the log and projection with the ingest reporting an
        // error. Held, it retries on every later ingest and applies once this
        // device's clock is within range (DATA-MODEL §1).
        for op in txn_ops {
            if let Some(h) = Hlc::parse(&op.hlc) {
                if let Err(wugs_model::hlc::ClockError::DriftTooLarge { remote_millis, wall }) = Clock::check_drift(&h) {
                    let hours = (remote_millis - wall) as f64 / 3_600_000.0;
                    return Ok(TxnOutcome::Held(format!(
                        "clock drift: op {} is stamped {hours:.1} h ahead of this device's clock (more than 24 h): the device that wrote it has, or had, a clock running ahead. Held here; it applies by itself once this device's clock reaches that time, or sooner if this clock is the one that is behind",
                        op.id
                    )));
                }
            }
        }
        let mut stats = ApplyStats::default();
        self.proj.savepoint("ingest_txn")?;
        for op in txn_ops {
            if let Err(e) = self.proj.apply(op, &mut stats) {
                self.proj.rollback_to("ingest_txn")?;
                return Ok(TxnOutcome::Held(format!("apply error: {e}")));
            }
        }
        let findings = run_all(&self.validators, &self.proj, txn_ops)?;
        if has_hold(&findings) {
            self.proj.rollback_to("ingest_txn")?;
            let reason = findings
                .iter()
                .filter(|f| f.severity == wugs_validate::Severity::Hold)
                .map(|f| format!("{}: {}", f.code, f.message))
                .collect::<Vec<_>>()
                .join("; ");
            return Ok(TxnOutcome::Held(reason));
        }
        if let Err(e) = self.log.append_all(txn_ops) {
            self.proj.rollback_to("ingest_txn")?;
            return Err(e.to_string());
        }
        self.proj.release("ingest_txn")?;
        for op in txn_ops {
            if let Some(h) = Hlc::parse(&op.hlc) {
                clock_observe(&mut self.clock, &h)?;
            }
        }
        Ok(TxnOutcome::Applied(stats))
    }

    /// Record a held txn with its ops, so summaries count them and retries
    /// can replay them without a network re-offer. The ops are also spilled
    /// to quarantine/<txn>.jsonl: the projection is a disposable cache, and
    /// a held txn may exist nowhere else (the sender's laptop can be gone),
    /// so losing it to a cache wipe would violate the only-source-of-truth
    /// rule from the other side.
    fn hold(&mut self, txn_id: &str, ops: &[Op], reason: &str) -> Result<(), String> {
        self.quarantine(txn_id, reason)?;
        for op in ops {
            self.proj.quarantine_store(op)?;
        }
        let dir = self.root.join("quarantine");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut lines = String::new();
        for op in ops {
            lines.push_str(&op.to_json());
            lines.push('\n');
        }
        fs::write(dir.join(format!("{txn_id}.jsonl")), lines).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn unspill(&self, txn_id: &str) {
        fs::remove_file(self.root.join("quarantine").join(format!("{txn_id}.jsonl"))).ok();
    }

    /// Re-ingest spilled held txns after a cache rebuild: each either
    /// applies (its blocker may be gone) or holds again.
    fn reload_quarantine(&mut self) -> Result<(), String> {
        let dir = self.root.join("quarantine");
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };
        let mut ops = Vec::new();
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                ops.push(Op::from_json(line)?);
            }
        }
        if !ops.is_empty() {
            self.ingest(ops)?;
        }
        Ok(())
    }

    fn quarantine(&self, txn: &str, reason: &str) -> Result<(), String> {
        let mut s = self.proj.db.prepare(
            "INSERT OR REPLACE INTO quarantine(txn,status,reason) VALUES(?1,'held',?2)",
        )?;
        s.bind_text(1, txn)?;
        s.bind_text(2, reason)?;
        s.step()?;
        Ok(())
    }

    pub fn device(&self) -> &str {
        &self.clock.device
    }

    /// Set when this folder was opened away from where it was last
    /// recorded. Warn-only: nothing is blocked while it is pending.
    pub fn relocation(&self) -> Option<&Relocation> {
        self.relocation.as_ref()
    }

    /// "I moved this folder": record the current path and keep the device id.
    pub fn relocation_accept_move(&mut self) {
        self.relocation = None;
        write_instance(&self.root).ok();
    }

    /// Give this folder a fresh `dvc_…` id, for a project folder copied from
    /// another machine (two devices writing one log file is the hazard).
    /// The id lives in `cache/device`, which is local and disposable, so
    /// only that file changes: log files under the old id stay valid
    /// history, replay reads them like any other device's, and from now on
    /// this device writes under the new id. The clock keeps its high-water
    /// mark, so new ops still sort after everything already issued.
    pub fn mint_device_id(&mut self) -> Result<String, String> {
        let id = new_id("dvc");
        write_device_id(&self.root, &id)?;
        self.clock.device = id.clone();
        self.text_peer = text_hash(&format!("{}:{}", id, wugs_model::ulid()));
        self.relocation = None;
        write_instance(&self.root).ok();
        Ok(id)
    }

    /// The mark phase of blob GC: every blob ref that ever appeared in this
    /// project's op log, in a quarantine spill, or in a parked or held op.
    /// The log is the source of truth and the projection can trail it after
    /// a crash, so the mark reads raw log lines, torn tails included, and
    /// never asks the projection what is current. Overwritten values and
    /// conflict losers stay live: an archive keeps every version's media.
    pub fn blob_refs(&self) -> Result<Vec<String>, String> {
        let mut found = BTreeSet::new();
        for scope_dir in read_dir_if_any(&self.log.log_root())? {
            if scope_dir.is_dir() {
                for file in read_dir_if_any(&scope_dir)? {
                    scan_jsonl(&file, &mut found)?;
                }
            }
        }
        for file in read_dir_if_any(&self.root.join("quarantine"))? {
            scan_jsonl(&file, &mut found)?;
        }
        for sql in [
            "SELECT op_json FROM pending WHERE op_json LIKE '%blake3:%'",
            "SELECT op_json FROM quarantine_ops WHERE op_json LIKE '%blake3:%'",
        ] {
            let mut s = self.proj.db.prepare(sql)?;
            while s.step()? {
                scan_blob_refs(&s.col_text(0), &mut found);
            }
        }
        Ok(found.into_iter().collect())
    }

    /// This session's text-CRDT peer id: a hash of the device id plus a
    /// per-open nonce. Per-session, not per-device, on purpose: after a
    /// crash between the log append and the savepoint release, the
    /// projection can trail the log, and a stable peer would then reuse
    /// loro op ids and corrupt the field's lineage. A fresh peer per open
    /// makes those edits merge instead.
    pub fn text_peer(&self) -> u64 {
        self.text_peer
    }
}

/// Fields that take NFC at entry: the language-bearing ones, plus a
/// writing system's `charset` and `sort`, which spell out the orthography
/// and must compare equal to the NFC text they describe. The last two are
/// not searchable, so they are not in LANGUAGE_FIELDS.
fn takes_nfc(field: &str) -> bool {
    wugs_model::is_language_field(field) || matches!(field, "charset" | "sort")
}

impl Project {
    fn normalize(&self, field: &str, value: Value) -> Value {
        match (&self.proj.normalizer, &value) {
            (Some(n), Value::Str(s)) if takes_nfc(field) => {
                let fixed = n.nfc(s);
                if fixed == *s { value } else { Value::Str(fixed) }
            }
            _ => value,
        }
    }
}

/// Entries of a directory, or none when it does not exist yet.
fn read_dir_if_any(dir: &Path) -> Result<Vec<PathBuf>, String> {
    match fs::read_dir(dir) {
        Ok(entries) => entries.map(|e| e.map(|e| e.path()).map_err(|e| e.to_string())).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", dir.display())),
    }
}

/// Scan a `.jsonl` file line by line as text: a torn or unparseable line
/// still yields the refs it holds.
fn scan_jsonl(path: &Path, out: &mut BTreeSet<String>) -> Result<(), String> {
    if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return Ok(());
    }
    let f = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    for line in BufReader::new(f).split(b'\n') {
        let line = line.map_err(|e| format!("{}: {e}", path.display()))?;
        scan_blob_refs(&String::from_utf8_lossy(&line), out);
    }
    Ok(())
}

/// Stable peer-id hash (fnv-1a, with loro's reserved PeerID::MAX avoided).
fn text_hash(s: &str) -> u64 {
    let h = s
        .bytes()
        .fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    if h == u64::MAX { 0 } else { h }
}

fn clock_observe(clock: &mut Clock, h: &Hlc) -> Result<(), String> {
    clock
        .observe(h)
        .map_err(|e| format!("clock drift rejected: {e:?}"))
}

fn load_or_init_project_json(root: &Path) -> Result<String, String> {
    let path = root.join("project.json");
    if path.exists() {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let v = json::parse(&text).map_err(|e| e.to_string())?;
        v.get_str("project_id")
            .map(str::to_string)
            .ok_or_else(|| "project.json missing project_id".to_string())
    } else {
        let id = new_id("prj");
        let v = Value::obj(vec![
            ("project_id", Value::str(&id)),
            ("schema", Value::Num(SCHEMA_VERSION as f64)),
            ("format", Value::str("wugs-oplog/0.1")),
        ]);
        fs::write(&path, v.to_json()).map_err(|e| e.to_string())?;
        Ok(id)
    }
}

fn load_or_init_device_id(root: &Path) -> Result<String, String> {
    let path = root.join("cache").join("device");
    if path.exists() {
        Ok(fs::read_to_string(&path)
            .map_err(|e| e.to_string())?
            .trim()
            .to_string())
    } else {
        let id = new_id("dvc");
        write_device_id(root, &id)?;
        Ok(id)
    }
}

fn write_device_id(root: &Path, id: &str) -> Result<(), String> {
    write_cache_file(root, "device", id.as_bytes())
}

/// Temp file, fsync, rename: a crash leaves the old content or the new,
/// never an empty file that the next open would read as a device id.
fn write_cache_file(root: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let path = root.join("cache").join(name);
    let tmp = root.join("cache").join(format!("{name}.{}.tmp", wugs_model::ulid()));
    let mut f = fs::File::create(&tmp).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::io::Write::write_all(&mut f, bytes).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| format!("save {}: {e}", path.display()))
}

/// Identity of the `cache/` directory itself. A recursive copy gets a new
/// one; a move or a stick that mounts elsewhere keeps it. Unix uses the inode
/// and deliberately not the device number, which differs per machine.
#[cfg(unix)]
fn fs_id(root: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(root.join("cache")).ok().map(|m| m.ino().to_string())
}

/// Windows: the creation time survives a same-volume move and changes on
/// every copy. Not exercised by this repo's tests, which run on Unix.
#[cfg(windows)]
fn fs_id(root: &Path) -> Option<String> {
    use std::os::windows::fs::MetadataExt;
    fs::metadata(root.join("cache")).ok().map(|m| m.creation_time().to_string())
}

#[cfg(not(any(unix, windows)))]
fn fs_id(_root: &Path) -> Option<String> {
    None
}

/// Records where this folder lives and which directory it is, so a later
/// open can tell a copy from a move.
fn write_instance(root: &Path) -> Result<(), String> {
    let canonical = fs::canonicalize(root).map_err(|e| format!("resolve {}: {e}", root.display()))?;
    let id = fs_id(root).ok_or("no filesystem identity for cache/")?;
    let v = Value::obj(vec![("root", Value::str(&canonical.to_string_lossy())), ("fs_id", Value::str(&id))]);
    write_cache_file(root, "instance.json", v.to_json().as_bytes())
}

/// A relocation is reported only when the recorded `cache/` identity differs
/// from this open's. A path change alone refreshes the marker silently. A
/// missing, old-format or unparseable marker is rewritten, not reported, and
/// nothing here can fail an open. A pending relocation leaves the marker
/// alone until it is resolved.
fn check_instance(root: &Path, project_id: &str) -> Option<Relocation> {
    let (Some(current_id), Ok(current)) = (fs_id(root), fs::canonicalize(root)) else {
        return None;
    };
    let stored = fs::read_to_string(root.join("cache").join("instance.json"))
        .ok()
        .and_then(|t| wugs_model::json::parse(&t).ok())
        .and_then(|v| Some((v.get_str("root")?.to_string(), v.get_str("fs_id")?.to_string())));
    let Some((stored_path, stored_id)) = stored else {
        write_instance(root).ok();
        return None;
    };
    if stored_id == current_id {
        if Path::new(&stored_path) != current {
            write_instance(root).ok();
        }
        return None;
    }
    let stored = PathBuf::from(&stored_path);
    let original_still_present = fs::canonicalize(&stored).is_ok_and(|c| c != current) && holds_project(&stored, project_id);
    Some(Relocation { stored_path, original_still_present })
}

fn holds_project(dir: &Path, project_id: &str) -> bool {
    fs::read_to_string(dir.join("project.json"))
        .ok()
        .and_then(|t| wugs_model::json::parse(&t).ok())
        .is_some_and(|v| v.get_str("project_id") == Some(project_id))
}

// ------------------------------------------------------------------ Txn

pub struct Txn<'a> {
    txn_id: String,
    intent: String,
    ops: Vec<Op>,
    project: &'a mut Project,
    session: Option<String>,
    resolving: bool,
}

impl<'a> Txn<'a> {
    pub fn with_session(mut self, session: &str) -> Self {
        self.session = Some(session.to_string());
        self
    }

    /// Mark this txn as a person's direct answer to what the field shows:
    /// its writes name every open version of the field (`heads`), so they
    /// close the field's conflicts on every device (DATA-MODEL §5.3). Only
    /// direct edits and picks use this; bulk, derived and import writes stay
    /// plain and name only the `base` they overwrite, so they never settle a
    /// conflict nobody looked at.
    pub fn resolving(mut self) -> Self {
        self.resolving = true;
        self
    }

    fn envelope(&mut self, verb: &str, entity: &str, target: &str, scope: &str) -> Op {
        Op {
            id: new_id("op"),
            hlc: self.project.clock.now().encode(),
            txn: self.txn_id.clone(),
            intent: Some(self.intent.clone()),
            actor: self.project.actor.clone(),
            device: self.project.clock.device.clone(),
            session: self.session.clone(),
            scope: scope.to_string(),
            schema: SCHEMA_VERSION,
            verb: verb.to_string(),
            entity: entity.to_string(),
            target: target.to_string(),
            field: None,
            value: Value::Null,
            base: None,
            heads: None,
        }
    }

    pub fn create(
        &mut self,
        entity: &str,
        prefix: &str,
        scope: &str,
        fields: Vec<(&str, Value)>,
    ) -> String {
        let id = new_id(prefix);
        self.push_create(entity, &id, scope, fields);
        id
    }

    /// Create an entity that is its own scope (a source or a text), so the
    /// entity and its first fields land in one txn with no orphan window.
    pub fn create_scope_root(&mut self, entity: &str, prefix: &str, fields: Vec<(&str, Value)>) -> String {
        let id = new_id(prefix);
        self.push_create(entity, &id, &id, fields);
        id
    }

    /// The one path every create op takes, so no create skips NFC.
    fn push_create(&mut self, entity: &str, id: &str, scope: &str, fields: Vec<(&str, Value)>) {
        let fields = fields
            .into_iter()
            .map(|(k, v)| (k, self.project.normalize(k, v)))
            .collect();
        let mut op = self.envelope("create", entity, id, scope);
        op.value = Value::obj(fields);
        self.ops.push(op);
    }

    /// Name what this write supersedes (DATA-MODEL §2.2): `base` is the
    /// winner it overwrites; a resolving txn's `heads` are every open version
    /// of the field, a plain txn's only `[base]`. A value written earlier in
    /// this txn is the only version the txn sees.
    fn supersede(&self, op: &mut Op, target: &str, field: &str, resolving: bool) -> Result<(), String> {
        let in_txn = self.ops.iter().rev().find(|o| {
            o.target == target
                && (o.field.as_deref() == Some(field) || (o.verb == "create" && o.value.get(field).is_some()))
        });
        let (base, heads) = match in_txn {
            Some(o) => (Some(o.hlc.clone()), vec![o.hlc.clone()]),
            None => {
                let proj = &self.project.proj;
                let base = proj.field(target, field)?.map(|(_, hlc)| hlc);
                let heads = if resolving { proj.heads(target, field)? } else { base.iter().cloned().collect() };
                (base, heads)
            }
        };
        op.base = base;
        op.set_heads(heads);
        Ok(())
    }

    pub fn set(&mut self, entity: &str, target: &str, scope: &str, field: &str, value: Value) -> Result<(), String> {
        let resolving = self.resolving;
        self.set_with(entity, target, scope, field, value, resolving)
    }

    /// One resolving `set` inside an otherwise plain txn: a field the person
    /// just typed, written by an intent whose other writes are derived.
    pub fn set_resolving(&mut self, entity: &str, target: &str, scope: &str, field: &str, value: Value) -> Result<(), String> {
        self.set_with(entity, target, scope, field, value, true)
    }

    fn set_with(&mut self, entity: &str, target: &str, scope: &str, field: &str, value: Value, resolving: bool) -> Result<(), String> {
        let mut op = self.envelope("set", entity, target, scope);
        op.field = Some(field.to_string());
        op.value = self.project.normalize(field, value);
        self.supersede(&mut op, target, field, resolving)?;
        self.ops.push(op);
        Ok(())
    }

    /// Collaborative text edit (DATA-MODEL §2.3). With a merger, the op
    /// carries a CRDT update plus the resolved text; without one it carries
    /// only the resolved text and merges by LWW. The edit is computed
    /// against the field's applied updates, so make one text_edit per field
    /// per txn.
    pub fn text_edit(
        &mut self,
        entity: &str,
        target: &str,
        scope: &str,
        field: &str,
        new_text: &str,
    ) -> Result<(), String> {
        // A second edit in one txn would be diffed against a document that
        // lacks the first and collide with its op ids.
        if self.ops.iter().any(|o| {
            o.verb == "text-edit" && o.target == target && o.field.as_deref() == Some(field)
        }) {
            return Err(format!("one text-edit per field per txn ({target}.{field})"));
        }
        let new_text = match self.project.normalize(field, Value::str(new_text)) {
            Value::Str(t) => t,
            _ => new_text.to_string(),
        };
        let new_text = new_text.as_str();
        let value = match self.project.proj.merger.as_deref() {
            Some(m) => {
                let updates = self.project.proj.text_updates(target, field)?;
                // Genesis seed: the op that wrote the current plain value.
                // Same op on two devices => identical genesis, deduped;
                // different ops => distinct geneses that merge (TextMerger).
                let prior = self
                    .project
                    .proj
                    .field_with_op(target, field)?
                    .and_then(|(v, _, op_id)| v.as_str().map(|t| (t.to_string(), text_hash(&op_id))));
                let peer = self.project.text_peer();
                match m.edit(&updates, prior.as_ref().map(|(t, s)| (t.as_str(), *s)), new_text, peer) {
                    Ok((update, resolved)) => Value::obj(vec![
                        ("loro", Value::str(&base64::encode(&update))),
                        ("resolved", Value::str(&resolved)),
                    ]),
                    // The field's updates no longer merge as a set (DATA-MODEL
                    // §5.4): write the plain text on the degradation path, so
                    // the edit, and the conflicts it settles, still go through.
                    // Any other failure is a real error on a field that merges.
                    Err(_) if m.resolve(&updates).is_err() => Value::obj(vec![("resolved", Value::str(new_text))]),
                    Err(e) => return Err(e),
                }
            }
            None => Value::obj(vec![("resolved", Value::str(new_text))]),
        };
        let mut op = self.envelope("text-edit", entity, target, scope);
        op.field = Some(field.to_string());
        op.value = value;
        self.supersede(&mut op, target, field, self.resolving)?;
        self.ops.push(op);
        Ok(())
    }

    /// Write a segment's `tokens` or a token's `morphs` list (DATA-MODEL §3)
    /// as one field, naming the lists this device sees, so concurrent lists
    /// from two devices conflict on that field. Nothing is written when
    /// the list already holds exactly these ids.
    pub(crate) fn set_members(&mut self, parent_kind: &str, parent: &str, scope: &str, ids: &[String]) -> Result<(), String> {
        let field = if parent_kind == "segment" { "tokens" } else { "morphs" };
        let value = Value::Arr(ids.iter().map(|id| Value::str(id)).collect());
        let current = self.project.proj.field(parent, field)?.map(|(v, _)| v);
        if current.as_ref() != Some(&value) {
            self.set(parent_kind, parent, scope, field, value)?;
        }
        Ok(())
    }

    pub fn delete(&mut self, entity: &str, target: &str, scope: &str) {
        let op = self.envelope("delete", entity, target, scope);
        self.ops.push(op);
    }

    /// Lift a tombstone (the inverse of `delete`; DATA-MODEL §2).
    pub fn restore(&mut self, entity: &str, target: &str, scope: &str) {
        let op = self.envelope("restore", entity, target, scope);
        self.ops.push(op);
    }

    /// Commit locally: apply (atomically), run validators warn-only, append
    /// to this device's log. Local work is never blocked (CLAUDE.md).
    pub fn commit(self) -> Result<CommitResult, String> {
        let Txn {
            txn_id,
            ops,
            project,
            ..
        } = self;
        if ops.is_empty() {
            return Ok(CommitResult {
                txn: txn_id,
                ops: 0,
                findings: Vec::new(),
            });
        }
        let mut stats = ApplyStats::default();
        project.proj.savepoint("local_txn")?;
        for op in &ops {
            if let Err(e) = project.proj.apply(op, &mut stats) {
                project.proj.rollback_to("local_txn")?;
                return Err(e);
            }
        }
        let findings = run_all(&project.validators, &project.proj, &ops)?;
        // Same ordering rule as ingest: log first, then release, so the
        // projection never holds state the log lacks.
        if let Err(e) = project.log.append_all(&ops) {
            project.proj.rollback_to("local_txn")?;
            return Err(e.to_string());
        }
        project.proj.release("local_txn")?;
        // A local edit may be exactly the fix a held txn was waiting for
        // (e.g. retiming away an overlap); without this, the held txn only
        // got another chance when something arrived over sync.
        if project.proj.count("quarantine")? > 0 {
            project.retry_held()?;
        }
        Ok(CommitResult {
            txn: txn_id,
            ops: ops.len(),
            findings,
        })
    }
}

// --------------------------------------------------- domain conveniences

impl Project {
    /// A live entity of the expected kind, else an error. The conveniences
    /// below verify their targets so a bad id from a UI or script cannot
    /// write ops into a scope log that can never be removed.
    fn require(&self, id: &str, kind: &str) -> Result<(), String> {
        match self.proj.entity_kind(id)? {
            Some(k) if k == kind => {
                if self.proj.is_tombstoned(id)? {
                    Err(format!("{kind} {id} is deleted"))
                } else {
                    Ok(())
                }
            }
            Some(k) => Err(format!("{id} is a {k}, not a {kind}")),
            None => Err(format!("no such {kind}: {id}")),
        }
    }

    /// The text that owns a segment, token or morph, via the projected
    /// parent chain.
    fn text_of(&self, id: &str) -> Result<Option<String>, String> {
        let mut cur = id.to_string();
        for _ in 0..4 {
            match self.proj.entity_kind(&cur)?.as_deref() {
                Some("segment") => return self.proj.field_str(&cur, "text"),
                Some("token") => match self.proj.field_str(&cur, "segment")? {
                    Some(seg) => cur = seg,
                    None => return Ok(None),
                },
                Some("morph") => match self.proj.field_str(&cur, "token")? {
                    Some(tok) => cur = tok,
                    None => return Ok(None),
                },
                _ => return Ok(None),
            }
        }
        Ok(None)
    }

    /// The target belongs to this text; the text is the op scope, so a
    /// mismatch would log one text's data under another text's ACL scope.
    fn require_in_text(&self, id: &str, kind: &str, text: &str) -> Result<(), String> {
        self.require(text, "text")?;
        self.require(id, kind)?;
        if self.text_of(id)?.as_deref() != Some(text) {
            return Err(format!("{kind} {id} does not belong to text {text}"));
        }
        Ok(())
    }

    /// Lexicon: new entry with one sense. Returns (lexeme, sense); `add_lexeme`
    /// also returns the commit's findings (a duplicate lemma, validator #8).
    pub fn create_lexeme(&mut self, lemma: &str, gloss: &str) -> Result<(String, String), String> {
        Ok(self.add_lexeme(lemma, gloss)?.0)
    }

    /// A born-written or transcribed text. The text's own id is its scope.
    pub fn create_text(&mut self, title: &str) -> Result<String, String> {
        let mut t = self.intent("create-text");
        let id = t.create_scope_root("text", "txt", vec![("title", Value::str(title))]);
        t.commit()?;
        Ok(id)
    }

    /// Returns the new segment's id together with the commit's findings, so
    /// a UI can surface warnings (e.g. tier-overlap) on adds, not only edits.
    pub fn add_segment(
        &mut self,
        text: &str,
        tier: &str,
        index: usize,
        start_ms: Option<f64>,
        end_ms: Option<f64>,
        content: &str,
    ) -> Result<(String, CommitResult), String> {
        self.require(text, "text")?;
        let mut t = self.intent("add-segment");
        let mut fields = vec![
            ("text", Value::str(text)),
            ("tier", Value::str(tier)),
            ("pos", Value::str(&wugs_model::pos_seq(index))),
            ("content", Value::str(content)),
        ];
        if let Some(s) = start_ms {
            fields.push(("start", Value::Num(s)));
        }
        if let Some(e) = end_ms {
            fields.push(("end", Value::Num(e)));
        }
        let seg = t.create("segment", "seg", text, fields);
        let res = t.commit()?;
        Ok((seg, res))
    }

    /// Retime a segment (the timeline's drag handles). Only the edges given
    /// are written, so two devices moving different edges never overwrite
    /// each other. Validators stay warn-only: an overlap is committed and
    /// reported, never blocked.
    pub fn set_segment_times(
        &mut self,
        text: &str,
        segment: &str,
        start_ms: Option<f64>,
        end_ms: Option<f64>,
    ) -> Result<CommitResult, String> {
        if start_ms.is_none() && end_ms.is_none() {
            return Err("set_segment_times needs a start or an end".into());
        }
        self.require_in_text(segment, "segment", text)?;
        let mut t = self.intent("retime-segment").resolving();
        if let Some(s) = start_ms {
            t.set("segment", segment, text, "start", Value::Num(s))?;
        }
        if let Some(e) = end_ms {
            t.set("segment", segment, text, "end", Value::Num(e))?;
        }
        t.commit()
    }

    /// Tombstone a segment. The op log keeps its history (never a hard
    /// delete), so no undo machinery is needed on top.
    pub fn delete_segment(&mut self, text: &str, segment: &str) -> Result<CommitResult, String> {
        self.require_in_text(segment, "segment", text)?;
        let mut t = self.intent("delete-segment");
        t.delete("segment", segment, text);
        t.commit()
    }

    /// A TierDef in the project-config scope (DATA-MODEL §3). `kind` is the
    /// ELAN tier type; `role` says what the tier is for (`transcription`,
    /// `translation`, `note`, …) and `language` points at the Language it is
    /// in. Core checks only that each is non-empty when given: the
    /// vocabulary belongs to kits. Kit-level templates live in the shell and
    /// map onto a name, a role and one of these kinds.
    pub fn add_tier(
        &mut self,
        name: &str,
        kind: &str,
        role: Option<&str>,
        language: Option<&str>,
    ) -> Result<(String, CommitResult), String> {
        if !matches!(kind, "time-aligned" | "symbolic-subdivision" | "symbolic-association") {
            return Err(format!("unknown tier kind {kind:?}"));
        }
        if name.trim().is_empty() {
            return Err("a tier needs a name".into());
        }
        let mut fields = vec![("name", Value::str(name)), ("kind", Value::str(kind))];
        for (field, given) in [("role", role), ("language", language)] {
            if let Some(v) = given {
                if v.trim().is_empty() {
                    return Err(format!("a tier {field} must not be empty"));
                }
                fields.push((field, Value::str(v)));
            }
        }
        let mut t = self.intent("add-tier");
        let id = t.create("tier", "tir", SCOPE_CONFIG, fields);
        let res = t.commit()?;
        Ok((id, res))
    }

    /// Break a segment into tokens, one per form, in order. A segment that
    /// already has tokens is diffed like `retokenize`, so a second "Break
    /// into words" (or one racing on another device) never doubles them.
    /// Returns the token ids with the commit's findings.
    pub fn tokenize(&mut self, text: &str, segment: &str, forms: &[&str]) -> Result<(Vec<String>, CommitResult), String> {
        let r = self.write_tokens("tokenize", text, segment, forms)?;
        Ok((r.tokens, r.commit))
    }

    /// Add morphs to the end of a token: &[(surface, type)]. Returns the new
    /// morph ids with the commit's findings (e.g. morphs that do not
    /// concatenate to the form).
    pub fn split_morph(
        &mut self,
        text: &str,
        token: &str,
        parts: &[(&str, &str)],
    ) -> Result<(Vec<String>, CommitResult), String> {
        self.require_in_text(token, "token", text)?;
        let mut members = self.proj.members(token)?;
        let mut t = self.intent("split-morph");
        let mut out = Vec::new();
        for (surface, mtype) in parts {
            let pos = wugs_model::pos_seq(members.len() + out.len());
            out.push(t.create(
                "morph",
                "mor",
                text,
                vec![
                    ("token", Value::str(token)),
                    ("pos", Value::str(&pos)),
                    ("surface", Value::str(surface)),
                    ("type", Value::str(mtype)),
                ],
            ));
        }
        members.extend(out.iter().cloned());
        t.set_members("token", token, text, &members)?;
        let res = t.commit()?;
        Ok((out, res))
    }

    /// Set a morph's gloss: the primary analysis language by default, or
    /// the sibling field `gloss:<lang>` for another one.
    pub fn set_gloss(
        &mut self,
        text: &str,
        morph: &str,
        gloss: &str,
        lang: Option<&str>,
    ) -> Result<CommitResult, String> {
        self.require_in_text(morph, "morph", text)?;
        let field = gloss_field(lang)?;
        let mut t = self.intent("set-gloss").resolving();
        t.set("morph", morph, text, &field, Value::str(gloss))?;
        t.commit()
    }

    pub fn link_sense(&mut self, text: &str, morph: &str, sense: &str) -> Result<CommitResult, String> {
        self.require_in_text(morph, "morph", text)?;
        let mut t = self.intent("link-sense").resolving();
        t.set("morph", morph, text, "sense", Value::str(sense))?;
        t.commit()
    }

    // People, consent and provenance (DATA-MODEL §3, "People, ethics,
    // context"). All live in the people scope, the most restricted one:
    // sync withholds it unless a policy explicitly shares it.

    /// A person: speaker, signer, consultant, researcher. `names` and
    /// `roles` are small JSON values, LWW as a unit.
    pub fn add_person(&mut self, names: Value, roles: Value, default_access: &str) -> Result<String, String> {
        let mut t = self.intent("add-person");
        let id = t.create(
            "person",
            "prs",
            SCOPE_PEOPLE,
            vec![
                ("names", names),
                ("roles", roles),
                ("default-access", Value::str(default_access)),
            ],
        );
        t.commit()?;
        Ok(id)
    }

    /// A consent record: what `person` agreed to for `covers` (a source or
    /// the whole project). `terms` is a JSON map such as
    /// {"archive-ok": true, "web-ok": false, "AI-ok": false}.
    pub fn record_consent(
        &mut self,
        person: &str,
        covers: &str,
        terms: Value,
        date: &str,
    ) -> Result<String, String> {
        // (feat/desktop-shell adds a shared require() helper; fold this
        // into it when the branches meet)
        match self.proj.entity_kind(person)?.as_deref() {
            Some("person") if !self.proj.is_tombstoned(person)? => {}
            Some("person") => return Err(format!("person {person} is deleted")),
            Some(k) => return Err(format!("{person} is a {k}, not a person")),
            None => return Err(format!("no such person: {person}")),
        }
        match self.proj.entity_kind(covers)? {
            Some(k) if (k == "source" || k == "text") && !self.proj.is_tombstoned(covers)? => {}
            Some(k) if k == "source" || k == "text" => return Err(format!("{k} {covers} is deleted")),
            Some(k) => return Err(format!("consent covers a {k}; it must cover a source or text")),
            None if covers == self.project_id => {}
            None => return Err(format!("no such source or text: {covers}")),
        }
        let mut t = self.intent("record-consent");
        let id = t.create(
            "consent",
            "cns",
            SCOPE_PEOPLE,
            vec![
                ("person", Value::str(person)),
                ("covers", Value::str(covers)),
                ("terms", terms),
                ("date", Value::str(date)),
            ],
        );
        t.commit()?;
        Ok(id)
    }

    /// An elicitation session; ops made with `.with_session(id)` carry its
    /// provenance. Participants are a JSON list of {person, role} for now;
    /// revisit as link entities when the UI needs per-participant queries.
    pub fn start_session(
        &mut self,
        date: &str,
        place: &str,
        method: &str,
        participants: Value,
    ) -> Result<String, String> {
        let mut t = self.intent("start-session");
        let id = t.create(
            "session",
            "ses",
            SCOPE_PEOPLE,
            vec![
                ("date", Value::str(date)),
                ("place", Value::str(place)),
                ("method", Value::str(method)),
                ("participants", participants),
            ],
        );
        t.commit()?;
        Ok(id)
    }

    /// Set a source's or text's access level: open, restricted or
    /// community. Sync derives its default policy from this field.
    pub fn set_access(&mut self, target: &str, level: &str) -> Result<CommitResult, String> {
        if !matches!(level, "open" | "restricted" | "community") {
            return Err(format!("unknown access level {level:?}"));
        }
        let kind = self
            .proj
            .entity_kind(target)?
            .ok_or_else(|| format!("no such entity: {target}"))?;
        if kind != "source" && kind != "text" {
            return Err(format!("{target} is a {kind}; access lives on sources and texts"));
        }
        if self.proj.is_tombstoned(target)? {
            return Err(format!("{kind} {target} is deleted"));
        }
        // The access op travels in the always-shared config scope, never in
        // the scope it gates: a peer that already has the text while it was
        // open must still learn it became restricted, and the text's own
        // scope stops being sent the moment the level changes.
        let mut t = self.intent("set-access").resolving();
        t.set(&kind, target, SCOPE_CONFIG, "access", Value::str(level))?;
        t.commit()
    }

    /// A writing system (DATA-MODEL §3): `charset` feeds validator #5,
    /// `sort` is the alphabet order in the format `wugs_model::sortkey`
    /// parses (units separated by whitespace, variants of one unit joined
    /// by `/`, e.g. `a/A b/B ch/Ch/CH`; ICU tailorings are the future edge
    /// upgrade), `direction` is "ltr" or "rtl" for the UI. `charset` and
    /// `sort` take NFC at entry like vernacular text. Lives in the project
    /// config scope.
    pub fn create_writing_system(
        &mut self,
        name: &str,
        script: &str,
        charset: &str,
        sort: &str,
        direction: &str,
    ) -> Result<String, String> {
        let mut t = self.intent("create-writing-system");
        let id = t.create(
            "ws",
            "ws",
            SCOPE_CONFIG,
            vec![
                ("name", Value::str(name)),
                ("script", Value::str(script)),
                ("normalization", Value::str("NFC")),
                ("charset", Value::str(charset)),
                ("sort", Value::str(sort)),
                ("direction", Value::str(direction)),
            ],
        );
        t.commit()?;
        Ok(id)
    }

    /// A recording or document source (DATA-MODEL §3). Each source is its
    /// own scope, so its tracks, texts and segments share its ACL. `blob`
    /// is an optional whole-file ref (a scan or document); audio and video
    /// attach through `add_media_track`. `access` (open, restricted or
    /// community) is set in the same txn, so a source is never visible to
    /// sync without the level it was created with.
    pub fn add_source(
        &mut self,
        label: &str,
        blob: Option<&str>,
        kind: &str,
        access: Option<&str>,
    ) -> Result<String, String> {
        if !matches!(kind, "audio" | "video" | "image" | "document") {
            return Err(format!("unknown source kind {kind:?}"));
        }
        if let Some(b) = blob {
            check_blob_ref(b)?;
        }
        if let Some(a) = access {
            if !matches!(a, "open" | "restricted" | "community") {
                return Err(format!("unknown access level {a:?}"));
            }
        }
        let mut t = self.intent("add-source");
        let mut fields = vec![("label", Value::str(label)), ("kind", Value::str(kind))];
        if let Some(b) = blob {
            fields.push(("blob", Value::str(b)));
        }
        let id = t.create_scope_root("source", "src", fields);
        if let Some(a) = access {
            // in the config scope, like set_access: the level must reach
            // peers that never receive the scope it gates
            t.set("source", &id, SCOPE_CONFIG, "access", Value::str(a))?;
        }
        t.commit()?;
        Ok(id)
    }

    /// One audio or video file of a source (DATA-MODEL §3 MediaTrack). It
    /// lives in the source's scope. `channel` distinguishes tracks of one
    /// recording (stereo channels, camera angles).
    pub fn add_media_track(
        &mut self,
        source: &str,
        blob: &str,
        kind: &str,
        label: &str,
        channel: i64,
    ) -> Result<String, String> {
        match self.proj.entity_kind(source)?.as_deref() {
            Some("source") if !self.proj.is_tombstoned(source)? => {}
            Some("source") => return Err(format!("source {source} is deleted")),
            Some(k) => return Err(format!("{source} is a {k}, not a source")),
            None => return Err(format!("no such source: {source}")),
        }
        if !matches!(kind, "audio" | "video") {
            return Err(format!("unknown track kind {kind:?}"));
        }
        if channel < 0 {
            return Err(format!("negative channel {channel}"));
        }
        check_blob_ref(blob)?;
        let mut t = self.intent("add-media-track");
        let id = t.create(
            "media-track",
            "trk",
            source,
            vec![
                ("source", Value::str(source)),
                ("blob", Value::str(blob)),
                ("kind", Value::str(kind)),
                ("label", Value::str(label)),
                ("channel", Value::Num(channel as f64)),
            ],
        );
        t.commit()?;
        Ok(id)
    }

    /// The orthography sandbox (PRD §7): test a charset against everything
    /// already written in this writing system, and report each field whose
    /// characters fall outside it, without writing anything. An entity
    /// without its own `ws` inherits one: segment from its text, token from
    /// its segment, morph from its token, form from its lexeme.
    ///
    /// Exempt without being listed: whitespace, digits 0-9, and
    /// `wugs_model::sortkey::{PUNCTUATION, BOUNDARY_MARKS}`. The exemption
    /// is a fixed list, because the core carries no Unicode category
    /// tables: any other punctuation (`¿`, `“`, `、`) must be in the
    /// charset or it is reported.
    pub fn audit_writing_system(&self, ws: &str) -> Result<Vec<(String, String, String)>, String> {
        if self.proj.entity_kind(ws)?.as_deref() != Some("ws") || self.proj.is_tombstoned(ws)? {
            return Err(format!("{ws} is not a live writing system"));
        }
        let charset = self
            .proj
            .field_str(ws, "charset")?
            .ok_or_else(|| format!("{ws} has no charset"))?;
        let exempt = |c: char| wugs_model::sortkey::is_separator(c) || c.is_ascii_digit();
        let mut out = Vec::new();
        for (kind, fields) in [
            ("token", &["form"][..]),
            ("morph", &["surface"][..]),
            ("lexeme", &["lemma", "citation"][..]),
            ("form", &["form"][..]),
            ("segment", &["content"][..]),
        ] {
            for id in self.proj.entities_of_kind(kind)? {
                if self.writing_system_of(&id, kind)?.as_deref() != Some(ws) {
                    continue;
                }
                for field in fields {
                    let Some(text) = self.proj.field_str(&id, field)? else { continue };
                    let bad: String = text.chars().filter(|c| !exempt(*c) && !charset.contains(*c)).collect();
                    if !bad.is_empty() {
                        out.push((id.clone(), field.to_string(), bad));
                    }
                }
            }
        }
        Ok(out)
    }

    /// The entity's own `ws`, else its parent's, up the ref chain
    /// morph → token → segment → text and form → lexeme.
    fn writing_system_of(&self, id: &str, kind: &str) -> Result<Option<String>, String> {
        let (mut id, mut kind) = (id.to_string(), kind.to_string());
        loop {
            if let Some(ws) = self.proj.field_str(&id, "ws")? {
                return Ok(Some(ws));
            }
            let parent_ref = match kind.as_str() {
                "morph" => "token",
                "token" => "segment",
                "segment" => "text",
                "form" => "lexeme",
                _ => return Ok(None),
            };
            let Some(parent) = self.proj.field_str(&id, parent_ref)? else { return Ok(None) };
            (id, kind) = (parent, parent_ref.to_string());
        }
    }

    /// Edit a segment's transcription text collaboratively.
    pub fn edit_content(&mut self, text: &str, segment: &str, new_text: &str) -> Result<CommitResult, String> {
        self.require_in_text(segment, "segment", text)?;
        let mut t = self.intent("edit-content").resolving();
        t.text_edit("segment", segment, text, "content", new_text)?;
        t.commit()
    }
}

/// `gloss`, or `gloss:<lang>` for a BCP-47-ish tag: letters, digits and
/// hyphens, 2 to 35 characters. Lowercased, because tags compare without
/// case and `gloss:ES` beside `gloss:es` would be two fields for one language.
fn gloss_field(lang: Option<&str>) -> Result<String, String> {
    let Some(lang) = lang else { return Ok("gloss".to_string()) };
    let ok = (2..=35).contains(&lang.len()) && lang.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if !ok {
        return Err(format!("{lang:?} is not a language tag (letters, digits and hyphens, 2 to 35 characters)"));
    }
    Ok(format!("{}{}", wugs_model::GLOSS_LANG_PREFIX, lang.to_ascii_lowercase()))
}

fn check_blob_ref(r: &str) -> Result<(), String> {
    match parse_blob_ref(r) {
        Some(_) => Ok(()),
        None => Err(format!("malformed blob ref {r:?}; expected blake3:<64 lowercase hex>")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project(actor: &str) -> (PathBuf, Project) {
        let root = std::env::temp_dir().join(format!("wugs-core-test-{}", wugs_model::ulid()));
        let p = Project::open(&root, actor).unwrap();
        (root, p)
    }

    #[test]
    fn full_glossing_flow_and_replay() {
        let (root, mut p) = temp_project("prs_Alice");
        let (_lex, sen) = p.create_lexeme("köpek", "dog").unwrap();
        let txt = p.create_text("First elicitation").unwrap();
        let seg = p
            .add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1500.0), "köpekler geldi")
            .unwrap()
            .0;
        let toks = p.tokenize(&txt, &seg, &["köpekler", "geldi"]).unwrap().0;
        let morphs = p
            .split_morph(&txt, &toks[0], &[("köpek", "root"), ("ler", "suffix")])
            .unwrap()
            .0;
        p.link_sense(&txt, &morphs[0], &sen).unwrap();
        let res = p.set_gloss(&txt, &morphs[1], "PL", None).unwrap();
        assert!(res.findings.iter().all(|f| f.code != "morph-concat"), "{:?}", res.findings);

        assert_eq!(p.proj.field_str(&morphs[1], "gloss").unwrap().unwrap(), "PL");

        // wipe the cache, reopen, replay must reproduce identical state
        drop(p);
        fs::remove_dir_all(root.join("cache")).unwrap();
        let p2 = Project::open(&root, "prs_Alice").unwrap();
        assert_eq!(
            p2.proj.field_str(&morphs[1], "gloss").unwrap().unwrap(),
            "PL"
        );
        assert_eq!(
            p2.proj.field_str(&morphs[0], "sense").unwrap().unwrap(),
            sen
        );
        fs::remove_dir_all(root).ok();
    }

    /// A crash after `commit`'s log append and before its savepoint
    /// release: the ops are in the log, the cache never saw them.
    fn crash_after_append(t: Txn<'_>) -> Vec<String> {
        t.project.log.append_all(&t.ops).unwrap();
        t.ops.iter().map(|o| o.id.clone()).collect()
    }

    #[test]
    fn open_catches_up_ops_the_log_has_and_the_cache_lacks() {
        let (root, mut p) = temp_project("prs_Alice");
        let txt = p.create_text("Story").unwrap();
        let seg = p.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(900.0), "first").unwrap().0;
        p.edit_content(&txt, &seg, "typed").unwrap();
        drop(p);
        // The reopen records where it read each file to; the crash below
        // appends past that, in files the catch-up reads only the end of.
        let mut p = Project::open(&root, "prs_Alice").unwrap();
        assert_eq!(p.caught_up(), 0);
        p.edit_content(&txt, &seg, "typed once more").unwrap();

        let mut t = p.intent("set-title");
        t.set("text", &txt, &txt, "title", Value::str("Story, retold")).unwrap();
        let mut ids = crash_after_append(t);
        let mut t = p.intent("edit-content").resolving();
        t.text_edit("segment", &seg, &txt, "content", "typed again").unwrap();
        ids.extend(crash_after_append(t));
        assert_eq!(p.proj.field_str(&txt, "title").unwrap().unwrap(), "Story", "the crash left the cache behind");
        drop(p);

        let p = Project::open(&root, "prs_Alice").unwrap();
        assert_eq!(p.caught_up(), 2);
        assert_eq!(p.proj.field_str(&txt, "title").unwrap().unwrap(), "Story, retold");
        assert_eq!(p.proj.field_str(&seg, "content").unwrap().unwrap(), "typed again");
        for id in &ids {
            assert!(p.proj.already_applied(id).unwrap(), "{id} is in ops_applied");
        }
        drop(p);

        let p = Project::open(&root, "prs_Alice").unwrap();
        assert_eq!(p.caught_up(), 0, "a clean reopen catches up nothing");
        drop(p);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_parked_op_stays_parked_and_uncounted_across_a_reopen() {
        let (root, mut p) = temp_project("prs_Alice");
        let txt = p.create_text("Story").unwrap();
        // An op on a target nobody created yet parks in `pending`; a reopen
        // must leave it parked, not apply or count it.
        let mut t = p.intent("orphan");
        t.set("segment", "seg_missing", &txt, "content", Value::str("x")).unwrap();
        t.commit().unwrap();
        assert_eq!(p.proj.count("pending").unwrap(), 1);
        drop(p);
        let p = Project::open(&root, "prs_Alice").unwrap();
        assert_eq!(p.caught_up(), 0);
        assert_eq!(p.proj.count("pending").unwrap(), 1);
        drop(p);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_torn_tail_still_opens_and_the_ops_before_it_catch_up() {
        let (root, mut p) = temp_project("prs_Alice");
        let txt = p.create_text("Story").unwrap();
        let mut t = p.intent("set-title");
        t.set("text", &txt, &txt, "title", Value::str("Retold")).unwrap();
        crash_after_append(t);
        let file = p.log.log_root().join(txt.as_str()).join(format!("{}.jsonl", p.device()));
        drop(p);
        let mut f = fs::OpenOptions::new().append(true).open(&file).unwrap();
        std::io::Write::write_all(&mut f, b"{\"id\":\"op_torn\",\"hl").unwrap();
        drop(f);

        let p = Project::open(&root, "prs_Alice").unwrap();
        assert_eq!(p.caught_up(), 1);
        assert_eq!(p.proj.field_str(&txt, "title").unwrap().unwrap(), "Retold");
        drop(p);
        fs::remove_dir_all(&root).ok();
    }

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!("wugs-core-test-{}", wugs_model::ulid()))
    }

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    fn marker(root: &Path) -> wugs_model::json::Value {
        wugs_model::json::parse(&fs::read_to_string(root.join("cache/instance.json")).unwrap()).unwrap()
    }

    fn stored_marker(root: &Path) -> String {
        marker(root).get_str("root").unwrap().to_string()
    }

    fn canonical(root: &Path) -> String {
        fs::canonicalize(root).unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn open_writes_the_instance_marker_and_reopening_in_place_reports_nothing() {
        let (root, p) = temp_project("prs_Alice");
        assert!(p.relocation().is_none());
        assert_eq!(stored_marker(&root), canonical(&root));
        assert_eq!(marker(&root).get_str("fs_id"), fs_id(&root).as_deref());
        drop(p);
        let p = Project::open(&root, "prs_Alice").unwrap();
        assert!(p.relocation().is_none());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_moved_folder_is_silent_and_its_marker_path_is_refreshed() {
        let (root, p) = temp_project("prs_Alice");
        let device = p.device().to_string();
        drop(p);
        let moved = temp_root();
        fs::rename(&root, &moved).unwrap();

        let p = Project::open(&moved, "prs_Alice").unwrap();
        assert!(p.relocation().is_none(), "same cache/ identity: a move, not a copy");
        assert_eq!(p.device(), device);
        assert_eq!(stored_marker(&moved), canonical(&moved));
        fs::remove_dir_all(moved).ok();
    }

    #[test]
    fn a_copy_is_reported_with_its_original_until_resolved() {
        let (root, p) = temp_project("prs_Alice");
        let device = p.device().to_string();
        drop(p);
        let copy = temp_root();
        copy_dir(&root, &copy);

        let p = Project::open(&copy, "prs_Alice").unwrap();
        let r = p.relocation().unwrap();
        assert_eq!(r.stored_path, canonical(&root));
        assert!(r.original_still_present);
        assert_eq!(p.device(), device);
        drop(p);
        let mut p = Project::open(&copy, "prs_Alice").unwrap();
        assert!(p.relocation().is_some(), "an unresolved relocation persists across opens");

        p.relocation_accept_move();
        assert!(p.relocation().is_none());
        drop(p);
        let p = Project::open(&copy, "prs_Alice").unwrap();
        assert!(p.relocation().is_none());
        assert_eq!(p.device(), device, "accepting keeps the device id");
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(copy).ok();
    }

    #[test]
    fn minting_clears_a_relocation_and_leaves_the_original_alone() {
        let (root, p) = temp_project("prs_Alice");
        let device = p.device().to_string();
        drop(p);
        let copy = temp_root();
        copy_dir(&root, &copy);

        let mut p = Project::open(&copy, "prs_Alice").unwrap();
        let fresh = p.mint_device_id().unwrap();
        assert!(p.relocation().is_none());
        drop(p);
        let p = Project::open(&copy, "prs_Alice").unwrap();
        assert!(p.relocation().is_none());
        assert_eq!(p.device(), fresh);
        drop(p);
        let original = Project::open(&root, "prs_Alice").unwrap();
        assert!(original.relocation().is_none(), "the original keeps its marker");
        assert_eq!(original.device(), device);
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(copy).ok();
    }

    #[test]
    fn delete_and_copy_back_to_the_same_path_is_a_relocation_without_an_original() {
        let (root, p) = temp_project("prs_Alice");
        drop(p);
        let backup = temp_root();
        copy_dir(&root, &backup);
        fs::remove_dir_all(&root).unwrap();
        // Some filesystems hand a freed inode straight back; occupy the freed
        // ones so the copy's cache/ cannot land on the old number.
        let fillers: Vec<PathBuf> = (0..64).map(|_| temp_root()).collect();
        fillers.iter().for_each(|f| fs::create_dir_all(f).unwrap());
        copy_dir(&backup, &root);

        let p = Project::open(&root, "prs_Alice").unwrap();
        fillers.iter().for_each(|f| fs::remove_dir_all(f).unwrap());
        let r = p.relocation().expect("a new cache/ directory is a new copy");
        assert_eq!(r.stored_path, canonical(&root));
        assert!(!r.original_still_present, "the stored path is this very folder");
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(backup).ok();
    }

    #[test]
    fn a_copy_whose_original_is_gone_or_another_project_is_not_marked_present() {
        let (root, p) = temp_project("prs_Alice");
        drop(p);
        let copy = temp_root();
        copy_dir(&root, &copy);

        fs::write(root.join("project.json"), "{\"project_id\":\"prj_other\"}").unwrap();
        let p = Project::open(&copy, "prs_Alice").unwrap();
        assert!(!p.relocation().unwrap().original_still_present, "different project id");
        drop(p);

        fs::remove_dir_all(&root).unwrap();
        let p = Project::open(&copy, "prs_Alice").unwrap();
        assert!(!p.relocation().unwrap().original_still_present, "original deleted");
        fs::remove_dir_all(copy).ok();
    }

    #[test]
    fn an_old_format_marker_is_treated_as_missing() {
        let (root, p) = temp_project("prs_Alice");
        drop(p);
        let marker_path = root.join("cache/instance.json");
        fs::write(&marker_path, "{\"root\":\"/somewhere/else\"}").unwrap();
        let p = Project::open(&root, "prs_Alice").unwrap();
        assert!(p.relocation().is_none());
        assert_eq!(marker(&root).get_str("fs_id"), fs_id(&root).as_deref());
        assert_eq!(stored_marker(&root), canonical(&root));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_garbage_or_missing_marker_is_rewritten_without_error() {
        let (root, p) = temp_project("prs_Alice");
        drop(p);
        let marker_path = root.join("cache/instance.json");
        for garbage in ["not json {", "{}", "{\"root\": 7, \"fs_id\": 1}", ""] {
            fs::write(&marker_path, garbage).unwrap();
            let p = Project::open(&root, "prs_Alice").unwrap();
            assert!(p.relocation().is_none(), "{garbage:?}");
            assert_eq!(stored_marker(&root), canonical(&root));
        }
        fs::remove_file(&marker_path).unwrap();
        let p = Project::open(&root, "prs_Alice").unwrap();
        assert!(p.relocation().is_none());
        assert!(marker_path.exists());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_failing_marker_write_leaves_mint_and_accept_consistent() {
        let (root, p) = temp_project("prs_Alice");
        drop(p);
        let copy = temp_root();
        copy_dir(&root, &copy);

        let blocked = |dir: &Path| {
            let m = dir.join("cache/instance.json");
            fs::remove_file(&m).unwrap();
            fs::create_dir(&m).unwrap();
        };
        let mut p = Project::open(&copy, "prs_Alice").unwrap();
        blocked(&copy);
        assert!(write_instance(&copy).is_err(), "the probe makes the marker unwritable");
        let id = p.mint_device_id().expect("a marker failure must not fail the mint");
        assert_eq!(p.device(), id, "the session switched to the new id");
        assert_eq!(fs::read_to_string(copy.join("cache/device")).unwrap().trim(), id, "disk agrees");
        assert!(p.relocation().is_none());
        drop(p);

        fs::remove_dir_all(copy.join("cache/instance.json")).unwrap();
        fs::write(copy.join("cache/instance.json"), "{\"root\":\"/x\",\"fs_id\":\"0\"}").unwrap();
        let mut p = Project::open(&copy, "prs_Alice").unwrap();
        assert!(p.relocation().is_some());
        blocked(&copy);
        p.relocation_accept_move();
        assert!(p.relocation().is_none(), "accept clears the banner even when the marker cannot be written");
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(copy).ok();
    }

    #[test]
    fn a_pending_relocation_blocks_no_work() {
        let (root, p) = temp_project("prs_Alice");
        drop(p);
        let copy = temp_root();
        copy_dir(&root, &copy);
        let mut p = Project::open(&copy, "prs_Alice").unwrap();
        assert!(p.relocation().is_some());
        let txt = p.create_text("Still works").unwrap();
        p.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1.0), "köpek").unwrap();
        assert!(p.relocation().is_some());
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(copy).ok();
    }

    #[test]
    fn a_minted_device_id_writes_a_new_log_file_and_replay_reads_both() {
        let (root, mut p) = temp_project("prs_Alice");
        let old = p.device().to_string();
        let first = p.create_text("before").unwrap();
        let new = p.mint_device_id().unwrap();
        assert_ne!(new, old);
        let second = p.create_text("after").unwrap();
        let scope_files = |text: &str| fs::read_dir(root.join("log").join(text)).unwrap().count();
        assert!(root.join("log").join(&first).join(format!("{old}.jsonl")).is_file());
        assert!(root.join("log").join(&second).join(format!("{new}.jsonl")).is_file());
        assert_eq!((scope_files(&first), scope_files(&second)), (1, 1), "each text's ops went to one device's file");
        drop(p);
        let reopened = Project::open(&root, "prs_Alice").unwrap();
        assert_eq!(reopened.device(), new, "the new id is what the folder carries now");
        drop(reopened);
        fs::remove_dir_all(root.join("cache")).unwrap();
        fs::create_dir_all(root.join("cache")).unwrap();
        fs::write(root.join("cache").join("device"), &new).unwrap();
        let replayed = Project::open(&root, "prs_Alice").unwrap();
        let mut titles: Vec<String> = replayed
            .proj
            .entities_of_kind("text")
            .unwrap()
            .iter()
            .map(|t| replayed.proj.field_str(t, "title").unwrap().unwrap())
            .collect();
        titles.sort();
        assert_eq!(titles, ["after", "before"], "replay sees both devices' files");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn add_segment_reports_findings_with_the_id() {
        let (root, mut p) = temp_project("prs_Alice");
        let txt = p.create_text("T").unwrap();
        let (first, res) = p.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
        assert!(res.findings.iter().all(|f| f.code != "tier-overlap"), "{:?}", res.findings);
        let (second, res) = p.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "b").unwrap();
        assert!(res.findings.iter().any(|f| f.code == "tier-overlap"), "{:?}", res.findings);
        assert_ne!(first, second);
        assert!(p.proj.entity_kind(&second).unwrap().is_some(), "warned, not blocked");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn set_segment_times_retimes_and_warns_on_overlap() {
        let (root, mut p) = temp_project("prs_Alice");
        let txt = p.create_text("T").unwrap();
        let (a, _) = p.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
        let (b, _) = p.add_segment(&txt, "tir_utt", 1, Some(1000.0), Some(2000.0), "b").unwrap();

        let res = p.set_segment_times(&txt, &b, Some(1200.0), Some(2200.0)).unwrap();
        assert!(res.findings.iter().all(|f| f.code != "tier-overlap"), "{:?}", res.findings);
        assert_eq!(p.proj.field_num(&b, "start").unwrap(), Some(1200.0));
        assert_eq!(p.proj.field_num(&b, "end").unwrap(), Some(2200.0));

        let res = p.set_segment_times(&txt, &b, Some(500.0), Some(2200.0)).unwrap();
        assert!(res.findings.iter().any(|f| f.code == "tier-overlap"), "{:?}", res.findings);
        assert_eq!(p.proj.field_num(&b, "start").unwrap(), Some(500.0), "warned, not blocked");
        assert_eq!(p.proj.field_num(&a, "start").unwrap(), Some(0.0), "the neighbour is untouched");

        let res = p.set_segment_times(&txt, &b, Some(600.0), None).unwrap();
        assert!(res.findings.iter().any(|f| f.code == "tier-overlap"), "{:?}", res.findings);
        assert_eq!(p.proj.field_num(&b, "start").unwrap(), Some(600.0));
        assert_eq!(p.proj.field_num(&b, "end").unwrap(), Some(2200.0), "an omitted edge is not rewritten");
        assert!(p.set_segment_times(&txt, &b, None, None).is_err(), "nothing to write");

        let err = p.set_segment_times(&txt, "seg_01JUNKJUNKJUNKJUNKJUNK00", Some(0.0), Some(1.0));
        assert!(err.is_err(), "unknown segment rejected");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn deleting_one_of_two_overlapping_segments_raises_no_overlap_finding() {
        let (root, mut p) = temp_project("prs_Alice");
        let txt = p.create_text("T").unwrap();
        let (a, _) = p.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
        let (b, res) = p.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "b").unwrap();
        assert!(res.findings.iter().any(|f| f.code == "tier-overlap"), "{:?}", res.findings);

        let res = p.delete_segment(&txt, &a).unwrap();
        assert!(res.findings.iter().all(|f| f.code != "tier-overlap"), "{:?}", res.findings);
        assert!(p.proj.is_tombstoned(&a).unwrap());
        assert!(!p.proj.is_tombstoned(&b).unwrap());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn delete_segment_tombstones_and_survives_replay() {
        let (root, mut p) = temp_project("prs_Alice");
        let txt = p.create_text("T").unwrap();
        let (seg, _) = p.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "gone").unwrap();
        p.delete_segment(&txt, &seg).unwrap();
        assert!(p.proj.is_tombstoned(&seg).unwrap());
        assert!(p.proj.children_by_ref("text", &txt, "segment").unwrap().is_empty());
        assert!(p.delete_segment(&txt, &seg).is_err(), "a deleted segment cannot be deleted again");

        drop(p);
        fs::remove_dir_all(root.join("cache")).unwrap();
        let p2 = Project::open(&root, "prs_Alice").unwrap();
        assert!(p2.proj.is_tombstoned(&seg).unwrap(), "the tombstone replays");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn add_tier_creates_a_config_scoped_tier_and_rejects_bad_kinds() {
        let (root, mut p) = temp_project("prs_Alice");
        let (id, res) = p.add_tier("notes", "time-aligned", None, None).unwrap();
        assert!(res.findings.is_empty(), "{:?}", res.findings);
        assert_eq!(p.proj.entity_kind(&id).unwrap().as_deref(), Some("tier"));
        assert_eq!(p.proj.field_str(&id, "name").unwrap().as_deref(), Some("notes"));
        assert_eq!(p.proj.field_str(&id, "kind").unwrap().as_deref(), Some("time-aligned"));
        assert!(root.join("log").join(SCOPE_CONFIG).is_dir(), "tiers live in the config scope");
        assert!(p.proj.field_str(&id, "role").unwrap().is_none(), "role and language are optional");
        assert!(p.add_tier("x", "freeform", None, None).is_err());
        assert!(p.add_tier("  ", "time-aligned", None, None).is_err());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tier_role_and_language_are_stored_and_checked_only_for_presence() {
        let (root, mut p) = temp_project("prs_Alice");
        let (id, _) = p.add_tier("English", "time-aligned", Some("translation"), Some("lng_EN")).unwrap();
        assert_eq!(p.proj.field_str(&id, "role").unwrap().as_deref(), Some("translation"));
        assert_eq!(p.proj.field_str(&id, "language").unwrap().as_deref(), Some("lng_EN"));
        let (kit, _) = p.add_tier("Phones", "time-aligned", Some("phonetic"), None).unwrap();
        assert_eq!(p.proj.field_str(&kit, "role").unwrap().as_deref(), Some("phonetic"), "core never interprets the vocabulary");
        assert!(p.add_tier("x", "time-aligned", Some(" "), None).is_err());
        assert!(p.add_tier("x", "time-aligned", None, Some("")).is_err());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn local_commit_never_blocks_but_reports() {
        let (root, mut p) = temp_project("prs_Alice");
        let txt = p.create_text("T").unwrap();
        p.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
        // overlapping locally: committed anyway, finding attached
        let mut t = p.intent("add-segment");
        let seg2 = t.create(
            "segment",
            "seg",
            &txt,
            vec![
                ("text", Value::str(&txt)),
                ("tier", Value::str("tir_utt")),
                ("start", Value::Num(500.0)),
                ("end", Value::Num(1500.0)),
            ],
        );
        let res = t.commit().unwrap();
        assert!(res.findings.iter().any(|f| f.code == "tier-overlap"));
        assert!(p.proj.entity_kind(&seg2).unwrap().is_some(), "local work not blocked");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn ingest_holds_bad_txn_and_applies_good() {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");

        // Alice's project has a text with one segment
        let txt = alice.create_text("Shared").unwrap();
        alice
            .add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a")
            .unwrap();

        // Bob gets Alice's ops (simulated full sync)
        let alice_ops = alice.log.read_all().unwrap().ops;
        let r = bob.ingest(alice_ops).unwrap();
        assert_eq!(r.txns_held, 0);
        assert!(bob.proj.entity_kind(&txt).unwrap().is_some());

        // Bob adds a NON-overlapping segment (good) and an overlapping one (bad)
        let good = bob
            .add_segment(&txt, "tir_utt", 1, Some(1000.0), Some(2000.0), "b")
            .unwrap()
            .0;
        let bad = bob
            .add_segment(&txt, "tir_utt", 2, Some(1500.0), Some(2500.0), "c")
            .unwrap()
            .0;

        // Alice ingests only Bob's new ops
        let bob_ops: Vec<Op> = bob
            .log
            .read_all()
            .unwrap()
            .ops
            .into_iter()
            .filter(|o| o.device == bob.device())
            .collect();
        let r = alice.ingest(bob_ops).unwrap();
        assert_eq!(r.txns_applied, 1, "good segment applied");
        assert_eq!(r.txns_held, 1, "overlapping segment held: {:?}", r.held);
        assert!(alice.proj.entity_kind(&good).unwrap().is_some());
        assert!(alice.proj.entity_kind(&bad).unwrap().is_none());
        assert_eq!(alice.proj.count("quarantine").unwrap(), 1);

        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }

    #[test]
    fn a_retry_that_crashed_after_its_log_append_is_released_at_open() {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");
        let txt = alice.create_text("Shared").unwrap();
        alice.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
        bob.ingest(alice.log.read_all().unwrap().ops).unwrap();
        let bad = bob.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "c").unwrap().0;
        let bob_ops: Vec<Op> = bob.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == bob.device()).collect();
        assert_eq!(alice.ingest(bob_ops).unwrap().txns_held, 1);

        // The retry got as far as the log append, then the process died:
        // the hold, its spill file and the cache are as they were.
        let (txn, held) = alice.proj.held_txns(None).unwrap().pop().unwrap();
        alice.log.append_all(&held).unwrap();
        drop(alice);

        let alice = Project::open(&root_a, "prs_Alice").unwrap();
        assert_eq!(alice.caught_up(), held.len());
        assert!(alice.proj.entity_kind(&bad).unwrap().is_some(), "the logged segment is visible");
        assert_eq!(alice.proj.count("quarantine").unwrap(), 0, "its hold is released");
        assert_eq!(alice.proj.count("quarantine_ops").unwrap(), 0);
        assert!(!root_a.join("quarantine").join(format!("{txn}.jsonl")).exists(), "its spill file is gone");
        drop(alice);
        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }

    #[test]
    fn held_txn_applies_once_its_fix_arrives() {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");
        let txt = alice.create_text("Shared").unwrap();
        alice.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
        bob.ingest(alice.log.read_all().unwrap().ops).unwrap();

        // Bob adds an overlapping segment, then fixes its times
        let bad = bob.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "b").unwrap().0;
        fn own_ops(p: &Project) -> Vec<Op> {
            let me = p.device().to_string();
            p.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == me).collect()
        }
        let create_ops = own_ops(&bob);
        let r = alice.ingest(create_ops.clone()).unwrap();
        assert_eq!((r.txns_held, r.txns_applied), (1, 0), "overlap held: {:?}", r.held);
        assert!(alice.proj.entity_kind(&bad).unwrap().is_none());

        let mut t = bob.intent("retime-segment");
        t.set("segment", &bad, &txt, "start", Value::Num(1000.0)).unwrap();
        t.set("segment", &bad, &txt, "end", Value::Num(1500.0)).unwrap();
        t.commit().unwrap();
        let fix_ops: Vec<Op> = own_ops(&bob).into_iter().filter(|o| o.verb == "set").collect();

        // The fix parks pending (its target is still held), the retry then
        // applies the held create, and the drained fix rides along, so the
        // validator sees the corrected times.
        let r = alice.ingest(fix_ops).unwrap();
        assert_eq!(r.txns_retried, 1, "the held txn applied on retry: {:?}", r.held);
        assert_eq!(r.held.len(), 0, "nothing left in quarantine: {:?}", r.held);
        assert_eq!(alice.proj.count("quarantine").unwrap(), 0);
        assert_eq!(alice.proj.count("quarantine_ops").unwrap(), 0);
        assert_eq!(alice.proj.field_num(&bad, "start").unwrap(), Some(1000.0), "fixed time projected");

        // replay reproduces: the log holds create + fix in applied order
        drop(alice);
        fs::remove_dir_all(root_a.join("cache")).unwrap();
        let a2 = Project::open(&root_a, "prs_Alice").unwrap();
        assert_eq!(a2.proj.field_num(&bad, "start").unwrap(), Some(1000.0));
        assert_eq!(a2.proj.count("quarantine").unwrap(), 0);
        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }

    #[test]
    fn held_txns_survive_a_cache_wipe() {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");
        let txt = alice.create_text("T").unwrap();
        alice.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
        bob.ingest(alice.log.read_all().unwrap().ops).unwrap();
        let bad = bob.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "late").unwrap().0;
        let bob_ops: Vec<Op> = {
            let me = bob.device().to_string();
            bob.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == me).collect()
        };
        alice.ingest(bob_ops).unwrap();
        assert_eq!(alice.proj.count("quarantine").unwrap(), 1);

        // the projection is disposable; the held txn must not die with it,
        // because Bob's laptop may never be seen again
        drop(alice);
        fs::remove_dir_all(root_a.join("cache")).unwrap();
        let mut alice = Project::open(&root_a, "prs_Alice").unwrap();
        assert_eq!(alice.proj.count("quarantine").unwrap(), 1, "held txn restored from the spill");
        assert_eq!(alice.proj.count("quarantine_ops").unwrap(), 1, "with its op");

        // and it can still heal
        let mine = alice
            .proj
            .children_by_ref("text", &txt, "segment")
            .unwrap()
            .into_iter()
            .find(|s| alice.proj.field_num(s, "end").unwrap() == Some(1000.0))
            .unwrap();
        let mut t = alice.intent("retime-segment");
        t.set("segment", &mine, &txt, "end", Value::Num(500.0)).unwrap();
        t.commit().unwrap();
        assert_eq!(alice.proj.count("quarantine").unwrap(), 0, "healed after the wipe");
        assert!(alice.proj.entity_kind(&bad).unwrap().is_some());
        assert!(
            !root_a.join("quarantine").join(format!("{}.jsonl", "x")).exists(),
            "sanity"
        );
        let spills = fs::read_dir(root_a.join("quarantine")).map(|d| d.count()).unwrap_or(0);
        assert_eq!(spills, 0, "the spill file is gone once the txn applied");
        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }

    #[test]
    fn ingest_is_idempotent_on_the_log() {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");
        bob.create_lexeme("wug", "bird").unwrap();
        let bob_ops = bob.log.read_all().unwrap().ops;

        let r1 = alice.ingest(bob_ops.clone()).unwrap();
        let log_after_first = alice.log.read_all().unwrap().ops.len();
        let r2 = alice.ingest(bob_ops.clone()).unwrap();

        assert_eq!(r1.txns_applied, 1);
        assert_eq!(r2.txns_applied, 0, "second ingest applies nothing");
        assert_eq!(r2.ops_skipped_dupes, bob_ops.len(), "all ops recognized as dupes");
        assert_eq!(
            alice.log.read_all().unwrap().ops.len(),
            log_after_first,
            "re-ingest wrote no duplicate log lines"
        );
        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }

    #[test]
    fn text_edit_without_merger_degrades_to_lww_with_conflict() {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");
        let txt = alice.create_text("T").unwrap();
        let seg = alice.add_segment(&txt, "tir_utt", 0, None, None, "köpek geldi").unwrap().0;
        bob.ingest(alice.log.read_all().unwrap().ops).unwrap();

        alice.edit_content(&txt, &seg, "köpekler geldi").unwrap();
        bob.edit_content(&txt, &seg, "köpek gitti").unwrap();
        assert_eq!(alice.proj.field_str(&seg, "content").unwrap().unwrap(), "köpekler geldi", "local edit lands as plain text");

        let bob_new: Vec<Op> = bob
            .log
            .read_all()
            .unwrap()
            .ops
            .into_iter()
            .filter(|o| o.device == bob.device())
            .collect();
        let r = alice.ingest(bob_new).unwrap();
        assert_eq!(r.conflicts, 1, "concurrent text edits surface without a merger");
        let got = alice.proj.field_str(&seg, "content").unwrap().unwrap();
        assert!(got == "köpekler geldi" || got == "köpek gitti", "one side wins by LWW, got {got}");
        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }

    /// A merger whose merge fails whenever the update set has more than
    /// one member: the colliding-lineage shape. The projection must keep
    /// the sound update and degrade the op, not delete lineage.
    struct Picky;
    impl wugs_model::TextMerger for Picky {
        fn resolve(&self, updates: &[Vec<u8>]) -> Result<String, String> {
            match updates {
                [one] => Ok(String::from_utf8_lossy(one).into_owned()),
                _ => Err("colliding set".into()),
            }
        }
        fn edit(&self, _: &[Vec<u8>], _: Option<(&str, u64)>, new_text: &str, _: u64) -> Result<(Vec<u8>, String), String> {
            Ok((new_text.as_bytes().to_vec(), new_text.to_string()))
        }
    }

    /// Merges one update; refuses a set of more, and refuses to edit on top
    /// of a set it cannot merge (as Loro does after colliding op ids).
    struct Strict;
    impl wugs_model::TextMerger for Strict {
        fn resolve(&self, updates: &[Vec<u8>]) -> Result<String, String> {
            Picky.resolve(updates)
        }
        fn edit(&self, updates: &[Vec<u8>], _: Option<(&str, u64)>, new_text: &str, _: u64) -> Result<(Vec<u8>, String), String> {
            if updates.len() > 1 {
                return Err("colliding set".into());
            }
            Ok((new_text.as_bytes().to_vec(), new_text.to_string()))
        }
    }

    #[test]
    fn an_edit_on_text_that_no_longer_merges_still_lands_and_settles_the_conflict() {
        let open = |actor: &str| {
            let root = std::env::temp_dir().join(format!("wugs-core-test-{}", wugs_model::ulid()));
            let p = Project::open_with(&root, actor, Engines { merger: Some(Box::new(Strict)), ..Default::default() }).unwrap();
            (root, p)
        };
        let ((ra, mut a), (rb, mut b)) = (open("prs_Alice"), open("prs_Bob"));
        let txt = a.create_text("T").unwrap();
        let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "start").unwrap().0;
        b.ingest(a.log.read_all().unwrap().ops).unwrap();
        a.edit_content(&txt, &seg, "alice").unwrap();
        b.edit_content(&txt, &seg, "bob").unwrap();
        let theirs: Vec<Op> = b.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == b.device()).collect();
        a.ingest(theirs).unwrap();
        assert_eq!(a.proj.count("conflicts").unwrap(), 1, "the two updates do not merge, so they conflict like sets");
        a.edit_content(&txt, &seg, "agreed").unwrap();
        assert_eq!(a.proj.field_str(&seg, "content").unwrap().as_deref(), Some("agreed"));
        assert_eq!(a.proj.count("conflicts").unwrap(), 0);
        fs::remove_dir_all(ra).ok();
        fs::remove_dir_all(rb).ok();
    }

    #[test]
    fn unmergeable_update_is_kept_and_degrades_to_lww() {
        let root = std::env::temp_dir().join(format!("wugs-core-test-{}", wugs_model::ulid()));
        let mut p = Project::open_with(&root, "prs_Alice", Engines { merger: Some(Box::new(Picky)), ..Default::default() }).unwrap();
        let txt = p.create_text("T").unwrap();
        let seg = p.add_segment(&txt, "tir_utt", 0, None, None, "start").unwrap().0;

        p.edit_content(&txt, &seg, "first").unwrap();
        assert_eq!(p.proj.field_str(&seg, "content").unwrap().unwrap(), "first", "single update merges");

        p.edit_content(&txt, &seg, "second").unwrap();
        assert_eq!(
            p.proj.field_str(&seg, "content").unwrap().unwrap(),
            "second",
            "unmergeable set degrades to LWW on resolved"
        );
        assert_eq!(
            p.proj.text_updates(&seg, "content").unwrap().len(),
            2,
            "the sound update stays stored; lineage is never deleted"
        );

        // replay must reproduce the degraded state, not error
        drop(p);
        fs::remove_dir_all(root.join("cache")).unwrap();
        let p2 = Project::open_with(&root, "prs_Alice", Engines { merger: Some(Box::new(Picky)), ..Default::default() }).unwrap();
        assert_eq!(p2.proj.field_str(&seg, "content").unwrap().unwrap(), "second", "rebuild agrees");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn concurrent_gloss_edit_surfaces_conflict() {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");

        let txt = alice.create_text("T").unwrap();
        let seg = alice.add_segment(&txt, "tir_utt", 0, None, None, "wug").unwrap().0;
        let toks = alice.tokenize(&txt, &seg, &["wugs"]).unwrap().0;
        let morphs = alice
            .split_morph(&txt, &toks[0], &[("wug", "root"), ("s", "suffix")])
            .unwrap()
            .0;

        let base_ops = alice.log.read_all().unwrap().ops;
        bob.ingest(base_ops).unwrap();

        // both gloss the same morph from the same base, offline
        alice.set_gloss(&txt, &morphs[1], "PL", None).unwrap();
        bob.set_gloss(&txt, &morphs[1], "PLU", None).unwrap();

        let bob_new: Vec<Op> = bob
            .log
            .read_all()
            .unwrap()
            .ops
            .into_iter()
            .filter(|o| o.device == bob.device())
            .collect();
        let r = alice.ingest(bob_new).unwrap();
        assert_eq!(r.conflicts, 1, "concurrent gloss edit must surface");
        assert_eq!(alice.proj.count("conflicts").unwrap(), 1);

        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }

    /// Alice and Bob gloss one morph concurrently; Alice ingests Bob's gloss.
    /// Returns (roots, alice, txt, morph).
    fn conflicted_gloss() -> ((PathBuf, PathBuf), Project, String, String) {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");
        let txt = alice.create_text("T").unwrap();
        let seg = alice.add_segment(&txt, "tir_utt", 0, None, None, "wugs").unwrap().0;
        let toks = alice.tokenize(&txt, &seg, &["wugs"]).unwrap().0;
        let morphs = alice.split_morph(&txt, &toks[0], &[("wug", "root"), ("s", "suffix")]).unwrap().0;
        bob.ingest(alice.log.read_all().unwrap().ops).unwrap();
        alice.set_gloss(&txt, &morphs[1], "PL", None).unwrap();
        bob.set_gloss(&txt, &morphs[1], "PLU", None).unwrap();
        let bob_new: Vec<Op> = bob.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == bob.device()).collect();
        alice.ingest(bob_new).unwrap();
        assert_eq!(alice.proj.count("conflicts").unwrap(), 1);
        ((root_a, root_b), alice, txt, morphs[1].clone())
    }

    #[test]
    fn a_plain_set_leaves_a_conflict_open_and_a_direct_edit_settles_it() {
        let ((ra, rb), mut alice, txt, morph) = conflicted_gloss();
        // A bulk or derived write names only the winner it overwrites.
        let mut t = alice.intent("bulk-fix");
        t.set("morph", &morph, &txt, "gloss", Value::str("PL.bulk")).unwrap();
        t.commit().unwrap();
        assert_eq!(alice.proj.count("conflicts").unwrap(), 1, "the losing gloss is still an open head nobody looked at");
        // A person's own edit of the gloss names every open version.
        alice.set_gloss(&txt, &morph, "PL", None).unwrap();
        assert_eq!(alice.proj.count("conflicts").unwrap(), 0);
        assert_eq!(alice.proj.heads(&morph, "gloss").unwrap().len(), 1);
        fs::remove_dir_all(ra).ok();
        fs::remove_dir_all(rb).ok();
    }

    #[test]
    fn retyping_a_word_settles_a_conflict_on_its_form() {
        let (ra, mut a) = temp_project("prs_Alice");
        let (rb, mut b) = temp_project("prs_Bob");
        let txt = a.create_text("T").unwrap();
        let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "wugs").unwrap().0;
        let tok = a.tokenize(&txt, &seg, &["wugs"]).unwrap().0[0].clone();
        b.ingest(a.log.read_all().unwrap().ops).unwrap();
        a.retokenize(&txt, &seg, &["WUGS"]).unwrap();
        b.retokenize(&txt, &seg, &["Wugs"]).unwrap();
        let theirs: Vec<Op> = b.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == b.device()).collect();
        a.ingest(theirs).unwrap();
        assert_eq!(a.proj.count("conflicts").unwrap(), 1, "the two spellings of one word conflict");
        a.retokenize(&txt, &seg, &["wugs"]).unwrap();
        assert_eq!(a.proj.field_str(&tok, "form").unwrap().as_deref(), Some("wugs"));
        assert_eq!(a.proj.count("conflicts").unwrap(), 0, "retyping the word is a direct edit of its form");
        fs::remove_dir_all(ra).ok();
        fs::remove_dir_all(rb).ok();
    }

    #[test]
    fn a_txn_from_a_clock_more_than_a_day_ahead_is_held_not_half_applied() {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");
        let txt = alice.create_text("T").unwrap();
        bob.ingest(alice.log.read_all().unwrap().ops).unwrap();
        let mut ops: Vec<Op> = {
            let mut t = alice.intent("future");
            t.set("text", &txt, &txt, "title", Value::str("from the future")).unwrap();
            t.ops.clone()
        };
        let far = wugs_model::id::now_millis() + 2 * wugs_model::hlc::MAX_DRIFT_MS;
        for op in &mut ops {
            op.hlc = Hlc { millis: far, counter: 0, device: alice.device().to_string() }.encode();
        }
        let before = bob.log.read_all().unwrap().ops.len();
        let r = bob.ingest(ops).unwrap();
        assert_eq!((r.txns_applied, r.txns_held), (0, 1));
        assert!(r.held[0].1.starts_with("clock drift"), "{:?}", r.held);
        assert_eq!(bob.proj.field_str(&txt, "title").unwrap().as_deref(), Some("T"), "nothing applied");
        assert_eq!(bob.log.read_all().unwrap().ops.len(), before, "nothing logged");
        assert_eq!(bob.proj.count("quarantine").unwrap(), 1);
        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }

    fn fake_ref(c: char) -> String {
        format!("blake3:{}", c.to_string().repeat(64))
    }

    #[test]
    fn intent_ref_check_agrees_with_the_shared_vectors() {
        for &(text, ok) in wugs_model::REF_VECTORS {
            assert_eq!(check_blob_ref(text).is_ok(), ok, "{text:?}");
        }
    }

    #[test]
    fn source_and_track_intents_validate_and_scope() {
        let (root, mut p) = temp_project("prs_Alice");
        let a = fake_ref('a');
        let src = p.add_source("Elicitation 1", None, "audio", None).unwrap();
        assert_eq!(p.proj.entity_kind(&src).unwrap().as_deref(), Some("source"));
        let trk = p.add_media_track(&src, &a, "audio", "left mic", 0).unwrap();
        assert_eq!(p.proj.entity_kind(&trk).unwrap().as_deref(), Some("media-track"));
        assert_eq!(p.proj.field_str(&trk, "blob").unwrap().as_deref(), Some(a.as_str()));
        assert_eq!(p.proj.field_str(&trk, "source").unwrap().as_deref(), Some(src.as_str()));
        let ops = p.log.read_all().unwrap().ops;
        let track_ops: Vec<&Op> = ops.iter().filter(|o| o.entity == "media-track").collect();
        assert_eq!(track_ops.len(), 1);
        assert!(track_ops.iter().all(|o| o.scope == src));

        let before = p.log.read_all().unwrap().ops.len();
        for bad in ["blake3:abc", "sha256:aaaa", &fake_ref('A'), &format!("{}0", fake_ref('a')), ""] {
            assert!(p.add_media_track(&src, bad, "audio", "x", 0).is_err(), "{bad}");
            assert!(p.add_source("s", Some(bad), "document", None).is_err(), "{bad}");
        }
        assert!(p.add_media_track("src_missing", &a, "audio", "x", 0).is_err());
        assert!(p.add_media_track(&trk, &a, "audio", "x", 0).is_err(), "track is not a source");
        assert!(p.add_media_track(&src, &a, "image", "x", 0).is_err());
        assert!(p.add_media_track(&src, &a, "audio", "x", -1).is_err(), "negative channel");
        assert!(p.add_source("s", None, "hologram", None).is_err());
        assert!(p.add_source("s", None, "audio", Some("secret")).is_err());
        assert_eq!(p.log.read_all().unwrap().ops.len(), before, "rejections write no ops");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn blob_refs_cover_live_tombstoned_pending_and_held() {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");
        let (live, gone, parked, held) = (fake_ref('1'), fake_ref('2'), fake_ref('3'), fake_ref('4'));

        let src = alice.add_source("S", None, "audio", None).unwrap();
        alice.add_media_track(&src, &live, "audio", "a", 0).unwrap();
        let dead = alice.add_media_track(&src, &gone, "audio", "b", 1).unwrap();
        let mut t = alice.intent("delete-track");
        t.delete("media-track", &dead, &src);
        t.commit().unwrap();
        let refs = alice.blob_refs().unwrap();
        assert_eq!(refs, vec![live.clone(), gone.clone()], "tombstoned track keeps its blob");

        // a set whose create has not arrived parks as pending
        let mut t = alice.intent("retarget");
        t.set("media-track", &dead, &src, "blob", Value::str(&parked)).unwrap();
        t.commit().unwrap();
        let ops = alice.log.read_all().unwrap().ops;
        let set_op: Vec<Op> = ops.into_iter().filter(|o| o.field.as_deref() == Some("blob") && o.value == Value::str(&parked)).collect();
        assert_eq!(set_op.len(), 1);
        bob.ingest(set_op).unwrap();
        assert_eq!(bob.proj.count("pending").unwrap(), 1);
        assert_eq!(bob.blob_refs().unwrap(), vec![parked.clone()]);

        // a held txn carries a ref that is in no field yet
        let txt = alice.create_text("T").unwrap();
        alice.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
        bob.ingest(alice.log.read_all().unwrap().ops).unwrap();
        let mut t = bob.intent("overlap-with-media");
        t.create(
            "segment",
            "seg",
            &txt,
            vec![
                ("text", Value::str(&txt)),
                ("tier", Value::str("tir_utt")),
                ("pos", Value::str(&wugs_model::pos_seq(1))),
                ("content", Value::str("b")),
                ("start", Value::Num(500.0)),
                ("end", Value::Num(1500.0)),
            ],
        );
        t.create("media-track", "trk", &txt, vec![("blob", Value::str(&held))]);
        t.commit().unwrap();
        let bob_ops: Vec<Op> = bob.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == bob.device()).collect();
        let r = alice.ingest(bob_ops).unwrap();
        assert_eq!(r.txns_held, 1, "{:?}", r.held);
        assert!(alice.blob_refs().unwrap().contains(&held));

        // after a cache wipe the held op lives only in the spill file
        drop(alice);
        fs::remove_file(root_a.join("cache/projection.sqlite")).unwrap();
        let fresh = Project::open(&root_a, "prs_Alice").unwrap();
        assert!(fresh.blob_refs().unwrap().contains(&held));

        // a spill file the projection has not loaded still marks its refs
        let spilled = fake_ref('5');
        fs::write(root_a.join("quarantine/unloaded.jsonl"), format!("{{\"value\":\"{spilled}\"}}\n")).unwrap();
        assert!(fresh.blob_refs().unwrap().contains(&spilled));
        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }

    #[test]
    fn source_access_is_set_in_the_config_scope_in_the_create_txn() {
        let (root, mut p) = temp_project("prs_Alice");
        let src = p.add_source("Consent interview", None, "audio", Some("restricted")).unwrap();
        assert_eq!(p.proj.field_str(&src, "access").unwrap().as_deref(), Some("restricted"));
        let ops = p.log.read_all().unwrap().ops;
        let access: Vec<&Op> = ops.iter().filter(|o| o.field.as_deref() == Some("access")).collect();
        assert_eq!(access.len(), 1);
        assert_eq!(access[0].scope, SCOPE_CONFIG);
        let create = ops.iter().find(|o| o.verb == "create" && o.target == src).unwrap();
        assert_eq!(access[0].txn, create.txn, "one txn: no window without the level");

        let open = p.add_source("Story", None, "audio", None).unwrap();
        assert_eq!(p.proj.field_str(&open, "access").unwrap(), None);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn conflicting_blob_writes_keep_both_refs_live() {
        let (root_a, mut alice) = temp_project("prs_Alice");
        let (root_b, mut bob) = temp_project("prs_Bob");
        let (orig, from_a, from_b) = (fake_ref('0'), fake_ref('a'), fake_ref('b'));
        let src = alice.add_source("S", None, "audio", None).unwrap();
        let trk = alice.add_media_track(&src, &orig, "audio", "main", 0).unwrap();
        bob.ingest(alice.log.read_all().unwrap().ops).unwrap();

        for (p, r) in [(&mut alice, &from_a), (&mut bob, &from_b)] {
            let mut t = p.intent("replace-media");
            t.set("media-track", &trk, &src, "blob", Value::str(r)).unwrap();
            t.commit().unwrap();
        }
        let own = |p: &Project| -> Vec<Op> {
            let dev = p.device().to_string();
            p.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == dev).collect()
        };
        let (a_ops, b_ops) = (own(&alice), own(&bob));
        assert_eq!(alice.ingest(b_ops).unwrap().conflicts, 1);
        assert_eq!(bob.ingest(a_ops).unwrap().conflicts, 1);

        for p in [&alice, &bob] {
            let refs = p.blob_refs().unwrap();
            for r in [&orig, &from_a, &from_b] {
                assert!(refs.contains(r), "{r} missing from {refs:?}");
            }
        }
        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }
}
