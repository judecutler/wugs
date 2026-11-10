//! wugs-auth: who may sync a project (DECISIONS 2026-11-08, "Sync sharing").
//!
//! - `key`: one Ed25519 device key per installation, behind the `KeyStore` seam.
//! - `roster`: the project's flat, append-only, signed membership list.
//! - `pair`: the 8-digit pairing code (CPace) that enrols a new device.
//!
//! The transport (wugs-net) uses the device key for TLS and the roster for
//! its trust decision. This crate implements no cryptographic primitive.

pub mod key;
pub mod pair;
pub mod roster;

pub use key::{DeviceKey, FileKeyStore, KeyStore, PublicKey};
pub use pair::{pair_host, pair_join, Joined, PairError, PairingCode};
pub use roster::{
    Body, Device, MergeError, MergeReport, Record, RecordHash, Roster, RosterFile, RosterState, SignedRecord, Standing, MAX_PEER_RECORDS,
};

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("not hex".into());
        }
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string())).collect()
    }
}
