//! wugs-sync: merkle-verified pair sync (DATA-MODEL.md §6).
//!
//! Divergence detection is a per-scope bucket map. Every applied op falls in a
//! time window (`hlc.millis >> 16`, about 65.5 s). A window's hash is the XOR of
//! `fnv1a64(op_id)` over its ops, so it is order-independent and a pure function
//! of the op-id set. Two devices compare one root, then only the windows whose
//! hashes differ.
//!
//! The hash is non-cryptographic. It detects divergence and accidental
//! corruption, not an adversary. The upgrade path is BLAKE3 once crates are
//! reachable; only `fnv1a64` and the two folds below change.
//!
//! ACL enforcement lives here. A scope the sender's `SyncPolicy` denies is never
//! summarized for the peer, never collected from the log, and so never reaches
//! the peer's `ingest`.

pub mod wire;

use std::collections::{BTreeMap, BTreeSet};

pub use wire::{pull, Requester, Responder};

use wugs_core::{IngestReport, Project};
use wugs_model::{Hlc, Op};
use wugs_oplog::LogDir;
use wugs_projection::Projection;

const WINDOW_SHIFT: u32 = 16;
/// Second level of the walk-down: a group is ~256 windows (~4.7 hours).
const GROUP_SHIFT: u32 = 8;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a64_from(mut h: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    fnv1a64_from(FNV_OFFSET, bytes)
}

fn window_of(hlc: &str) -> Option<u64> {
    Hlc::parse(hlc).map(|h| h.millis >> WINDOW_SHIFT)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeSummary {
    pub scope: String,
    pub root: u64,
    pub windows: BTreeMap<u64, u64>,
}

impl ScopeSummary {
    fn from_windows(scope: &str, windows: BTreeMap<u64, u64>) -> ScopeSummary {
        let root = windows.iter().fold(FNV_OFFSET, |acc, (w, h)| {
            fnv1a64_from(fnv1a64_from(acc, &w.to_le_bytes()), &h.to_le_bytes())
        });
        ScopeSummary { scope: scope.to_string(), root, windows }
    }
}

/// Read-only over `ops_applied` plus the held ops in quarantine. A summary
/// answers "which ops does this device HAVE", applied or held, so peers
/// stop re-offering a held txn and roots converge while it sits in
/// quarantine; retrying it is the holder's own job (wugs-core).
pub fn summary(proj: &Projection, scope: &str) -> Result<ScopeSummary, String> {
    let mut s = proj
        .db
        .prepare("SELECT op_id, hlc FROM ops_applied WHERE scope=?1")?;
    s.bind_text(1, scope)?;
    let mut windows: BTreeMap<u64, u64> = BTreeMap::new();
    while s.step()? {
        let hlc = s.col_text(1);
        let w = window_of(&hlc).ok_or_else(|| format!("unparseable hlc in ops_applied: {hlc}"))?;
        *windows.entry(w).or_insert(0) ^= fnv1a64(s.col_text(0).as_bytes());
    }
    for (op_id, hlc) in proj.held_op_ids(scope)? {
        let w = window_of(&hlc).ok_or_else(|| format!("unparseable hlc in quarantine_ops: {hlc}"))?;
        *windows.entry(w).or_insert(0) ^= fnv1a64(op_id.as_bytes());
    }
    // Parked ops are in the log and will apply when their dependency
    // arrives; without them the sender re-offers (and the receiver re-logs)
    // a parked op on every sync, forever.
    for (op_id, hlc) in proj.pending_op_ids(scope)? {
        let w = window_of(&hlc).ok_or_else(|| format!("unparseable hlc in pending: {hlc}"))?;
        *windows.entry(w).or_insert(0) ^= fnv1a64(op_id.as_bytes());
    }
    Ok(ScopeSummary::from_windows(scope, windows))
}

impl ScopeSummary {
    /// Group hash = order-independent fold of this group's (window, hash)
    /// pairs, for the walk-down's middle level.
    pub fn groups(&self) -> BTreeMap<u64, u64> {
        let mut out: BTreeMap<u64, u64> = BTreeMap::new();
        for (w, h) in &self.windows {
            *out.entry(w >> GROUP_SHIFT).or_insert(0) ^=
                fnv1a64_from(fnv1a64(&w.to_le_bytes()), &h.to_le_bytes());
        }
        out
    }

    /// The windows of one group. The group number can come from a peer, so
    /// a value whose window range would overflow yields nothing instead of
    /// panicking the server.
    pub fn windows_in(&self, group: u64) -> BTreeMap<u64, u64> {
        let Some(start) = group.checked_shl(GROUP_SHIFT).filter(|s| s >> GROUP_SHIFT == group) else {
            return BTreeMap::new();
        };
        let end = start.saturating_add(1 << GROUP_SHIFT);
        self.windows
            .range(start..end)
            .map(|(w, h)| (*w, *h))
            .collect()
    }
}

pub fn scopes(proj: &Projection) -> Result<Vec<String>, String> {
    let mut s = proj
        .db
        .prepare("SELECT DISTINCT scope FROM ops_applied ORDER BY scope")?;
    let mut out = Vec::new();
    while s.step()? {
        out.push(s.col_text(0));
    }
    Ok(out)
}

/// Windows whose hash differs or that exist on one side only. Symmetric.
pub fn diff_windows(a: &ScopeSummary, b: &ScopeSummary) -> Vec<u64> {
    if a.root == b.root {
        return Vec::new();
    }
    a.windows
        .keys()
        .chain(b.windows.keys())
        .copied()
        .collect::<BTreeSet<u64>>()
        .into_iter()
        .filter(|w| a.windows.get(w) != b.windows.get(w))
        .collect()
}

/// Ops of `scope` in `windows`, widened to every op of the txns they belong
/// to. A txn straddling a window boundary must travel whole because the
/// receiver applies and validates txns atomically. Held and parked ops are
/// served alongside the log: a summary counts them, so a peer that lacks
/// them will ask this device for them even when the original sender is gone.
// TODO: v0 streams the whole log twice; read only `log/<scope>/` files instead.
pub fn collect_ops(
    log: &LogDir,
    proj: &Projection,
    scope: &str,
    windows: &BTreeSet<u64>,
) -> Result<Vec<Op>, String> {
    if windows.is_empty() {
        return Ok(Vec::new());
    }
    let extras: Vec<Op> = proj
        .held_ops_of_scope(scope)?
        .into_iter()
        .chain(proj.pending_ops(scope)?)
        .collect();
    let bad_hlc = |hlc: &str| format!("bad hlc {hlc}");
    let mut txns: BTreeSet<String> = BTreeSet::new();
    let mut stream = log.stream_all().map_err(|e| e.to_string())?;
    while let Some(op) = stream.next_op().map_err(|e| e.to_string())? {
        if op.scope != scope {
            continue;
        }
        let w = window_of(&op.hlc).ok_or_else(|| bad_hlc(&op.hlc))?;
        if windows.contains(&w) {
            txns.insert(op.txn);
        }
    }
    for op in &extras {
        let w = window_of(&op.hlc).ok_or_else(|| bad_hlc(&op.hlc))?;
        if windows.contains(&w) {
            txns.insert(op.txn.clone());
        }
    }
    let mut out = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stream = log.stream_all().map_err(|e| e.to_string())?;
    while let Some(op) = stream.next_op().map_err(|e| e.to_string())? {
        if op.scope == scope && txns.contains(&op.txn) && seen.insert(op.id.clone()) {
            out.push(op);
        }
    }
    for op in extras {
        if txns.contains(&op.txn) && seen.insert(op.id.clone()) {
            out.push(op);
        }
    }
    Ok(out)
}

/// Which scopes a device may send. Two variants instead of a flag pair so a
/// policy cannot be both allow-list and deny-list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncPolicy {
    Deny(BTreeSet<String>),
    Only(BTreeSet<String>),
}

impl SyncPolicy {
    pub fn allow_all() -> SyncPolicy {
        SyncPolicy::Deny(BTreeSet::new())
    }

    pub fn deny<I: IntoIterator<Item = S>, S: Into<String>>(scopes: I) -> SyncPolicy {
        SyncPolicy::Deny(scopes.into_iter().map(Into::into).collect())
    }

    pub fn only<I: IntoIterator<Item = S>, S: Into<String>>(scopes: I) -> SyncPolicy {
        SyncPolicy::Only(scopes.into_iter().map(Into::into).collect())
    }

    pub fn permits(&self, scope: &str) -> bool {
        match self {
            SyncPolicy::Deny(d) => !d.contains(scope),
            SyncPolicy::Only(o) => o.contains(scope),
        }
    }
}

/// The default sending policy, derived from the data instead of a hand
/// built list (DATA-MODEL §4), and fail-closed: it ALLOWS the lexicon, the
/// project config, and each live source or text whose effective access is
/// "open"; every other scope is withheld — a deleted or held text, a scope
/// created after the policy was built, or any scope string a peer invents.
/// A text with no access of its own inherits its linked source's. `share`
/// names scopes the user explicitly chose to send anyway — for example
/// after confirming the peer is the community's own archive laptop. Until
/// peers have identities (the pairing/PSK follow-up), explicit per-scope
/// consent is the only authorization there is, so withholding is the
/// default, not an option. Rebuild the policy per session: it is a
/// snapshot, and an allow computed before a restriction landed would leak.
pub fn policy_from_access(proj: &Projection, share: &[&str]) -> Result<SyncPolicy, String> {
    let mut allow: BTreeSet<String> = BTreeSet::new();
    allow.insert("lexicon".to_string());
    allow.insert("project".to_string());
    let effective = |id: &str| -> Result<String, String> {
        if let Some(a) = proj.field_str(id, "access")? {
            return Ok(a);
        }
        // DATA-MODEL §3: a text inherits its linked source's access
        if let Some(src) = proj.field_str(id, "source")? {
            if let Some(a) = proj.field_str(&src, "access")? {
                return Ok(a);
            }
        }
        Ok("open".to_string())
    };
    for kind in ["source", "text"] {
        for id in proj.entities_of_kind(kind)? {
            if effective(&id)? == "open" {
                allow.insert(id);
            }
        }
    }
    for s in share {
        allow.insert((*s).to_string());
    }
    Ok(SyncPolicy::Only(allow))
}


#[derive(Debug, Default)]
pub struct Transfer {
    pub windows: usize,
    pub ops: usize,
    pub ops_by_scope: BTreeMap<String, usize>,
    pub ingest: IngestReport,
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub a_to_b: Transfer,
    pub b_to_a: Transfer,
}

/// Both directions over the same request/response machine the network
/// transport uses: first `a` answers `b`'s pull, then the roles swap. Ops
/// `b` just mirrored from `a` are already in both ops_applied sets by the
/// second pull, so they never bounce back.
pub fn sync_pair(
    a: &mut Project,
    b: &mut Project,
    a_sends: &SyncPolicy,
    b_sends: &SyncPolicy,
) -> Result<SyncReport, String> {
    Ok(SyncReport {
        a_to_b: transfer(wire::pull(b, a, a_sends)?),
        b_to_a: transfer(wire::pull(a, b, b_sends)?),
    })
}

fn transfer(r: wire::Requester) -> Transfer {
    Transfer {
        windows: r.windows_wanted(),
        ops: r.ops_received,
        ops_by_scope: r.ops_by_scope.clone(),
        ingest: r.report.unwrap_or_default(),
    }
}

