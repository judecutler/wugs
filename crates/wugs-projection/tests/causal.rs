//! Conflict detection against a causal oracle (DATA-MODEL §5). Random
//! histories of 2–3 devices and 1–2 people writing one field with sets and
//! text-edits, with random partial syncs between them. Every device writes
//! with the heads its own projection shows, the way `Txn` does. Checks:
//!
//! - every arrival order that keeps each device's own order gives the
//!   projection replay gives (value, heads, conflict rows);
//! - the heads are exactly the writes nothing written later knew of, and
//!   the conflict rows pair the winner with each other one, except two
//!   merged text-edits, and except two writes holding the same value;
//! - the value is the merged text when a merged text-edit wins and the
//!   updates merge as a set, else the winner's own value. A fragile merger
//!   refuses some sets, as colliding Loro op ids would. With a merger, a third of the text-edits come from
//!   a replica without one (no loro payload) and take the LWW path.
//!
//! `WUGS_CAUSAL_CASES` sets the number of histories per mode (default 200).

use std::collections::{BTreeMap, BTreeSet};

use wugs_model::{base64, Clock, Hlc, Op, TextMerger, Value};
use wugs_projection::{ApplyStats, Projection};

const TARGET: &str = "mor_1";
const FIELD: &str = "gloss";

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Merges by keeping every edit's text, sorted: a function of the set of
/// updates, like Loro's merge, and easy to predict.
/// A fragile one refuses any set holding two edits whose numbers add up to
/// a multiple of 7, the way colliding Loro op ids make a set unmergeable
/// whatever the order.
struct SetMerger {
    fragile: bool,
}

impl TextMerger for SetMerger {
    fn resolve(&self, updates: &[Vec<u8>]) -> Result<String, String> {
        let texts: BTreeSet<String> = updates.iter().map(|u| String::from_utf8_lossy(u).into_owned()).collect();
        if self.fragile {
            let nums: Vec<u64> = texts.iter().filter_map(|t| t.trim_start_matches('v').parse().ok()).collect();
            for (i, a) in nums.iter().enumerate() {
                if nums[i + 1..].iter().any(|b| (a + b) % 7 == 0) {
                    return Err("updates collide".into());
                }
            }
        }
        Ok(texts.into_iter().collect::<Vec<_>>().join("+"))
    }
    fn edit(&self, updates: &[Vec<u8>], _prior: Option<(&str, u64)>, new_text: &str, _peer: u64) -> Result<(Vec<u8>, String), String> {
        let mut all = updates.to_vec();
        all.push(new_text.as_bytes().to_vec());
        Ok((new_text.as_bytes().to_vec(), self.resolve(&all)?))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    /// No merger: every text-edit takes the LWW path.
    Plain,
    Merged,
    /// A merger that refuses some sets of updates.
    Fragile,
}

impl Mode {
    fn merger(self) -> Option<SetMerger> {
        match self {
            Mode::Plain => None,
            Mode::Merged => Some(SetMerger { fragile: false }),
            Mode::Fragile => Some(SetMerger { fragile: true }),
        }
    }
}

fn projection(mode: Mode) -> Projection {
    let mut p = Projection::open_memory().unwrap();
    if let Some(m) = mode.merger() {
        p.merger = Some(Box::new(m));
    }
    p
}

/// The loro updates of these ops, and whether they merge as a set.
fn loro_set(ops: &[&Op], mode: Mode) -> (Vec<Vec<u8>>, bool) {
    let texts: BTreeSet<&str> = ops.iter().filter_map(|o| o.value.get_str("loro")).collect();
    let updates: Vec<Vec<u8>> = texts.iter().map(|t| base64::decode(t).unwrap()).collect();
    let merges = mode.merger().is_some_and(|m| m.resolve(&updates).is_ok());
    (updates, merges)
}

struct Device {
    proj: Projection,
    clock: Clock,
    /// Ops held, in arrival order.
    held: Vec<Op>,
}

impl Device {
    fn has(&self, id: &str) -> bool {
        self.held.iter().any(|o| o.id == id)
    }
    fn receive(&mut self, op: &Op) {
        self.clock.observe(&Hlc::parse(&op.hlc).unwrap()).unwrap();
        self.proj.apply(op, &mut ApplyStats::default()).unwrap();
        self.held.push(op.clone());
    }
}

/// Everything a reader can see of the field.
#[derive(Debug, PartialEq)]
struct FieldState {
    value: Option<String>,
    heads: Vec<String>,
    /// (winner op, loser op)
    conflicts: Vec<(String, String)>,
}

fn state(p: &Projection) -> FieldState {
    let mut s = p.db.prepare("SELECT winner_op, loser_op FROM conflicts WHERE target=?1 AND field=?2 ORDER BY loser_op").unwrap();
    s.bind_text(1, TARGET).unwrap();
    s.bind_text(2, FIELD).unwrap();
    let mut conflicts = Vec::new();
    while s.step().unwrap() {
        conflicts.push((s.col_text(0), s.col_text(1)));
    }
    FieldState {
        value: p.field(TARGET, FIELD).unwrap().map(|(v, _)| v.to_json()),
        heads: p.heads(TARGET, FIELD).unwrap(),
        conflicts,
    }
}

fn replay(ops: &[Op], mode: Mode) -> FieldState {
    let p = projection(mode);
    let mut sorted = ops.to_vec();
    sorted.sort_by(|a, b| a.hlc.cmp(&b.hlc));
    for op in &sorted {
        p.apply(op, &mut ApplyStats::default()).unwrap();
    }
    state(&p)
}

/// One field write and what its writer held when it wrote.
struct Write {
    op: Op,
    knew: BTreeSet<String>,
}

struct History {
    writes: Vec<Write>,
    devices: Vec<Device>,
}

fn generate(rng: &mut Rng, mode: Mode) -> History {
    let n_dev = 2 + rng.below(2);
    let actors = ["prs_Ana", "prs_Bo"];
    let n_actors = 1 + rng.below(2);
    let mut devices: Vec<Device> = (0..n_dev)
        .map(|i| Device { proj: projection(mode), clock: Clock::new(format!("dvc_{i}")), held: Vec::new() })
        .collect();
    let mut writes: Vec<Write> = Vec::new();
    let mut seq = 0;
    let mut write = |dev: &mut Device, verb: &str, value: Value, actor: &str, writes: &mut Vec<Write>| {
        seq += 1;
        let mut op = Op {
            id: format!("op_{seq:03}"),
            hlc: dev.clock.now().encode(),
            txn: format!("txn_{seq:03}"),
            intent: None,
            actor: actor.into(),
            device: dev.clock.device.clone(),
            session: None,
            scope: "txt_1".into(),
            schema: wugs_model::SCHEMA_VERSION,
            verb: verb.into(),
            entity: "morph".into(),
            target: TARGET.into(),
            field: (verb != "create").then(|| FIELD.to_string()),
            value,
            base: dev.proj.field(TARGET, FIELD).unwrap().map(|(_, h)| h),
            heads: None,
        };
        op.set_heads(dev.proj.heads(TARGET, FIELD).unwrap());
        let knew = dev.held.iter().map(|o| o.id.clone()).collect();
        dev.proj.apply(&op, &mut ApplyStats::default()).unwrap();
        dev.held.push(op.clone());
        writes.push(Write { op, knew });
    };
    let create = Value::obj(vec![(FIELD, Value::str(""))]);
    write(&mut devices[0], "create", create, actors[0], &mut writes);
    let first = writes[0].op.clone();
    for d in &mut devices[1..] {
        d.receive(&first);
    }
    for _ in 0..4 + rng.below(11) {
        if rng.below(10) < 6 {
            let d = rng.below(n_dev);
            let actor = actors[rng.below(n_actors)];
            let mut text = format!("v{}", writes.len());
            // Now and then a set repeats an earlier set's value: two open
            // versions with one value are not a conflict.
            let earlier: Vec<String> = writes.iter().filter(|w| w.op.verb != "create").filter_map(|w| w.op.value.get("resolved").unwrap_or(&w.op.value).as_str().map(str::to_string)).collect();
            if !earlier.is_empty() && rng.below(6) == 0 {
                text = earlier[rng.below(earlier.len())].clone();
            }
            if rng.below(2) == 0 {
                write(&mut devices[d], "set", Value::str(&text), actor, &mut writes);
            } else if mode != Mode::Plain && rng.below(3) == 0 {
                // A text-edit from a replica without a merger: resolved text
                // only, so it takes the degradation path (LWW) everywhere.
                let value = Value::obj(vec![("resolved", Value::str(&text))]);
                write(&mut devices[d], "text-edit", value, actor, &mut writes);
            } else {
                let dev = &devices[d];
                let updates = dev.proj.text_updates(TARGET, FIELD).unwrap();
                let value = match mode.merger().map(|m| m.edit(&updates, None, &text, 0)) {
                    Some(Ok((update, resolved))) => {
                        Value::obj(vec![("loro", Value::str(&base64::encode(&update))), ("resolved", Value::str(&resolved))])
                    }
                    // No merger, or one that cannot merge what this device
                    // holds: the writer degrades, as `Txn::text_edit` does.
                    _ => Value::obj(vec![("resolved", Value::str(&text))]),
                };
                write(&mut devices[d], "text-edit", value, actor, &mut writes);
            }
        } else {
            // Pull part of one device's log from a peer that holds it, in
            // that log's order, as sync does.
            let (from, to) = (rng.below(n_dev), rng.below(n_dev));
            if from == to {
                continue;
            }
            let author = format!("dvc_{}", rng.below(n_dev));
            let missing: Vec<Op> = devices[from]
                .held
                .iter()
                .filter(|o| o.device == author && !devices[to].has(&o.id))
                .cloned()
                .collect();
            if missing.is_empty() {
                continue;
            }
            for op in &missing[..1 + rng.below(missing.len())] {
                devices[to].receive(op);
            }
        }
    }
    History { writes, devices }
}

/// Causal ancestors of each write: what its writer held, and everything
/// those writes' writers held, transitively.
fn ancestors(writes: &[Write]) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for w in writes {
        let mut all = BTreeSet::new();
        for k in &w.knew {
            all.insert(k.clone());
            all.extend(out[k].iter().cloned());
        }
        out.insert(w.op.id.clone(), all);
    }
    out
}

/// What the projection must show for a set of writes that holds every
/// ancestor of each of its writes.
fn oracle(writes: &[&Write], anc: &BTreeMap<String, BTreeSet<String>>, mode: Mode) -> (Vec<String>, Vec<(String, String)>) {
    let maximal: Vec<&Op> = writes
        .iter()
        .map(|w| &w.op)
        .filter(|op| !writes.iter().any(|w| anc[&w.op.id].contains(&op.id)))
        .collect();
    let winner = writes.iter().map(|w| &w.op).max_by(|a, b| a.hlc.cmp(&b.hlc)).unwrap();
    let ops: Vec<&Op> = writes.iter().map(|w| &w.op).collect();
    let (_, merges) = loro_set(&ops, mode);
    let text = |op: &Op| merges && op.verb == "text-edit" && op.value.get_str("loro").is_some();
    let own = |op: &Op| op.value.get("resolved").unwrap_or(&op.value).to_json();
    let mut heads: Vec<String> = maximal.iter().map(|o| o.hlc.clone()).collect();
    heads.sort();
    let mut conflicts: Vec<(String, String)> = maximal
        .iter()
        .filter(|o| o.id != winner.id && !(text(o) && text(winner)) && own(o) != own(winner))
        .map(|o| (winner.id.clone(), o.id.clone()))
        .collect();
    conflicts.sort_by(|a, b| a.1.cmp(&b.1));
    (heads, conflicts)
}

/// A random interleaving of `ops` that keeps each device's own order.
fn shuffle_by_device(ops: &[Op], rng: &mut Rng) -> Vec<Op> {
    let mut queues: BTreeMap<&str, Vec<&Op>> = BTreeMap::new();
    for op in ops {
        queues.entry(op.device.as_str()).or_default().push(op);
    }
    for q in queues.values_mut() {
        q.sort_by(|a, b| a.hlc.cmp(&b.hlc));
        q.reverse();
    }
    let mut out = Vec::new();
    while !queues.is_empty() {
        let keys: Vec<&str> = queues.keys().copied().collect();
        let k = keys[rng.below(keys.len())];
        let q = queues.get_mut(k).unwrap();
        out.push(q.pop().unwrap().clone());
        if q.is_empty() {
            queues.remove(k);
        }
    }
    out
}

fn run(mode: Mode, seed: u64) {
    let cases: u64 = std::env::var("WUGS_CAUSAL_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
    let mut conflicted = 0;
    let mut unmergeable = 0;
    for case in 0..cases {
        let mut rng = Rng(seed ^ (case + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let h = generate(&mut rng, mode);
        let anc = ancestors(&h.writes);
        let ctx = |what: &str| format!("case {case} ({mode:?}): {what}");

        for (i, d) in h.devices.iter().enumerate() {
            let got = state(&d.proj);
            assert_eq!(got, replay(&d.held, mode), "{}", ctx(&format!("device {i} incremental vs replay")));
            let mine: Vec<&Write> = h.writes.iter().filter(|w| d.has(&w.op.id)).collect();
            let closed = mine.iter().all(|w| anc[&w.op.id].iter().all(|a| d.has(a)));
            if closed {
                let (heads, conflicts) = oracle(&mine, &anc, mode);
                assert_eq!((&got.heads, &got.conflicts), (&heads, &conflicts), "{}", ctx(&format!("device {i} vs oracle")));
            }
        }

        let all: Vec<Op> = h.writes.iter().map(|w| w.op.clone()).collect();
        let want = replay(&all, mode);
        let every: Vec<&Write> = h.writes.iter().collect();
        let (heads, conflicts) = oracle(&every, &anc, mode);
        assert_eq!((&want.heads, &want.conflicts), (&heads, &conflicts), "{}", ctx("full history vs oracle"));
        let winner = &h.writes.iter().max_by(|a, b| a.op.hlc.cmp(&b.op.hlc)).unwrap().op;
        let (updates, merges) = loro_set(&all.iter().collect::<Vec<_>>(), mode);
        if merges && winner.verb == "text-edit" && winner.value.get_str("loro").is_some() {
            let merged_text = mode.merger().unwrap().resolve(&updates).unwrap();
            assert_eq!(want.value, Some(Value::str(&merged_text).to_json()), "{}", ctx("merged text"));
        } else if winner.verb != "create" {
            // ...or any write when the updates do not merge as a set.
            // A set, or a text-edit on the degradation path: its own value.
            let own = winner.value.get("resolved").unwrap_or(&winner.value);
            assert_eq!(want.value, Some(own.to_json()), "{}", ctx("winner's own value"));
        }
        for _ in 0..4 {
            let p = projection(mode);
            for op in shuffle_by_device(&all, &mut rng) {
                p.apply(&op, &mut ApplyStats::default()).unwrap();
            }
            assert_eq!(state(&p), want, "{}", ctx("arrival order"));
        }
        if !want.conflicts.is_empty() {
            conflicted += 1;
        }
        if mode == Mode::Fragile && !merges {
            unmergeable += 1;
        }
    }
    assert!(conflicted * 5 > cases, "the generator must produce conflicts often enough to test them ({conflicted}/{cases})");
    if mode == Mode::Fragile {
        assert!(unmergeable * 10 > cases && unmergeable < cases, "both merging and refusing sets must occur ({unmergeable}/{cases})");
    }
}

#[test]
fn conflicts_match_causality_in_every_arrival_order_with_text_merging() {
    run(Mode::Merged, 0x5EED_0001);
}

#[test]
fn conflicts_match_causality_in_every_arrival_order_without_a_merger() {
    run(Mode::Plain, 0x5EED_0002);
}

#[test]
fn conflicts_and_values_match_in_every_arrival_order_when_updates_stop_merging() {
    run(Mode::Fragile, 0x5EED_0003);
}
