//! The media bin's waveforms (roadmap 66, docs/design/EDITOR.md): a short
//! overview of each recording, made off the session thread so opening the
//! Media panel never holds up saving or selecting.
//!
//! The session thread only finds the recording's file and peak cache
//! (`locate`); the overview is read from that cache, or computed and saved
//! there when it is missing (the same `.wpk` the Timeline reads, so the work
//! is done once), on a worker thread, one recording at a time (`OVERVIEWS`).

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use wugs_peaks::{Params, Peaks};

use crate::session::{self, Session};

/// Points in a card's waveform.
pub const POINTS: usize = 120;

/// One overview computed at a time: a bin of twenty hour-long recordings
/// reads them one after another, not all at once.
pub static OVERVIEWS: Mutex<()> = Mutex::new(());

/// Where a recording's audio and peak cache are.
pub struct Located {
    pub media: PathBuf,
    pub cache_dir: PathBuf,
    pub key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum Overview {
    /// The waveform, `POINTS` long, in 16-bit sample values.
    Ready { duration_ms: f64, mins: Vec<i16>, maxs: Vec<i16> },
    /// The recording's audio is not on this computer (not synced, or removed).
    Missing,
}

/// On the session thread: the recording's files, or `None` when its audio is not here.
pub fn locate(s: &mut Session, source_id: &str) -> Result<Option<Located>, String> {
    let p = session::project(s)?;
    let blob = session::audio_blob(p, source_id)?;
    let key = wugs_blob::parse_ref(&blob).ok_or_else(|| format!("malformed blob ref {blob:?}"))?.to_string();
    let media = p.root.join("blobs").join(&key);
    let cache_dir = p.root.join("cache");
    if !media.is_file() {
        return Ok(None);
    }
    Ok(Some(Located { media, cache_dir, key }))
}

/// Off the session thread: the overview from the cache, computing it first if needed.
pub fn overview(at: &Located) -> Result<Overview, String> {
    let _one = OVERVIEWS.lock().unwrap_or_else(|e| e.into_inner());
    let peaks = match Peaks::load(&at.cache_dir, &at.key) {
        Ok(Some(peaks)) => peaks,
        Ok(None) | Err(_) => {
            let peaks = Peaks::compute_path(&at.media, Params::default(), None).map_err(|e| format!("could not read the recording: {e}"))?;
            // A failed save costs only a recompute next time.
            let _ = peaks.save(&at.cache_dir, &at.key);
            peaks
        }
    };
    let info = peaks.info();
    let view = peaks.query(0, 0..info.frames, POINTS);
    let (mins, maxs) = fold(view.buckets, POINTS);
    let duration_ms = if info.sample_rate == 0 { 0.0 } else { info.frames as f64 * 1000.0 / info.sample_rate as f64 };
    Ok(Overview::Ready { duration_ms, mins, maxs })
}

/// `buckets` folded into at most `n` points: each point the lowest minimum
/// and highest maximum of its share, so no peak is lost.
pub fn fold(buckets: &[(i16, i16)], n: usize) -> (Vec<i16>, Vec<i16>) {
    if buckets.len() <= n || n == 0 {
        return buckets.iter().copied().unzip();
    }
    let mut mins = Vec::with_capacity(n);
    let mut maxs = Vec::with_capacity(n);
    for i in 0..n {
        let from = i * buckets.len() / n;
        let to = ((i + 1) * buckets.len() / n).max(from + 1);
        let part = &buckets[from..to];
        mins.push(part.iter().map(|b| b.0).min().unwrap_or(0));
        maxs.push(part.iter().map(|b| b.1).max().unwrap_or(0));
    }
    (mins, maxs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_keeps_every_peak() {
        let b: Vec<(i16, i16)> = (0..10).map(|i| (-(i as i16), i as i16)).collect();
        assert_eq!(fold(&b, 20), (b.iter().map(|x| x.0).collect(), b.iter().map(|x| x.1).collect()));
        assert_eq!(fold(&b, 3), (vec![-2, -5, -9], vec![2, 5, 9]));
        assert_eq!(fold(&b, 3).0.len(), 3);
    }

    #[test]
    fn the_sample_has_an_overview_and_a_missing_file_says_so() {
        let dir = std::env::temp_dir().join(format!("wugs-bin-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let src = session::list_sources(&mut s).unwrap()[0].id.clone();
        let at = locate(&mut s, &src).unwrap().expect("audio here");
        match overview(&at).unwrap() {
            Overview::Ready { duration_ms, mins, maxs } => {
                assert!(duration_ms > 1000.0);
                assert_eq!(mins.len(), POINTS);
                assert!(maxs.iter().any(|&m| m > 1000), "speech shows");
            }
            Overview::Missing => panic!("audio is here"),
        }
        std::fs::remove_file(&at.media).unwrap();
        assert!(locate(&mut s, &src).unwrap().is_none());
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
