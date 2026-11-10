//! Authenticated sessions on 127.0.0.1. `Project` is not `Send`, so each
//! project lives on one thread; rosters are files, set up before a project
//! opens, and keys move freely between threads.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rustls::{ClientConnection, StreamOwned};
use wugs_auth::{pair_host, pair_join, Body, DeviceKey, MergeError, PairingCode, PublicKey, Roster, RosterFile, SignedRecord, Standing, MAX_PEER_RECORDS};
use wugs_core::{Project, SCOPE_LEXICON};
use wugs_model::Value;
use wugs_net::frame::{read_frame, write_frame};
use wugs_net::{connect, default_policy, serve, sync_with, tls, ClientStream, Refusal, Role, SessionOutcome, Warning, NET_VERSION};
use wugs_sync::{scopes, summary, ScopeSummary};

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> TempRoot {
        TempRoot(std::env::temp_dir().join(format!("wugs-auth-net-{}", wugs_model::ulid())))
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

/// A founding device: its project and its genesis roster.
fn found(root: &TempRoot) -> (Project, DeviceKey) {
    let p = Project::open(&root.0, "prs_A").unwrap();
    let key = DeviceKey::generate();
    RosterFile::in_project(&root.0).create(&Roster::genesis(&p.project_id, &key, "A")).unwrap();
    (p, key)
}

fn roster(p: &Project) -> Roster {
    RosterFile::in_project(&p.root).load(&p.project_id).unwrap()
}

/// `by` enrols a new device and hands it a copy of the roster, as pairing
/// would; returns the new device's key.
fn enrol(host: &Project, by: &DeviceKey, joiner_root: &TempRoot, label: &str, guest: bool) -> DeviceKey {
    let key = DeviceKey::generate();
    let mut r = roster(host);
    let rec = r.enrol(by, key.public(), label, guest).unwrap();
    RosterFile::in_project(&host.root).append(&[rec]).unwrap();
    RosterFile::in_project(&joiner_root.0).create(&r).unwrap();
    key
}

/// `p` attests its own `dvc` id now, as its first session would, so a
/// later session appends nothing of its own.
fn attest(p: &Project, key: &DeviceKey) {
    let mut r = roster(p);
    let rec = r.append(key, Body::Attest { key: key.public(), dvc: p.device().to_string() }).unwrap();
    RosterFile::in_project(&p.root).append(&[rec]).unwrap();
}

/// A TLS client that does the handshake and nothing else, for speaking
/// the session by hand.
fn raw_tls(key: &DeviceKey, server: PublicKey, addr: SocketAddr, alpn: Option<&[u8]>) -> ClientStream {
    let mut config = (*tls::client_config(key, BTreeSet::from([server])).unwrap()).clone();
    config.alpn_protocols = alpn.into_iter().map(<[u8]>::to_vec).collect();
    let conn = ClientConnection::new(Arc::new(config), tls::server_name()).unwrap();
    let tcp = TcpStream::connect(addr).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    StreamOwned::new(conn, tcp)
}

fn refused(o: &SessionOutcome) -> &Refusal {
    match o {
        SessionOutcome::Refused(r) => r,
        other => panic!("session was not refused: {other:?}"),
    }
}

#[test]
fn enrolled_members_converge_over_tls() {
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    let txt = a.create_text("Story").unwrap();
    a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "köpek").unwrap();
    let root_b = TempRoot::new();
    let key_b = enrol(&a, &key_a, &root_b, "B", false);
    let (listener, addr) = listen();
    let pid = a.project_id.clone();

    let client = thread::spawn(move || {
        let mut b = Project::open_joined(&root_b.0, "prs_B", &pid).unwrap();
        b.create_lexeme("su", "water").unwrap();
        let r = sync_with(&mut b, addr, &key_b, &default_policy).unwrap();
        (r.pulled.ops, r.peer.unwrap().role, summaries(&b), roster(&b).state())
    });
    let report = serve(&mut a, listener, &key_a, 1, &default_policy).unwrap();
    let (b_pulled, server_role, b_summaries, b_roster) = client.join().unwrap();

    let s = &report.sessions[0];
    assert_eq!(s.peer.as_ref().unwrap().role, Role::Member);
    assert_eq!(server_role, Role::Member);
    assert!(b_pulled > 0, "B pulled A's text");
    assert!(matches!(&s.outcome, SessionOutcome::Converged(t) if t.ops > 0), "A pulled B's lexeme: {:?}", s.outcome);
    assert!(summaries(&a).iter().any(|s| s.scope == SCOPE_LEXICON));
    assert_eq!(summaries(&a), b_summaries, "every scope's summary matches");
    let a_roster = roster(&a).state();
    assert_eq!(a_roster.attested.len(), 2, "both devices attested their dvc ids: {:?}", a_roster.attested);
    assert_eq!(a_roster, b_roster, "the rosters converged too");
}

#[test]
fn strangers_and_plain_tcp_get_no_wugs_frames_and_the_server_survives() {
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    a.create_lexeme("su", "water").unwrap();
    let root_b = TempRoot::new();
    let key_b = enrol(&a, &key_a, &root_b, "B", false);
    let (listener, addr) = listen();
    let (pid, a_pub) = (a.project_id.clone(), key_a.public());

    let client = thread::spawn(move || {
        // A stranger with its own roster that trusts A's key: A still refuses it.
        let root_x = TempRoot::new();
        let key_x = DeviceKey::generate();
        let mut rx = Roster::genesis(&pid, &key_x, "X");
        rx.enrol(&key_x, a_pub, "A", false).unwrap();
        RosterFile::in_project(&root_x.0).create(&rx).unwrap();
        let mut x = Project::open_joined(&root_x.0, "prs_X", &pid).unwrap();
        let stranger = sync_with(&mut x, addr, &key_x, &default_policy).map(|r| r.pulled.ops);

        // A plain-TCP v1 client: its hello is not TLS, and nothing it reads is a frame.
        let mut raw = TcpStream::connect(addr).unwrap();
        raw.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let hello = Value::obj(vec![("t", Value::str("hello")), ("v", Value::Num(1.0)), ("project", Value::str(&pid))]);
        write_frame(&mut raw, &hello).unwrap();
        let reply = read_frame(&mut raw);
        let mut rest = Vec::new();
        raw.read_to_end(&mut rest).ok();

        let mut b = Project::open_joined(&root_b.0, "prs_B", &pid).unwrap();
        let ok = sync_with(&mut b, addr, &key_b, &default_policy).map(|r| r.pulled.ops);
        (stranger, reply.is_ok(), String::from_utf8_lossy(&rest).contains("roster"), ok)
    });
    let report = serve(&mut a, listener, &key_a, 3, &default_policy).unwrap();
    let (stranger, raw_got_frame, raw_saw_roster, ok) = client.join().unwrap();

    assert!(stranger.is_err(), "the stranger syncs nothing");
    assert!(
        matches!(refused(&report.sessions[0].outcome), Refusal::Handshake(m) if m.contains("not on this project's roster")),
        "{:?}",
        report.sessions[0].outcome
    );
    assert!(!raw_got_frame && !raw_saw_roster, "a plain-TCP client gets no wugs frame");
    assert!(matches!(refused(&report.sessions[1].outcome), Refusal::Handshake(_)));
    assert!(ok.unwrap() > 0, "an enrolled device still syncs after both");
    assert!(matches!(report.sessions[2].outcome, SessionOutcome::Converged(_)));
}

#[test]
fn a_revoked_device_is_refused_before_hello() {
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    a.create_lexeme("su", "water").unwrap();
    let root_c = TempRoot::new();
    let key_c = enrol(&a, &key_a, &root_c, "C", false);
    let mut r = roster(&a);
    let rec = r.revoke(&key_a, key_c.public()).unwrap();
    RosterFile::in_project(&a.root).append(&[rec]).unwrap();
    let (listener, addr) = listen();
    let pid = a.project_id.clone();

    let client = thread::spawn(move || {
        let mut c = Project::open_joined(&root_c.0, "prs_C", &pid).unwrap();
        let r = sync_with(&mut c, addr, &key_c, &default_policy).map(|r| r.pulled.ops);
        (r, c.log.read_all().unwrap().ops.len())
    });
    let report = serve(&mut a, listener, &key_a, 1, &default_policy).unwrap();
    let (r, c_ops) = client.join().unwrap();
    assert!(r.is_err());
    assert_eq!(c_ops, 0, "C received nothing");
    assert!(report.sessions[0].peer.is_none(), "the session never got past TLS");
    assert!(matches!(refused(&report.sessions[0].outcome), Refusal::Handshake(_)));
}

fn req(t: &str, fields: Vec<(&str, Value)>) -> Value {
    let mut all = vec![("t", Value::str(t)), ("v", Value::Num(1.0))];
    all.extend(fields);
    Value::obj(all)
}

fn ask(s: &mut (impl Read + std::io::Write), msg: Value) -> Value {
    write_frame(s, &msg).unwrap();
    read_frame(s).unwrap()
}

#[test]
fn a_guest_reads_open_scopes_only_and_can_never_push() {
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    let open = a.create_text("Open").unwrap();
    a.add_segment(&open, "tir_utt", 0, None, None, "açık").unwrap();
    let closed = a.create_text("Restricted").unwrap();
    a.add_segment(&closed, "tir_utt", 0, None, None, "gizli söz").unwrap();
    a.set_access(&closed, "restricted").unwrap();
    a.create_lexeme("su", "water").unwrap();
    let root_g = TempRoot::new();
    let key_g = enrol(&a, &key_a, &root_g, "visitor", true);
    let root_b = TempRoot::new();
    let key_b = enrol(&a, &key_a, &root_b, "B", false);
    let (listener, addr) = listen();
    let (pid, open_c, closed_c) = (a.project_id.clone(), open.clone(), closed.clone());
    // The restricted text's real groups and windows, as a member would ask for them.
    let real = summary(&a.proj, &closed).unwrap();
    let keys = |m: Vec<u64>| Value::Arr(m.iter().map(|k| Value::str(&k.to_string())).collect());
    let per_scope = |v: Value| Value::Obj([(closed.clone(), v)].into_iter().collect());
    let want_groups = per_scope(keys(real.groups().into_keys().collect()));
    let want_windows = per_scope(keys(real.windows.keys().copied().collect()));
    assert!(!real.windows.is_empty());

    let client = thread::spawn(move || {
        let mut g = Project::open_joined(&root_g.0, "prs_G", &pid).unwrap();
        let walk = |s: &mut ClientStream| {
            assert_eq!(ask(s, req("hello", vec![("project", Value::str(&pid))])).get_str("t"), Some("hello"));
            let roots = ask(s, req("get_roots", vec![])).to_json();
            let groups = ask(s, req("get_groups", vec![("scopes", Value::Arr(vec![Value::str(&closed_c)]))]));
            let windows = ask(s, req("get_windows", vec![("scopes", want_groups.clone())]));
            let ops = ask(s, req("get_ops", vec![("scopes", want_windows.clone())]));
            let count = |v: &Value| v.get("scopes").and_then(Value::as_obj).and_then(|m| m.get(&closed_c)).and_then(Value::as_obj).map_or(0, |m| m.len());
            let ops = match ops.get("ops") {
                Some(Value::Arr(a)) => a.len(),
                _ => 0,
            };
            (roots, count(&groups), count(&windows), ops)
        };

        // Session 1: a guest speaking the wire protocol by hand.
        let mut sess = connect(&g, addr, &key_g).unwrap();
        assert_eq!(sess.role, Role::Guest);
        let (roots, groups, windows, ops) = walk(&mut sess.stream);
        let push = ask(&mut sess.stream, Value::obj(vec![("t", Value::str("pull_me"))]));
        let wire = (roots.contains(&open_c), roots.contains(&closed_c), groups, windows, ops, push.get_str("t") == Some("error"));
        drop(sess);

        // The same requests from a member get the restricted text.
        let b = Project::open_joined(&root_b.0, "prs_B", &pid).unwrap();
        let mut sess = connect(&b, addr, &key_b).unwrap();
        let (_, m_groups, m_windows, m_ops) = walk(&mut sess.stream);
        let member = (m_groups, m_windows, m_ops);
        drop(sess);

        // Session 2: the ordinary client. The guest's own edit stays home.
        g.create_lexeme("yerel", "local only").unwrap();
        sync_with(&mut g, addr, &key_g, &default_policy).unwrap();
        let has_open_text = g.proj.entity_kind(&open_c).unwrap().is_some();
        let log = |g: &Project| g.log.read_all().unwrap().ops;
        let leaked = log(&g).iter().any(|o| o.scope == closed_c || o.to_json().contains("gizli"));

        // Session 3: A restricted the open text in between.
        sync_with(&mut g, addr, &key_g, &default_policy).unwrap();
        let later = log(&g).iter().any(|o| o.to_json().contains("sonra"));
        (wire, member, has_open_text, leaked, later)
    });

    let s1 = serve(&mut a, listener.try_clone().unwrap(), &key_a, 1, &default_policy).unwrap();
    serve(&mut a, listener.try_clone().unwrap(), &key_a, 1, &default_policy).unwrap();
    let s2 = serve(&mut a, listener.try_clone().unwrap(), &key_a, 1, &default_policy).unwrap();
    a.set_access(&open, "restricted").unwrap();
    a.add_segment(&open, "tir_utt", 1, None, None, "sonra").unwrap();
    let s3 = serve(&mut a, listener, &key_a, 1, &default_policy).unwrap();
    let (wire, member, has_open_text, leaked, later) = client.join().unwrap();

    let (open_root, closed_root, groups, windows, ops, push_refused) = wire;
    assert!(open_root, "the open text's root is offered to the guest");
    assert!(!closed_root, "the restricted text is not even named in roots");
    assert_eq!((groups, windows, ops), (0, 0, 0), "every level of the walk-down withholds it");
    let (m_groups, m_windows, m_ops) = member;
    assert!(m_groups > 0 && m_windows > 0 && m_ops > 0, "a member gets it for the same requests: {member:?}");
    assert!(push_refused, "pull_me from a guest gets an error frame");
    assert_eq!(refused(&s1.sessions[0].outcome), &Refusal::GuestPush);
    assert_eq!(s1.sessions[0].peer.as_ref().unwrap().role, Role::Guest);

    assert!(has_open_text && !leaked, "the guest got open data and nothing restricted");
    assert!(matches!(&s2.sessions[0].outcome, SessionOutcome::Converged(t) if t.ops == 0), "A pulled nothing from the guest");
    assert!(!a.proj.entities_of_kind("lexeme").unwrap().iter().any(|l| a.proj.field_str(l, "lemma").unwrap().as_deref() == Some("yerel")));
    assert!(!later, "a restriction made between sessions binds the next one");
    assert!(matches!(s3.sessions[0].outcome, SessionOutcome::Converged(_)));
}

#[test]
fn a_revocation_spreads_ahead_of_data() {
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    a.create_lexeme("su", "water").unwrap();
    let (root_b, root_c) = (TempRoot::new(), TempRoot::new());
    let key_b = enrol(&a, &key_a, &root_b, "B", false);
    let key_c = enrol(&a, &key_a, &root_c, "C", false);
    // B has met C: its roster lists C as a member.
    let b_file = RosterFile::in_project(&root_b.0);
    fs::remove_file(b_file.path()).unwrap();
    b_file.create(&roster(&a)).unwrap();
    let mut r = roster(&a);
    let rec = r.revoke(&key_a, key_c.public()).unwrap();
    RosterFile::in_project(&a.root).append(&[rec]).unwrap();
    let pid = a.project_id.clone();
    drop(a);

    // B was offline for the revocation; it syncs with A.
    let (la, addr_a) = listen();
    let a_root = root_a.0.clone();
    let server_a = thread::spawn(move || {
        let mut a = Project::open(&a_root, "prs_A").unwrap();
        let rep = serve(&mut a, la, &key_a, 1, &default_policy).unwrap();
        format!("{:?}", rep.sessions[0].outcome)
    });
    let mut b = Project::open_joined(&root_b.0, "prs_B", &pid).unwrap();
    assert_eq!(roster(&b).state().standing(&key_c.public()), Standing::Member, "B has not heard yet");
    sync_with(&mut b, addr_a, &key_b, &default_policy).unwrap();
    assert!(server_a.join().unwrap().starts_with("Converged"));
    assert_eq!(roster(&b).state().standing(&key_c.public()), Standing::Revoked, "the revocation came with the sync");

    // Now C reaches B, and B refuses it.
    let (lb, addr_b) = listen();
    let client_c = thread::spawn(move || {
        let mut c = Project::open_joined(&root_c.0, "prs_C", &pid).unwrap();
        sync_with(&mut c, addr_b, &key_c, &default_policy).map(|r| r.pulled.ops)
    });
    let rep = serve(&mut b, lb, &key_b, 1, &default_policy).unwrap();
    assert!(client_c.join().unwrap().is_err());
    assert!(matches!(refused(&rep.sessions[0].outcome), Refusal::Handshake(_)), "{:?}", rep.sessions[0].outcome);
}

#[test]
fn a_device_id_attested_to_another_key_warns_and_still_syncs() {
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    a.create_lexeme("su", "water").unwrap();
    let (root_b, root_c) = (TempRoot::new(), TempRoot::new());
    let key_b = enrol(&a, &key_a, &root_b, "B", false);
    // C's folder is a copy of A's, made before A had attested its dvc id.
    let key_c = enrol(&a, &key_a, &root_c, "C", false);
    let (pid, dvc_a) = (a.project_id.clone(), a.device().to_string());
    let (pub_a, pub_c) = (key_a.public(), key_c.public());
    drop(Project::open_joined(&root_c.0, "prs_C", &pid).unwrap());
    fs::write(root_c.0.join("cache").join("device"), &dvc_a).unwrap();
    let (listener, addr) = listen();

    let clients = thread::spawn(move || {
        let mut b = Project::open_joined(&root_b.0, "prs_B", &pid).unwrap();
        sync_with(&mut b, addr, &key_b, &default_policy).unwrap();
        let mut c = Project::open(&root_c.0, "prs_C").unwrap();
        assert_eq!(c.device(), dvc_a);
        sync_with(&mut c, addr, &key_c, &default_policy).map(|r| (r.pulled.ops, r.warnings))
    });
    let rep = serve(&mut a, listener, &key_a, 2, &default_policy).unwrap();
    let (c_pulled, c_warnings) = clients.join().unwrap().unwrap();

    assert!(matches!(rep.sessions[0].outcome, SessionOutcome::Converged(_)));
    assert!(rep.sessions[0].warnings.is_empty());
    assert!(matches!(rep.sessions[1].outcome, SessionOutcome::Converged(_)), "{:?}", rep.sessions[1].outcome);
    assert!(c_pulled > 0, "the copied folder still syncs");
    let dvc = a.device().to_string();
    assert!(rep.sessions[1].warnings.contains(&Warning::PeerDeviceIdClaimed { dvc: dvc.clone(), other: pub_a }), "{:?}", rep.sessions[1].warnings);
    assert!(rep.sessions[1].warnings.contains(&Warning::OwnDeviceIdClaimed { dvc: dvc.clone(), other: pub_c }));
    assert!(c_warnings.contains(&Warning::OwnDeviceIdClaimed { dvc: dvc.clone(), other: pub_a }), "{c_warnings:?}");
    assert!(c_warnings[0].to_string().contains("create a fresh device identity"));
}

#[test]
fn a_guest_contributes_nothing_to_the_roster() {
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    a.create_lexeme("su", "water").unwrap();
    let root_b = TempRoot::new();
    let key_b = enrol(&a, &key_a, &root_b, "B", false);
    let root_g = TempRoot::new();
    let key_g = enrol(&a, &key_a, &root_g, "visitor", true);
    attest(&a, &key_a);
    // The guest's file holds a member's record A lacks and records of strangers.
    let mut at_g = roster(&a);
    let mut extra = vec![at_g.append(&key_b, Body::Attest { key: key_b.public(), dvc: "dvc_b".into() }).unwrap()];
    for i in 0..200u8 {
        let k = DeviceKey::from_seed([i; 32]);
        extra.push(SignedRecord::sign(&k, &a.project_id, None, Body::Enrol { key: k.public(), label: "x".into(), guest: false }));
    }
    RosterFile::in_project(&root_g.0).append(&extra).unwrap();
    let before = fs::read(RosterFile::in_project(&a.root).path()).unwrap();
    let (listener, addr) = listen();
    let pid = a.project_id.clone();

    let client = thread::spawn(move || {
        let mut g = Project::open_joined(&root_g.0, "prs_G", &pid).unwrap();
        sync_with(&mut g, addr, &key_g, &default_policy).map(|r| r.pulled.ops)
    });
    let rep = serve(&mut a, listener, &key_a, 1, &default_policy).unwrap();
    assert!(client.join().unwrap().unwrap() > 0, "the guest still pulled A's roster and data");
    assert!(matches!(rep.sessions[0].outcome, SessionOutcome::Converged(_)), "{:?}", rep.sessions[0].outcome);
    assert_eq!(fs::read(RosterFile::in_project(&a.root).path()).unwrap(), before, "nothing the guest sent was stored");
}

#[test]
fn an_oversized_roster_frame_is_refused_and_nothing_is_stored() {
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    let root_b = TempRoot::new();
    let key_b = enrol(&a, &key_a, &root_b, "B", false);
    attest(&a, &key_a);
    let before = fs::read(RosterFile::in_project(&a.root).path()).unwrap();
    let (listener, addr) = listen();
    let (a_pub, genesis) = (key_a.public(), roster(&a).genesis_line());

    let client = thread::spawn(move || {
        let mut s = raw_tls(&key_b, a_pub, addr, Some(tls::ALPN));
        let frame = Value::obj(vec![
            ("t", Value::str("roster")),
            ("v", Value::Num(NET_VERSION)),
            ("device", Value::str("dvc_b")),
            ("records", Value::Arr(vec![Value::str(&genesis); MAX_PEER_RECORDS + 1])),
        ]);
        write_frame(&mut s, &frame).unwrap();
        read_frame(&mut s).unwrap()
    });
    let rep = serve(&mut a, listener, &key_a, 1, &default_policy).unwrap();
    let reply = client.join().unwrap();
    assert_eq!(refused(&rep.sessions[0].outcome), &Refusal::Roster(MergeError::TooManyRecords { records: MAX_PEER_RECORDS + 1, limit: MAX_PEER_RECORDS }));
    assert_eq!(reply.get_str("t"), Some("error"), "the peer is told why");
    assert_eq!(fs::read(RosterFile::in_project(&a.root).path()).unwrap(), before);
}

#[test]
fn a_client_without_the_session_alpn_is_refused_before_any_frame() {
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    let root_b = TempRoot::new();
    let key_b = enrol(&a, &key_a, &root_b, "B", false);
    let (listener, addr) = listen();
    let a_pub = key_a.public();

    let client = thread::spawn(move || {
        let mut s = raw_tls(&key_b, a_pub, addr, None);
        let reply = read_frame(&mut s).unwrap();
        (s.conn.alpn_protocol().is_none(), reply)
    });
    let rep = serve(&mut a, listener, &key_a, 1, &default_policy).unwrap();
    let (no_alpn, reply) = client.join().unwrap();
    assert!(no_alpn);
    assert_eq!(refused(&rep.sessions[0].outcome), &Refusal::SessionVersion(None));
    assert_eq!(reply.get_str("t"), Some("error"), "the only frame is the refusal, not the roster");
}

#[test]
fn a_peer_from_before_op_heads_is_refused_at_the_handshake() {
    // A wugs/2 build drops `heads` it cannot read, and window hashes cover
    // op ids only, so its stripped copies would pass as the real ops.
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    let root_b = TempRoot::new();
    let key_b = enrol(&a, &key_a, &root_b, "B", false);
    let (listener, addr) = listen();
    let a_pub = key_a.public();

    let client = thread::spawn(move || {
        let mut s = raw_tls(&key_b, a_pub, addr, Some(b"wugs/2"));
        read_frame(&mut s).is_err()
    });
    let rep = serve(&mut a, listener, &key_a, 1, &default_policy).unwrap();
    assert!(client.join().unwrap(), "the old peer gets no frame");
    assert!(
        matches!(refused(&rep.sessions[0].outcome), Refusal::Handshake(_)),
        "{:?}",
        rep.sessions[0].outcome
    );
}

#[test]
fn pairing_over_tcp_then_the_new_device_syncs() {
    let root_a = TempRoot::new();
    let (mut a, key_a) = found(&root_a);
    let txt = a.create_text("Story").unwrap();
    a.add_segment(&txt, "tir_utt", 0, None, None, "ilk").unwrap();
    let (pair_l, pair_addr) = listen();
    let (sync_l, sync_addr) = listen();
    let mut code = PairingCode::generate(Duration::from_secs(120));
    let typed = code.digits().to_string();

    let joiner = thread::spawn(move || {
        let key = DeviceKey::generate();
        let mut tcp = TcpStream::connect(pair_addr).unwrap();
        let joined = pair_join(&mut tcp, &typed, &key, "tablet").unwrap();
        let root = TempRoot::new();
        RosterFile::in_project(&root.0).create(&joined.roster).unwrap();
        let mut b = Project::open_joined(&root.0, "prs_B", &joined.project_id).unwrap();
        let r = sync_with(&mut b, sync_addr, &key, &default_policy).unwrap();
        (r.pulled.ops, summaries(&b))
    });

    let (mut tcp, _) = pair_l.accept().unwrap();
    let mut r = roster(&a);
    let file_a = RosterFile::in_project(&a.root);
    let _rec = pair_host(&mut tcp, &mut code, &key_a, &mut r, false, &mut |rec| file_a.append(std::slice::from_ref(rec))).unwrap();
    let rep = serve(&mut a, sync_l, &key_a, 1, &default_policy).unwrap();
    let (pulled, b_summaries) = joiner.join().unwrap();

    assert_eq!(rep.sessions[0].peer.as_ref().unwrap().label, "tablet");
    assert!(pulled > 0);
    assert_eq!(summaries(&a), b_summaries);
}
