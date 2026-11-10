//! Min/max peak pyramid for waveform display.
//!
//! One streaming pass over a WAV file builds, per channel, `(min, max)` pairs
//! (normalized to `i16`) over fixed-size buckets of frames. Higher levels fold
//! pairs of buckets from the level below until about a thousand buckets remain.
//! [`Peaks::query`] picks the level that fits a pixel budget and returns a
//! borrowed slice, so scroll-frame queries allocate nothing.
//!
//! Peaks are a derived artifact: [`Peaks::save`] and [`Peaks::load`] keep them
//! under `<cache_dir>/peaks/<key>.wpk`, and recomputing is always safe.

mod cache;
pub mod wav;

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::ops::Range;
use std::path::Path;

pub use cache::CacheError;
pub use wav::{SampleKind, WavFormat, WavReader};

pub const DEFAULT_BASE_BUCKET: u32 = 256;
/// Folding stops at the first level with at most this many buckets.
const TOP_BUCKETS: u64 = 1024;
const READ_FRAMES: usize = 64 * 1024;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    NotWav(String),
    Unsupported(String),
    Malformed(String),
    BadParams(String),
    BadKey(String),
    Cache(CacheError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {e}"),
            Error::NotWav(m) => write!(f, "not a WAV file: {m}"),
            Error::Unsupported(m) => write!(f, "unsupported WAV: {m}"),
            Error::Malformed(m) => write!(f, "malformed WAV: {m}"),
            Error::BadParams(m) => write!(f, "bad parameters: {m}"),
            Error::BadKey(m) => write!(f, "bad cache key: {m}"),
            Error::Cache(c) => write!(f, "peak cache rejected: {c}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    /// Frames per level-0 bucket. Must be at least 1.
    pub base_bucket: u32,
}

impl Default for Params {
    fn default() -> Self {
        Params { base_bucket: DEFAULT_BASE_BUCKET }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub frames_done: u64,
    /// From the `data` chunk header; `None` for an open-ended stream.
    pub frames_total: Option<u64>,
}

/// What the pyramid was computed from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceInfo {
    pub sample_rate: u32,
    pub channels: u16,
    /// Frames actually read, which is fewer than declared when `truncated`.
    pub frames: u64,
    /// The file ended before the `data` chunk's declared length.
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Level {
    pub(crate) bucket_frames: u64,
    /// One `(min, max)` series per channel, all the same length.
    pub(crate) channels: Vec<Vec<(i16, i16)>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peaks {
    pub(crate) info: SourceInfo,
    pub(crate) base_bucket: u32,
    pub(crate) levels: Vec<Level>,
}

/// A zoom-range answer: consecutive buckets of one channel at one level.
#[derive(Clone, Copy, Debug)]
pub struct PeakView<'a> {
    pub level: usize,
    pub bucket_frames: u64,
    /// Frame at which `buckets[0]` starts.
    pub first_frame: u64,
    pub buckets: &'a [(i16, i16)],
}

/// Bucket counts of every level for a file of `frames` frames. The single
/// source of the level rule, shared by compute and cache validation.
pub(crate) fn level_lens(frames: u64, base_bucket: u32) -> Vec<u64> {
    let mut n = frames.div_ceil(base_bucket as u64);
    let mut lens = vec![n];
    while n > TOP_BUCKETS {
        n = n.div_ceil(2);
        lens.push(n);
    }
    lens
}

fn fold_half(series: &[(i16, i16)]) -> Vec<(i16, i16)> {
    series
        .chunks(2)
        .map(|pair| {
            pair.iter()
                .fold((i16::MAX, i16::MIN), |(lo, hi), &(l, h)| (lo.min(l), hi.max(h)))
        })
        .collect()
}

impl Peaks {
    /// One streaming pass over a WAV stream. Wrap unbuffered sources in a
    /// `BufReader` only if reads are tiny; the pass already reads in 64K-frame blocks.
    pub fn compute<R: Read>(
        reader: R,
        params: Params,
        mut progress: Option<&mut dyn FnMut(Progress)>,
    ) -> Result<Peaks, Error> {
        if params.base_bucket == 0 {
            return Err(Error::BadParams("base_bucket must be at least 1".into()));
        }
        let mut wav = WavReader::new(reader)?;
        let fmt = wav.format();
        let ch = fmt.channels as usize;
        let base = params.base_bucket as usize;
        let frames_total = wav.declared_frames();

        let reserve = frames_total.map_or(0, |f| f.div_ceil(base as u64).min(1 << 22) as usize);
        let mut series: Vec<Vec<(i16, i16)>> = (0..ch).map(|_| Vec::with_capacity(reserve)).collect();
        let mut cur = vec![(i16::MAX, i16::MIN); ch];
        let mut in_bucket = 0usize;
        let mut frames = 0u64;
        // Sized in bytes, not frames: a hostile channel count in the header
        // must not pick the allocation (reviewed; was 4.2 GB at 32767 ch).
        let frames_per_read = READ_FRAMES.min(((1 << 19) / ch).max(1));
        let mut buf = vec![0i16; frames_per_read * ch];

        loop {
            let n = wav.read_frames(&mut buf)?;
            if n == 0 {
                break;
            }
            let mut f = 0;
            while f < n {
                let take = (base - in_bucket).min(n - f);
                let seg = &buf[f * ch..(f + take) * ch];
                for (c, acc) in cur.iter_mut().enumerate() {
                    for &s in seg[c..].iter().step_by(ch) {
                        acc.0 = acc.0.min(s);
                        acc.1 = acc.1.max(s);
                    }
                }
                in_bucket += take;
                f += take;
                if in_bucket == base {
                    for (s, acc) in series.iter_mut().zip(cur.iter_mut()) {
                        s.push(std::mem::replace(acc, (i16::MAX, i16::MIN)));
                    }
                    in_bucket = 0;
                }
            }
            frames += n as u64;
            if let Some(cb) = progress.as_mut() {
                cb(Progress { frames_done: frames, frames_total });
            }
        }
        if in_bucket > 0 {
            for (s, acc) in series.iter_mut().zip(cur) {
                s.push(acc);
            }
        }

        let lens = level_lens(frames, params.base_bucket);
        let mut levels = vec![Level { bucket_frames: base as u64, channels: series }];
        for &len in &lens[1..] {
            let prev = levels.last().expect("level 0 exists");
            let channels: Vec<_> = prev.channels.iter().map(|s| fold_half(s)).collect();
            debug_assert!(channels.iter().all(|s| s.len() as u64 == len));
            levels.push(Level { bucket_frames: prev.bucket_frames * 2, channels });
        }

        Ok(Peaks {
            info: SourceInfo {
                sample_rate: fmt.sample_rate,
                channels: fmt.channels,
                frames,
                truncated: wav.truncated(),
            },
            base_bucket: params.base_bucket,
            levels,
        })
    }

    pub fn compute_path(
        path: &Path,
        params: Params,
        progress: Option<&mut dyn FnMut(Progress)>,
    ) -> Result<Peaks, Error> {
        Peaks::compute(File::open(path)?, params, progress)
    }

    pub fn info(&self) -> SourceInfo {
        self.info
    }

    pub fn base_bucket(&self) -> u32 {
        self.base_bucket
    }

    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    pub fn bucket_frames(&self, level: usize) -> u64 {
        self.levels[level].bucket_frames
    }

    /// The coarsest level whose buckets are no wider than `frames_per_pixel`,
    /// or level 0 when the zoom is finer than the base bucket.
    pub fn level_for(&self, frames_per_pixel: u64) -> usize {
        self.levels
            .iter()
            .rposition(|l| l.bucket_frames <= frames_per_pixel)
            .unwrap_or(0)
    }

    /// Buckets covering `range` (clamped to the file) at the level that gives
    /// at least `pixels` buckets across it. Returns a borrowed slice; never
    /// allocates. Panics if `channel >= info().channels`.
    pub fn query(&self, channel: usize, range: Range<u64>, pixels: usize) -> PeakView<'_> {
        let end = range.end.min(self.info.frames);
        let start = range.start.min(end);
        let frames_per_pixel = ((end - start) / pixels.max(1) as u64).max(1);
        let level = self.level_for(frames_per_pixel);
        let l = &self.levels[level];
        let series = &l.channels[channel];
        if start == end {
            return PeakView { level, bucket_frames: l.bucket_frames, first_frame: start, buckets: &series[0..0] };
        }
        let lo = (start / l.bucket_frames) as usize;
        let hi = (end.div_ceil(l.bucket_frames) as usize).min(series.len());
        PeakView {
            level,
            bucket_frames: l.bucket_frames,
            first_frame: lo as u64 * l.bucket_frames,
            buckets: &series[lo..hi],
        }
    }

    /// Like [`Peaks::query`] with the budget given as a zoom, in pixels per second.
    pub fn query_pps(&self, channel: usize, range: Range<u64>, pixels_per_second: f64) -> PeakView<'_> {
        let len = range.end.min(self.info.frames).saturating_sub(range.start) as f64;
        let pixels = (len * pixels_per_second / self.info.sample_rate as f64).ceil() as usize;
        self.query(channel, range, pixels)
    }
}
