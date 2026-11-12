//! `.wpk` serialization. Layout, all little-endian:
//! `"WUGSPEAK"` magic, `u32` version, `u32` sample rate, `u16` channels,
//! `u16` flags (bit 0 truncated), `u64` frames, `u32` base bucket, then every
//! level's channel series as `(i16 min, i16 max)` pairs, then a `u32` CRC-32
//! of everything before it. Level sizes follow from frames and base bucket,
//! together with the fixed v1 pyramid rule: levels fold /2 until a level has
//! at most 1024 buckets. That rule is part of the v1 layout — changing it
//! means a new version, or every v1 file fails to load as `BadLayout`.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{level_lens, Error, Level, Peaks, SourceInfo};

const MAGIC: &[u8; 8] = b"WUGSPEAK";
pub const VERSION: u32 = 1;
const HEADER_LEN: usize = 8 + 4 + 4 + 2 + 2 + 8 + 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheError {
    Truncated,
    BadMagic,
    BadVersion(u32),
    BadChecksum,
    BadLayout,
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheError::Truncated => write!(f, "file too short"),
            CacheError::BadMagic => write!(f, "not a peak cache file"),
            CacheError::BadVersion(v) => write!(f, "version {v}, expected {VERSION}"),
            CacheError::BadChecksum => write!(f, "checksum mismatch"),
            CacheError::BadLayout => write!(f, "header disagrees with file size"),
        }
    }
}

const CRC_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |c, &b| CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8))
}

fn cache_path(cache_dir: &Path, key: &str) -> Result<PathBuf, Error> {
    // Lowercase only: project folders sync across machines, and on
    // case-insensitive filesystems "Key" and "key" would be one file.
    let stem = key.split('.').next().unwrap_or("");
    const RESERVED: [&str; 4] = ["con", "prn", "aux", "nul"]; // Windows device names
    let reserved = RESERVED.contains(&stem)
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    let ok = !key.is_empty()
        && !key.starts_with('.')
        && !reserved
        && key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'));
    if !ok {
        return Err(Error::BadKey(format!(
            "{key:?}: use lowercase ASCII letters, digits, '.', '_' or '-', not starting with '.' and not a Windows device name"
        )));
    }
    Ok(cache_dir.join("peaks").join(format!("{key}.wpk")))
}

impl Peaks {
    pub fn to_bytes(&self) -> Vec<u8> {
        let pairs: usize = self.levels.iter().map(|l| l.channels.iter().map(Vec::len).sum::<usize>()).sum();
        let mut out = Vec::with_capacity(HEADER_LEN + pairs * 4 + 4);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&self.info.sample_rate.to_le_bytes());
        out.extend_from_slice(&self.info.channels.to_le_bytes());
        out.extend_from_slice(&u16::from(self.info.truncated).to_le_bytes());
        out.extend_from_slice(&self.info.frames.to_le_bytes());
        out.extend_from_slice(&self.base_bucket.to_le_bytes());
        for level in &self.levels {
            for series in &level.channels {
                for &(lo, hi) in series {
                    out.extend_from_slice(&lo.to_le_bytes());
                    out.extend_from_slice(&hi.to_le_bytes());
                }
            }
        }
        let crc = crc32(&out);
        out.extend_from_slice(&crc.to_le_bytes());
        out
    }

    /// Rejects anything that is not exactly a version-1 file this crate wrote.
    pub fn from_bytes(b: &[u8]) -> Result<Peaks, CacheError> {
        if b.len() < HEADER_LEN + 4 {
            return Err(CacheError::Truncated);
        }
        if &b[0..8] != MAGIC {
            return Err(CacheError::BadMagic);
        }
        let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        let version = u32_at(8);
        if version != VERSION {
            return Err(CacheError::BadVersion(version));
        }
        let (body, tail) = b.split_at(b.len() - 4);
        if crc32(body) != u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]) {
            return Err(CacheError::BadChecksum);
        }
        let sample_rate = u32_at(12);
        let channels = u16::from_le_bytes([b[16], b[17]]);
        let flags = u16::from_le_bytes([b[18], b[19]]);
        let frames = u64::from_le_bytes(b[20..28].try_into().expect("8 bytes"));
        let base_bucket = u32_at(28);
        if channels == 0 || sample_rate == 0 || base_bucket == 0 || flags > 1 {
            return Err(CacheError::BadLayout);
        }

        let lens = level_lens(frames, base_bucket);
        let total_pairs: u128 = lens.iter().map(|&n| n as u128 * channels as u128).sum();
        if total_pairs * 4 != (body.len() - HEADER_LEN) as u128 {
            return Err(CacheError::BadLayout);
        }

        let mut pos = HEADER_LEN;
        let mut levels = Vec::with_capacity(lens.len());
        let mut bucket_frames = base_bucket as u64;
        for &n in &lens {
            let mut series = Vec::with_capacity(channels as usize);
            for _ in 0..channels {
                let end = pos + n as usize * 4;
                series.push(
                    body[pos..end]
                        .chunks_exact(4)
                        .map(|p| (i16::from_le_bytes([p[0], p[1]]), i16::from_le_bytes([p[2], p[3]])))
                        .collect(),
                );
                pos = end;
            }
            levels.push(Level { bucket_frames, channels: series });
            bucket_frames *= 2;
        }
        Ok(Peaks {
            info: SourceInfo { sample_rate, channels, frames, truncated: flags == 1 },
            base_bucket,
            levels,
        })
    }

    /// Writes `<cache_dir>/peaks/<key>.wpk` through a temp file and rename.
    /// A crash leaves either nothing or a file whose checksum fails on load.
    pub fn save(&self, cache_dir: &Path, key: &str) -> Result<PathBuf, Error> {
        let path = cache_path(cache_dir, key)?;
        fs::create_dir_all(path.parent().expect("cache path has a parent"))?;
        // Unique per save, so two threads saving one recording never share a file.
        static SAVES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = SAVES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = path.with_extension(format!("wpk.tmp{}-{n}", std::process::id()));
        fs::write(&tmp, self.to_bytes())?;
        fs::rename(&tmp, &path)?;
        Ok(path)
    }

    /// `Ok(None)` when no cache file exists. A bad file is an error the caller
    /// answers by recomputing.
    pub fn load(cache_dir: &Path, key: &str) -> Result<Option<Peaks>, Error> {
        let path = cache_path(cache_dir, key)?;
        match fs::read(&path) {
            Ok(bytes) => Peaks::from_bytes(&bytes).map(Some).map_err(Error::Cache),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
