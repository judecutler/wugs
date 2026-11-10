//! ULID generation (own implementation; swappable for the `ulid` crate).
//! 48-bit millisecond timestamp + 80 random bits, Crockford base32, 26 chars.

use std::cell::RefCell;
use std::collections::hash_map::RandomState;
use std::fs::File;
use std::hash::{BuildHasher, Hash, Hasher};
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

pub const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

thread_local! {
    static RNG: RefCell<Rng> = RefCell::new(Rng::seeded());
}

struct Rng {
    s: [u64; 2],
}

impl Rng {
    fn seeded() -> Rng {
        let mut buf = [0u8; 16];
        let seeded = File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut buf))
            .is_ok();
        if !seeded {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            buf = fallback_seed(nanos);
        }
        let a = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let b = u64::from_le_bytes(buf[8..16].try_into().unwrap());
        Rng { s: [a | 1, b | 1] }
    }

    /// xorshift128+ — fine for ids (not cryptographic; ids are not secrets).
    fn next(&mut self) -> u64 {
        let mut x = self.s[0];
        let y = self.s[1];
        self.s[0] = y;
        x ^= x << 23;
        self.s[1] = x ^ y ^ (x >> 17) ^ (y >> 26);
        self.s[1].wrapping_add(y)
    }
}

/// Seed bytes for platforms without `/dev/urandom`. The clock alone repeats
/// within one tick (about 100ns on Windows), so each word also mixes a fresh
/// `RandomState`, which std seeds from the OS and keys per instance.
fn fallback_seed(nanos: u128) -> [u8; 16] {
    let stack_addr = &nanos as *const u128 as usize;
    let word = || {
        let mut h = RandomState::new().build_hasher();
        nanos.hash(&mut h);
        std::thread::current().id().hash(&mut h);
        stack_addr.hash(&mut h);
        h.finish()
    };
    let (a, b) = (word(), word());
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&a.to_le_bytes());
    out[8..].copy_from_slice(&b.to_le_bytes());
    let t = nanos.to_le_bytes();
    for (o, t) in out.iter_mut().zip(t) {
        *o ^= t;
    }
    out
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn ulid() -> String {
    let ms = now_millis();
    let (r1, r2) = RNG.with(|r| {
        let mut r = r.borrow_mut();
        (r.next(), r.next())
    });
    encode_ulid(ms, r1, r2)
}

fn encode_ulid(ms: u64, r1: u64, r2: u64) -> String {
    let mut out = [0u8; 26];
    let mut t = ms & 0xFFFF_FFFF_FFFF;
    for i in (0..10).rev() {
        out[i] = CROCKFORD[(t & 31) as usize];
        t >>= 5;
    }
    let mut rand = (((r1 as u128) << 64) | r2 as u128) & ((1u128 << 80) - 1);
    for i in (10..26).rev() {
        out[i] = CROCKFORD[(rand & 31) as usize];
        rand >>= 5;
    }
    String::from_utf8(out.to_vec()).unwrap()
}

/// `new_id("tok")` → `tok_01ARZ…`
pub fn new_id(prefix: &str) -> String {
    format!("{}_{}", prefix, ulid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn fallback_seeds_differ_within_one_clock_tick() {
        let seeds: HashSet<[u8; 16]> = (0..1000).map(|_| fallback_seed(42)).collect();
        assert_eq!(seeds.len(), 1000);
    }

    #[test]
    fn fallback_seeds_differ_across_threads() {
        let handles: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| fallback_seed(42)))
            .collect();
        let seeds: HashSet<[u8; 16]> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(seeds.len(), 8);
    }

    #[test]
    fn unique_and_sorted_by_time() {
        let mut seen = HashSet::new();
        let mut prev = String::new();
        for _ in 0..10_000 {
            let u = ulid();
            assert_eq!(u.len(), 26);
            assert!(seen.insert(u.clone()), "duplicate ulid");
            // time prefix is monotone non-decreasing within one thread
            assert!(u[..10] >= prev[..prev.len().min(10)]);
            prev = u;
        }
    }
}
