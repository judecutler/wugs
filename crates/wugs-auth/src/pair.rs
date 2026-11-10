//! Pairing: a member's device shows an 8-digit code, the new device types
//! it, and CPace turns the shared code into a strong session key that an
//! eavesdropper or a wrong guess learns nothing from. The joiner sends its
//! public key under that key; the host signs its enrol record and sends
//! back the whole roster. A code is single-use: any attempt burns it, so a
//! guesser gets one try in 10^8 per code shown.
//!
//! Messages over the channel, each a u32 little-endian length plus bytes:
//!
//! ```text
//! joiner -> host   CPace message (session id || Y_a)
//! host   -> joiner CPace message Y_b, then host's confirmation tag
//! joiner -> host   joiner's confirmation tag
//! joiner -> host   sealed {key, label}            (only if the host's tag verified)
//! host   -> joiner sealed {project, roster lines}
//! ```
//!
//! The joiner sends its tag before checking the host's, so both sides learn
//! of a wrong code the same way: the host's tag check fails and it reports
//! `WrongCode`, the joiner's fails and it reports `WrongCode`.

use std::fmt;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use pake_cpace::{ChannelIdentifier, CPace, NoAD, SessionAD, SESSION_ID_BYTES};
use sha2::{Digest, Sha512};
use wugs_model::{json, Value};
use zeroize::Zeroizing;

use crate::key::{DeviceKey, PublicKey};
use crate::roster::{Roster, SignedRecord, Standing};

pub const CODE_DIGITS: usize = 8;
const MAX_MESSAGE: usize = 4 * 1024 * 1024;
const CHANNEL: ChannelIdentifier<'static> = ChannelIdentifier::identities_only(b"wugs-pair-joiner", b"wugs-pair-host");
const AAD: &[u8] = b"wugs-pair-v1";

#[derive(Debug)]
pub enum PairError {
    /// The two sides typed different codes, or someone in between tampered.
    WrongCode,
    Expired,
    /// The code was already tried once.
    AlreadyUsed,
    /// What the user typed is not 8 digits.
    BadCode,
    /// The other side hung up mid-pairing.
    Closed,
    Protocol(String),
    Io(io::Error),
}

impl fmt::Display for PairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PairError::WrongCode => write!(f, "the pairing code did not match"),
            PairError::Expired => write!(f, "the pairing code has expired"),
            PairError::AlreadyUsed => write!(f, "the pairing code was already used; show a new one"),
            PairError::BadCode => write!(f, "a pairing code is {CODE_DIGITS} digits"),
            PairError::Closed => write!(f, "the other device closed the connection"),
            PairError::Protocol(m) => write!(f, "pairing protocol error: {m}"),
            PairError::Io(e) => write!(f, "i/o error: {e}"),
        }
    }
}

impl From<io::Error> for PairError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe => PairError::Closed,
            _ => PairError::Io(e),
        }
    }
}

/// A code shown on the host. It expires by the host's monotonic clock
/// (`Instant`), never a wall clock, and is burned by the first attempt.
pub struct PairingCode {
    digits: Zeroizing<String>,
    expires: Instant,
    used: bool,
}

impl PairingCode {
    pub fn generate(ttl: Duration) -> PairingCode {
        // Rejection sampling keeps all 10^8 codes equally likely.
        let n = loop {
            let mut b = [0u8; 4];
            getrandom::getrandom(&mut b).expect("OS randomness");
            let n = u32::from_le_bytes(b);
            if n < 4_200_000_000 {
                break n % 100_000_000;
            }
        };
        PairingCode { digits: Zeroizing::new(format!("{n:08}")), expires: Instant::now() + ttl, used: false }
    }

    pub fn digits(&self) -> &str {
        &self.digits
    }

    pub fn is_live(&self) -> bool {
        !self.used && Instant::now() < self.expires
    }
}

/// Accepts "1234 5678" or "1234-5678" as typed.
fn normalize(code: &str) -> Result<Zeroizing<String>, PairError> {
    let digits: Zeroizing<String> = Zeroizing::new(code.chars().filter(|c| !matches!(c, ' ' | '-')).collect());
    if digits.len() != CODE_DIGITS || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PairError::BadCode);
    }
    Ok(digits)
}

fn send(ch: &mut impl Write, bytes: &[u8]) -> Result<(), PairError> {
    ch.write_all(&(bytes.len() as u32).to_le_bytes())?;
    ch.write_all(bytes)?;
    ch.flush()?;
    Ok(())
}

fn recv(ch: &mut impl Read) -> Result<Vec<u8>, PairError> {
    let mut len = [0u8; 4];
    ch.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_MESSAGE {
        return Err(PairError::Protocol(format!("message of {len} bytes")));
    }
    let mut buf = vec![0u8; len];
    ch.read_exact(&mut buf)?;
    Ok(buf)
}

/// Two one-message keys from the CPace session key, one per direction, so
/// the fixed nonce never repeats under a key. Each is derived by hashing a
/// direction label with the session key, so the two keys are independent.
struct Seal {
    to_host: Zeroizing<[u8; 32]>,
    to_joiner: Zeroizing<[u8; 32]>,
}

impl Seal {
    fn new(isk: Zeroizing<[u8; 64]>) -> Seal {
        let derive = |label: &[u8]| {
            let mut out = Zeroizing::new([0u8; 32]);
            out.copy_from_slice(&Sha512::new().chain_update(label).chain_update(&isk[..]).finalize()[..32]);
            out
        };
        Seal { to_host: derive(b"wugs-pair-host\0"), to_joiner: derive(b"wugs-pair-join\0") }
    }

    fn seal(key: &[u8; 32], msg: &Value) -> Vec<u8> {
        let body = msg.to_json();
        ChaCha20Poly1305::new(Key::from_slice(key))
            .encrypt(Nonce::from_slice(&[0u8; 12]), Payload { msg: body.as_bytes(), aad: AAD })
            .expect("encryption of an in-memory buffer cannot fail")
    }

    fn open(key: &[u8; 32], sealed: &[u8]) -> Result<Value, PairError> {
        let plain = ChaCha20Poly1305::new(Key::from_slice(key))
            .decrypt(Nonce::from_slice(&[0u8; 12]), Payload { msg: sealed, aad: AAD })
            .map_err(|_| PairError::Protocol("sealed payload failed to authenticate".into()))?;
        let text = String::from_utf8(plain).map_err(|_| PairError::Protocol("payload is not UTF-8".into()))?;
        json::parse(&text).map_err(|e| PairError::Protocol(format!("payload is not JSON: {e}")))
    }
}

fn cpace_err(e: impl fmt::Debug) -> PairError {
    PairError::Protocol(format!("cpace: {e:?}"))
}

/// The host side. On success the new device's enrol record is already in
/// `roster`; the caller persists it (`RosterFile::append`). `guest` is the
/// host user's choice of what the new device may do.
/// `persist` runs between signing the enrolment and sending it to the
/// joiner, and must make the record durable (append it to the roster
/// file). A fork in a signer's chain is fatal (equivocation fails
/// closed), so the host must never hand out a record it could forget in
/// a crash: persisted-then-sent means a crash leaves at worst an
/// enrolled device that never heard back, which is harmless.
pub fn pair_host(
    ch: &mut (impl Read + Write),
    code: &mut PairingCode,
    host: &DeviceKey,
    roster: &mut Roster,
    guest: bool,
    persist: &mut dyn FnMut(&SignedRecord) -> Result<(), String>,
) -> Result<SignedRecord, PairError> {
    if code.used {
        return Err(PairError::AlreadyUsed);
    }
    code.used = true;
    if Instant::now() >= code.expires {
        return Err(PairError::Expired);
    }
    let msg_a = recv(ch)?;
    let (processed, ad) = CPace::responder_process_message::<SessionAD>(msg_a).map_err(cpace_err)?;
    let ctx = CPace::new(&ad.session_id, code.digits.as_bytes(), &CHANNEL, &NoAD {}).map_err(cpace_err)?;
    let negotiation = processed.finish(ctx).map_err(cpace_err)?;
    send(ch, negotiation.own_message())?;
    send(ch, &negotiation.tag())?;
    let tag_a = recv(ch)?;
    let seal = Seal::new(Zeroizing::new(negotiation.verify_tag(&tag_a).map_err(|_| PairError::WrongCode)?));

    let req = Seal::open(&seal.to_host, &recv(ch)?)?;
    let key = PublicKey::from_hex(req.get_str("key").unwrap_or_default()).map_err(PairError::Protocol)?;
    let label = req.get_str("label").unwrap_or("unnamed device");
    let rec = roster.enrol(host, key, label, guest).map_err(PairError::Protocol)?;
    persist(&rec).map_err(|e| PairError::Protocol(format!("persist before send: {e}")))?;
    let resp = Value::obj(vec![
        ("project", Value::str(roster.project())),
        ("roster", Value::Arr(roster.lines().iter().map(|l| Value::str(l)).collect())),
    ]);
    send(ch, &Seal::seal(&seal.to_joiner, &resp))?;
    Ok(rec)
}

pub struct Joined {
    pub project_id: String,
    pub roster: Roster,
    pub standing: Standing,
}

/// The joining side. `key` is this installation's existing device key (one
/// per installation, shared across projects), so pairing never mints one.
/// The caller writes `roster` to the new project folder (`RosterFile::create`)
/// and opens the project with `Project::open_joined(.., &project_id)`.
pub fn pair_join(ch: &mut (impl Read + Write), code: &str, key: &DeviceKey, label: &str) -> Result<Joined, PairError> {
    let code = normalize(code)?;
    let mut session_id = [0u8; SESSION_ID_BYTES];
    getrandom::getrandom(&mut session_id).map_err(|e| PairError::Protocol(e.to_string()))?;
    let ctx = CPace::new(&session_id, code.as_bytes(), &CHANNEL, &SessionAD { session_id }).map_err(cpace_err)?;
    send(ch, ctx.message())?;
    let msg_b = recv(ch)?;
    let tag_b = recv(ch)?;
    let (negotiation, _) = ctx.initiator_process_message::<NoAD>(msg_b).map_err(cpace_err)?;
    send(ch, &negotiation.tag())?;
    let seal = Seal::new(Zeroizing::new(negotiation.verify_tag(&tag_b).map_err(|_| PairError::WrongCode)?));

    let req = Value::obj(vec![("key", Value::str(&key.public().to_hex())), ("label", Value::str(label))]);
    send(ch, &Seal::seal(&seal.to_host, &req))?;
    let resp = Seal::open(&seal.to_joiner, &recv(ch)?)?;
    let project_id = resp.get_str("project").ok_or_else(|| PairError::Protocol("no project id".into()))?.to_string();
    let Some(Value::Arr(lines)) = resp.get("roster") else {
        return Err(PairError::Protocol("no roster".into()));
    };
    let lines: Vec<&str> = lines.iter().filter_map(Value::as_str).collect();
    let (roster, _) = Roster::from_lines(&project_id, lines).map_err(PairError::Protocol)?;
    let standing = roster.state().standing(&key.public());
    if !matches!(standing, Standing::Member | Standing::Guest) {
        return Err(PairError::Protocol("the host's roster does not enrol this device".into()));
    }
    Ok(Joined { project_id, roster, standing })
}
