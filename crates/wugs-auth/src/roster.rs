//! The project roster: which device keys may sync this project, as an
//! append-only set of signed records in `auth/roster.jsonl`.
//!
//! Each line is `{"rec": "<body json>", "sig": "<hex>"}`. The signature
//! covers `DOMAIN || rec` byte for byte, so verification never depends on
//! re-serializing JSON. A body names its project, its signer, `prev` (the
//! hash of the signer's previous record, empty for its first), and one of
//! three kinds:
//!
//! - `enrol {key, label, guest}` adds a device. A guest syncs open scopes
//!   read-only and cannot sign records. There is no demotion: an enrol of a
//!   member as a guest leaves it a member, so revoke is the only way down.
//! - `revoke {key, upto}` marks a device lost. `upto` is the hash of the
//!   last record of that key's chain the revoker had seen (empty revokes
//!   everything): the records on the chain up to it stand, later ones are
//!   void. A device revoking itself keeps its chain up to and including
//!   that revoke, or the revoke would void itself.
//! - `attest {key, dvc}` binds a `dvc_…` id to the device key that signs it,
//!   as that key's current id: the newest standing attest on the key's own
//!   chain supersedes its older ones, so a copied folder that mints a fresh
//!   id stops claiming the old one on every device.
//!
//! # Chains and equivocation
//!
//! A signer's records form one hash chain through `prev`, so "the records
//! before the cutoff" names exact records, not counter values a key holder
//! could sign again. Two records by one signer with the same `prev` are a
//! fork: only someone holding the key can make one, and the honest device
//! never does, so a fork means the key is in two hands. A forked key fails
//! closed: it counts as revoked at the fork point, the records past it on
//! every branch are void, and the key is listed in `contested`. Records
//! whose `prev` is unknown are void until their predecessor arrives.
//!
//! Hashing covers `rec` only, never the signature: `rec` is the record's
//! identity (a replayed line is the same record), so a second valid
//! signature over the same `rec` cannot give one record two hashes.
//!
//! The limit, recorded once: a key holder can fork at any point of the
//! chain, including before the enrol of another device, and that enrol is
//! then void too. A stolen key can destroy what it signed (fail closed),
//! never add to it once a revoke names its cutoff. Devices enrolled by more
//! than one member survive the loss of any one of them.
//!
//! # Bootstrap
//!
//! The first line of a project's roster is its genesis, a self-signed enrol
//! of the creating device with an empty `prev`. It is the only record that
//! needs no prior authority, and it is pinned: a peer whose roster starts
//! with a different genesis is a different roster, and merging is refused.
//! Every other record counts only if its signer is an enrolled, non-guest
//! device and the record lies on the signer's chain inside its cutoff.
//!
//! The roster's meaning is a function of its record set alone: `state()`
//! gives the same answer whatever order records arrived in and however
//! often a record was replayed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha512_256};
use wugs_model::{json, Value};

use crate::hex;
use crate::key::{DeviceKey, PublicKey};

const DOMAIN: &[u8] = b"wugs-roster-v2\0";
const HASH_DOMAIN: &[u8] = b"wugs-roster-hash-v2\0";
const VERSION: f64 = 2.0;

/// The most records a peer's roster frame may carry. A real project's
/// roster is a few records per device; this bound only stops a peer from
/// making every session load and walk an unbounded file.
pub const MAX_PEER_RECORDS: usize = 16_384;

/// SHA-512/256 of a record's signed text: how `prev` and `upto` name it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordHash([u8; 32]);

impl RecordHash {
    fn of(rec: &str) -> RecordHash {
        RecordHash(Sha512_256::new().chain_update(HASH_DOMAIN).chain_update(rec.as_bytes()).finalize().into())
    }

    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }

    pub fn from_hex(s: &str) -> Result<RecordHash, String> {
        let arr: [u8; 32] = hex::decode(s)?.try_into().map_err(|_| "record hash is not 32 bytes".to_string())?;
        Ok(RecordHash(arr))
    }
}

impl fmt::Debug for RecordHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RecordHash({})", &self.to_hex()[..16])
    }
}

fn hash_value(h: &Option<RecordHash>) -> Value {
    Value::str(&h.map(|h| h.to_hex()).unwrap_or_default())
}

fn parse_hash(s: &str) -> Result<Option<RecordHash>, String> {
    if s.is_empty() {
        Ok(None)
    } else {
        RecordHash::from_hex(s).map(Some)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    Enrol { key: PublicKey, label: String, guest: bool },
    /// `upto`: the last record of `key`'s chain the revoker had seen;
    /// `None` voids every record of `key`.
    Revoke { key: PublicKey, upto: Option<RecordHash> },
    Attest { key: PublicKey, dvc: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub project: String,
    pub signer: PublicKey,
    /// The signer's previous record; `None` for its first.
    pub prev: Option<RecordHash>,
    pub body: Body,
}

impl Record {
    fn to_body_json(&self) -> String {
        let mut fields = vec![
            ("v", Value::Num(VERSION)),
            ("project", Value::str(&self.project)),
            ("signer", Value::str(&self.signer.to_hex())),
            ("prev", hash_value(&self.prev)),
        ];
        match &self.body {
            Body::Enrol { key, label, guest } => fields.extend([
                ("kind", Value::str("enrol")),
                ("key", Value::str(&key.to_hex())),
                ("label", Value::str(label)),
                ("guest", Value::Bool(*guest)),
            ]),
            Body::Revoke { key, upto } => fields.extend([
                ("kind", Value::str("revoke")),
                ("key", Value::str(&key.to_hex())),
                ("upto", hash_value(upto)),
            ]),
            Body::Attest { key, dvc } => fields.extend([
                ("kind", Value::str("attest")),
                ("key", Value::str(&key.to_hex())),
                ("dvc", Value::str(dvc)),
            ]),
        }
        Value::obj(fields).to_json()
    }

    fn parse(rec: &str) -> Result<Record, String> {
        let v = json::parse(rec).map_err(|e| e.to_string())?;
        if v.get("v").and_then(Value::as_f64) != Some(VERSION) {
            return Err("unsupported roster record version".into());
        }
        let s = |k: &str| v.get_str(k).ok_or_else(|| format!("record without {k}"));
        let key = PublicKey::from_hex(s("key")?)?;
        let body = match s("kind")? {
            "enrol" => Body::Enrol {
                key,
                label: s("label")?.to_string(),
                guest: match v.get("guest") {
                    Some(Value::Bool(b)) => *b,
                    _ => return Err("enrol without guest".into()),
                },
            },
            "revoke" => Body::Revoke { key, upto: parse_hash(s("upto")?)? },
            "attest" => Body::Attest { key, dvc: s("dvc")?.to_string() },
            other => return Err(format!("unknown record kind {other}")),
        };
        Ok(Record {
            project: s("project")?.to_string(),
            signer: PublicKey::from_hex(s("signer")?)?,
            prev: parse_hash(s("prev")?)?,
            body,
        })
    }
}

/// A record with a verified signature. `rec` is the exact signed text and
/// doubles as the record's identity, so a replayed line is the same record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedRecord {
    rec: String,
    sig: [u8; 64],
    record: Record,
    hash: RecordHash,
}

impl SignedRecord {
    /// Sign any record with any key. No authority check: authority is
    /// decided when a roster materializes, never at signing time.
    pub fn sign(key: &DeviceKey, project: &str, prev: Option<RecordHash>, body: Body) -> SignedRecord {
        let record = Record { project: project.to_string(), signer: key.public(), prev, body };
        let rec = record.to_body_json();
        let sig = key.sign(&signed_bytes(&rec));
        SignedRecord { hash: RecordHash::of(&rec), rec, sig, record }
    }

    /// Parse and verify one roster line.
    pub fn from_line(line: &str) -> Result<SignedRecord, String> {
        let v = json::parse(line).map_err(|e| format!("roster line is not JSON: {e}"))?;
        let rec = v.get_str("rec").ok_or("roster line without rec")?.to_string();
        let sig: [u8; 64] = hex::decode(v.get_str("sig").ok_or("roster line without sig")?)?
            .try_into()
            .map_err(|_| "signature is not 64 bytes".to_string())?;
        let record = Record::parse(&rec)?;
        if !record.signer.verify(&signed_bytes(&rec), &sig) {
            return Err("bad signature".into());
        }
        Ok(SignedRecord { hash: RecordHash::of(&rec), rec, sig, record })
    }

    pub fn to_line(&self) -> String {
        Value::obj(vec![("rec", Value::str(&self.rec)), ("sig", Value::str(&hex::encode(&self.sig)))]).to_json()
    }

    pub fn record(&self) -> &Record {
        &self.record
    }

    pub fn hash(&self) -> RecordHash {
        self.hash
    }
}

fn signed_bytes(rec: &str) -> Vec<u8> {
    [DOMAIN, rec.as_bytes()].concat()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Member,
    Guest,
    Revoked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub label: String,
    pub guest: bool,
}

/// What a roster means right now.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RosterState {
    /// Every key a valid enrol named, revoked ones included: a revoked
    /// device's records before its cutoff still stand.
    pub devices: BTreeMap<PublicKey, Device>,
    /// Revoked keys and the last of their records that stands (`None`:
    /// none do). Later records on their chain are void.
    pub revoked: BTreeMap<PublicKey, Option<RecordHash>>,
    /// Device keys each `dvc` id is the current id of: only each key's
    /// newest standing attest counts. More than one key for a `dvc` means a
    /// project folder was probably copied between machines.
    pub attested: BTreeMap<String, BTreeSet<PublicKey>>,
    /// Keys that need a person's look, all failing closed: keys whose chain
    /// forked (the key is in two hands), and revocations that hold only
    /// through a paradox (two devices revoking each other before either
    /// saw the other's revoke).
    pub contested: BTreeSet<PublicKey>,
}

impl RosterState {
    pub fn standing(&self, key: &PublicKey) -> Standing {
        if self.revoked.contains_key(key) {
            return Standing::Revoked;
        }
        match self.devices.get(key) {
            Some(d) if d.guest => Standing::Guest,
            Some(_) => Standing::Member,
            None => Standing::Unknown,
        }
    }

    /// Keys other than `key` that `dvc` is attested to.
    pub fn dvc_claimed_by_other(&self, dvc: &str, key: &PublicKey) -> Option<PublicKey> {
        self.attested.get(dvc)?.iter().find(|k| *k != key).copied()
    }
}

/// A verified record set for one project, genesis first.
#[derive(Debug, Clone)]
pub struct Roster {
    project: String,
    records: Vec<SignedRecord>,
    ids: BTreeSet<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MergeReport {
    pub added: Vec<SignedRecord>,
    /// Lines that failed to parse, failed their signature, or name another project.
    pub rejected: usize,
    /// Valid records not kept because their signer is not an enrolled,
    /// non-guest device even after the merge.
    pub dropped: usize,
}

/// Why a peer's roster was not merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// It starts with another genesis: another project, or a re-created one.
    ForeignGenesis,
    TooManyRecords { records: usize, limit: usize },
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::ForeignGenesis => {
                write!(f, "peer's roster has a different genesis: it belongs to another project or was re-created")
            }
            MergeError::TooManyRecords { records, limit } => {
                write!(f, "peer's roster has {records} records; at most {limit} are accepted")
            }
        }
    }
}

/// Each signer's records laid out along its hash chain.
struct Chains<'a> {
    /// Per signer, the unforked run of records from its first, in chain order.
    line: BTreeMap<PublicKey, Vec<&'a SignedRecord>>,
    /// Each lined-up record's position on its signer's line.
    pos: BTreeMap<RecordHash, usize>,
    by_hash: BTreeMap<RecordHash, &'a SignedRecord>,
    forked: BTreeSet<PublicKey>,
}

impl<'a> Chains<'a> {
    fn of(records: &'a [SignedRecord]) -> Chains<'a> {
        let mut children: BTreeMap<(PublicKey, Option<RecordHash>), Vec<&SignedRecord>> = BTreeMap::new();
        for r in records {
            children.entry((r.record.signer, r.record.prev)).or_default().push(r);
        }
        let signers: BTreeSet<PublicKey> = records.iter().map(|r| r.record.signer).collect();
        let mut ch = Chains { line: BTreeMap::new(), pos: BTreeMap::new(), by_hash: BTreeMap::new(), forked: BTreeSet::new() };
        for s in signers {
            let mut line = Vec::new();
            let mut at = None;
            loop {
                match children.get(&(s, at)).map(Vec::as_slice) {
                    Some([next]) => {
                        ch.pos.insert(next.hash, line.len());
                        line.push(*next);
                        at = Some(next.hash);
                    }
                    Some([_, _, ..]) => {
                        ch.forked.insert(s);
                        break;
                    }
                    _ => break,
                }
            }
            ch.line.insert(s, line);
        }
        ch.by_hash = records.iter().map(|r| (r.hash, r)).collect();
        ch
    }

    fn len(&self, key: &PublicKey) -> usize {
        self.line.get(key).map_or(0, Vec::len)
    }

    /// How many of `key`'s lined-up records stand under a revoke naming
    /// `upto`. A hash on a fork branch keeps the line up to the fork; an
    /// unknown hash keeps nothing until its record arrives (fail closed).
    fn cut(&self, key: &PublicKey, upto: Option<RecordHash>) -> usize {
        let mut at = upto;
        while let Some(r) = at.and_then(|h| self.by_hash.get(&h)).filter(|r| r.record.signer == *key) {
            if let Some(p) = self.pos.get(&r.hash) {
                return p + 1;
            }
            at = r.record.prev;
        }
        0
    }
}

impl Roster {
    /// A new project's roster: the creating device enrols itself.
    pub fn genesis(project: &str, key: &DeviceKey, label: &str) -> Roster {
        let g = SignedRecord::sign(key, project, None, Body::Enrol { key: key.public(), label: label.to_string(), guest: false });
        Roster { project: project.to_string(), ids: BTreeSet::from([g.rec.clone()]), records: vec![g] }
    }

    /// Rebuild from lines, the first of which must be a self-signed genesis
    /// enrol for `project`. Later lines that fail verification are dropped
    /// and counted, so a torn tail from a crash does not lose the roster.
    pub fn from_lines<'a>(project: &str, lines: impl IntoIterator<Item = &'a str>) -> Result<(Roster, usize), String> {
        let mut it = lines.into_iter().filter(|l| !l.trim().is_empty());
        let first = it.next().ok_or("roster is empty")?;
        let g = SignedRecord::from_line(first).map_err(|e| format!("roster genesis: {e}"))?;
        let r = &g.record;
        let self_enrol = matches!(&r.body, Body::Enrol { key, guest: false, .. } if *key == r.signer);
        if r.project != project || !self_enrol || r.prev.is_some() {
            return Err("roster does not start with this project's genesis".into());
        }
        let mut roster = Roster { project: project.to_string(), ids: BTreeSet::from([g.rec.clone()]), records: vec![g] };
        let report = roster.merge_lines(it);
        Ok((roster, report.rejected))
    }

    pub fn project(&self) -> &str {
        &self.project
    }

    pub fn genesis_line(&self) -> String {
        self.records[0].to_line()
    }

    pub fn records(&self) -> &[SignedRecord] {
        &self.records
    }

    pub fn lines(&self) -> Vec<String> {
        self.records.iter().map(SignedRecord::to_line).collect()
    }

    /// Union with lines from this device's own file or a trusted source:
    /// every valid record is kept. Returns the records that were new.
    pub fn merge_lines<'a>(&mut self, lines: impl IntoIterator<Item = &'a str>) -> MergeReport {
        let mut report = MergeReport::default();
        for line in lines {
            match SignedRecord::from_line(line) {
                Ok(r) if r.record.project == self.project => {
                    if self.insert(r.clone()) {
                        report.added.push(r);
                    }
                }
                _ => report.rejected += 1,
            }
        }
        report
    }

    /// Merge a peer's full roster: refused unless it starts with our genesis
    /// and fits `MAX_PEER_RECORDS`. Only records whose signer is an enrolled,
    /// non-guest device in the merged roster are kept; anything else could
    /// never count, and storing it would let a peer grow every roster.
    pub fn merge_peer(&mut self, lines: &[String]) -> Result<MergeReport, MergeError> {
        if lines.len() > MAX_PEER_RECORDS {
            return Err(MergeError::TooManyRecords { records: lines.len(), limit: MAX_PEER_RECORDS });
        }
        let theirs = lines.first().and_then(|l| SignedRecord::from_line(l).ok());
        if theirs.as_ref() != Some(&self.records[0]) {
            return Err(MergeError::ForeignGenesis);
        }
        let mut union = self.clone();
        let mut report = union.merge_lines(lines.iter().map(String::as_str));
        let st = union.state();
        let signs = |r: &SignedRecord| st.devices.get(&r.record.signer).is_some_and(|d| !d.guest);
        let before = report.added.len();
        report.added.retain(signs);
        report.dropped = before - report.added.len();
        for r in &report.added {
            self.insert(r.clone());
        }
        Ok(report)
    }

    fn insert(&mut self, r: SignedRecord) -> bool {
        if !self.ids.insert(r.rec.clone()) {
            return false;
        }
        self.records.push(r);
        true
    }

    /// Sign and add a record as `key` at the tip of its chain, refusing
    /// what would be void anyway.
    pub fn append(&mut self, key: &DeviceKey, body: Body) -> Result<SignedRecord, String> {
        let me = key.public();
        match self.state().standing(&me) {
            Standing::Member => {}
            Standing::Guest => return Err("guest devices cannot change the roster".into()),
            Standing::Revoked => return Err("this device has been revoked".into()),
            Standing::Unknown => return Err("this device is not enrolled in the project".into()),
        }
        let r = SignedRecord::sign(key, &self.project, self.tip(&me), body);
        self.insert(r.clone());
        Ok(r)
    }

    /// The last record on `key`'s chain this roster holds.
    fn tip(&self, key: &PublicKey) -> Option<RecordHash> {
        Chains::of(&self.records).line.get(key).and_then(|l| l.last()).map(|r| r.hash)
    }

    pub fn enrol(&mut self, by: &DeviceKey, key: PublicKey, label: &str, guest: bool) -> Result<SignedRecord, String> {
        self.append(by, Body::Enrol { key, label: label.to_string(), guest })
    }

    /// Mark `key` lost, keeping the records of it this roster already holds.
    pub fn revoke(&mut self, by: &DeviceKey, key: PublicKey) -> Result<SignedRecord, String> {
        let upto = self.tip(&key);
        self.append(by, Body::Revoke { key, upto })
    }

    /// Materialize the record set.
    ///
    /// Validity has a negative dependency (a revoke voids its target's later
    /// records, which may have authorized the revoker), so this is computed
    /// as a fixpoint over the revocation cutoffs, each a count of standing
    /// records along a key's chain. `pass(P)` applies the lined-up records
    /// its signers may sign under cutoffs `P` and returns the cutoffs its
    /// valid revokes imply. Iterating from "no cutoffs" alternates, the even
    /// steps growing and the odd steps shrinking (the alternating fixpoint
    /// of well-founded semantics), so it terminates. If both sides meet,
    /// that is the unique answer. If not, the records form a paradox and the
    /// more restrictive side wins: fail closed. Forked keys are cut at their
    /// fork in every pass. Every step depends only on the record set, never
    /// on arrival order.
    pub fn state(&self) -> RosterState {
        let ch = Chains::of(&self.records);
        let mut before: Option<Cutoffs> = None;
        let mut cur = Cutoffs::new();
        let (chosen, mut contested) = loop {
            let next = self.pass(&ch, &cur).1;
            if next == cur {
                break (cur, BTreeSet::new());
            }
            if before.as_ref() == Some(&next) {
                let differ = cur.keys().chain(next.keys()).filter(|k| cur.get(*k) != next.get(*k)).copied().collect();
                break (meet(&cur, &next), differ);
            }
            before = Some(std::mem::replace(&mut cur, next));
        };
        let (mut st, _) = self.pass(&ch, &chosen);
        contested.extend(ch.forked.iter().copied());
        st.contested = contested;
        st.revoked = chosen.iter().map(|(k, n)| (*k, n.checked_sub(1).map(|i| ch.line[k][i].hash))).collect();
        st
    }

    fn pass(&self, ch: &Chains, cutoffs: &Cutoffs) -> (RosterState, Cutoffs) {
        let mut st = RosterState::default();
        let genesis = &self.records[0];
        let Body::Enrol { key, label, guest } = &genesis.record.body else {
            unreachable!("genesis is checked to be an enrol")
        };
        st.devices.insert(*key, Device { label: label.clone(), guest: *guest });
        let mut revoked: Cutoffs = ch.forked.iter().map(|k| (*k, ch.len(k))).collect();
        let mut pending: Vec<(usize, &SignedRecord)> =
            ch.line.values().flatten().filter(|r| r.hash != genesis.hash).map(|r| (ch.pos[&r.hash], *r)).collect();
        pending.sort_by(|a, b| (a.0, &a.1.rec).cmp(&(b.0, &b.1.rec)));
        let mut current_dvc: BTreeMap<PublicKey, (usize, &str)> = BTreeMap::new();
        while let Some(i) = pending.iter().position(|(p, r)| may_sign(&st.devices, cutoffs, &r.record.signer, *p)) {
            let (p, r) = pending.remove(i);
            let signer = r.record.signer;
            match &r.record.body {
                Body::Enrol { key, label, guest } => {
                    let d = st.devices.entry(*key).or_insert(Device { label: label.clone(), guest: *guest });
                    d.guest &= *guest;
                }
                Body::Revoke { key, upto } => {
                    let mut n = ch.cut(key, *upto);
                    if *key == signer {
                        n = n.max(p + 1);
                    }
                    let cut = revoked.entry(*key).or_insert(n);
                    *cut = (*cut).min(n);
                }
                Body::Attest { key, dvc } => {
                    if *key == signer {
                        let cur = current_dvc.entry(signer).or_insert((p, dvc));
                        if p >= cur.0 {
                            *cur = (p, dvc);
                        }
                    }
                }
            }
        }
        for (key, (_, dvc)) in current_dvc {
            st.attested.entry(dvc.to_string()).or_default().insert(key);
        }
        (st, revoked)
    }
}

/// Per revoked key, how many records of its chain stand.
type Cutoffs = BTreeMap<PublicKey, usize>;

/// Only an enrolled non-guest may sign, and a revoked one only inside its cutoff.
fn may_sign(devices: &BTreeMap<PublicKey, Device>, cutoffs: &Cutoffs, signer: &PublicKey, pos: usize) -> bool {
    let enrolled = devices.get(signer).is_some_and(|d| !d.guest);
    enrolled && cutoffs.get(signer).is_none_or(|cut| pos < *cut)
}

/// Both cutoff maps at once: every key either revokes, at the lower cutoff.
fn meet(a: &Cutoffs, b: &Cutoffs) -> Cutoffs {
    let mut out = a.clone();
    for (k, v) in b {
        let e = out.entry(*k).or_insert(*v);
        *e = (*e).min(*v);
    }
    out
}

/// `auth/roster.jsonl` inside a project folder. Not part of the archive
/// format: sealed packages leave `auth/` out.
pub struct RosterFile {
    path: PathBuf,
}

impl RosterFile {
    pub fn in_project(project_root: impl AsRef<Path>) -> RosterFile {
        RosterFile { path: project_root.as_ref().join("auth").join("roster.jsonl") }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    pub fn load(&self, project: &str) -> Result<Roster, String> {
        let text = fs::read_to_string(&self.path).map_err(|e| format!("read {}: {e}", self.path.display()))?;
        Ok(Roster::from_lines(project, text.lines())?.0)
    }

    /// Write a whole roster to a file that must not exist yet.
    pub fn create(&self, roster: &Roster) -> Result<(), String> {
        if self.path.exists() {
            return Err(format!("{} already exists", self.path.display()));
        }
        self.append(roster.records())
    }

    /// Append records (and fsync). Readers drop a torn last line, so a crash
    /// mid-append loses at most the record being written.
    pub fn append(&self, records: &[SignedRecord]) -> Result<(), String> {
        if records.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(self.path.parent().expect("auth dir")).map_err(|e| e.to_string())?;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("open {}: {e}", self.path.display()))?;
        // A torn last line must not swallow the first record appended after it.
        let torn = fs::read(&self.path).map(|b| b.last().is_some_and(|c| *c != b'\n')).unwrap_or(false);
        let mut buf = String::from(if torn { "\n" } else { "" });
        for r in records {
            buf.push_str(&r.to_line());
            buf.push('\n');
        }
        f.write_all(buf.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())
    }
}
