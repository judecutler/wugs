//! Device identity: one Ed25519 keypair per installation, shared by every
//! project that installation joins. Keys live in app data, never in a
//! project folder, so copying a project to another machine does not copy
//! the right to sync as this device.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use ed25519_dalek::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use zeroize::Zeroizing;

use crate::hex;

/// An Ed25519 public key; its lowercase hex is how rosters name devices.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicKey([u8; 32]);

impl PublicKey {
    pub fn from_hex(s: &str) -> Result<PublicKey, String> {
        let bytes = hex::decode(s)?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| "public key is not 32 bytes".to_string())?;
        VerifyingKey::from_bytes(&arr).map_err(|e| format!("not an Ed25519 public key: {e}"))?;
        Ok(PublicKey(arr))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The SubjectPublicKeyInfo DER that TLS raw public keys carry (RFC 7250).
    pub fn spki_der(&self) -> Vec<u8> {
        self.verifying()
            .to_public_key_der()
            .expect("an Ed25519 key always encodes")
            .into_vec()
    }

    pub fn from_spki_der(der: &[u8]) -> Result<PublicKey, String> {
        let vk = VerifyingKey::from_public_key_der(der).map_err(|e| format!("not an Ed25519 SPKI: {e}"))?;
        Ok(PublicKey(vk.to_bytes()))
    }

    /// Strict verification: rejects non-canonical signatures and small-order
    /// keys, so one message has exactly one valid signature per key.
    pub fn verify(&self, msg: &[u8], sig: &[u8; 64]) -> bool {
        self.verifying().verify_strict(msg, &Signature::from_bytes(sig)).is_ok()
    }

    fn verifying(&self) -> VerifyingKey {
        VerifyingKey::from_bytes(&self.0).expect("validated at construction")
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", &self.to_hex()[..16])
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// The device's private key. `SigningKey` zeroizes itself on drop.
pub struct DeviceKey(SigningKey);

impl DeviceKey {
    pub fn generate() -> DeviceKey {
        DeviceKey(SigningKey::generate(&mut rand_core::OsRng))
    }

    /// Deterministic keys for tests and fixtures; never for real devices.
    pub fn from_seed(seed: [u8; 32]) -> DeviceKey {
        DeviceKey(SigningKey::from_bytes(&seed))
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.verifying_key().to_bytes())
    }

    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        self.0.sign(msg).to_bytes()
    }

    /// PKCS#8 v1 DER, the form rustls loads as a signing key and the form
    /// a key store keeps; `from_pkcs8_der` reads it back.
    pub fn pkcs8_der(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(
            self.0
                .to_pkcs8_der()
                .expect("an Ed25519 key always encodes")
                .as_bytes()
                .to_vec(),
        )
    }

    /// A stored key, as `pkcs8_der` wrote it.
    pub fn from_pkcs8_der(der: &[u8]) -> Result<DeviceKey, String> {
        SigningKey::from_pkcs8_der(der).map(DeviceKey).map_err(|e| format!("not an Ed25519 PKCS#8 key: {e}"))
    }
}

impl fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceKey({:?})", self.public())
    }
}

/// Where an installation keeps its device key. The OS keychain is the
/// intended home (DECISIONS 2026-11-08); it belongs to the desktop shell,
/// because the `keyring` crate pulls platform D-Bus/Security bindings this
/// engine should not carry. `FileKeyStore` is the portable fallback.
pub trait KeyStore {
    fn load(&self) -> Result<Option<DeviceKey>, String>;
    fn store(&self, key: &DeviceKey) -> Result<(), String>;

    fn load_or_create(&self) -> Result<DeviceKey, String> {
        if let Some(k) = self.load()? {
            return Ok(k);
        }
        let k = DeviceKey::generate();
        self.store(&k)?;
        Ok(k)
    }
}

/// The key seed as hex in `<app_data>/device-key`, mode 0600 on unix. The
/// caller passes the app-data directory; it must not be a project folder.
pub struct FileKeyStore {
    path: PathBuf,
}

impl FileKeyStore {
    pub fn new(app_data: impl AsRef<Path>) -> FileKeyStore {
        FileKeyStore { path: app_data.as_ref().join("device-key") }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl KeyStore for FileKeyStore {
    fn load(&self) -> Result<Option<DeviceKey>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => Zeroizing::new(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("read {}: {e}", self.path.display())),
        };
        let seed = Zeroizing::new(hex::decode(text.trim())?);
        let seed: &[u8; 32] = seed.as_slice().try_into().map_err(|_| "device key file is not a 32-byte seed".to_string())?;
        Ok(Some(DeviceKey(SigningKey::from_bytes(seed))))
    }

    /// Writes a temp file with owner-only permissions, fsyncs it, then
    /// renames it over the key, so a crash never leaves a torn key.
    fn store(&self, key: &DeviceKey) -> Result<(), String> {
        let dir = self.path.parent().ok_or("key path has no directory")?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("tmp");
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        let seed = Zeroizing::new(hex::encode(key.0.as_bytes()));
        f.write_all(seed.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_key_reads_back_as_the_same_key() {
        let key = DeviceKey::generate();
        let back = DeviceKey::from_pkcs8_der(&key.pkcs8_der()).unwrap();
        assert_eq!(back.public(), key.public());
        assert!(key.public().verify(b"m", &back.sign(b"m")));
        assert!(DeviceKey::from_pkcs8_der(b"not der").is_err());
    }
}
