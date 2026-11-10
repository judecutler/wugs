use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::thread;

use wugs_auth::{DeviceKey, KeyStore};
use wugs_desktop::identity::{device_identity, StoreKind};
use wugs_desktop::layout::{load_layout, save_layout};
use wugs_desktop::session::{self, DevicesView, Session, StandingView, SyncResult};
use wugs_model::ulid;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!("wugs-desktop-auth-{}", ulid())))
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn host_with_story() -> (TempDir, Session, DeviceKey) {
    let dir = TempDir::new();
    let mut s: Session = None;
    session::create_project(&mut s, dir.path(), "prs_A").unwrap();
    let text = session::create_text(&mut s, "Story").unwrap();
    session::add_segment(&mut s, &text, None, Some(0.0), Some(900.0), "ilk").unwrap();
    (dir, s, DeviceKey::generate())
}

fn titles(s: &mut Session) -> Vec<String> {
    let mut t: Vec<String> = session::list_texts(s).unwrap().into_iter().map(|r| r.title).collect();
    t.sort();
    t
}

fn row<'a>(v: &'a DevicesView, label: &str) -> &'a session::DeviceRow {
    v.devices.iter().find(|d| d.label == label).unwrap_or_else(|| panic!("no device {label} in {:?}", v.devices))
}

/// Pair a joiner into a fresh folder over loopback and let it run `then`
/// on its open session, while the host runs `host_then` once the pairing
/// is done. The host side runs on the calling thread, the way the shell
/// runs it on the session thread.
fn pair_and<R: Send + 'static, H>(
    host: &mut Session,
    host_key: &DeviceKey,
    guest: bool,
    then: impl FnOnce(&mut Session, &DeviceKey) -> R + Send + 'static,
    host_then: impl FnOnce(&mut Session) -> H,
) -> (session::DeviceRow, R, H, TempDir) {
    let (offer, pairing) = session::pair_start(host, host_key, "laptop", guest).unwrap();
    assert_eq!(offer.digits.len(), 8);
    assert!(offer.expires_in > 0);
    let dir = TempDir::new();
    let path = dir.path().to_string();
    let joiner = thread::spawn(move || {
        let key = DeviceKey::generate();
        let mut s: Session = None;
        let joined = session::pair_join(&mut s, &key, &path, "prs_B", &format!("127.0.0.1:{}", offer.port), &offer.digits, "tablet").unwrap();
        (joined.standing, joined.project.project_id, then(&mut s, &key))
    });
    let tcp = pairing.wait().unwrap();
    let enrolled = session::pair_finish(host, host_key, pairing, tcp).unwrap();
    let host_out = host_then(host);
    let (standing, project_id, out) = joiner.join().unwrap();
    assert_eq!(project_id, host.as_ref().unwrap().project_id, "the joiner adopted the host's project");
    assert_eq!(standing, if guest { StandingView::Guest } else { StandingView::Member });
    (enrolled, out, host_out, dir)
}

fn pair<R: Send + 'static>(
    host: &mut Session,
    host_key: &DeviceKey,
    guest: bool,
    then: impl FnOnce(&mut Session, &DeviceKey) -> R + Send + 'static,
) -> (session::DeviceRow, R, TempDir) {
    let (enrolled, out, (), dir) = pair_and(host, host_key, guest, then, |_| ());
    (enrolled, out, dir)
}

#[test]
fn pairing_over_loopback_enrols_the_joiner_on_both_rosters() {
    let (_dir, mut host, key_a) = host_with_story();
    assert!(!session::devices_list(&mut host, &key_a).unwrap().paired, "no roster before the first code");

    let (enrolled, joiner_view, _jdir) = pair(&mut host, &key_a, false, |s, k| session::devices_list(s, k).unwrap());

    assert_eq!(enrolled.label, "tablet");
    assert_eq!(enrolled.standing, StandingView::Member);
    assert!(!enrolled.this_device);
    let host_view = session::devices_list(&mut host, &key_a).unwrap();
    assert!(host_view.paired);
    assert_eq!(host_view.devices.len(), 2);
    assert!(row(&host_view, "laptop").this_device, "the host founded the roster under its own label");
    assert_eq!(row(&host_view, "tablet").key, enrolled.key);
    assert_eq!(row(&joiner_view, "laptop").standing, StandingView::Member);
    assert!(row(&joiner_view, "tablet").this_device);
    assert!(host_view.copy_warning.is_none() && joiner_view.copy_warning.is_none());
    assert_eq!(row(&host_view, "tablet").fingerprint.len(), 19, "four groups of four hex digits");
}

#[test]
fn a_guest_pairing_shows_as_guest() {
    let (_dir, mut host, key_a) = host_with_story();
    let (enrolled, _, _jdir) = pair(&mut host, &key_a, true, |_, _| ());
    assert!(enrolled.guest);
    assert_eq!(enrolled.standing, StandingView::Guest);
}

#[test]
fn a_wrong_code_enrols_nobody() {
    let (_dir, mut host, key_a) = host_with_story();
    let (offer, pairing) = session::pair_start(&mut host, &key_a, "laptop", false).unwrap();
    let wrong = if offer.digits == "00000000" { "11111111" } else { "00000000" };
    let jdir = TempDir::new();
    let path = jdir.path().to_string();
    let joiner = thread::spawn(move || {
        let mut s: Session = None;
        session::pair_join(&mut s, &DeviceKey::generate(), &path, "prs_B", &format!("127.0.0.1:{}", offer.port), wrong, "tablet").map(|_| ())
    });
    let tcp = pairing.wait().unwrap();
    let host_err = session::pair_finish(&mut host, &key_a, pairing, tcp).unwrap_err();
    assert!(host_err.contains("did not match"), "{host_err}");
    assert!(joiner.join().unwrap().unwrap_err().contains("did not match"));
    assert!(!jdir.0.join("project.json").exists(), "a failed join leaves no project behind");
    assert_eq!(session::devices_list(&mut host, &key_a).unwrap().devices.len(), 1);
}

#[test]
fn marking_a_device_lost_shows_it_revoked_and_refuses_itself() {
    let (_dir, mut host, key_a) = host_with_story();
    let (enrolled, _, _jdir) = pair(&mut host, &key_a, false, |_, _| ());

    let me = key_a.public().to_hex();
    assert!(session::device_mark_lost(&mut host, &key_a, &me).is_err(), "a device cannot mark itself lost");
    let view = session::device_mark_lost(&mut host, &key_a, &enrolled.key).unwrap();
    assert_eq!(row(&view, "tablet").standing, StandingView::Revoked);
    assert_eq!(row(&view, "laptop").standing, StandingView::Member);

    let reopened = session::devices_list(&mut host, &key_a).unwrap();
    assert_eq!(row(&reopened, "tablet").standing, StandingView::Revoked, "the revocation is on disk");
}

/// The joiner pairs, writes a text of its own, then syncs with the host's
/// listener; the host serves it through the same relay the shell uses.
fn pair_then_sync(host: &mut Session, key_a: &DeviceKey, copy_dvc_from: Option<PathBuf>) -> (SyncResult, Vec<String>, DevicesView, session::ServedSession, TempDir) {
    let l = session::sync_listen(host, key_a, "laptop").unwrap();
    let sync_addr = format!("127.0.0.1:{}", l.listener.local_addr().unwrap().port());
    let project_id = l.project_id.clone();
    let joiner = move |s: &mut Session, k: &DeviceKey| {
        let mut s = match copy_dvc_from {
            None => std::mem::take(s),
            Some(from) => {
                // A folder copied from the host carries the host's device id.
                let root = s.as_ref().unwrap().root.clone();
                *s = None;
                fs::copy(from.join("cache").join("device"), root.join("cache").join("device")).unwrap();
                let mut reopened: Session = None;
                session::open_project(&mut reopened, root.to_str().unwrap(), "prs_B").unwrap();
                reopened
            }
        };
        session::create_text(&mut s, "From the tablet").unwrap();
        let r = session::sync_now(&mut s, k, &sync_addr).unwrap();
        (r, titles(&mut s), session::devices_list(&mut s, k).unwrap())
    };
    let serve = |h: &mut Session| {
        let (conn, _) = l.listener.accept().unwrap();
        session::serve_connection(h, key_a, &project_id, conn).unwrap()
    };
    let (_, (result, joiner_titles, joiner_view), served, jdir) = pair_and(host, key_a, false, joiner, serve);
    (result, joiner_titles, joiner_view, served, jdir)
}

#[test]
fn paired_devices_converge_through_sync_now() {
    let (_dir, mut host, key_a) = host_with_story();
    let (result, joiner_titles, _, served, _jdir) = pair_then_sync(&mut host, &key_a, None);

    assert_eq!(served.outcome, "converged");
    assert_eq!(served.peer_label.as_deref(), Some("tablet"));
    assert!(served.pulled_ops > 0, "the host pulled the tablet's text");
    assert_eq!(result.peer_label.as_deref(), Some("laptop"));
    assert!(result.pulled_ops > 0);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(titles(&mut host), vec!["From the tablet", "Story"]);
    assert_eq!(joiner_titles, titles(&mut host), "both devices hold the same texts");
}

#[test]
fn a_copied_project_folder_is_surfaced_on_sync_and_in_the_device_list() {
    let (dir, mut host, key_a) = host_with_story();
    let (result, _, joiner_view, served, _jdir) = pair_then_sync(&mut host, &key_a, Some(dir.0.clone()));

    assert_eq!(served.outcome, "converged", "a claimed device id warns, it never blocks");
    let warning = result.warnings.first().expect("the sync reports the copied folder");
    assert!(warning.contains("may be a copy"), "{warning}");
    assert!(!served.warnings.is_empty(), "the serving side hears of it too");
    assert!(joiner_view.copy_warning.as_deref().is_some_and(|w| w.contains("fresh device identity")));
}

#[test]
fn a_fresh_device_identity_clears_the_copy_warning_and_is_attested_on_the_next_sync() {
    let (dir, mut host, key_a) = host_with_story();
    let host_dvc = host.as_ref().unwrap().device().to_string();
    let l = session::sync_listen(&mut host, &key_a, "laptop").unwrap();
    let addr = format!("127.0.0.1:{}", l.listener.local_addr().unwrap().port());
    let project_id = l.project_id.clone();
    let from = dir.0.clone();
    let joiner = move |s: &mut Session, k: &DeviceKey| {
        let root = s.as_ref().unwrap().root.clone();
        *s = None;
        fs::copy(from.join("cache").join("device"), root.join("cache").join("device")).unwrap();
        let mut s: Session = None;
        session::open_project(&mut s, root.to_str().unwrap(), "prs_B").unwrap();
        let copied = session::sync_now(&mut s, k, &addr).unwrap();
        let minted = session::mint_device_identity(&mut s, k).unwrap();
        let fresh = s.as_ref().unwrap().device().to_string();
        session::create_text(&mut s, "After the fresh id").unwrap();
        let after = session::sync_now(&mut s, k, &addr).unwrap();
        (copied.warnings, minted, fresh, after.warnings, session::devices_list(&mut s, k).unwrap(), k.public(), root)
    };
    let serve = |h: &mut Session| {
        let mut warned = Vec::new();
        for _ in 0..2 {
            let (conn, _) = l.listener.accept().unwrap();
            let served = session::serve_connection(h, &key_a, &project_id, conn).unwrap();
            assert_eq!(served.outcome, "converged");
            warned.push(served.warnings);
        }
        warned
    };
    let (_, (copied, minted, fresh, joiner_after, after_sync, joiner_key, joiner_root), host_warned, _jdir) =
        pair_and(&mut host, &key_a, false, joiner, serve);

    assert!(copied.first().is_some_and(|w| w.contains("may be a copy")), "{copied:?}");
    assert_ne!(fresh, host_dvc);
    assert!(minted.copy_warning.is_none(), "the fresh id is this device's alone");
    assert!(after_sync.copy_warning.is_none());
    assert!(!host_warned[0].is_empty(), "the host heard of the copy before the fresh id");
    assert!(host_warned[1].is_empty(), "the host's own id is its alone again: {:?}", host_warned[1]);
    assert!(joiner_after.is_empty(), "{joiner_after:?}");
    assert!(session::devices_list(&mut host, &key_a).unwrap().copy_warning.is_none(), "the host's device list is clean too");
    let roster = wugs_auth::RosterFile::in_project(&dir.0).load(&host.as_ref().unwrap().project_id).unwrap();
    assert!(roster.state().attested.get(&fresh).is_some_and(|ks| ks.contains(&joiner_key)), "the next sync attested the fresh id");
    assert!(titles(&mut host).contains(&"After the fresh id".to_string()));
    let text_logs = fs::read_dir(joiner_root.join("log")).unwrap().filter_map(|e| e.ok()).filter(|e| e.path().join(format!("{fresh}.jsonl")).is_file()).count();
    assert_eq!(text_logs, 1, "new ops land in a log file under the fresh id");
}

#[test]
fn sync_without_pairing_is_refused_in_words() {
    let (_dir, mut host, _) = host_with_story();
    let err = session::sync_now(&mut host, &DeviceKey::generate(), "127.0.0.1:9").unwrap_err();
    assert!(err.contains("pair it first"), "{err}");
}

// ---------------------------------------------------------------- identity

/// A keychain that is there and keeps what it is given.
#[derive(Default)]
/// It holds the same PKCS#8 bytes the real keychain store does.
struct MemoryKeychain(RefCell<Option<Vec<u8>>>);

impl KeyStore for MemoryKeychain {
    fn load(&self) -> Result<Option<DeviceKey>, String> {
        self.0.borrow().as_deref().map(DeviceKey::from_pkcs8_der).transpose()
    }
    fn store(&self, key: &DeviceKey) -> Result<(), String> {
        *self.0.borrow_mut() = Some(key.pkcs8_der().to_vec());
        Ok(())
    }
}

/// A headless Linux box: no Secret Service on the bus.
struct NoKeychain;

impl KeyStore for NoKeychain {
    fn load(&self) -> Result<Option<DeviceKey>, String> {
        Err("no secret service".into())
    }
    fn store(&self, _: &DeviceKey) -> Result<(), String> {
        Err("no secret service".into())
    }
}

#[test]
fn without_a_keychain_the_key_falls_back_to_a_file_and_says_so() {
    let dir = TempDir::new();
    let (key, info) = device_identity(&dir.0, &NoKeychain).unwrap();
    assert_eq!(info.store, StoreKind::File);
    assert!(info.fallback_reason.as_deref().is_some_and(|r| r.contains("no secret service")));
    assert!(dir.0.join("device-key").is_file());
    assert_eq!(info.public_key, key.public().to_hex());

    let keychain_now_works = MemoryKeychain::default();
    let (again, info) = device_identity(&dir.0, &keychain_now_works).unwrap();
    assert_eq!(again.public(), key.public(), "the device keeps its identity once a keychain appears");
    assert_eq!(info.store, StoreKind::File);
    assert!(keychain_now_works.0.borrow().is_none(), "nothing was minted in the keychain");
}

#[test]
fn a_keychain_key_never_silently_becomes_a_new_file_key() {
    let dir = TempDir::new();
    let keychain = MemoryKeychain::default();
    let (key, info) = device_identity(&dir.0, &keychain).unwrap();
    assert_eq!(info.store, StoreKind::Keychain);
    assert!(info.fallback_reason.is_none());
    assert_eq!(device_identity(&dir.0, &keychain).unwrap().0.public(), key.public());

    let err = device_identity(&dir.0, &NoKeychain).unwrap_err();
    assert!(err.contains("unlock the keychain"), "{err}");
    assert!(!dir.0.join("device-key").exists(), "no second identity was minted");
}

#[test]
fn a_lost_marker_keeps_the_key_file_instead_of_minting_in_the_keychain() {
    let dir = TempDir::new();
    let (key, _) = device_identity(&dir.0, &NoKeychain).unwrap();
    fs::remove_file(dir.0.join("key-store")).unwrap();
    let keychain = MemoryKeychain::default();
    let (again, info) = device_identity(&dir.0, &keychain).unwrap();
    assert_eq!(again.public(), key.public());
    assert_eq!(info.store, StoreKind::File);
    assert!(keychain.0.borrow().is_none(), "no second identity in the keychain");
    assert_eq!(fs::read_to_string(dir.0.join("key-store")).unwrap(), "file");
    let leftovers: Vec<_> = fs::read_dir(&dir.0).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
    assert!(leftovers.is_empty(), "the marker write leaves no temp file");
}

// ------------------------------------------------------------------ layout

#[test]
fn a_saved_layout_comes_back_per_kit() {
    let dir = TempDir::new();
    assert_eq!(load_layout(&dir.0, "tongues").unwrap(), None, "no layout before the first save");
    let layout = r#"{"grid":{"root":{"type":"branch","data":[]}},"panels":{"timeline":{"id":"timeline"}}}"#;
    save_layout(&dir.0, "tongues", layout).unwrap();
    assert_eq!(load_layout(&dir.0, "tongues").unwrap().as_deref(), Some(layout));
    assert_eq!(load_layout(&dir.0, "scrolls").unwrap(), None, "layouts are per kit");
    let moved = r#"{"grid":{},"panels":{}}"#;
    save_layout(&dir.0, "tongues", moved).unwrap();
    assert_eq!(load_layout(&dir.0, "tongues").unwrap().as_deref(), Some(moved), "a later save replaces the earlier");
}

#[test]
fn layouts_refuse_non_objects_and_odd_kit_names_and_skip_a_torn_file() {
    let dir = TempDir::new();
    assert!(save_layout(&dir.0, "tongues", "[1,2]").is_err());
    assert!(save_layout(&dir.0, "../escape", "{}").is_err());
    save_layout(&dir.0, "tongues", r#"{"grid":{}}"#).unwrap();
    fs::write(dir.0.join("layouts").join("tongues.json"), r#"{"grid":"#).unwrap();
    assert_eq!(load_layout(&dir.0, "tongues").unwrap(), None, "a torn layout falls back to the default");
}
