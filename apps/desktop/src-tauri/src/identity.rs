//! This installation's device key: in the OS keychain when there is one,
//! in a file in app data when there is not (headless Linux, CI, a desktop
//! without a Secret Service). Which store holds the key is written down
//! the first time, so a keychain that is merely locked today never makes
//! the device quietly mint a second identity.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::Serialize;
use wugs_auth::{DeviceKey, FileKeyStore, KeyStore, PublicKey};
use wugs_model::ulid;
use zeroize::Zeroizing;

const KEYCHAIN_SERVICE: &str = "dev.wugs.desktop";
const KEYCHAIN_USER: &str = "device-key";
const STORE_MARKER: &str = "key-store";

pub struct KeychainKeyStore;

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER).map_err(|e| format!("OS keychain: {e}"))
}

impl KeyStore for KeychainKeyStore {
    fn load(&self) -> Result<Option<DeviceKey>, String> {
        let der = match entry()?.get_secret() {
            Ok(der) => Zeroizing::new(der),
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(e) => return Err(format!("OS keychain: {e}")),
        };
        DeviceKey::from_pkcs8_der(&der).map(Some).map_err(|e| format!("the keychain's device key is unreadable: {e}"))
    }

    fn store(&self, key: &DeviceKey) -> Result<(), String> {
        entry()?.set_secret(&key.pkcs8_der()).map_err(|e| format!("OS keychain: {e}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreKind {
    Keychain,
    File,
}

impl StoreKind {
    fn name(self) -> &'static str {
        match self {
            StoreKind::Keychain => "keychain",
            StoreKind::File => "file",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityInfo {
    pub public_key: String,
    pub fingerprint: String,
    pub store: StoreKind,
    /// Why the key is in a file rather than the keychain; `None` when it is in the keychain.
    pub fallback_reason: Option<String>,
}

/// The first 16 hex digits of a key in groups of four: enough to tell
/// devices apart by eye, the full key stays in the roster.
pub fn fingerprint(key: &PublicKey) -> String {
    let hex = key.to_hex();
    hex.as_bytes()[..16].chunks(4).map(|c| std::str::from_utf8(c).expect("hex is ascii")).collect::<Vec<_>>().join(" ")
}

/// Load or create this installation's key. The first run tries `keychain`
/// and falls back to `<app_data>/device-key` if it cannot be used; later
/// runs go to whichever store the first run chose. A key file that is
/// already there (the marker was lost, or an older build wrote the key)
/// is used as it is, so a lost marker never mints a second identity.
pub fn device_identity(app_data: &Path, keychain: &dyn KeyStore) -> Result<(DeviceKey, IdentityInfo), String> {
    let marker = app_data.join(STORE_MARKER);
    let file = FileKeyStore::new(app_data);
    let chosen = match fs::read_to_string(&marker) {
        Ok(s) => Some(s.trim().to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("read {}: {e}", marker.display())),
    };
    let (key, store, fallback_reason) = match chosen.as_deref() {
        Some("keychain") => {
            let key = keychain
                .load()
                .map_err(|e| format!("this device's key is in the OS keychain, which cannot be read right now ({e}); unlock the keychain and restart wugs"))?
                .ok_or_else(|| {
                    format!(
                        "this device's key is gone from the OS keychain; to start over with a new device identity (other devices will need to pair it again), delete {}",
                        marker.display()
                    )
                })?;
            (key, StoreKind::Keychain, None)
        }
        Some("file") => (file.load_or_create()?, StoreKind::File, Some(file_reason(&file, "an earlier run found no usable OS keychain"))),
        Some(other) => return Err(format!("{} names an unknown key store {other:?}", marker.display())),
        None => match file.load()? {
            Some(key) => (key, StoreKind::File, Some(file_reason(&file, "a key file was already there"))),
            None => match keychain.load_or_create() {
                Ok(key) => (key, StoreKind::Keychain, None),
                Err(e) => (file.load_or_create()?, StoreKind::File, Some(file_reason(&file, &e))),
            },
        },
    };
    if chosen.is_none() {
        write_marker(app_data, &marker, store)?;
    }
    let public = key.public();
    let info = IdentityInfo { public_key: public.to_hex(), fingerprint: fingerprint(&public), store, fallback_reason };
    Ok((key, info))
}

/// Temp file, fsync, rename: a torn marker would read as an unknown store
/// and lock the device out of its own key.
fn write_marker(app_data: &Path, marker: &Path, store: StoreKind) -> Result<(), String> {
    fs::create_dir_all(app_data).map_err(|e| e.to_string())?;
    let tmp = app_data.join(format!(".{STORE_MARKER}.{}.tmp", ulid()));
    let write = || -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(store.name().as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, marker)
    };
    write().map_err(|e| {
        fs::remove_file(&tmp).ok();
        format!("write {}: {e}", marker.display())
    })
}

fn file_reason(file: &FileKeyStore, why: &str) -> String {
    format!("the OS keychain is unavailable ({why}), so the device key is kept in {}", file.path().display())
}
