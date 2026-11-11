//! The request/response machine itself: walk-down narrowing, held-op root
//! convergence, per-message ACL, and protocol refusals.

use std::fs;
use std::path::PathBuf;

use wugs_core::Project;
use wugs_model::{Op, Value};
use wugs_sync::{pull, summary, sync_pair, Requester, Responder, SyncPolicy};

struct Rig {
    roots: Vec<PathBuf>,
}

impl Rig {
    fn new() -> Rig {
        Rig { roots: Vec::new() }
    }
    fn root(&mut self) -> PathBuf {
        let root = std::env::temp_dir().join(format!("wugs-wire-test-{}", wugs_model::ulid()));
        self.roots.push(root.clone());
        root
    }
    fn project(&mut self, actor: &str) -> Project {
        Project::open(self.root(), actor).unwrap()
    }
    fn joined(&mut self, actor: &str, of: &Project) -> Project {
        Project::open_joined(self.root(), actor, &of.project_id).unwrap()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        for r in &self.roots {
            fs::remove_dir_all(r).ok();
        }
    }
}

fn all() -> SyncPolicy {
    SyncPolicy::allow_all()
}


#[test]
fn held_ops_converge_roots_and_are_never_resent() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let txt = a.create_text("T").unwrap();
    a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
    b.ingest(a.log.read_all().unwrap().ops).unwrap();

    b.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "overlap").unwrap();
    let r = sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    assert_eq!(r.b_to_a.ingest.txns_held, 1, "the overlap is held on A");

    assert_eq!(
        summary(&a.proj, &txt).unwrap(),
        summary(&b.proj, &txt).unwrap(),
        "roots converge while the txn sits in quarantine"
    );
    let r = sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    assert_eq!((r.a_to_b.ops, r.b_to_a.ops), (0, 0), "a re-sync moves nothing at all");
    assert_eq!(r.b_to_a.windows, 0, "not even window hashes are requested");
}

#[test]
fn held_txn_heals_over_sync_when_the_fix_arrives() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let txt = a.create_text("T").unwrap();
    a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
    b.ingest(a.log.read_all().unwrap().ops).unwrap();

    let bad = b.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "late").unwrap().0;
    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    assert_eq!(a.proj.count("quarantine").unwrap(), 1);

    let mut t = b.intent("retime-segment");
    t.set("segment", &bad, &txt, "start", Value::Num(1000.0)).unwrap();
    t.set("segment", &bad, &txt, "end", Value::Num(1500.0)).unwrap();
    t.commit().unwrap();

    let r = sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    assert_eq!(r.b_to_a.ingest.txns_retried, 1, "the held txn applied once its fix arrived");
    assert_eq!(a.proj.count("quarantine").unwrap(), 0);
    assert_eq!(a.proj.field_num(&bad, "start").unwrap(), Some(1000.0));
    assert_eq!(
        summary(&a.proj, &txt).unwrap(),
        summary(&b.proj, &txt).unwrap(),
        "fully converged after healing"
    );
}

/// Ops six hours apart land in different walk-down groups, so a divergence
/// in recent work must not fetch window hashes for the old group.
#[test]
fn walk_down_requests_only_the_differing_group() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");

    // Two bursts of history, >4.7h apart, forged via raw ops with spread hlcs.
    let scope = "txt_01WIREWALKDOWNSCOPE0000000";
    let base = 1_700_000_000_000u64;
    let mut seed = Vec::new();
    for (burst, at) in [(0u64, base), (1, base + 6 * 3600 * 1000)] {
        for i in 0..20u64 {
            let id = wugs_model::new_id("seg");
            seed.push(Op {
                id: wugs_model::new_id("op"),
                hlc: format!("{:012x}-{:04x}-dvc_01WIREFORGEDDEVICE00000000", at + i * 70_000, 0),
                txn: wugs_model::new_id("txn"),
                intent: None,
                actor: "prs_A".into(),
                device: "dvc_01WIREFORGEDDEVICE00000000".into(),
                session: None,
                scope: scope.into(),
                schema: 1,
                verb: "create".into(),
                entity: "segment".into(),
                target: id,
                field: None,
                value: Value::obj(vec![("content", Value::str(&format!("b{burst} s{i}")))]),
                base: None,
                heads: None,
            });
        }
    }
    a.ingest(seed.clone()).unwrap();
    let mut b = rig.joined("prs_B", &a);
    b.ingest(seed).unwrap();

    let groups = summary(&a.proj, scope).unwrap().groups();
    assert!(groups.len() >= 2, "the bursts span {} group(s)", groups.len());

    // diverge only the recent burst
    let extra = {
        let mut op = a.log.read_all().unwrap().ops.pop().unwrap();
        op.id = wugs_model::new_id("op");
        op.txn = wugs_model::new_id("txn");
        op.target = wugs_model::new_id("seg");
        op.hlc = format!("{:012x}-{:04x}-dvc_01WIREFORGEDDEVICE00000001", base + 6 * 3600 * 1000 + 30 * 70_000, 0);
        op.device = "dvc_01WIREFORGEDDEVICE00000001".into();
        op
    };
    a.ingest(vec![extra]).unwrap();

    // drive the pull by hand and capture the windows request
    let permissive = all();
    let mut responder = Responder::new(&a, &permissive);
    let mut req = Requester::new(&b.project_id);
    let mut asked_groups: Option<usize> = None;
    while let Some(msg) = req.request() {
        if msg.get_str("t") == Some("get_windows") {
            if let Some(Value::Obj(scopes)) = msg.get("scopes") {
                asked_groups = Some(
                    scopes
                        .values()
                        .map(|gs| if let Value::Arr(a) = gs { a.len() } else { 0 })
                        .sum(),
                );
            }
        }
        let resp = responder.handle(&msg).unwrap();
        req.respond(&mut b, &resp).unwrap();
    }
    assert_eq!(asked_groups, Some(1), "only the diverged group's windows were requested");
    assert_eq!(req.ops_received, 1, "exactly the one new op crossed");
    assert_eq!(
        summary(&a.proj, scope).unwrap(),
        summary(&b.proj, scope).unwrap(),
        "converged"
    );
}

#[test]
fn every_message_applies_the_sender_policy() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let txt = a.create_text("Restricted").unwrap();
    a.add_segment(&txt, "tir_utt", 0, None, None, "secret").unwrap();
    let policy = SyncPolicy::deny([txt.clone()]);
    let mut responder = Responder::new(&a, &policy);
    let hello = Value::obj(vec![
        ("t", Value::str("hello")),
        ("v", Value::Num(1.0)),
        ("project", Value::str(&a.project_id)),
    ]);
    responder.handle(&hello).unwrap();

    let ask = |t: &str, scopes: Value| {
        Value::obj(vec![("t", Value::str(t)), ("v", Value::Num(1.0)), ("scopes", scopes)])
    };
    let roots = responder.handle(&Value::obj(vec![("t", Value::str("get_roots")), ("v", Value::Num(1.0))])).unwrap();
    assert!(!roots.to_json().contains(&txt), "denied scope absent from roots");

    // ask for the denied scope explicitly at every level
    let g = responder.handle(&ask("get_groups", Value::Arr(vec![Value::str(&txt)]))).unwrap();
    assert!(!g.to_json().contains(&txt), "groups reveal nothing: {}", g.to_json());
    let w = responder
        .handle(&ask("get_windows", Value::obj(vec![(txt.as_str(), Value::Arr(vec![Value::str("0")]))])))
        .unwrap();
    assert!(!w.to_json().contains(&txt), "windows reveal nothing");
    let o = responder
        .handle(&ask("get_ops", Value::obj(vec![(txt.as_str(), Value::Arr(vec![Value::str("0")]))])))
        .unwrap();
    assert!(!o.to_json().contains("secret"), "ops reveal nothing");
}

#[test]
fn protocol_refusals() {
    let mut rig = Rig::new();
    let a = rig.project("prs_A");
    let mut stranger = rig.project("prs_X");
    assert!(
        pull(&mut stranger, &a, &all()).is_err(),
        "two different projects refuse to pair"
    );

    let permissive = all();
    let mut responder = Responder::new(&a, &permissive);
    assert!(
        responder
            .handle(&Value::obj(vec![("t", Value::str("get_roots")), ("v", Value::Num(1.0))]))
            .is_err(),
        "no data before a hello naming the project"
    );
    let hello = Value::obj(vec![
        ("t", Value::str("hello")),
        ("v", Value::Num(2.0)),
        ("project", Value::str(&a.project_id)),
    ]);
    assert!(responder.handle(&hello).is_err(), "wrong protocol version refused");
    assert!(responder.handle(&Value::obj(vec![("t", Value::str("gimme"))])).is_err());
    assert!(responder.handle(&Value::obj(vec![("x", Value::Num(1.0))])).is_err());
}

#[test]
fn hostile_group_numbers_are_data_not_a_crash() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let txt = a.create_text("T").unwrap();
    a.add_segment(&txt, "tir_utt", 0, None, None, "x").unwrap();
    let permissive = all();
    let mut responder = Responder::new(&a, &permissive);
    responder
        .handle(&Value::obj(vec![
            ("t", Value::str("hello")),
            ("v", Value::Num(1.0)),
            ("project", Value::str(&a.project_id)),
        ]))
        .unwrap();
    for g in ["72057594037927935", "18446744073709551615", "0"] {
        let msg = Value::obj(vec![
            ("t", Value::str("get_windows")),
            ("v", Value::Num(1.0)),
            ("scopes", Value::obj(vec![(txt.as_str(), Value::Arr(vec![Value::str(g)]))])),
        ]);
        assert!(responder.handle(&msg).is_ok(), "group {g} must not panic or error");
    }
}

#[test]
fn large_wants_are_fetched_in_bounded_batches() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let scope = "txt_01WIRECHUNKSCOPE0000000000";
    let base = 1_700_000_000_000u64;
    let mut seed = Vec::new();
    for i in 0..40u64 {
        seed.push(Op {
            id: wugs_model::new_id("op"),
            // 70s apart: every op its own window, so 40 windows > one chunk
            hlc: format!("{:012x}-{:04x}-dvc_01WIREFORGEDDEVICE00000000", base + i * 70_000, 0),
            txn: wugs_model::new_id("txn"),
            intent: None,
            actor: "prs_A".into(),
            device: "dvc_01WIREFORGEDDEVICE00000000".into(),
            session: None,
            scope: scope.into(),
            schema: 1,
            verb: "create".into(),
            entity: "segment".into(),
            target: wugs_model::new_id("seg"),
            field: None,
            value: Value::obj(vec![("content", Value::str(&format!("s{i}")))]),
            base: None,
            heads: None,
        });
    }
    a.ingest(seed).unwrap();
    let mut b = rig.joined("prs_B", &a);

    let permissive = all();
    let mut responder = Responder::new(&a, &permissive);
    let mut req = Requester::new(&b.project_id);
    let mut ops_requests = 0;
    while let Some(msg) = req.request() {
        if msg.get_str("t") == Some("get_ops") {
            ops_requests += 1;
        }
        let resp = responder.handle(&msg).unwrap();
        req.respond(&mut b, &resp).unwrap();
    }
    assert!(ops_requests >= 3, "40 windows travel in several bounded batches, got {ops_requests}");
    assert_eq!(req.ops_received, 40, "all ops arrive across the batches");
    assert_eq!(
        summary(&a.proj, scope).unwrap(),
        summary(&b.proj, scope).unwrap(),
        "converged"
    );
}

#[test]
fn a_parked_follow_up_is_not_relogged_or_resent() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let txt = a.create_text("T").unwrap();
    a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
    b.ingest(a.log.read_all().unwrap().ops).unwrap();

    // B's overlapping segment is held on A; B then edits the held segment,
    // so the follow-up parks on A (its target does not exist there yet)
    let bad = b.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "late").unwrap().0;
    let mut t = b.intent("note");
    t.set("segment", &bad, &txt, "x.note", Value::str("check this")).unwrap();
    t.commit().unwrap();

    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    let log_after_first = a.log.read_all().unwrap().ops.len();
    assert_eq!(
        summary(&a.proj, &txt).unwrap(),
        summary(&b.proj, &txt).unwrap(),
        "roots converge with one txn held and one op parked"
    );

    for _ in 0..3 {
        let r = sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
        assert_eq!((r.a_to_b.ops, r.b_to_a.ops), (0, 0), "nothing is re-sent");
    }
    assert_eq!(
        a.log.read_all().unwrap().ops.len(),
        log_after_first,
        "the parked follow-up is logged exactly once, not once per sync"
    );
}

#[test]
fn a_third_device_is_served_held_ops() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let txt = a.create_text("T").unwrap();
    a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
    b.ingest(a.log.read_all().unwrap().ops).unwrap();
    b.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "late").unwrap();
    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    assert_eq!(a.proj.count("quarantine").unwrap(), 1, "A holds B's txn");

    // C only ever meets A; B's laptop is gone
    let mut c = rig.joined("prs_C", &a);
    sync_pair(&mut a, &mut c, &all(), &all()).unwrap();
    assert_eq!(
        summary(&a.proj, &txt).unwrap(),
        summary(&c.proj, &txt).unwrap(),
        "C converges with A on the first session, held ops included"
    );
    assert_eq!(c.proj.count("quarantine").unwrap(), 1, "C holds the same txn");

    let r = sync_pair(&mut a, &mut c, &all(), &all()).unwrap();
    assert_eq!((r.a_to_b.ops, r.b_to_a.ops), (0, 0), "and nothing loops afterwards");
}

#[test]
fn a_local_fix_releases_a_held_txn_without_sync() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let txt = a.create_text("T").unwrap();
    let mine = a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap().0;
    b.ingest(a.log.read_all().unwrap().ops).unwrap();
    let theirs = b.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "late").unwrap().0;
    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    assert_eq!(a.proj.count("quarantine").unwrap(), 1);

    // A resolves the conflict by shortening its own segment
    let mut t = a.intent("retime-segment");
    t.set("segment", &mine, &txt, "end", Value::Num(500.0)).unwrap();
    t.commit().unwrap();

    assert_eq!(a.proj.count("quarantine").unwrap(), 0, "the held txn applied on the local commit");
    assert!(a.proj.entity_kind(&theirs).unwrap().is_some(), "B's segment is visible on A");
}

#[test]
fn deleting_one_of_two_overlapping_segments_syncs_without_a_hold() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let txt = a.create_text("T").unwrap();
    let mine = a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap().0;
    b.ingest(a.log.read_all().unwrap().ops).unwrap();
    let theirs = b.add_segment(&txt, "tir_utt", 1, Some(500.0), Some(1500.0), "b").unwrap().0;
    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();

    // A deletes its own segment to fix the overlap
    a.delete_segment(&txt, &mine).unwrap();
    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();

    assert_eq!(b.proj.count("quarantine").unwrap(), 0, "the delete is not held on B");
    assert!(b.proj.is_tombstoned(&mine).unwrap(), "deleted on B");
    assert!(a.proj.is_tombstoned(&mine).unwrap(), "deleted on A");
    assert!(!b.proj.is_tombstoned(&theirs).unwrap());
}

#[test]
fn moving_different_edges_on_two_devices_keeps_both() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap().0;
    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();

    b.set_segment_times(&txt, &seg, Some(100.0), None).unwrap();
    a.set_segment_times(&txt, &seg, None, Some(900.0)).unwrap();
    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();

    for (name, p) in [("A", &a), ("B", &b)] {
        assert_eq!(p.proj.field_num(&seg, "start").unwrap(), Some(100.0), "start on {name}");
        assert_eq!(p.proj.field_num(&seg, "end").unwrap(), Some(900.0), "end on {name}");
        assert_eq!(p.proj.count("conflicts").unwrap(), 0, "no conflict rows on {name}");
    }
}

#[test]
fn access_levels_drive_the_default_policy() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);

    let open_txt = a.create_text("Open story").unwrap();
    a.add_segment(&open_txt, "tir_utt", 0, None, None, "herkese açık").unwrap();
    let secret_txt = a.create_text("Restricted story").unwrap();
    a.add_segment(&secret_txt, "tir_utt", 0, None, None, "kutsal bilgi").unwrap();
    a.set_access(&secret_txt, "restricted").unwrap();
    let names = Value::obj(vec![("full", Value::str("Ayşe Yılmaz"))]);
    a.add_person(names, Value::Arr(vec![Value::str("speaker")]), "restricted").unwrap();

    let policy = wugs_sync::policy_from_access(&a.proj, &[]).unwrap();
    sync_pair(&mut a, &mut b, &policy, &all()).unwrap();

    assert!(b.proj.entity_kind(&open_txt).unwrap().is_some(), "open text crossed");
    assert!(b.proj.entity_kind(&secret_txt).unwrap().is_none(), "restricted text withheld");
    assert_eq!(b.proj.entities_of_kind("person").unwrap().len(), 0, "people scope withheld");
    let b_log = b.log.read_all().unwrap().ops;
    assert!(
        b_log.iter().all(|o| o.scope != secret_txt && o.scope != "people"),
        "no restricted op even reached B's log"
    );

    // explicit consent to share with this peer: the one scope opens up
    let policy = wugs_sync::policy_from_access(&a.proj, &[&secret_txt]).unwrap();
    sync_pair(&mut a, &mut b, &policy, &all()).unwrap();
    assert!(b.proj.entity_kind(&secret_txt).unwrap().is_some(), "explicitly shared scope crossed");
    assert_eq!(b.proj.entities_of_kind("person").unwrap().len(), 0, "people still withheld");
}

#[test]
fn consent_and_sessions_live_in_the_people_scope() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let src_txt = a.create_text("Recording").unwrap();
    let names = Value::obj(vec![("full", Value::str("Ali Demir"))]);
    let person = a.add_person(names, Value::Arr(vec![Value::str("speaker")]), "open").unwrap();
    let terms = Value::obj(vec![("archive-ok", Value::Bool(true)), ("AI-ok", Value::Bool(false))]);
    let consent = a.record_consent(&person, &src_txt, terms, "2026-11-07").unwrap();
    let ses = a
        .start_session("2026-11-07", "Ankara", "elicitation", Value::Arr(vec![Value::str(&person)]))
        .unwrap();

    for id in [&person, &consent, &ses] {
        let ops: Vec<_> = a
            .log
            .read_all()
            .unwrap()
            .ops
            .into_iter()
            .filter(|o| o.target == *id)
            .collect();
        assert!(!ops.is_empty());
        assert!(ops.iter().all(|o| o.scope == "people"), "{id} ops in the people scope");
    }

    assert!(a.record_consent("prs_NOPE000000000000000000000", &src_txt, Value::Null, "x").is_err());
    assert!(a.record_consent(&person, "txt_NOPE000000000000000000000", Value::Null, "x").is_err());
    assert!(a.set_access(&person, "restricted").is_err(), "access lives on sources and texts");
    assert!(a.set_access(&src_txt, "sacred").is_err(), "unknown level refused");
}

#[test]
fn deleting_a_restricted_text_does_not_open_it() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let secret = a.create_text("Restricted").unwrap();
    a.add_segment(&secret, "tir_utt", 0, None, None, "kutsal bilgi").unwrap();
    a.set_access(&secret, "restricted").unwrap();
    let mut t = a.intent("delete-text");
    t.delete("text", &secret, &secret);
    t.commit().unwrap();

    let policy = wugs_sync::policy_from_access(&a.proj, &[]).unwrap();
    sync_pair(&mut a, &mut b, &policy, &all()).unwrap();
    assert!(
        b.log.read_all().unwrap().ops.iter().all(|o| o.scope != secret),
        "a deleted restricted text's ops must never leave the device"
    );
}

#[test]
fn unknown_scopes_fail_closed() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    // ops in a scope with no live source/text root: a held create's
    // follow-ups, an import artifact, or anything a peer could name
    let ghost = "txt_01GHOSTSCOPE00000000000000";
    let op = Op {
        id: wugs_model::new_id("op"),
        hlc: format!("{:012x}-{:04x}-dvc_01GHOSTDEVICE0000000000000", 1_700_000_000_000u64, 0),
        txn: wugs_model::new_id("txn"),
        intent: None,
        actor: "prs_A".into(),
        device: "dvc_01GHOSTDEVICE0000000000000".into(),
        session: None,
        scope: ghost.into(),
        schema: 1,
        verb: "create".into(),
        entity: "segment".into(),
        target: wugs_model::new_id("seg"),
        field: None,
        value: Value::obj(vec![("content", Value::str("gizli"))]),
        base: None,
        heads: None,
    };
    a.ingest(vec![op]).unwrap();
    let policy = wugs_sync::policy_from_access(&a.proj, &[]).unwrap();
    assert!(!policy.permits(ghost), "a scope without a live open root is withheld");
    assert!(policy.permits("lexicon") && policy.permits("project"), "shared scopes stay shared");
}

#[test]
fn a_restriction_travels_to_peers_that_already_have_the_text() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let mut c = rig.joined("prs_C", &a);

    let txt = a.create_text("Was open").unwrap();
    a.add_segment(&txt, "tir_utt", 0, None, None, "önce açık").unwrap();
    let policy = wugs_sync::policy_from_access(&a.proj, &[]).unwrap();
    sync_pair(&mut a, &mut b, &policy, &all()).unwrap();
    assert!(b.proj.entity_kind(&txt).unwrap().is_some(), "B got it while open");

    a.set_access(&txt, "restricted").unwrap();
    let policy = wugs_sync::policy_from_access(&a.proj, &[]).unwrap();
    sync_pair(&mut a, &mut b, &policy, &all()).unwrap();
    assert_eq!(
        b.proj.field_str(&txt, "access").unwrap().as_deref(),
        Some("restricted"),
        "the restriction reached B through the config scope"
    );

    // and B now withholds it from C
    let b_policy = wugs_sync::policy_from_access(&b.proj, &[]).unwrap();
    sync_pair(&mut b, &mut c, &b_policy, &all()).unwrap();
    assert!(
        c.log.read_all().unwrap().ops.iter().all(|o| o.scope != txt),
        "B does not re-share a text it now knows is restricted"
    );
}

#[test]
fn a_text_inherits_its_sources_access() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    // a source entity and a text linked to it, via raw intents (no source
    // facade yet)
    let src = {
        let mut t = a.intent("add-source");
        let id = t.create_scope_root("source", "src", vec![("kind", Value::str("audio"))]);
        t.commit().unwrap();
        id
    };
    let txt = a.create_text("From the recording").unwrap();
    let mut t = a.intent("link-source");
    t.set("text", &txt, &txt, "source", Value::str(&src)).unwrap();
    t.commit().unwrap();
    a.set_access(&src, "community").unwrap();

    let policy = wugs_sync::policy_from_access(&a.proj, &[]).unwrap();
    assert!(!policy.permits(&txt), "a text with no access of its own inherits its source's");
    assert!(!policy.permits(&src));
}

/// Two synced projects and one fresh op on `a`, so a pull from `b` asks for
/// exactly one scope, group and window. The tamper closure edits each
/// response before `b` sees it; the pull's result is returned.
fn tampered_pull(tamper: impl Fn(&mut Value)) -> Result<(), String> {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let scope = "txt_01WIRETAMPERSCOPE00000000";
    let base = 1_700_000_000_000u64;
    let seed: Vec<Op> = (0..5u64)
        .map(|i| Op {
            id: wugs_model::new_id("op"),
            hlc: format!("{:012x}-{:04x}-dvc_01WIREFORGEDDEVICE00000000", base + i * 70_000, 0),
            txn: wugs_model::new_id("txn"),
            intent: None,
            actor: "prs_A".into(),
            device: "dvc_01WIREFORGEDDEVICE00000000".into(),
            session: None,
            scope: scope.into(),
            schema: 1,
            verb: "create".into(),
            entity: "segment".into(),
            target: wugs_model::new_id("seg"),
            field: None,
            value: Value::obj(vec![("content", Value::str(&format!("s{i}")))]),
            base: None,
            heads: None,
        })
        .collect();
    a.ingest(seed.clone()).unwrap();
    let mut b = rig.joined("prs_B", &a);
    b.ingest(seed).unwrap();
    let extra = {
        let mut op = a.log.read_all().unwrap().ops.pop().unwrap();
        op.id = wugs_model::new_id("op");
        op.txn = wugs_model::new_id("txn");
        op.target = wugs_model::new_id("seg");
        op
    };
    a.ingest(vec![extra]).unwrap();

    let permissive = all();
    let mut responder = Responder::new(&a, &permissive);
    let mut req = Requester::new(&b.project_id);
    while let Some(msg) = req.request() {
        let mut resp = responder.handle(&msg).unwrap();
        tamper(&mut resp);
        req.respond(&mut b, &resp)?;
    }
    Ok(())
}

fn field_mut<'a>(v: &'a mut Value, key: &str) -> Option<&'a mut Value> {
    if let Value::Obj(m) = v { m.get_mut(key) } else { None }
}

fn rename_scope_keys(resp: &mut Value, kind: &str, to: &str) {
    if resp.get_str("t") != Some(kind) {
        return;
    }
    if let Some(Value::Obj(scopes)) = field_mut(resp, "scopes") {
        let renamed: Vec<(String, Value)> = scopes.iter().map(|(_, v)| (to.to_string(), v.clone())).collect();
        scopes.clear();
        scopes.extend(renamed);
    }
}

#[test]
fn groups_for_an_unasked_scope_are_refused() {
    let err = tampered_pull(|resp| rename_scope_keys(resp, "groups", "txt_01WIRESOMEOTHERSCOPE00000")).unwrap_err();
    assert!(err.contains("unasked scope"), "{err}");
}

#[test]
fn windows_for_an_unasked_scope_are_refused() {
    let err = tampered_pull(|resp| rename_scope_keys(resp, "windows", "txt_01WIRESOMEOTHERSCOPE00000")).unwrap_err();
    assert!(err.contains("unasked scope"), "{err}");
}

#[test]
fn an_op_of_an_unasked_scope_is_refused() {
    let err = tampered_pull(|resp| {
        if resp.get_str("t") != Some("ops") {
            return;
        }
        if let Some(Value::Arr(items)) = field_mut(resp, "ops") {
            for item in items {
                if let Value::Obj(op) = item {
                    op.insert("scope".into(), Value::str("txt_01WIRESOMEOTHERSCOPE00000"));
                }
            }
        }
    })
    .unwrap_err();
    assert!(err.contains("an op of scope"), "{err}");
}

#[test]
fn a_txn_with_no_op_in_an_asked_window_is_refused() {
    // Every op's hlc moves 256 windows ahead, so no txn anchors in an
    // asked window; the batch must be refused whole.
    let err = tampered_pull(|resp| {
        if resp.get_str("t") != Some("ops") {
            return;
        }
        if let Some(Value::Arr(items)) = field_mut(resp, "ops") {
            for item in items {
                if let Value::Obj(op) = item {
                    let hlc = op.get("hlc").and_then(|v| if let Value::Str(s) = v { Some(s.clone()) } else { None }).unwrap();
                    let (millis, rest) = hlc.split_once('-').unwrap();
                    let moved = u64::from_str_radix(millis, 16).unwrap() + (256u64 << 16);
                    op.insert("hlc".into(), Value::str(&format!("{moved:012x}-{rest}")));
                }
            }
        }
    })
    .unwrap_err();
    assert!(err.contains("no op in an asked window"), "{err}");
}

#[test]
fn an_untampered_pull_still_converges() {
    tampered_pull(|_| {}).unwrap();
}
