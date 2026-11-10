//! PRD gate 2 (engine half): peaks for a 2-hour stereo 44.1 kHz 16-bit WAV.
//!
//!   cargo run -p wugs-peaks --release --example gate2 [workdir]
//!
//! Generates the WAV (deterministic sine + noise), measures one compute pass,
//! the cache, and 1000 scroll-frame queries, then deletes the workdir.

use std::fs::{self, File};
use std::hint::black_box;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use wugs_peaks::{Params, Peaks};

const RATE: u32 = 44_100;
const SECS: u64 = 7_200;
const CH: usize = 2;
const BLOCK_FRAMES: usize = 64 * 1024;
/// Every SPIKE_EVERY-th base bucket gets an extreme sample on its exact
/// first frame (ch 0) and last frame (ch 1).
const SPIKE_EVERY: u64 = 50_000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn header(frames: u64) -> Vec<u8> {
    let data = (frames * CH as u64 * 2) as u32;
    let mut h = Vec::new();
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&(CH as u16).to_le_bytes());
    h.extend_from_slice(&RATE.to_le_bytes());
    h.extend_from_slice(&(RATE * CH as u32 * 2).to_le_bytes());
    h.extend_from_slice(&(CH as u16 * 2).to_le_bytes());
    h.extend_from_slice(&16u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data.to_le_bytes());
    h
}

/// Returns each channel's true (min, max), tracked while writing.
fn generate(path: &Path, frames: u64) -> std::io::Result<[(i16, i16); CH]> {
    let table: Vec<i32> = (0..4096)
        .map(|i| ((i as f64 / 4096.0 * std::f64::consts::TAU).sin() * 32767.0) as i32)
        .collect();
    let mut w = BufWriter::with_capacity(1 << 20, File::create(path)?);
    w.write_all(&header(frames))?;
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let incs = [((440.0 / RATE as f64) * 4_294_967_296.0) as u32, ((660.0 / RATE as f64) * 4_294_967_296.0) as u32];
    let mut phase = [0u32; CH];
    let mut ext = [(i16::MAX, i16::MIN); CH];
    let mut buf = Vec::with_capacity(BLOCK_FRAMES * CH * 2);
    let mut f = 0u64;
    while f < frames {
        buf.clear();
        let n = (frames - f).min(BLOCK_FRAMES as u64);
        for k in 0..n {
            let t = (f + k) % 441_000;
            let tri = if t < 220_500 { t } else { 441_000 - t } as i32;
            let env = 4_000 + tri * 16_000 / 220_500;
            for c in 0..CH {
                phase[c] = phase[c].wrapping_add(incs[c]);
                let tone = table[(phase[c] >> 20) as usize] * if c == 0 { env } else { 12_000 } / 32_768;
                let noise = (rng.next() >> 33) as i32 % 6_001 - 3_000;
                let mut s = (tone + noise).clamp(-32_768, 32_767) as i16;
                // Spikes on exact bucket edges, so an off-by-one in bucket
                // boundaries fails verification instead of hiding in noise.
                let g = f + k;
                if c == 0 && g.is_multiple_of(256 * SPIKE_EVERY) {
                    s = i16::MAX;
                } else if c == 1 && (g + 1).is_multiple_of(256 * SPIKE_EVERY) {
                    s = i16::MIN;
                }
                ext[c].0 = ext[c].0.min(s);
                ext[c].1 = ext[c].1.max(s);
                buf.extend_from_slice(&s.to_le_bytes());
            }
        }
        w.write_all(&buf)?;
        f += n;
    }
    w.flush()?;
    Ok(ext)
}

fn io_only_pass(path: &Path) -> std::io::Result<Duration> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; BLOCK_FRAMES * CH * 2];
    let t = Instant::now();
    let mut total = 0usize;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n;
        black_box(buf[0]);
    }
    black_box(total);
    Ok(t.elapsed())
}

fn vm_hwm_mb() -> Option<f64> {
    let s = fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

fn pct(sorted: &[u128], p: f64) -> u128 {
    sorted[((sorted.len() as f64 * p).ceil() as usize).clamp(1, sorted.len()) - 1]
}

fn report(label: &str, mut ns: Vec<u128>) {
    ns.sort_unstable();
    println!(
        "{label:<22} p50 {:>7} ns   p95 {:>7} ns   p99 {:>7} ns   max {:>8} ns",
        pct(&ns, 0.50),
        pct(&ns, 0.95),
        pct(&ns, 0.99),
        ns[ns.len() - 1]
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("target/gate2-work"));
    fs::create_dir_all(&dir)?;
    let cleanup = Cleanup(dir.clone());
    let wav = dir.join("gate2.wav");
    let cache = dir.join("cache");
    let frames = SECS * RATE as u64;

    let t = Instant::now();
    let truth = generate(&wav, frames)?;
    let size = fs::metadata(&wav)?.len();
    println!("generated {} frames, {:.0} MB WAV in {:.1} s", frames, size as f64 / 1e6, t.elapsed().as_secs_f64());

    let drop_caches = || std::process::Command::new("sync").status().is_ok() && fs::write("/proc/sys/vm/drop_caches", "3").is_ok();
    let dropped = drop_caches();
    println!("page cache dropped before io pass: {dropped}");
    let io_before = io_only_pass(&wav)?;
    // Drop again, so COMPUTE pays the disk too; without this the number is
    // warm-cache only (reviewed).
    let dropped_compute = drop_caches();
    println!("page cache dropped before compute: {dropped_compute}");

    let mut calls = 0u64;
    let mut cb = |_p: wugs_peaks::Progress| calls += 1;
    let t = Instant::now();
    let peaks = Peaks::compute_path(&wav, Params::default(), Some(&mut cb))?;
    let compute = t.elapsed();
    let io_after = io_only_pass(&wav)?;

    let mb = size as f64 / 1e6;
    println!("io-only read pass (first)  {:.2} s  ({:.0} MB/s)", io_before.as_secs_f64(), mb / io_before.as_secs_f64());
    println!("io-only read pass (after)  {:.2} s  ({:.0} MB/s)", io_after.as_secs_f64(), mb / io_after.as_secs_f64());
    println!(
        "COMPUTE one pass ({})      {:.2} s  ({:.0} MB/s, {:.0}x realtime, {} progress calls)",
        if dropped_compute { "cold cache" } else { "warm cache" },
        compute.as_secs_f64(),
        mb / compute.as_secs_f64(),
        SECS as f64 / compute.as_secs_f64(),
        calls
    );
    println!("peak memory (VmHWM)        {} MB", vm_hwm_mb().map_or("n/a".into(), |m| format!("{m:.1}")));
    println!("levels {}  base buckets/channel {}", peaks.level_count(), peaks.query(0, 0..frames, frames as usize / 256).buckets.len());

    let top = peaks.query(0, 0..frames, 1).level;
    for (c, want) in truth.iter().enumerate() {
        let got = peaks
            .query(c, 0..frames, 1)
            .buckets
            .iter()
            .fold((i16::MAX, i16::MIN), |a, &(lo, hi)| (a.0.min(lo), a.1.max(hi)));
        assert_eq!(got, *want, "channel {c} global min/max from level {top}");
    }
    let mut f = File::open(&wav)?;
    let nb = frames.div_ceil(256);
    let check_bucket = |b: u64, f: &mut File| -> std::io::Result<()> {
        let (a, e) = (b * 256, ((b + 1) * 256).min(frames));
        let mut raw = vec![0u8; (e - a) as usize * CH * 2];
        f.seek(SeekFrom::Start(44 + a * CH as u64 * 2))?;
        f.read_exact(&mut raw)?;
        for c in 0..CH {
            let it = raw.chunks_exact(CH * 2).map(|fr| i16::from_le_bytes([fr[c * 2], fr[c * 2 + 1]]));
            let want = (it.clone().min().unwrap(), it.max().unwrap());
            assert_eq!(peaks.query(c, a..e, 256).buckets[0], want, "bucket {b} ch {c}");
        }
        Ok(())
    };
    let mut rng = Rng(42);
    for _ in 0..50 {
        check_bucket(rng.next() % nb, &mut f)?;
    }
    // Spiked buckets carry an extreme on their exact first and last frame,
    // so a boundary off-by-one fails here instead of hiding in noise.
    let mut spiked = 0;
    for b in (0..nb).step_by(SPIKE_EVERY as usize) {
        check_bucket(b, &mut f)?;
        spiked += 1;
    }
    println!("verified: global min/max per channel, 50 random base buckets and {spiked} edge-spiked buckets match the raw file");

    let t = Instant::now();
    let path = peaks.save(&cache, "gate2-blake3-0000.256")?;
    let save = t.elapsed();
    let t = Instant::now();
    let loaded = Peaks::load(&cache, "gate2-blake3-0000.256")?.expect("just saved");
    let load = t.elapsed();
    assert!(loaded == peaks, "cache round trip");
    println!(
        "cache file                 {:.1} MB ({:.2}% of WAV)   save {:.0} ms   load {:.0} ms",
        fs::metadata(&path)?.len() as f64 / 1e6,
        fs::metadata(&path)?.len() as f64 / size as f64 * 100.0,
        save.as_secs_f64() * 1e3,
        load.as_secs_f64() * 1e3
    );

    let mut rng = Rng(7);
    let qs: Vec<(usize, u64, u64)> = (0..1000)
        .map(|_| {
            let fpp = 1u64 << (8 + rng.next() % 10);
            let window = 2000 * fpp;
            ((rng.next() % 2) as usize, rng.next() % (frames - window + 1), window)
        })
        .collect();
    for q in &qs {
        black_box(loaded.query(q.0, q.1..q.1 + q.2, 2000));
    }
    let (mut only, mut scan) = (Vec::new(), Vec::new());
    for &(c, s, w) in &qs {
        let t = Instant::now();
        let v = loaded.query(c, s..s + w, 2000);
        black_box(&v);
        only.push(t.elapsed().as_nanos());
        let t = Instant::now();
        let v = loaded.query(c, s..s + w, 2000);
        let mut acc = 0i64;
        for &(lo, hi) in v.buckets {
            acc += lo as i64 + hi as i64;
        }
        black_box(acc);
        scan.push(t.elapsed().as_nanos());
    }
    println!("1000 queries, 2000 px window, 256..131072 frames/px (2^8..2^17), random start/channel:");
    report("query only", only);
    report("query + read every bucket", scan);

    drop(cleanup);
    println!("workdir removed: {}", !dir.exists());
    Ok(())
}
