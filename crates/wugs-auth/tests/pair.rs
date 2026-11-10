use std::io::{self, Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

use wugs_auth::{pair_host, pair_join, DeviceKey, FileKeyStore, KeyStore, PairError, PairingCode, Roster, Standing};

/// One end of an in-memory duplex: writes go to the peer, reads drain what
/// the peer wrote; dropping an end is end-of-file for the other.
struct Pipe {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    buf: Vec<u8>,
}

fn duplex() -> (Pipe, Pipe) {
    let (a_tx, b_rx) = channel();
    let (b_tx, a_rx) = channel();
    (Pipe { tx: a_tx, rx: a_rx, buf: Vec::new() }, Pipe { tx: b_tx, rx: b_rx, buf: Vec::new() })
}

impl Read for Pipe {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.buf.is_empty() {
            match self.rx.recv() {
                Ok(chunk) => self.buf = chunk,
                Err(_) => return Ok(0),
            }
        }
        let n = out.len().min(self.buf.len());
        out[..n].copy_from_slice(&self.buf[..n]);
        self.buf.drain(..n);
        Ok(n)
    }
}

impl Write for Pipe {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.tx.send(data.to_vec()).map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

const P: &str = "prj_pair";

#[test]
fn pairing_enrols_the_joiner_and_hands_it_the_roster() {
    let host = DeviceKey::generate();
    let mut roster = Roster::genesis(P, &host, "field laptop");
    let mut code = PairingCode::generate(Duration::from_secs(300));
    let typed = format!("{} {}", &code.digits()[..4], &code.digits()[4..]);
    let (mut h, mut j) = duplex();

    let joiner = thread::spawn(move || {
        let key = DeviceKey::generate();
        let joined = pair_join(&mut j, &typed, &key, "tablet").unwrap();
        (key.public(), joined.project_id, joined.standing, joined.roster.state())
    });
    let rec = pair_host(&mut h, &mut code, &host, &mut roster, false, &mut |_| Ok(())).unwrap();
    let (pk, project, standing, joiner_state) = joiner.join().unwrap();

    assert_eq!(project, P);
    assert_eq!(standing, Standing::Member);
    assert_eq!(roster.state().standing(&pk), Standing::Member);
    assert_eq!(joiner_state, roster.state(), "both sides hold the same roster");
    assert_eq!(rec.record().signer, host.public());
    assert!(!code.is_live(), "a successful pairing burns the code too");
}

#[test]
fn a_guest_pairing_enrols_a_guest() {
    let host = DeviceKey::generate();
    let mut roster = Roster::genesis(P, &host, "A");
    let mut code = PairingCode::generate(Duration::from_secs(60));
    let typed = code.digits().to_string();
    let (mut h, mut j) = duplex();
    let joiner = thread::spawn(move || pair_join(&mut j, &typed, &DeviceKey::generate(), "visitor").unwrap().standing);
    pair_host(&mut h, &mut code, &host, &mut roster, true, &mut |_| Ok(())).unwrap();
    assert_eq!(joiner.join().unwrap(), Standing::Guest);
}

#[test]
fn a_wrong_code_fails_on_both_sides_and_burns_the_code() {
    let host = DeviceKey::generate();
    let mut roster = Roster::genesis(P, &host, "A");
    let mut code = PairingCode::generate(Duration::from_secs(300));
    let wrong = format!("{:08}", (code.digits().parse::<u32>().unwrap() + 1) % 100_000_000);
    let (mut h, mut j) = duplex();

    let joiner = thread::spawn(move || pair_join(&mut j, &wrong, &DeviceKey::generate(), "thief"));
    let host_result = pair_host(&mut h, &mut code, &host, &mut roster, false, &mut |_| Ok(()));
    drop(h);
    let joiner_result = joiner.join().unwrap();

    assert!(matches!(host_result, Err(PairError::WrongCode)), "host: {host_result:?}");
    assert!(matches!(joiner_result, Err(PairError::WrongCode)), "joiner: {:?}", joiner_result.err());
    assert_eq!(roster.records().len(), 1, "nothing was enrolled");

    // The right code, retried on the burned slot, is refused without a read.
    let (mut h2, _j2) = duplex();
    assert!(matches!(pair_host(&mut h2, &mut code, &host, &mut roster, false, &mut |_| Ok(())), Err(PairError::AlreadyUsed)));
}

#[test]
fn an_expired_code_is_refused_and_malformed_input_never_hits_the_wire() {
    let host = DeviceKey::generate();
    let mut roster = Roster::genesis(P, &host, "A");
    let mut code = PairingCode::generate(Duration::ZERO);
    let (mut h, _j) = duplex();
    assert!(matches!(pair_host(&mut h, &mut code, &host, &mut roster, false, &mut |_| Ok(())), Err(PairError::Expired)));

    let (_h, mut j) = duplex();
    for bad in ["1234567", "123456789", "1234abcd", ""] {
        assert!(matches!(pair_join(&mut j, bad, &DeviceKey::generate(), "x"), Err(PairError::BadCode)), "{bad:?}");
    }
}

#[test]
fn file_key_store_round_trips_with_owner_only_permissions() {
    let dir = std::env::temp_dir().join(format!("wugs-auth-keys-{}", wugs_model::ulid()));
    let store = FileKeyStore::new(&dir);
    assert!(store.load().unwrap().is_none());
    let k = store.load_or_create().unwrap();
    assert_eq!(store.load_or_create().unwrap().public(), k.public(), "one key per installation");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(store.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_failed_persist_errors_before_the_joiner_learns_anything() {
    // A fork in a signer's chain is fatal, so the host must never send an
    // enrolment it has not made durable: persist runs between signing and
    // sending, and its failure stops the send.
    let host = DeviceKey::generate();
    let mut roster = Roster::genesis(P, &host, "field laptop");
    let mut code = PairingCode::generate(Duration::from_secs(300));
    let typed = format!("{} {}", &code.digits()[..4], &code.digits()[4..]);
    let (mut h, mut j) = duplex();

    let joiner = thread::spawn(move || {
        let key = DeviceKey::generate();
        pair_join(&mut j, &typed, &key, "tablet")
    });
    let res = pair_host(&mut h, &mut code, &host, &mut roster, false, &mut |_| {
        Err("disk full".into())
    });
    drop(h); // the host hangs up without sending the sealed payload
    let err = res.unwrap_err();
    assert!(format!("{err:?}").contains("persist before send"), "{err:?}");
    assert!(joiner.join().unwrap().is_err(), "the joiner never got the enrolment");
}
