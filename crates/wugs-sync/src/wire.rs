//! The sync protocol as a pure request/response machine, with no I/O, so a
//! transport (wugs-net) only moves JSON values and the in-process
//! `sync_pair` runs the identical logic.
//!
//! One connection, half-duplex, two pulls: each peer in turn plays the
//! `Requester` against the other's `Responder`. A pull walks down
//! root → group hashes → window hashes → ops, fetching only what differs,
//! then ingests. Hashes are 64-bit and JSON numbers are f64, so hashes
//! travel as hex strings; windows and groups travel as decimal string keys.
//!
//! The responder applies the sender-side `SyncPolicy` to every message, so
//! a request for a denied scope reveals nothing, whatever the requester
//! asks (DATA-MODEL §4).

use std::collections::{BTreeMap, BTreeSet};

use wugs_core::{IngestReport, Project};
use wugs_model::{json, Op, Value};

use crate::{collect_ops, scopes, summary, window_of, SyncPolicy, GROUP_SHIFT};

pub const PROTOCOL: f64 = 1.0;

fn obj(kind: &str, fields: Vec<(&str, Value)>) -> Value {
    let mut all = vec![("t", Value::str(kind)), ("v", Value::Num(PROTOCOL))];
    all.extend(fields);
    Value::obj(all)
}

fn hex(h: u64) -> Value {
    Value::str(&format!("{h:016x}"))
}

fn parse_hex(v: &Value) -> Result<u64, String> {
    u64::from_str_radix(v.as_str().ok_or("hash is not a string")?, 16)
        .map_err(|e| format!("bad hash: {e}"))
}

fn parse_u64_key(k: &str) -> Result<u64, String> {
    k.parse::<u64>().map_err(|e| format!("bad numeric key {k:?}: {e}"))
}

fn hash_map_value(m: &BTreeMap<u64, u64>) -> Value {
    Value::Obj(m.iter().map(|(k, v)| (k.to_string(), hex(*v))).collect())
}

fn parse_hash_map(v: &Value) -> Result<BTreeMap<u64, u64>, String> {
    let Value::Obj(m) = v else {
        return Err("expected an object of hashes".into());
    };
    m.iter()
        .map(|(k, v)| Ok((parse_u64_key(k)?, parse_hex(v)?)))
        .collect()
}

fn str_list(v: Option<&Value>) -> Result<Vec<String>, String> {
    match v {
        Some(Value::Arr(a)) => a
            .iter()
            .map(|s| s.as_str().map(str::to_string).ok_or_else(|| "expected a string".to_string()))
            .collect(),
        _ => Err("expected an array of strings".into()),
    }
}

fn kind(msg: &Value) -> Result<&str, String> {
    msg.get_str("t").ok_or_else(|| "message without a type".to_string())
}

// ----------------------------------------------------------------- responder

/// Answers one request from its peer. Every answer is computed from the
/// project at that moment; the only state is the hello gate: no data
/// request is answered before a hello naming this project and protocol,
/// so a raw client cannot skip the identity exchange and pull scopes.
pub struct Responder<'a> {
    pub project: &'a Project,
    pub policy: &'a SyncPolicy,
    greeted: bool,
}

impl<'a> Responder<'a> {
    pub fn new(project: &'a Project, policy: &'a SyncPolicy) -> Responder<'a> {
        Responder { project, policy, greeted: false }
    }

    pub fn handle(&mut self, msg: &Value) -> Result<Value, String> {
        let k = kind(msg)?;
        if k != "hello" && !self.greeted {
            return Err("hello comes first".into());
        }
        match k {
            "hello" => {
                if msg.get("v").and_then(Value::as_f64) != Some(PROTOCOL) {
                    return Err("unsupported protocol version".into());
                }
                let theirs = msg.get_str("project").unwrap_or_default();
                if theirs != self.project.project_id {
                    return Err(format!(
                        "peer syncs project {theirs}, this is {}",
                        self.project.project_id
                    ));
                }
                self.greeted = true;
                Ok(obj(
                    "hello",
                    vec![
                        ("project", Value::str(&self.project.project_id)),
                        ("device", Value::str(self.project.device())),
                    ],
                ))
            }
            "get_roots" => {
                let mut roots = Vec::new();
                for scope in scopes(&self.project.proj)? {
                    if !self.policy.permits(&scope) {
                        continue;
                    }
                    let s = summary(&self.project.proj, &scope)?;
                    roots.push(Value::obj(vec![("scope", Value::str(&scope)), ("root", hex(s.root))]));
                }
                Ok(obj("roots", vec![("scopes", Value::Arr(roots))]))
            }
            "get_groups" => {
                let mut out: Vec<(String, Value)> = Vec::new();
                for scope in str_list(msg.get("scopes"))? {
                    if !self.policy.permits(&scope) {
                        continue;
                    }
                    out.push((scope.clone(), hash_map_value(&summary(&self.project.proj, &scope)?.groups())));
                }
                Ok(obj("groups", vec![("scopes", Value::Obj(out.into_iter().collect()))]))
            }
            "get_windows" => {
                let Some(Value::Obj(want)) = msg.get("scopes") else {
                    return Err("get_windows without scopes".into());
                };
                let mut out: Vec<(String, Value)> = Vec::new();
                for (scope, groups) in want {
                    if !self.policy.permits(scope) {
                        continue;
                    }
                    let s = summary(&self.project.proj, scope)?;
                    let mut windows = BTreeMap::new();
                    for g in str_list(Some(groups))? {
                        windows.extend(s.windows_in(parse_u64_key(&g)?));
                    }
                    out.push((scope.clone(), hash_map_value(&windows)));
                }
                Ok(obj("windows", vec![("scopes", Value::Obj(out.into_iter().collect()))]))
            }
            "get_ops" => {
                let Some(Value::Obj(want)) = msg.get("scopes") else {
                    return Err("get_ops without scopes".into());
                };
                let mut ops_json = Vec::new();
                for (scope, windows) in want {
                    if !self.policy.permits(scope) {
                        continue;
                    }
                    let set: BTreeSet<u64> = str_list(Some(windows))?
                        .iter()
                        .map(|w| parse_u64_key(w))
                        .collect::<Result<_, _>>()?;
                    for op in collect_ops(&self.project.log, &self.project.proj, scope, &set)? {
                        ops_json.push(json::parse(&op.to_json()).map_err(|e| e.to_string())?);
                    }
                }
                Ok(obj("ops", vec![("ops", Value::Arr(ops_json))]))
            }
            other => Err(format!("unknown request {other}")),
        }
    }
}

// ----------------------------------------------------------------- requester

/// One pull: what the requester wants next, fed by each response. Drive it:
/// loop { take request, get response, feed it } until `None`, then read
/// `report`.
/// Windows per get_ops request. A txn-widened window batch must fit one
/// 32 MiB transport frame, and a joining laptop may want a whole project,
/// so ops travel in bounded batches rather than one frame.
const OPS_CHUNK_WINDOWS: usize = 16;

pub struct Requester {
    state: State,
    /// Windows this side wants, by scope, filled during the walk-down.
    wanted: BTreeMap<String, BTreeSet<u64>>,
    /// (scope, windows) batches still to fetch, derived from `wanted`.
    queue: Vec<(String, Vec<u64>)>,
    /// The requester's own project id and the peer's hello check.
    project_id: String,
    pub report: Option<IngestReport>,
    /// Fresh ops ingested; ops this side already had are dropped on
    /// arrival and counted here instead. A txn-widened window can carry
    /// ops the requester owns (its own ops share windows with the peer's),
    /// so the drop is part of the protocol, not an optimization.
    pub ops_received: usize,
    pub ops_dropped_known: usize,
    pub ops_by_scope: BTreeMap<String, usize>,
}

enum State {
    Hello,
    Roots,
    Groups(Vec<String>),
    Windows(BTreeMap<String, Vec<u64>>),
    Ops,
    Done,
}

impl Requester {
    pub fn new(project_id: &str) -> Requester {
        Requester {
            state: State::Hello,
            wanted: BTreeMap::new(),
            queue: Vec::new(),
            project_id: project_id.to_string(),
            report: None,
            ops_received: 0,
            ops_dropped_known: 0,
            ops_by_scope: BTreeMap::new(),
        }
    }

    /// How many windows this pull asked ops for.
    pub fn windows_wanted(&self) -> usize {
        self.wanted.values().map(BTreeSet::len).sum()
    }

    /// The next request to send, or None when the pull is complete.
    pub fn request(&self) -> Option<Value> {
        match &self.state {
            State::Hello => Some(obj("hello", vec![("project", Value::str(&self.project_id))])),
            State::Roots => Some(obj("get_roots", vec![])),
            State::Groups(scopes) => Some(obj(
                "get_groups",
                vec![("scopes", Value::Arr(scopes.iter().map(|s| Value::str(s)).collect()))],
            )),
            State::Windows(want) => Some(obj(
                "get_windows",
                vec![(
                    "scopes",
                    Value::Obj(
                        want.iter()
                            .map(|(s, gs)| {
                                (s.clone(), Value::Arr(gs.iter().map(|g| Value::str(&g.to_string())).collect()))
                            })
                            .collect(),
                    ),
                )],
            )),
            State::Ops => {
                let (scope, windows) = self.queue.last()?;
                Some(obj(
                    "get_ops",
                    vec![(
                        "scopes",
                        Value::Obj(
                            [(scope.clone(), Value::Arr(windows.iter().map(|w| Value::str(&w.to_string())).collect()))]
                                .into_iter()
                                .collect(),
                        ),
                    )],
                ))
            }
            State::Done => None,
        }
    }

    /// Feed the response to the last request. `project` is this side's own.
    pub fn respond(&mut self, project: &mut Project, resp: &Value) -> Result<(), String> {
        match (&self.state, kind(resp)?) {
            (State::Hello, "hello") => {
                let theirs = resp.get_str("project").unwrap_or_default();
                if theirs != project.project_id {
                    return Err(format!(
                        "peer syncs project {theirs}, this is {}",
                        project.project_id
                    ));
                }
                self.state = State::Roots;
            }
            (State::Roots, "roots") => {
                let Some(Value::Arr(items)) = resp.get("scopes") else {
                    return Err("roots without scopes".into());
                };
                let mut differ = Vec::new();
                for item in items {
                    let scope = item.get_str("scope").ok_or("root without scope")?;
                    let root = parse_hex(item.get("root").ok_or("root without hash")?)?;
                    if summary(&project.proj, scope)?.root != root {
                        differ.push(scope.to_string());
                    }
                }
                self.state = if differ.is_empty() { State::Done } else { State::Groups(differ) };
            }
            (State::Groups(asked), "groups") => {
                let Some(Value::Obj(scopes)) = resp.get("scopes") else {
                    return Err("groups without scopes".into());
                };
                let mut want: BTreeMap<String, Vec<u64>> = BTreeMap::new();
                for (scope, theirs) in scopes {
                    // The peer is authenticated, not trusted: a scope this
                    // side never asked about must not enter the walk-down.
                    if !asked.contains(scope) {
                        return Err(format!("peer sent groups for unasked scope {scope}"));
                    }
                    let theirs = parse_hash_map(theirs)?;
                    let mine = summary(&project.proj, scope)?.groups();
                    let differ: Vec<u64> = theirs
                        .iter()
                        .filter(|(g, h)| mine.get(g) != Some(h))
                        .map(|(g, _)| *g)
                        .collect();
                    if !differ.is_empty() {
                        want.insert(scope.clone(), differ);
                    }
                }
                self.state = if want.is_empty() { State::Done } else { State::Windows(want) };
            }
            (State::Windows(asked), "windows") => {
                let Some(Value::Obj(scopes)) = resp.get("scopes") else {
                    return Err("windows without scopes".into());
                };
                for (scope, theirs) in scopes {
                    let Some(asked_groups) = asked.get(scope) else {
                        return Err(format!("peer sent windows for unasked scope {scope}"));
                    };
                    let theirs = parse_hash_map(theirs)?;
                    if let Some(w) = theirs.keys().find(|w| !asked_groups.contains(&(*w >> GROUP_SHIFT))) {
                        return Err(format!("peer sent window {w} outside the asked groups of {scope}"));
                    }
                    let mine = summary(&project.proj, scope)?;
                    let want: BTreeSet<u64> = theirs
                        .iter()
                        .filter(|(w, h)| mine.windows.get(w) != Some(h))
                        .map(|(w, _)| *w)
                        .collect();
                    if !want.is_empty() {
                        self.wanted.insert(scope.clone(), want);
                    }
                }
                for (scope, ws) in &self.wanted {
                    let ws: Vec<u64> = ws.iter().copied().collect();
                    for chunk in ws.chunks(OPS_CHUNK_WINDOWS) {
                        self.queue.push((scope.clone(), chunk.to_vec()));
                    }
                }
                self.state = if self.queue.is_empty() { State::Done } else { State::Ops };
            }
            (State::Ops, "ops") => {
                let Some(Value::Arr(items)) = resp.get("ops") else {
                    return Err("ops without ops".into());
                };
                let (asked_scope, asked_windows) = self.queue.last().ok_or("ops response with nothing asked")?;
                let asked_windows: BTreeSet<u64> = asked_windows.iter().copied().collect();
                // The peer is authenticated, not trusted: the batch must be
                // the requested scope, and every txn in it must anchor in an
                // asked window (whole txns travel, so the other ops of an
                // anchored txn may fall outside — that is the widening rule,
                // checked here as the responder computes it).
                let mut parsed = Vec::with_capacity(items.len());
                let mut anchored: BTreeSet<&str> = BTreeSet::new();
                for item in items {
                    let op = Op::from_json(&item.to_json())?;
                    if op.scope != *asked_scope {
                        return Err(format!("peer sent an op of scope {} to a request for {asked_scope}", op.scope));
                    }
                    parsed.push(op);
                }
                for op in &parsed {
                    let w = window_of(&op.hlc).ok_or_else(|| format!("peer sent an op with unparseable hlc {}", op.hlc))?;
                    if asked_windows.contains(&w) {
                        anchored.insert(&op.txn);
                    }
                }
                if let Some(op) = parsed.iter().find(|op| !anchored.contains(op.txn.as_str())) {
                    return Err(format!("peer sent txn {} with no op in an asked window", op.txn));
                }
                let mut ops = Vec::with_capacity(parsed.len());
                for op in parsed {
                    if project.proj.already_applied(&op.id)? {
                        self.ops_dropped_known += 1;
                        continue;
                    }
                    *self.ops_by_scope.entry(op.scope.clone()).or_default() += 1;
                    ops.push(op);
                }
                self.ops_received += ops.len();
                let report = project.ingest(ops)?;
                match self.report.as_mut() {
                    Some(acc) => {
                        acc.txns_applied += report.txns_applied;
                        acc.txns_held += report.txns_held;
                        acc.ops_applied += report.ops_applied;
                        acc.ops_skipped_dupes += report.ops_skipped_dupes;
                        acc.conflicts += report.conflicts;
                        acc.txns_retried += report.txns_retried;
                        acc.held.extend(report.held);
                    }
                    None => self.report = Some(report),
                }
                self.queue.pop();
                if self.queue.is_empty() {
                    self.state = State::Done;
                }
            }
            (_, other) => return Err(format!("unexpected response {other}")),
        }
        Ok(())
    }
}

/// Drive one pull against an in-process responder; the network transport
/// does exactly this with frames in between.
pub fn pull(receiver: &mut Project, sender: &Project, sender_policy: &SyncPolicy) -> Result<Requester, String> {
    let mut responder = Responder::new(sender, sender_policy);
    let mut req = Requester::new(&receiver.project_id);
    while let Some(msg) = req.request() {
        let resp = responder.handle(&msg)?;
        req.respond(receiver, &resp)?;
    }
    Ok(req)
}
