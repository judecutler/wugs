mod common;

use std::io::Cursor;

use common::*;
use wugs_peaks::{CacheError, Error, Params, Peaks, Progress};

fn compute(samples: &[i16], channels: u16, base: u32) -> Peaks {
    let bytes = Wav::pcm(16, channels, enc16(samples)).build();
    Peaks::compute(Cursor::new(bytes), Params { base_bucket: base }, None).unwrap()
}

fn random_samples(seed: u64, n: usize) -> Vec<i16> {
    let mut rng = Rng(seed);
    (0..n).map(|_| rng.i16()).collect()
}

/// Straight-from-the-definition min/max of one channel over frames [a, b).
fn naive(samples: &[i16], channels: usize, ch: usize, a: usize, b: usize) -> (i16, i16) {
    let it = (a..b).map(|f| samples[f * channels + ch]);
    (it.clone().min().unwrap(), it.max().unwrap())
}

fn assert_matches_reference(samples: &[i16], channels: usize, base: u32) {
    let p = compute(samples, channels as u16, base);
    let frames = samples.len() / channels;
    assert_eq!(p.info().frames, frames as u64);
    for level in 0..p.level_count() {
        let bf = p.bucket_frames(level) as usize;
        for ch in 0..channels {
            let all = p.query(ch, 0..frames as u64, (frames / bf).max(1));
            assert_eq!(all.level, level, "pixel budget sized for level {level} lands on it");
            assert_eq!(all.buckets.len(), frames.div_ceil(bf), "level {level} bucket count");
            for (j, &got) in all.buckets.iter().enumerate() {
                let a = j * bf;
                let b = ((j + 1) * bf).min(frames);
                assert_eq!(got, naive(samples, channels, ch, a, b), "level {level} ch {ch} bucket {j}");
            }
        }
    }
}

#[test]
fn every_level_matches_naive_reference() {
    for (seed, channels, frames, base) in [
        (1, 1, 50_000, 4),
        (2, 2, 40_001, 4),
        (3, 2, 30_000, 7),
        (4, 3, 12_345, 1),
        (5, 1, 400_000, 256),
        (6, 2, 123_457, 256),
        // base 7 never divides a read block, and 200k frames spans several
        // blocks, so a bucket straddles two reads (reviewed: untested before).
        (7, 2, 200_003, 7),
    ] {
        let s = random_samples(seed, frames * channels);
        assert_matches_reference(&s, channels, base);
    }
}

#[test]
fn all_levels_are_reachable_and_checked() {
    let s = random_samples(9, 2 * 50_000);
    let p = compute(&s, 2, 4);
    assert!(p.level_count() >= 5);
    let frames = 50_000usize;
    let mut seen = vec![false; p.level_count()];
    for level in 0..p.level_count() {
        let bf = p.bucket_frames(level) as usize;
        seen[p.query(0, 0..frames as u64, frames / bf).level] = true;
    }
    assert!(seen.iter().all(|&x| x), "queries sized per level reach every level");
}

#[test]
fn level_shape_stops_near_a_thousand_buckets() {
    let p = compute(&random_samples(3, 100_000), 1, 4);
    let top = p.query(0, 0..100_000, 1).buckets.len();
    assert!(top <= 1024 && top > 256, "top level has {top} buckets");
    assert_eq!(p.bucket_frames(0), 4);
    for l in 1..p.level_count() {
        assert_eq!(p.bucket_frames(l), p.bucket_frames(l - 1) * 2);
    }
}

#[test]
fn edge_sizes_around_the_bucket_boundary() {
    for frames in [0usize, 1, 3, 4, 5, 8, 9] {
        let s = random_samples(frames as u64 + 1, frames * 2);
        let p = compute(&s, 2, 4);
        assert_eq!(p.info().frames, frames as u64);
        if frames > 0 {
            assert_matches_reference(&s, 2, 4);
        } else {
            assert!(p.query(0, 0..100, 10).buckets.is_empty());
        }
    }
}

#[test]
fn asking_for_fewer_pixels_returns_a_higher_level() {
    let frames = 4 * 65_536;
    let p = compute(&random_samples(7, frames), 1, 4);
    assert_eq!(p.level_count(), 7);
    let fine = p.query(0, 0..frames as u64, 65_536);
    let mid = p.query(0, 0..frames as u64, 16_384);
    let coarse = p.query(0, 0..frames as u64, 2_048);
    let top = p.query(0, 0..frames as u64, 8);
    assert_eq!((fine.level, mid.level, coarse.level), (0, 2, 5));
    assert_eq!(top.level, 6, "the pyramid has nothing coarser than its ~1k-bucket top");
    assert_eq!(top.buckets.len(), 1024);
    assert!(fine.buckets.len() >= 65_536 && mid.buckets.len() >= 16_384 && coarse.buckets.len() >= 2_048);
}

#[test]
fn finer_than_base_zoom_stays_on_level_zero() {
    let p = compute(&random_samples(8, 100_000), 1, 256);
    assert_eq!(p.query(0, 0..2_000, 2_000).level, 0);
}

#[test]
fn query_windows_align_clamp_and_pps_agrees() {
    let frames = 200_000usize;
    let s = random_samples(11, frames * 2);
    let p = compute(&s, 2, 16);
    let v = p.query(1, 5_000..9_000, 100);
    assert_eq!(v.first_frame % v.bucket_frames, 0);
    assert!(v.first_frame <= 5_000);
    assert!(v.first_frame + v.buckets.len() as u64 * v.bucket_frames >= 9_000);
    assert_eq!(v.buckets.len() as u64, 9_000u64.div_ceil(v.bucket_frames) - 5_000 / v.bucket_frames);

    let clamped = p.query(0, 190_000..u64::MAX, 50);
    assert!(clamped.first_frame + clamped.buckets.len() as u64 * clamped.bucket_frames >= frames as u64);
    assert!(p.query(0, 500..500, 10).buckets.is_empty());
    assert!(p.query(0, 300_000..400_000, 10).buckets.is_empty());

    let secs = 4_000f64 / 44100.0;
    let by_pps = p.query_pps(1, 5_000..9_000, 100.0 / secs);
    assert_eq!(by_pps.level, v.level);
}

#[test]
fn progress_is_monotonic_and_ends_at_the_total() {
    let s = random_samples(5, 200_000 * 2);
    let bytes = Wav::pcm(16, 2, enc16(&s)).build();
    let mut seen: Vec<Progress> = Vec::new();
    let mut cb = |p: Progress| seen.push(p);
    Peaks::compute(Cursor::new(bytes), Params::default(), Some(&mut cb)).unwrap();
    assert!(seen.len() >= 3);
    assert!(seen.windows(2).all(|w| w[0].frames_done < w[1].frames_done));
    let last = seen.last().unwrap();
    assert_eq!(last.frames_done, 200_000);
    assert_eq!(last.frames_total, Some(200_000));
}

#[test]
fn compute_path_reads_a_file() {
    let dir = TempDir::new("path");
    let s = random_samples(5, 5_000);
    let path = dir.0.join("a.wav");
    std::fs::write(&path, Wav::pcm(16, 1, enc16(&s)).build()).unwrap();
    let from_file = Peaks::compute_path(&path, Params::default(), None).unwrap();
    assert_eq!(from_file, compute(&s, 1, 256));
    assert!(matches!(Peaks::compute_path(&dir.0.join("nope.wav"), Params::default(), None), Err(Error::Io(_))));
}

#[test]
fn zero_base_bucket_is_rejected() {
    let bytes = Wav::pcm(16, 1, enc16(&[1, 2])).build();
    let r = Peaks::compute(Cursor::new(bytes), Params { base_bucket: 0 }, None);
    assert!(matches!(r, Err(Error::BadParams(_))));
}

#[test]
fn cache_round_trip_gives_identical_queries() {
    let dir = TempDir::new("rt");
    let frames = 120_000usize;
    let p = compute(&random_samples(21, frames * 2), 2, 8);
    let path = p.save(&dir.0, "blake3-abc.256").unwrap();
    assert_eq!(path, dir.0.join("peaks").join("blake3-abc.256.wpk"));
    let q = Peaks::load(&dir.0, "blake3-abc.256").unwrap().expect("cache present");
    assert_eq!(p, q);
    let mut rng = Rng(77);
    for _ in 0..200 {
        let a = rng.next() % frames as u64;
        let b = a + rng.next() % (frames as u64 - a + 1);
        let px = 1 + (rng.next() % 3000) as usize;
        let ch = (rng.next() % 2) as usize;
        let (x, y) = (p.query(ch, a..b, px), q.query(ch, a..b, px));
        assert_eq!((x.level, x.first_frame, x.buckets), (y.level, y.first_frame, y.buckets));
    }
    assert_eq!(std::fs::read(&path).unwrap(), q.to_bytes());
}

#[test]
fn truncated_flag_survives_the_cache() {
    let mut w = Wav::pcm(16, 1, enc16(&[1, 2, 3]));
    w.declared_len = Some(100);
    let p = Peaks::compute(Cursor::new(w.build()), Params::default(), None).unwrap();
    assert!(Peaks::from_bytes(&p.to_bytes()).unwrap().info().truncated);
}

#[test]
fn missing_cache_is_none() {
    let dir = TempDir::new("missing");
    assert!(Peaks::load(&dir.0, "nothing").unwrap().is_none());
}

fn saved_bytes(name: &str) -> (TempDir, std::path::PathBuf, Vec<u8>) {
    let dir = TempDir::new(name);
    let p = compute(&random_samples(31, 20_000), 2, 4);
    let path = p.save(&dir.0, "k").unwrap();
    let bytes = std::fs::read(&path).unwrap();
    (dir, path, bytes)
}

fn load_err(dir: &TempDir) -> CacheError {
    match Peaks::load(&dir.0, "k") {
        Err(Error::Cache(c)) => c,
        other => panic!("expected a cache error, got {other:?}"),
    }
}

#[test]
fn wrong_version_is_rejected() {
    let (dir, path, mut b) = saved_bytes("ver");
    b[8..12].copy_from_slice(&2u32.to_le_bytes());
    std::fs::write(&path, &b).unwrap();
    assert_eq!(load_err(&dir), CacheError::BadVersion(2));
}

#[test]
fn bad_magic_truncation_and_bit_flips_are_rejected() {
    let (dir, path, b) = saved_bytes("corrupt");

    let mut bad = b.clone();
    bad[0] = b'X';
    std::fs::write(&path, &bad).unwrap();
    assert_eq!(load_err(&dir), CacheError::BadMagic);

    std::fs::write(&path, &b[..b.len() / 2]).unwrap();
    assert_eq!(load_err(&dir), CacheError::BadChecksum);

    std::fs::write(&path, &b[..10]).unwrap();
    assert_eq!(load_err(&dir), CacheError::Truncated);

    for pos in [40, b.len() / 2, b.len() - 1] {
        let mut flipped = b.clone();
        flipped[pos] ^= 0x01;
        std::fs::write(&path, &flipped).unwrap();
        assert_eq!(load_err(&dir), CacheError::BadChecksum, "flip at {pos}");
    }
}

#[test]
fn header_claiming_a_different_size_is_rejected_even_with_a_valid_checksum() {
    let (_dir, _path, mut b) = saved_bytes("layout");
    b[20..28].copy_from_slice(&1_000_000u64.to_le_bytes());
    let body_len = b.len() - 4;
    let crc = crc32_for_test(&b[..body_len]);
    b[body_len..].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(Peaks::from_bytes(&b).unwrap_err(), CacheError::BadLayout);
}

fn crc32_for_test(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &byte in data {
        c ^= byte as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

#[test]
fn keys_cannot_escape_the_cache_directory() {
    let dir = TempDir::new("keys");
    let p = compute(&[1, 2, 3], 1, 4);
    for key in ["", "../x", "a/b", ".hidden", "a\\b", "x\0y"] {
        assert!(matches!(p.save(&dir.0, key), Err(Error::BadKey(_))), "{key:?}");
        assert!(matches!(Peaks::load(&dir.0, key), Err(Error::BadKey(_))), "{key:?}");
    }
}
