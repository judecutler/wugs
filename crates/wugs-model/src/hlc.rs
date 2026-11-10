//! Hybrid logical clock (DATA-MODEL.md §1).
//! Encoded form sorts lexicographically identically to (millis, counter, device):
//! `{millis:012x}-{counter:04x}-{device}`.

use crate::id::now_millis;

/// Maximum tolerated wall-clock drift for remote timestamps (24 h).
pub const MAX_DRIFT_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hlc {
    pub millis: u64,
    pub counter: u32,
    pub device: String,
}

impl Hlc {
    pub fn encode(&self) -> String {
        format!("{:012x}-{:04x}-{}", self.millis, self.counter, self.device)
    }

    pub fn parse(s: &str) -> Option<Hlc> {
        let mut parts = s.splitn(3, '-');
        let millis = u64::from_str_radix(parts.next()?, 16).ok()?;
        let counter = u32::from_str_radix(parts.next()?, 16).ok()?;
        let device = parts.next()?.to_string();
        if device.is_empty() {
            return None;
        }
        Some(Hlc {
            millis,
            counter,
            device,
        })
    }
}

#[derive(Debug)]
pub struct Clock {
    pub device: String,
    last_millis: u64,
    last_counter: u32,
}

#[derive(Debug)]
pub enum ClockError {
    /// A remote timestamp is further in the future than MAX_DRIFT_MS allows.
    DriftTooLarge { remote_millis: u64, wall: u64 },
}

impl Clock {
    pub fn new(device: String) -> Clock {
        Clock {
            device,
            last_millis: 0,
            last_counter: 0,
        }
    }

    /// Issue the next local timestamp (strictly greater than anything issued
    /// or observed so far on this clock).
    pub fn now(&mut self) -> Hlc {
        let wall = now_millis();
        if wall > self.last_millis {
            self.last_millis = wall;
            self.last_counter = 0;
        } else {
            self.last_counter += 1;
            if self.last_counter > 0xFFFF {
                self.last_millis += 1;
                self.last_counter = 0;
            }
        }
        Hlc {
            millis: self.last_millis,
            counter: self.last_counter,
            device: self.device.clone(),
        }
    }

    /// Fold in a timestamp from this project's own log without the drift
    /// check: the op is already part of the record, so refusing it would only
    /// stop the project from opening (e.g. after this device's clock was set
    /// back). Later local timestamps sort after it, as HLCs must.
    pub fn absorb(&mut self, remote: &Hlc) {
        if remote.millis > self.last_millis {
            self.last_millis = remote.millis;
            self.last_counter = remote.counter;
        } else if remote.millis == self.last_millis && remote.counter > self.last_counter {
            self.last_counter = remote.counter;
        }
    }

    /// Would `observe` accept this timestamp? Ingest asks before it commits
    /// a txn, so an op from a far-future clock is held, never half-applied.
    pub fn check_drift(remote: &Hlc) -> Result<(), ClockError> {
        let wall = now_millis();
        if remote.millis > wall + MAX_DRIFT_MS {
            return Err(ClockError::DriftTooLarge {
                remote_millis: remote.millis,
                wall,
            });
        }
        Ok(())
    }

    /// Fold a remote timestamp into this clock so future local timestamps
    /// sort after it. Rejects absurd future drift (DATA-MODEL.md §1).
    pub fn observe(&mut self, remote: &Hlc) -> Result<(), ClockError> {
        Clock::check_drift(remote)?;
        self.absorb(remote);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_parse_roundtrip() {
        let h = Hlc {
            millis: 0x018f3c2a7b40,
            counter: 3,
            device: "dvc_01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
        };
        let s = h.encode();
        assert_eq!(Hlc::parse(&s).unwrap(), h);
    }

    #[test]
    fn string_order_matches_struct_order() {
        let a = Hlc { millis: 5, counter: 9, device: "dvc_B".into() };
        let b = Hlc { millis: 5, counter: 10, device: "dvc_A".into() };
        let c = Hlc { millis: 6, counter: 0, device: "dvc_A".into() };
        assert!(a < b && b < c);
        assert!(a.encode() < b.encode());
        assert!(b.encode() < c.encode());
    }

    #[test]
    fn clock_is_monotone() {
        let mut c = Clock::new("dvc_X".into());
        let mut prev = c.now();
        for _ in 0..5000 {
            let next = c.now();
            assert!(next > prev);
            prev = next;
        }
    }

    #[test]
    fn observe_advances() {
        let mut c = Clock::new("dvc_X".into());
        let local = c.now();
        let remote = Hlc {
            millis: local.millis + 1000,
            counter: 7,
            device: "dvc_Y".into(),
        };
        c.observe(&remote).unwrap();
        assert!(c.now() > remote);
    }

    #[test]
    fn observe_rejects_huge_drift() {
        let mut c = Clock::new("dvc_X".into());
        let remote = Hlc {
            millis: now_millis() + MAX_DRIFT_MS + 60_000,
            counter: 0,
            device: "dvc_Y".into(),
        };
        assert!(c.observe(&remote).is_err());
    }
}
