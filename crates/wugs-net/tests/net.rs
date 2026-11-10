//! Real sockets on 127.0.0.1. `Project` is not `Send`, so the server project
//! lives on the test thread and each client builds its own project inside
//! its thread; only plain data (ids, summaries, reports) crosses threads.

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use wugs_core::{Project, SCOPE_LEXICON};
use wugs_model::Value;
use wugs_net::frame::{read_frame, write_frame, FrameError, MAX_FRAME};
use wugs_net::{discover, serve_open, serve_open_with, sync_open, SessionOutcome};
use wugs_sync::{scopes, summary, ScopeSummary, SyncPolicy};

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> TempRoot {
        TempRoot(std::env::temp_dir().join(format!("wugs-net-test-{}", wugs_model::ulid())))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn listen() -> (TcpListener, SocketAddr) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    (l, addr)
}

fn summaries(p: &Project) -> Vec<ScopeSummary> {
    scopes(&p.proj).unwrap().iter().map(|s| summary(&p.proj, s).unwrap()).collect()
}

fn all() -> SyncPolicy {
    SyncPolicy::allow_all()
}

fn converged(o: &SessionOutcome) -> &wugs_sync::Transfer {
    match o {
        SessionOutcome::Converged(t) => t,
        other => panic!("session did not converge: {other:?}"),
    }
}

#[test]
fn one_session_converges_both_directions() {
    let root_a = TempRoot::new();
    let mut a = Project::open(&root_a.0, "prs_A").unwrap();
    let txt = a.create_text("Story").unwrap();
    a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "köpek").unwrap();
    let (listener, addr) = listen();
    let pid = a.project_id.clone();

    let (tx, rx) = mpsc::channel();
    let client = thread::spawn(move || {
        let root_b = TempRoot::new();
        let mut b = Project::open_joined(&root_b.0, "prs_B", &pid).unwrap();
        b.create_lexeme("su", "water").unwrap();
        let r = sync_open(&mut b, addr, &all()).unwrap();
        tx.send((r.pulled.ops, summaries(&b))).unwrap();
    });

    let report = serve_open(&mut a, listener, &all(), 1).unwrap();
    client.join().unwrap();
    let (b_pulled, b_summaries) = rx.recv().unwrap();

    let session = &report.sessions[0];
    assert!(session.answered > 0, "the server answered the client's pull");
    assert!(b_pulled > 0, "B pulled A's text");
    assert!(converged(&session.outcome).ops > 0, "A pulled B's lexeme in the same session");
    assert!(
        summaries(&a).iter().any(|s| s.scope == SCOPE_LEXICON),
        "A now has the lexicon scope"
    );
    assert_eq!(summaries(&a), b_summaries, "every scope's summary matches");
}

#[test]
fn held_txn_over_the_wire_converges_then_heals() {
    let root_a = TempRoot::new();
    let mut a = Project::open(&root_a.0, "prs_A").unwrap();
    let txt = a.create_text("T").unwrap();
    a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
    let (listener, addr) = listen();
    let (pid, scope) = (a.project_id.clone(), txt.clone());

    let (tx, rx) = mpsc::channel();
    let client = thread::spawn(move || {
        let root_b = TempRoot::new();
        let mut b = Project::open_joined(&root_b.0, "prs_B", &pid).unwrap();
        sync_open(&mut b, addr, &all()).unwrap();

        let bad = b.add_segment(&scope, "tir_utt", 1, Some(500.0), Some(1500.0), "late").unwrap().0;
        sync_open(&mut b, addr, &all()).unwrap();

        let quiet = sync_open(&mut b, addr, &all()).unwrap();
        tx.send(("quiet pull", quiet.pulled.ops, quiet.answered)).unwrap();

        let mut t = b.intent("retime-segment");
        t.set("segment", &bad, &scope, "start", Value::Num(1000.0)).unwrap();
        t.set("segment", &bad, &scope, "end", Value::Num(1500.0)).unwrap();
        t.commit().unwrap();
        sync_open(&mut b, addr, &all()).unwrap();
        (bad, summary(&b.proj, &scope).unwrap())
    });

    let seed = serve_open(&mut a, listener.try_clone().unwrap(), &all(), 1).unwrap();
    assert_eq!(converged(&seed.sessions[0].outcome).ops, 0, "B had nothing to give yet");

    let overlap = serve_open(&mut a, listener.try_clone().unwrap(), &all(), 1).unwrap();
    assert_eq!(converged(&overlap.sessions[0].outcome).ingest.txns_held, 1, "A holds the overlap");
    assert_eq!(a.proj.count("quarantine").unwrap(), 1);

    let quiet = serve_open(&mut a, listener.try_clone().unwrap(), &all(), 1).unwrap();
    assert_eq!(converged(&quiet.sessions[0].outcome).ops, 0, "A pulls nothing; roots already agree");
    assert_eq!(converged(&quiet.sessions[0].outcome).windows, 0, "not even window hashes");
    let (_, b_ops, _) = rx.recv().unwrap();
    assert_eq!(b_ops, 0, "B pulls nothing either");

    let heal = serve_open(&mut a, listener, &all(), 1).unwrap();
    let (bad, b_summary) = client.join().unwrap();
    assert_eq!(converged(&heal.sessions[0].outcome).ingest.txns_retried, 1, "the held txn applied");
    assert_eq!(a.proj.count("quarantine").unwrap(), 0);
    assert_eq!(a.proj.field_num(&bad, "start").unwrap(), Some(1000.0));
    assert_eq!(summary(&a.proj, &txt).unwrap(), b_summary, "fully converged after healing");
}

#[test]
fn server_policy_keeps_a_denied_scope_off_the_wire() {
    let root_a = TempRoot::new();
    let mut a = Project::open(&root_a.0, "prs_A").unwrap();
    let secret = a.create_text("Restricted").unwrap();
    a.add_segment(&secret, "tir_utt", 0, None, None, "secret words").unwrap();
    a.create_lexeme("su", "water").unwrap();
    let (listener, addr) = listen();
    let (pid, denied) = (a.project_id.clone(), secret.clone());

    let client = thread::spawn(move || {
        let root_b = TempRoot::new();
        let mut b = Project::open_joined(&root_b.0, "prs_B", &pid).unwrap();
        sync_open(&mut b, addr, &all()).unwrap();
        let ops = b.log.read_all().unwrap().ops;
        (
            scopes(&b.proj).unwrap(),
            ops.iter().any(|o| o.scope == denied),
            ops.iter().any(|o| o.to_json().contains("secret words")),
        )
    });

    serve_open(&mut a, listener, &SyncPolicy::deny([secret.clone()]), 1).unwrap();
    let (b_scopes, op_in_scope, secret_text) = client.join().unwrap();
    assert!(b_scopes.iter().any(|s| s == SCOPE_LEXICON), "permitted scopes crossed: {b_scopes:?}");
    assert!(!b_scopes.contains(&secret), "the denied scope never appears on B");
    assert!(!op_in_scope && !secret_text, "no op of the denied scope reached B's log");
}

#[test]
fn bad_peers_are_dropped_and_the_server_keeps_serving() {
    let root_a = TempRoot::new();
    let mut a = Project::open(&root_a.0, "prs_A").unwrap();
    a.create_lexeme("su", "water").unwrap();
    let (listener, addr) = listen();
    let pid = a.project_id.clone();

    let client = thread::spawn(move || {
        let mut garbage = TcpStream::connect(addr).unwrap();
        garbage.write_all(b"this is not a frame").unwrap();
        let mut rest = Vec::new();
        garbage.read_to_end(&mut rest).ok();

        let mut not_json = TcpStream::connect(addr).unwrap();
        not_json.write_all(&5u32.to_le_bytes()).unwrap();
        not_json.write_all(b"{nope").unwrap();
        not_json.read_to_end(&mut rest).ok();

        let root_x = TempRoot::new();
        let mut stranger = Project::open(&root_x.0, "prs_X").unwrap();
        let refused = sync_open(&mut stranger, addr, &all()).unwrap_err();

        let root_b = TempRoot::new();
        let mut b = Project::open_joined(&root_b.0, "prs_B", &pid).unwrap();
        let ok = sync_open(&mut b, addr, &all()).map(|r| r.pulled.ops);
        (refused, ok)
    });

    let report = serve_open(&mut a, listener, &all(), 4).unwrap();
    let (refused, ok) = client.join().unwrap();
    let outcomes: Vec<_> = report.sessions.iter().map(|s| &s.outcome).collect();

    assert!(
        matches!(outcomes[0], SessionOutcome::Failed(m) if m.contains("exceeds")),
        "garbage reads as an oversized length: {:?}",
        outcomes[0]
    );
    assert!(
        matches!(outcomes[1], SessionOutcome::Failed(m) if m.contains("not JSON")),
        "invalid JSON closes the session: {:?}",
        outcomes[1]
    );
    assert!(refused.contains("peer syncs project"), "the stranger learns why: {refused}");
    assert!(
        matches!(outcomes[2], SessionOutcome::Failed(m) if m.contains("peer syncs project")),
        "the server refuses the stranger itself, not via the client's error: {:?}",
        outcomes[2]
    );
    assert!(ok.unwrap() > 0, "a real peer still syncs after all that");
    converged(outcomes[3]);
}

#[test]
fn a_peer_that_leaves_after_its_pull_is_a_partial_session() {
    let root_a = TempRoot::new();
    let mut a = Project::open(&root_a.0, "prs_A").unwrap();
    a.create_lexeme("su", "water").unwrap();
    let (listener, addr) = listen();

    let pid = a.project_id.clone();
    let client = thread::spawn(move || {
        let mut s = TcpStream::connect(addr).unwrap();
        write_frame(
            &mut s,
            &Value::obj(vec![
                ("t", Value::str("hello")),
                ("v", Value::Num(1.0)),
                ("project", Value::str(&pid)),
            ]),
        )
        .unwrap();
        read_frame(&mut s).unwrap()
    });

    let report = serve_open(&mut a, listener, &all(), 1).unwrap();
    let hello = client.join().unwrap();
    assert_eq!(hello.get_str("t"), Some("hello"));
    assert_eq!(report.sessions[0].answered, 1);
    assert!(matches!(report.sessions[0].outcome, SessionOutcome::PeerLeft));
}

#[test]
fn frames_round_trip_unicode_and_reject_oversize() {
    let v = Value::obj(vec![
        ("tr", Value::str("köpek")),
        ("ja", Value::str("日本語")),
        ("emoji", Value::str("🦜 wug 🐦")),
        ("n", Value::Arr(vec![Value::Num(1.5), Value::Null, Value::Bool(true)])),
    ]);
    let mut buf = Vec::new();
    write_frame(&mut buf, &v).unwrap();
    write_frame(&mut buf, &v).unwrap();
    let mut r = &buf[..];
    assert_eq!(read_frame(&mut r).unwrap(), v);
    assert_eq!(read_frame(&mut r).unwrap(), v, "frames are self-delimiting");
    assert!(matches!(read_frame(&mut r), Err(FrameError::Closed)), "clean EOF between frames");

    let mut header = ((MAX_FRAME + 1) as u32).to_le_bytes().to_vec();
    header.extend_from_slice(b"{}");
    assert!(matches!(read_frame(&mut &header[..]), Err(FrameError::Protocol(_))), "oversized length");

    let big = Value::str(&"x".repeat(MAX_FRAME));
    assert!(matches!(write_frame(&mut Vec::new(), &big), Err(FrameError::Protocol(_))), "oversized write");

    let cut = &buf[..buf.len() / 2 - 3];
    assert!(matches!(read_frame(&mut &cut[..]), Err(FrameError::Protocol(_))), "a cut-off frame is a protocol error");
    assert!(matches!(read_frame(&mut &buf[..2]), Err(FrameError::Protocol(_))), "a cut-off header too");

    let mut bad_utf8 = 2u32.to_le_bytes().to_vec();
    bad_utf8.extend_from_slice(&[0xff, 0xfe]);
    assert!(matches!(read_frame(&mut &bad_utf8[..]), Err(FrameError::Protocol(_))), "invalid UTF-8");
}

/// Multicast may be unavailable (CI sandboxes, some hotspots): an empty
/// browse is accepted, but anything found must belong to the same project.
#[test]
fn discovery_finds_only_the_same_project() {
    let pid = wugs_model::new_id("prj");
    let port = 47_311;
    let _ad = match discover::announce(&pid, &wugs_model::new_id("dvc"), port) {
        Ok(ad) => ad,
        Err(e) => {
            eprintln!("mdns unavailable here, announce failed cleanly: {e}");
            return;
        }
    };
    match discover::browse(&pid, Duration::from_secs(2)) {
        Ok(found) => {
            eprintln!("mdns browse found {found:?}");
            assert!(found.iter().all(|a| a.port() == port), "only our announcement matches: {found:?}");
        }
        Err(e) => eprintln!("mdns browse failed cleanly: {e}"),
    }
    let other = discover::browse(&wugs_model::new_id("prj"), Duration::from_millis(500)).unwrap_or_default();
    assert!(other.is_empty(), "a different project id finds nothing: {other:?}");
}

#[test]
fn data_requests_without_a_hello_are_refused() {
    let root_a = TempRoot::new();
    let mut a = Project::open(&root_a.0, "prs_A").unwrap();
    a.create_lexeme("gizli", "secret").unwrap();
    let (listener, addr) = listen();

    let client = thread::spawn(move || {
        let mut s = TcpStream::connect(addr).unwrap();
        write_frame(&mut s, &Value::obj(vec![("t", Value::str("get_roots")), ("v", Value::Num(1.0))])).unwrap();
        read_frame(&mut s).unwrap()
    });
    let report = serve_open(&mut a, listener, &all(), 1).unwrap();
    let reply = client.join().unwrap();
    assert_eq!(reply.get_str("t"), Some("error"), "a raw pull gets an error, not data: {reply:?}");
    assert!(
        matches!(&report.sessions[0].outcome, SessionOutcome::Failed(m) if m.contains("hello")),
        "{:?}",
        report.sessions[0].outcome
    );
}

#[test]
fn serve_with_rebuilds_the_policy_between_sessions() {
    let root_a = TempRoot::new();
    let mut a = Project::open(&root_a.0, "prs_A").unwrap();
    let txt = a.create_text("Open at first").unwrap();
    a.add_segment(&txt, "tir_utt", 0, None, None, "önce açık").unwrap();
    let (listener, addr) = listen();
    let pid = a.project_id.clone();
    let txt_for_client = txt.clone();

    let client = thread::spawn(move || {
        let r1 = TempRoot::new();
        let mut b = Project::open_joined(&r1.0, "prs_B", &pid).unwrap();
        sync_open(&mut b, addr, &all()).unwrap();
        let b_has = b.proj.entity_kind(&txt_for_client).unwrap().is_some();
        // B restricts the text and pushes that back in session two
        b.set_access(&txt_for_client, "restricted").unwrap();
        sync_open(&mut b, addr, &all()).unwrap();
        // session three: a fresh device C must now be denied the text
        let r2 = TempRoot::new();
        let mut c = Project::open_joined(&r2.0, "prs_C", &b.project_id).unwrap();
        sync_open(&mut c, addr, &all()).unwrap();
        let c_ops = c.log.read_all().unwrap().ops;
        (b_has, c_ops.iter().all(|o| o.scope != txt_for_client))
    });

    serve_open_with(&mut a, listener, 3, &|p| wugs_sync::policy_from_access(&p.proj, &[])).unwrap();
    let (b_had_it_open, c_denied) = client.join().unwrap();
    assert!(b_had_it_open, "B received the text while it was open");
    assert!(c_denied, "a restriction learned mid-serve binds the very next session");
}
