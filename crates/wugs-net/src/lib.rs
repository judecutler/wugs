//! wugs-net: carries the wugs-sync wire machine over TCP, so two field
//! laptops on the same Wi-Fi or phone hotspot sync with no server.
//!
//! One TCP session converges both directions. The client pulls first
//! (its `Requester` against the server's `Responder`), then sends
//! `{"t":"pull_me"}` and the roles swap on the same connection: the server
//! pulls from the client, sends `{"t":"done"}` and closes. Frames are
//! length-prefixed JSON (`frame`); peers find each other over mDNS
//! (`discover`) or by a typed-in address.
//!
//! Either side that hits an error sends `{"t":"error","error":...}` before
//! closing, so the peer can report why instead of seeing a bare hang-up.
//!
//! `serve` handles one connection at a time. `Project` is single-writer and
//! not `Send` by design, so concurrent sessions would need a second writer;
//! a second laptop connecting mid-session waits in the listen backlog. A
//! caller that owns its own accept loop hands each connection to
//! `serve_stream` instead.
//!
//! Every authenticated connection runs under a `Watchdog`: a peer gets
//! `HANDSHAKE_DEADLINE` to finish TLS and the roster prelude, and the
//! whole session must end within `SESSION_DEADLINE`, or the socket is shut
//! down under it. Read timeouts alone reset on every byte, so without the
//! wall clock a peer dripping one byte at a time could hold the
//! single-writer project indefinitely.
//!
//! # Authentication (session version 2)
//!
//! `serve` and `sync_with` run every session inside TLS 1.3 with raw
//! public keys (`tls`): both sides present their device key, and each
//! accepts only keys its project roster (`wugs_auth`) lists unrevoked.
//! Before any sync message the two sides swap rosters (`{"t":"roster"}`
//! frames) and keep the union of valid records, so a revocation reaches a
//! peer before any data does, and a peer revoked by that union is refused
//! before hello. Then what each side sends depends on who the peer is
//! (`PeerPolicy`): members get every scope, guests get open scopes only and
//! are never pulled from. A client that does not speak TLS gets no wugs
//! frame at all, and one that negotiates no `wugs/3` ALPN is refused right
//! after the handshake.
//!
//! Only members contribute to the roster exchange: a guest receives the
//! roster but nothing it sends is merged (any member relays legitimate
//! records soon enough), and a merge stores only records signed by
//! enrolled members, so no peer can grow another's roster with records
//! that could never count.
//!
//! A `dvc_…` id attested to more than one device key never refuses a
//! session. Two keys on one `dvc` usually means a project folder was copied
//! between machines, so two devices would write one log file; it can also
//! be another member's slip. Either way the session goes ahead and carries
//! a `Warning`, and the shell offers to mint a fresh device identity for
//! the copied folder. The hazard is surfaced, not blocked, so a mistaken or
//! hostile attest can never lock a device out.
//!
//! `serve_open`, `serve_open_with` and `sync_open` are the old
//! unauthenticated sessions. They exist for tests and development only:
//! anyone who can reach the port can sync whatever the policy permits.

pub mod discover;
pub mod frame;
pub mod tls;

use std::fmt;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::{ClientConnection, ServerConnection, StreamOwned};
use wugs_auth::{Body, DeviceKey, MergeError, PublicKey, Roster, RosterFile, RosterState, Standing};
use wugs_core::Project;
use wugs_model::Value;
use wugs_sync::{policy_from_access, Requester, Responder, SyncPolicy, Transfer};

use frame::{read_frame, write_frame, FrameError};

/// Version of the session around the wire protocol: TLS, ALPN and the
/// roster prelude. Version 1 was plain TCP.
pub const NET_VERSION: f64 = 2.0;

/// Writes and connects fail fast; reads wait out the peer's turn, because
/// the other side may be ingesting a large batch or walking a long log
/// before its next frame. These reset on every byte, so the `Watchdog`
/// deadlines below are what bound a stalling peer.
pub const IO_TIMEOUT: Duration = Duration::from_secs(10);
pub const TURN_TIMEOUT: Duration = Duration::from_secs(120);

/// From TCP connect to the end of the roster prelude. Until then the peer
/// is not known to be enrolled, so it gets only this long.
pub const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(15);
/// From TCP connect to the end of the whole session.
pub const SESSION_DEADLINE: Duration = Duration::from_secs(5 * 60);

/// A wall-clock limit on one connection: when it passes, the socket is
/// shut down, which ends any read or write blocked on it. Disarmed on drop.
pub struct Watchdog {
    start: Instant,
    deadline: mpsc::Sender<Instant>,
    fired: Arc<AtomicBool>,
}

impl Watchdog {
    pub fn arm(tcp: &TcpStream, within: Duration) -> Result<Watchdog, String> {
        let sock = tcp.try_clone().map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel::<Instant>();
        let fired = Arc::new(AtomicBool::new(false));
        let flag = fired.clone();
        let start = Instant::now();
        let mut deadline = start + within;
        std::thread::Builder::new()
            .name("wugs-net-watchdog".into())
            .spawn(move || loop {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(next) => deadline = next,
                    Err(RecvTimeoutError::Disconnected) => return,
                    Err(RecvTimeoutError::Timeout) if Instant::now() >= deadline => {
                        flag.store(true, Ordering::SeqCst);
                        sock.shutdown(Shutdown::Both).ok();
                        return;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Watchdog { start, deadline: tx, fired })
    }

    /// Move the limit to `total` after the connection was armed.
    pub fn allow(&self, total: Duration) {
        self.deadline.send(self.start + total).ok();
    }

    /// The limit passed and the socket was shut down.
    pub fn fired(&self) -> bool {
        self.fired.load(Ordering::SeqCst)
    }

    fn explain(&self, h: Halt) -> Halt {
        if self.fired() {
            Halt::Failed(format!("the connection was cut after {} s, its time limit", self.start.elapsed().as_secs()))
        } else {
            h
        }
    }
}

/// Wait until the peer has sent something, without consuming it. Lets an
/// accept loop keep a silent connection away from the single-writer
/// project. Leaves the read timeout unset.
pub fn wait_for_first_byte(tcp: &TcpStream, within: Duration) -> Result<(), String> {
    tcp.set_read_timeout(Some(within)).map_err(|e| e.to_string())?;
    let got = tcp.peek(&mut [0u8; 1]);
    tcp.set_read_timeout(None).map_err(|e| e.to_string())?;
    match got {
        Ok(0) => Err("the peer closed the connection without sending anything".into()),
        Ok(_) => Ok(()),
        Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
            Err(format!("the peer sent nothing within {} s", within.as_secs_f32()))
        }
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Member,
    Guest,
}

/// Something a session found that a person should see; it never stops the sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// The peer's `dvc_…` id is attested to another device key too.
    PeerDeviceIdClaimed { dvc: String, other: PublicKey },
    /// This device's own `dvc_…` id is attested to another device key too.
    OwnDeviceIdClaimed { dvc: String, other: PublicKey },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (whose, dvc, other) = match self {
            Warning::PeerDeviceIdClaimed { dvc, other } => ("the other device's", dvc, other),
            Warning::OwnDeviceIdClaimed { dvc, other } => ("this device's", dvc, other),
        };
        write!(
            f,
            "{whose} device id {dvc} is also attested to device key {other}; this project folder may be a copy of another machine's, so create a fresh device identity for it"
        )
    }
}

/// The authenticated device on the other end of a session.
#[derive(Debug, Clone)]
pub struct Peer {
    pub key: PublicKey,
    pub role: Role,
    pub label: String,
    /// The `dvc_…` id the peer says its project folder carries.
    pub device: String,
}

/// What this side sends to a given peer. Rebuilt every session, because a
/// policy is a snapshot: a restriction that lands in session N must bind
/// in session N+1.
pub type PeerPolicy<'a> = &'a dyn Fn(&Project, &Peer) -> Result<SyncPolicy, String>;

/// Members see everything (DECISIONS 2026-11-08); guests see the scopes
/// whose access is open.
pub fn default_policy(project: &Project, peer: &Peer) -> Result<SyncPolicy, String> {
    match peer.role {
        Role::Member => Ok(SyncPolicy::allow_all()),
        Role::Guest => policy_from_access(&project.proj, &[]),
    }
}

/// Why an authenticated session was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// TLS failed: the peer's key is not on the roster, it offered another
    /// session version, or the bytes were not TLS at all.
    Handshake(String),
    /// The handshake finished without agreeing on the `wugs/3` ALPN (the
    /// peer offered none, or another); holds what was negotiated, if anything.
    SessionVersion(Option<String>),
    /// This device is not on its own project roster (never paired).
    NotEnrolled,
    /// This device has been marked lost.
    SelfRevoked,
    /// The peer's key was revoked by a record the roster exchange brought in.
    PeerRevoked(PublicKey),
    /// The peer's roster starts with another genesis, or carries more
    /// records than any real project has.
    Roster(MergeError),
    /// A guest asked to be pulled from; guests are read-only.
    GuestPush,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Handshake(m) => write!(f, "tls handshake failed: {m}"),
            Refusal::NotEnrolled => write!(f, "this device is not enrolled in the project; pair it first"),
            Refusal::SelfRevoked => write!(f, "this device has been marked lost and can no longer sync"),
            Refusal::PeerRevoked(k) => write!(f, "device {k} has been marked lost"),
            Refusal::SessionVersion(got) => {
                write!(f, "the peer did not agree on session version wugs/3 (negotiated {})", got.as_deref().unwrap_or("nothing"))
            }
            Refusal::Roster(e) => write!(f, "{e}"),
            Refusal::GuestPush => write!(f, "guest devices are read-only; nothing is pulled from them"),
        }
    }
}

#[derive(Debug)]
pub struct ServeReport {
    pub sessions: Vec<SessionReport>,
}

#[derive(Debug)]
pub struct SessionReport {
    pub peer_addr: Option<SocketAddr>,
    /// The authenticated peer; `None` for open sessions and refused handshakes.
    pub peer: Option<Peer>,
    /// Requests the server answered during the peer's pull.
    pub answered: usize,
    pub warnings: Vec<Warning>,
    pub outcome: SessionOutcome,
}

#[derive(Debug)]
pub enum SessionOutcome {
    /// Both pulls ran (or the one a guest is allowed); this is what the
    /// server's own pull brought in.
    Converged(Transfer),
    /// The peer hung up before the session finished. Whatever it pulled
    /// is already ingested on its side, so this is a partial sync, not a fault.
    PeerLeft,
    Refused(Refusal),
    Failed(String),
}

#[derive(Debug)]
pub struct ClientReport {
    /// What the client's pull brought in.
    pub pulled: Transfer,
    /// Requests the client answered during the server's reverse pull.
    pub answered: usize,
    pub peer: Option<Peer>,
    pub warnings: Vec<Warning>,
}

enum Halt {
    PeerLeft,
    Refused(Refusal),
    Failed(String),
}

impl From<FrameError> for Halt {
    fn from(e: FrameError) -> Self {
        match e {
            FrameError::Closed => Halt::PeerLeft,
            other => Halt::Failed(other.to_string()),
        }
    }
}

impl Halt {
    fn into_message(self) -> String {
        match self {
            Halt::PeerLeft => "peer closed the connection before the session finished".into(),
            Halt::Refused(r) => r.to_string(),
            Halt::Failed(m) => m,
        }
    }

    fn into_outcome(self) -> SessionOutcome {
        match self {
            Halt::PeerLeft => SessionOutcome::PeerLeft,
            Halt::Refused(r) => SessionOutcome::Refused(r),
            Halt::Failed(m) => SessionOutcome::Failed(m),
        }
    }
}

fn control(t: &str) -> Value {
    Value::obj(vec![("t", Value::str(t))])
}

fn send_error(s: &mut impl Write, msg: &str) {
    write_frame(s, &Value::obj(vec![("t", Value::str("error")), ("error", Value::str(msg))])).ok();
}

fn refuse(s: &mut impl Write, r: Refusal) -> Halt {
    send_error(s, &r.to_string());
    Halt::Refused(r)
}

fn peer_error(msg: &Value) -> Option<String> {
    (msg.get_str("t") == Some("error"))
        .then(|| format!("peer refused: {}", msg.get_str("error").unwrap_or("no reason given")))
}

fn configure(s: &TcpStream) -> Result<(), String> {
    s.set_read_timeout(Some(TURN_TIMEOUT)).map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(IO_TIMEOUT)).map_err(|e| e.to_string())?;
    s.set_nodelay(true).map_err(|e| e.to_string())
}

/// Answer the peer's pull until one of `until` arrives, and say which.
/// Each message re-borrows the project immutably, so the caller's `&mut`
/// is free again afterwards.
fn answer<'u>(project: &Project, policy: &SyncPolicy, s: &mut (impl Read + Write), until: &[&'u str], answered: &mut usize) -> Result<&'u str, Halt> {
    let mut responder = Responder::new(project, policy);
    loop {
        let msg = read_frame(s)?;
        if let Some(end) = until.iter().find(|u| msg.get_str("t") == Some(u)) {
            return Ok(end);
        }
        if let Some(e) = peer_error(&msg) {
            return Err(Halt::Failed(e));
        }
        match responder.handle(&msg) {
            Ok(resp) => write_frame(s, &resp)?,
            Err(e) => {
                send_error(s, &e);
                return Err(Halt::Failed(e));
            }
        }
        *answered += 1;
    }
}

/// Run one pull against the peer's responder over the stream.
fn drive_pull(project: &mut Project, s: &mut (impl Read + Write)) -> Result<Transfer, Halt> {
    let mut req = Requester::new(&project.project_id.clone());
    while let Some(msg) = req.request() {
        write_frame(s, &msg)?;
        let resp = read_frame(s)?;
        if let Some(e) = peer_error(&resp) {
            return Err(Halt::Failed(e));
        }
        if let Err(e) = req.respond(project, &resp) {
            send_error(s, &e);
            return Err(Halt::Failed(e));
        }
    }
    Ok(Transfer {
        windows: req.windows_wanted(),
        ops: req.ops_received,
        ops_by_scope: req.ops_by_scope.clone(),
        ingest: req.report.unwrap_or_default(),
    })
}

/// The server's half once the peer is known: answer its pull, then pull
/// from it, unless it is a guest, which is never pulled from.
fn server_turns(project: &mut Project, policy: &SyncPolicy, s: &mut (impl Read + Write), peer: Role, answered: &mut usize) -> Result<Transfer, Halt> {
    match (answer(project, policy, s, &["pull_me", "done"], answered)?, peer) {
        ("done", _) => Ok(Transfer::default()),
        (_, Role::Guest) => Err(refuse(s, Refusal::GuestPush)),
        (_, Role::Member) => {
            let pulled = drive_pull(project, s)?;
            write_frame(s, &control("done"))?;
            Ok(pulled)
        }
    }
}

/// The client's half: pull from the server unless it is a guest, then
/// offer the reverse pull unless this device is a guest.
fn client_turns(project: &mut Project, s: &mut (impl Read + Write), policy: &SyncPolicy, me: Role, peer: Role) -> Result<(Transfer, usize), Halt> {
    let pulled = match peer {
        Role::Member => drive_pull(project, s)?,
        Role::Guest => Transfer::default(),
    };
    let mut answered = 0;
    match me {
        Role::Member => {
            write_frame(s, &control("pull_me"))?;
            answer(project, policy, s, &["done"], &mut answered)?;
        }
        Role::Guest => write_frame(s, &control("done"))?,
    }
    Ok((pulled, answered))
}

// ------------------------------------------------------------ authentication

/// This device's roster, checked and attested before a session: it must
/// be enrolled and unrevoked, and a member that has not attested its `dvc`
/// does so now.
fn prepare(project: &Project, key: &DeviceKey) -> Result<(Roster, Role), Halt> {
    let file = RosterFile::in_project(&project.root);
    if !file.exists() {
        return Err(Halt::Refused(Refusal::NotEnrolled));
    }
    let mut roster = file.load(&project.project_id).map_err(Halt::Failed)?;
    let st = roster.state();
    let me = key.public();
    let role = own_role(&st, &me)?;
    let dvc = project.device();
    let attested = st.attested.get(dvc).is_some_and(|ks| ks.contains(&me));
    if role == Role::Member && !attested {
        let rec = roster.append(key, Body::Attest { key: me, dvc: dvc.to_string() }).map_err(Halt::Failed)?;
        file.append(&[rec]).map_err(Halt::Failed)?;
    }
    Ok((roster, role))
}

fn own_role(st: &RosterState, me: &PublicKey) -> Result<Role, Halt> {
    match st.standing(me) {
        Standing::Member => Ok(Role::Member),
        Standing::Guest => Ok(Role::Guest),
        Standing::Revoked => Err(Halt::Refused(Refusal::SelfRevoked)),
        Standing::Unknown => Err(Halt::Refused(Refusal::NotEnrolled)),
    }
}

/// Keys the TLS verifier accepts: on the roster and not revoked.
fn admitted(st: &RosterState) -> std::collections::BTreeSet<PublicKey> {
    st.devices.keys().filter(|k| !st.revoked.contains_key(*k)).copied().collect()
}

fn roster_frame(project: &Project, roster: &Roster) -> Value {
    Value::obj(vec![
        ("t", Value::str("roster")),
        ("v", Value::Num(NET_VERSION)),
        ("device", Value::str(project.device())),
        ("records", Value::Arr(roster.lines().iter().map(|l| Value::str(l)).collect())),
    ])
}

/// Read the peer's roster frame and, if the peer is a member of our roster
/// as it stands before the merge, merge it into ours (and our file).
fn take_roster(project: &Project, roster: &mut Roster, peer_key: &PublicKey, s: &mut (impl Read + Write)) -> Result<String, Halt> {
    let msg = read_frame(s)?;
    if let Some(e) = peer_error(&msg) {
        return Err(Halt::Failed(e));
    }
    if msg.get_str("t") != Some("roster") || msg.get("v").and_then(Value::as_f64) != Some(NET_VERSION) {
        return Err(Halt::Failed("expected a roster frame of this session version".into()));
    }
    let lines: Vec<String> = match msg.get("records") {
        Some(Value::Arr(a)) => a.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        _ => return Err(Halt::Failed("roster frame without records".into())),
    };
    if roster.state().standing(peer_key) == Standing::Member {
        let merged = roster.merge_peer(&lines).map_err(|e| refuse(s, Refusal::Roster(e)))?;
        RosterFile::in_project(&project.root).append(&merged.added).map_err(Halt::Failed)?;
    }
    Ok(msg.get_str("device").unwrap_or_default().to_string())
}

/// After the merge: both ends must still stand. A `dvc` id attested to
/// another key, the peer's or our own, is a warning.
fn judge(roster: &Roster, me: &PublicKey, own_device: &str, peer_key: PublicKey, device: String) -> Result<(Peer, Vec<Warning>), Refusal> {
    let st = roster.state();
    match st.standing(me) {
        Standing::Member | Standing::Guest => {}
        Standing::Revoked => return Err(Refusal::SelfRevoked),
        Standing::Unknown => return Err(Refusal::NotEnrolled),
    }
    let role = match st.standing(&peer_key) {
        Standing::Member => Role::Member,
        Standing::Guest => Role::Guest,
        _ => return Err(Refusal::PeerRevoked(peer_key)),
    };
    let mut warnings = Vec::new();
    if let Some(other) = st.dvc_claimed_by_other(&device, &peer_key) {
        warnings.push(Warning::PeerDeviceIdClaimed { dvc: device.clone(), other });
    }
    if let Some(other) = st.dvc_claimed_by_other(own_device, me) {
        warnings.push(Warning::OwnDeviceIdClaimed { dvc: own_device.to_string(), other });
    }
    let label = st.devices.get(&peer_key).map(|d| d.label.clone()).unwrap_or_default();
    Ok((Peer { key: peer_key, role, label, device }, warnings))
}

fn handshake<C, S>(tls: &mut StreamOwned<C, TcpStream>) -> Result<(), Halt>
where
    StreamOwned<C, TcpStream>: Read + Write,
    C: std::ops::DerefMut<Target = rustls::ConnectionCommon<S>>,
    S: rustls::SideData,
{
    while tls.conn.is_handshaking() {
        tls.conn.complete_io(&mut tls.sock).map_err(|e| Halt::Refused(Refusal::Handshake(e.to_string())))?;
    }
    match tls.conn.alpn_protocol() {
        Some(p) if p == tls::ALPN => Ok(()),
        other => Err(refuse(tls, Refusal::SessionVersion(other.map(|p| String::from_utf8_lossy(p).into_owned())))),
    }
}

pub type ServerStream = StreamOwned<ServerConnection, TcpStream>;
pub type ClientStream = StreamOwned<ClientConnection, TcpStream>;

/// An authenticated connection whose roster prelude is done; the next frame
/// is the client's hello. `sync_with` drives one; tests and tools can use
/// `stream` directly to speak the wire protocol.
/// It holds the connection's `Watchdog`, so the stream is shut down once
/// `SESSION_DEADLINE` passes.
pub struct Session<S> {
    pub stream: S,
    pub peer: Peer,
    /// This device's own role in the merged roster.
    pub role: Role,
    pub warnings: Vec<Warning>,
    watchdog: Watchdog,
}

/// Accept one authenticated session on an accepted TCP stream.
fn accept(project: &Project, key: &DeviceKey, tcp: TcpStream, deadlines: Deadlines) -> Result<Session<ServerStream>, Halt> {
    let watchdog = Watchdog::arm(&tcp, deadlines.handshake).map_err(Halt::Failed)?;
    accept_within(project, key, tcp, deadlines.session, &watchdog)
        .map_err(|h| watchdog.explain(h))
        .map(|(stream, peer, role, warnings)| Session { stream, peer, role, warnings, watchdog })
}

type Accepted<S> = (S, Peer, Role, Vec<Warning>);

fn accept_within(project: &Project, key: &DeviceKey, tcp: TcpStream, session: Duration, watchdog: &Watchdog) -> Result<Accepted<ServerStream>, Halt> {
    configure(&tcp).map_err(Halt::Failed)?;
    let (mut roster, _) = prepare(project, key)?;
    let config = tls::server_config(key, admitted(&roster.state())).map_err(Halt::Failed)?;
    let conn = ServerConnection::new(config).map_err(|e| Halt::Failed(e.to_string()))?;
    let mut s = StreamOwned::new(conn, tcp);
    handshake(&mut s)?;
    let peer_key = tls::peer_key(s.conn.peer_certificates()).map_err(Halt::Failed)?;
    let device = take_roster(project, &mut roster, &peer_key, &mut s)?;
    let (peer, warnings) = judge(&roster, &key.public(), project.device(), peer_key, device).map_err(|r| refuse(&mut s, r))?;
    write_frame(&mut s, &roster_frame(project, &roster))?;
    let role = own_role(&roster.state(), &key.public())?;
    watchdog.allow(session);
    Ok((s, peer, role, warnings))
}

/// Open an authenticated session to `addr`: TLS, then the roster prelude.
pub fn connect(project: &Project, addr: SocketAddr, key: &DeviceKey) -> Result<Session<ClientStream>, String> {
    connect_inner(project, addr, key).map_err(Halt::into_message)
}

fn connect_inner(project: &Project, addr: SocketAddr, key: &DeviceKey) -> Result<Session<ClientStream>, Halt> {
    let (roster, _) = prepare(project, key)?;
    let tcp = TcpStream::connect_timeout(&addr, IO_TIMEOUT).map_err(|e| Halt::Failed(format!("connect {addr}: {e}")))?;
    let watchdog = Watchdog::arm(&tcp, HANDSHAKE_DEADLINE).map_err(Halt::Failed)?;
    connect_within(project, key, roster, tcp, &watchdog)
        .map_err(|h| watchdog.explain(h))
        .map(|(stream, peer, role, warnings)| Session { stream, peer, role, warnings, watchdog })
}

fn connect_within(project: &Project, key: &DeviceKey, mut roster: Roster, tcp: TcpStream, watchdog: &Watchdog) -> Result<Accepted<ClientStream>, Halt> {
    configure(&tcp).map_err(Halt::Failed)?;
    let config = tls::client_config(key, admitted(&roster.state())).map_err(Halt::Failed)?;
    let conn = ClientConnection::new(config, tls::server_name()).map_err(|e| Halt::Failed(e.to_string()))?;
    let mut s = StreamOwned::new(conn, tcp);
    handshake(&mut s)?;
    let peer_key = tls::peer_key(s.conn.peer_certificates()).map_err(Halt::Failed)?;
    write_frame(&mut s, &roster_frame(project, &roster))?;
    let device = take_roster(project, &mut roster, &peer_key, &mut s)?;
    let (peer, warnings) = judge(&roster, &key.public(), project.device(), peer_key, device).map_err(|r| refuse(&mut s, r))?;
    let role = own_role(&roster.state(), &key.public())?;
    watchdog.allow(SESSION_DEADLINE);
    Ok((s, peer, role, warnings))
}

/// Serve `sessions` authenticated connections, one after another. The
/// roster is reloaded before each, so what the last session brought in
/// (a revocation, a new member) binds the next. A failed or refused
/// session is recorded and the next one accepted; only a failing listener
/// is an error.
pub fn serve(project: &mut Project, listener: TcpListener, key: &DeviceKey, sessions: usize, policy: PeerPolicy) -> Result<ServeReport, String> {
    let mut report = ServeReport { sessions: Vec::with_capacity(sessions) };
    for _ in 0..sessions {
        let (tcp, _) = listener.accept().map_err(|e| format!("accept: {e}"))?;
        report.sessions.push(serve_stream(project, tcp, key, policy));
    }
    Ok(report)
}

#[derive(Debug, Clone, Copy)]
struct Deadlines {
    handshake: Duration,
    session: Duration,
}

/// Serve one authenticated session on a connection the caller accepted,
/// within `HANDSHAKE_DEADLINE` and `SESSION_DEADLINE`. Every failure is in
/// the report's outcome, never an `Err`: the caller's accept loop goes on.
pub fn serve_stream(project: &mut Project, tcp: TcpStream, key: &DeviceKey, policy: PeerPolicy) -> SessionReport {
    serve_stream_within(project, tcp, key, policy, Deadlines { handshake: HANDSHAKE_DEADLINE, session: SESSION_DEADLINE })
}

fn serve_stream_within(project: &mut Project, tcp: TcpStream, key: &DeviceKey, policy: PeerPolicy, deadlines: Deadlines) -> SessionReport {
    let peer_addr = tcp.peer_addr().ok();
    let mut answered = 0;
    let (peer, warnings, outcome) = match accept(project, key, tcp, deadlines) {
        Err(h) => (None, Vec::new(), h.into_outcome()),
        Ok(mut sess) => {
            let outcome = policy(project, &sess.peer)
                .map_err(Halt::Failed)
                .and_then(|p| server_turns(project, &p, &mut sess.stream, sess.peer.role, &mut answered))
                .map_err(|h| sess.watchdog.explain(h))
                .map_or_else(Halt::into_outcome, SessionOutcome::Converged);
            (Some(sess.peer), sess.warnings, outcome)
        }
    };
    SessionReport { peer_addr, peer, answered, warnings, outcome }
}

/// One full authenticated session as the client.
pub fn sync_with(project: &mut Project, addr: SocketAddr, key: &DeviceKey, policy: PeerPolicy) -> Result<ClientReport, String> {
    let mut sess = connect(project, addr, key)?;
    let p = policy(project, &sess.peer)?;
    let (pulled, answered) = client_turns(project, &mut sess.stream, &p, sess.role, sess.peer.role)
        .map_err(|h| sess.watchdog.explain(h).into_message())?;
    Ok(ClientReport { pulled, answered, peer: Some(sess.peer), warnings: sess.warnings })
}

// --------------------------------------------------- unauthenticated (dev)

/// Unauthenticated: test and development use only. Serve `sessions`
/// plain-TCP connections with one fixed policy.
pub fn serve_open(project: &mut Project, listener: TcpListener, policy: &SyncPolicy, sessions: usize) -> Result<ServeReport, String> {
    serve_open_with(project, listener, sessions, &|_| Ok(policy.clone()))
}

/// Unauthenticated: test and development use only. Like `serve_open`, but
/// the policy is rebuilt before every session.
pub fn serve_open_with(
    project: &mut Project,
    listener: TcpListener,
    sessions: usize,
    policy: &dyn Fn(&Project) -> Result<SyncPolicy, String>,
) -> Result<ServeReport, String> {
    let mut report = ServeReport { sessions: Vec::with_capacity(sessions) };
    for _ in 0..sessions {
        let (mut s, addr) = listener.accept().map_err(|e| format!("accept: {e}"))?;
        let p = policy(project)?;
        let mut answered = 0;
        let outcome = configure(&s)
            .map_err(Halt::Failed)
            .and_then(|_| server_turns(project, &p, &mut s, Role::Member, &mut answered))
            .map_or_else(Halt::into_outcome, SessionOutcome::Converged);
        report.sessions.push(SessionReport { peer_addr: Some(addr), peer: None, answered, warnings: Vec::new(), outcome });
    }
    Ok(report)
}

/// Unauthenticated: test and development use only. One plain-TCP session
/// as the client.
pub fn sync_open(project: &mut Project, addr: SocketAddr, policy: &SyncPolicy) -> Result<ClientReport, String> {
    let mut s = TcpStream::connect_timeout(&addr, IO_TIMEOUT).map_err(|e| format!("connect {addr}: {e}"))?;
    configure(&s)?;
    let (pulled, answered) = client_turns(project, &mut s, policy, Role::Member, Role::Member).map_err(Halt::into_message)?;
    Ok(ClientReport { pulled, answered, peer: None, warnings: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_peer_dripping_bytes_is_cut_at_the_handshake_deadline() {
        let root = std::env::temp_dir().join(format!("wugs-net-drip-{}", wugs_model::ulid()));
        let mut project = Project::open(&root, "prs_A").unwrap();
        let key = DeviceKey::generate();
        RosterFile::in_project(&root).create(&Roster::genesis(&project.project_id, &key, "host")).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let drip = std::thread::spawn(move || {
            let mut s = TcpStream::connect(addr).unwrap();
            // A TLS record header, then one byte at a time, each well inside
            // the read timeout: only a wall clock ends this.
            for b in [0x16u8, 0x03, 0x01, 0x40, 0x00].iter().cycle().take(400) {
                if s.write_all(&[*b]).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        });
        let (tcp, _) = listener.accept().unwrap();
        let started = Instant::now();
        let deadlines = Deadlines { handshake: Duration::from_millis(400), session: Duration::from_secs(60) };
        let report = serve_stream_within(&mut project, tcp, &key, &default_policy, deadlines);
        let took = started.elapsed();
        assert!(took < Duration::from_secs(5), "the drip held the session for {took:?}");
        match &report.outcome {
            SessionOutcome::Refused(Refusal::Handshake(_)) | SessionOutcome::Failed(_) => {}
            other => panic!("expected a cut session, got {other:?}"),
        }
        drip.join().unwrap();
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_silent_peer_never_gets_past_the_first_byte_wait() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let silent = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (tcp, _) = listener.accept().unwrap();
        let started = Instant::now();
        assert!(wait_for_first_byte(&tcp, Duration::from_millis(200)).is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        let mut talker = silent;
        talker.write_all(b"x").unwrap();
        wait_for_first_byte(&tcp, Duration::from_secs(2)).unwrap();
        let mut b = [0u8; 1];
        (&tcp).read_exact(&mut b).unwrap();
        assert_eq!(&b, b"x", "the wait does not consume the byte");
    }
}
