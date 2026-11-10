use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use wugs_auth::{Body, DeviceKey, RosterFile, SignedRecord};
use wugs_desktop::listen::{self, Listening};
use wugs_desktop::session::{self, ServedSession, Session, StandingView};
use wugs_desktop::SessionThread;
use wugs_model::ulid;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!("wugs-desktop-hard-{}", ulid())))
    }
    fn path(&self) -> String {
        self.0.to_str().unwrap().to_string()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

/// A host project on its own session thread, the way the app runs it.
fn host_thread() -> (TempDir, SessionThread, Arc<DeviceKey>) {
    let dir = TempDir::new();
    let thread = SessionThread::spawn();
    let path = dir.path();
    thread.run(move |s| session::create_project(s, &path, "prs_A")).unwrap();
    thread.run(|s| session::create_text(s, "Story")).unwrap();
    (dir, thread, Arc::new(DeviceKey::generate()))
}

fn start_listening(host: &SessionThread, key: &Arc<DeviceKey>) -> (Listening, mpsc::Receiver<ServedSession>, Arc<AtomicUsize>) {
    let k = key.clone();
    let l = host.run(move |s| session::sync_listen(s, &k, "laptop")).unwrap();
    let (tx, rx) = mpsc::channel();
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let listening = listen::start(host.clone(), key.clone(), l, move |served| {
        c.fetch_add(1, Ordering::SeqCst);
        tx.send(served).ok();
    })
    .unwrap();
    (listening, rx, count)
}

/// How long one ordinary command waits behind whatever the session thread is doing.
fn command_latency(host: &SessionThread) -> Duration {
    let started = Instant::now();
    host.run(session::list_texts).unwrap();
    started.elapsed()
}

#[test]
fn a_silent_peer_never_holds_the_session_thread() {
    let (_dir, host, key) = host_thread();
    let (listening, _rx, _) = start_listening(&host, &key);
    let _silent = TcpStream::connect(("127.0.0.1", listening.port)).unwrap();
    // Long enough for the accept loop to have picked the connection up.
    thread::sleep(Duration::from_millis(400));
    let waited = command_latency(&host);
    assert!(waited < Duration::from_secs(3), "a silent connection held the session thread for {waited:?}");
}

#[test]
fn a_peer_dripping_bytes_is_cut_at_the_handshake_deadline() {
    let (_dir, host, key) = host_thread();
    let (listening, rx, _) = start_listening(&host, &key);
    let port = listening.port;
    thread::spawn(move || {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        // A TLS record header promising 16 KiB, then a byte every 200 ms:
        // every read succeeds well inside its timeout.
        let started = Instant::now();
        for b in [0x16u8, 0x03, 0x01, 0x40, 0x00].into_iter().chain(std::iter::repeat(0)) {
            if s.write_all(&[b]).is_err() || started.elapsed() > Duration::from_secs(90) {
                return;
            }
            thread::sleep(Duration::from_millis(200));
        }
    });
    thread::sleep(Duration::from_millis(600));
    let waited = command_latency(&host);
    let bound = wugs_net::HANDSHAKE_DEADLINE + Duration::from_secs(10);
    assert!(waited < bound, "a dripping peer held the session thread for {waited:?}");
    let served = rx.recv_timeout(Duration::from_secs(5)).expect("the cut session is reported");
    assert!(served.outcome.contains("time limit"), "{}", served.outcome);
}

/// Pair a joiner (on its own session thread) with the host.
fn pair(host: &SessionThread, key: &Arc<DeviceKey>) -> (SessionThread, Arc<DeviceKey>, TempDir) {
    let k = key.clone();
    let (offer, pairing) = host.run(move |s| session::pair_start(s, &k, "laptop", false)).unwrap();
    let joiner = SessionThread::spawn();
    let joiner_key = Arc::new(DeviceKey::generate());
    let dir = TempDir::new();
    let (j, jk, path) = (joiner.clone(), joiner_key.clone(), dir.path());
    let joining = thread::spawn(move || {
        j.run(move |s| session::pair_join(s, &jk, &path, "prs_B", &format!("127.0.0.1:{}", offer.port), &offer.digits, "tablet"))
    });
    let tcp = pairing.wait().unwrap();
    let k = key.clone();
    host.run(move |s| session::pair_finish(s, &k, pairing, tcp)).unwrap();
    joining.join().unwrap().unwrap();
    (joiner, joiner_key, dir)
}

fn sync_to(joiner: &SessionThread, key: &Arc<DeviceKey>, port: u16) -> session::SyncResult {
    let k = key.clone();
    joiner.run(move |s| session::sync_now(s, &k, &format!("127.0.0.1:{port}"))).unwrap()
}

#[test]
fn a_dropped_listener_closes_its_port_and_a_new_one_serves_each_session_once() {
    let (_dir, host, key) = host_thread();
    let (joiner, joiner_key, _jdir) = pair(&host, &key);

    let (first, rx1, count1) = start_listening(&host, &key);
    let port1 = first.port;
    assert!(sync_to(&joiner, &joiner_key, port1).pulled_ops > 0);
    assert_eq!(rx1.recv_timeout(Duration::from_secs(10)).unwrap().outcome, "converged");
    drop(first);
    assert!(TcpStream::connect_timeout(&([127, 0, 0, 1], port1).into(), Duration::from_secs(2)).is_err(), "the old port no longer accepts");

    let (second, rx2, count2) = start_listening(&host, &key);
    sync_to(&joiner, &joiner_key, second.port);
    assert_eq!(rx2.recv_timeout(Duration::from_secs(10)).unwrap().outcome, "converged");
    thread::sleep(Duration::from_millis(300));
    assert_eq!((count1.load(Ordering::SeqCst), count2.load(Ordering::SeqCst)), (1, 1), "each session was served exactly once");
}

fn enrolments_of(host_root: &str, key: &DeviceKey) -> usize {
    let text = fs::read_to_string(RosterFile::in_project(host_root).path()).unwrap();
    text.lines()
        .filter_map(|l| SignedRecord::from_line(l).ok())
        .filter(|r| matches!(&r.record().body, Body::Enrol { key: k, .. } if *k == key.public()))
        .count()
}

#[test]
fn a_join_that_stopped_after_the_roster_resumes_without_enrolling_twice() {
    let host_dir = TempDir::new();
    let mut host: Session = None;
    session::create_project(&mut host, &host_dir.path(), "prs_A").unwrap();
    let host_key = DeviceKey::generate();
    let joiner_key = Arc::new(DeviceKey::generate());
    let dir = TempDir::new();

    // The first join gets as far as writing the roster, then the app dies.
    let (offer, pairing) = session::pair_start(&mut host, &host_key, "laptop", false).unwrap();
    let (jk, path) = (joiner_key.clone(), dir.path());
    let crashed = thread::spawn(move || {
        let mut tcp = TcpStream::connect(("127.0.0.1", offer.port)).unwrap();
        let joined = wugs_auth::pair_join(&mut tcp, &offer.digits, &jk, "tablet").unwrap();
        RosterFile::in_project(&path).create(&joined.roster).unwrap();
    });
    let tcp = pairing.wait().unwrap();
    session::pair_finish(&mut host, &host_key, pairing, tcp).unwrap();
    crashed.join().unwrap();
    assert_eq!(enrolments_of(&host_dir.path(), &joiner_key), 1);

    // The person tries again with a fresh code into the same folder.
    let (offer, pairing) = session::pair_start(&mut host, &host_key, "laptop", false).unwrap();
    let (jk, path) = (joiner_key.clone(), dir.path());
    let retry = thread::spawn(move || {
        let mut s: Session = None;
        let r = session::pair_join(&mut s, &jk, &path, "prs_B", &format!("127.0.0.1:{}", offer.port), &offer.digits, "tablet");
        r.map(|j| (j.standing, j.project.project_id))
    });
    let waiter = thread::spawn(move || pairing.wait().map(|tcp| (pairing, tcp)));
    let mut waiter = Some(waiter);
    while !retry.is_finished() {
        if waiter.as_ref().is_some_and(|w| w.is_finished()) {
            if let Ok((pairing, tcp)) = waiter.take().unwrap().join().unwrap() {
                session::pair_finish(&mut host, &host_key, pairing, tcp).ok();
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    let (standing, project_id) = retry.join().unwrap().expect("the retry opens the joined project");
    assert_eq!(standing, StandingView::Member);
    assert_eq!(project_id, host.as_ref().unwrap().project_id);
    assert_eq!(enrolments_of(&host_dir.path(), &joiner_key), 1, "the host enrolled the joiner exactly once");
}

const HELPER_PATH: &str = "WUGS_LOCK_HELPER_PATH";

/// Run by the test below as a child process: hold the project open until
/// stdin closes.
#[test]
#[ignore = "helper process for another_process_cannot_open_a_project_this_one_holds"]
fn lock_helper_holds_a_project_open() {
    let Ok(path) = std::env::var(HELPER_PATH) else { return };
    let mut s: Session = None;
    session::open_project(&mut s, &path, "prs_Helper").unwrap();
    println!("holding the project");
    std::io::stdout().flush().unwrap();
    std::io::stdin().lines().for_each(drop);
}

#[test]
fn another_process_cannot_open_a_project_this_one_holds() {
    let dir = TempDir::new();
    let mut s: Session = None;
    session::create_project(&mut s, &dir.path(), "prs_A").unwrap();
    session::open_project(&mut s, &dir.path(), "prs_A").expect("reopening the folder this session holds is fine");
    let mut other: Session = None;
    let err = session::open_project(&mut other, &dir.path(), "prs_A").map(|_| ()).unwrap_err();
    assert!(err.contains("open in another wugs window"), "{err}");
    drop(s);

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["lock_helper_holds_a_project_open", "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env(HELPER_PATH, dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    assert!(lines.any(|l| l.unwrap().contains("holding the project")), "the helper opened the project");

    let err = session::open_project(&mut other, &dir.path(), "prs_A").map(|_| ()).unwrap_err();
    assert!(err.contains("open in another wugs window"), "{err}");
    assert!(other.is_none());

    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
    session::open_project(&mut other, &dir.path(), "prs_A").expect("the lock goes with the process that held it");
}
