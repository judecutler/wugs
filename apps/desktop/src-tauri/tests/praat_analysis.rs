use std::fs;
use std::path::{Path, PathBuf};

use wugs_desktop::praat_analysis::{self, analysis_script, parse_chunk, CHUNK_MS, MAX_CHUNKS};
use wugs_desktop::session::{self, Session};
use wugs_desktop::praat;
use wugs_model::ulid;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let d = std::env::temp_dir().join(format!("wugs-praat-analysis-{}", ulid()));
        fs::create_dir_all(&d).unwrap();
        TempDir(d)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

/// A mono 16 kHz sawtooth at `hz`, quiet for its last second.
fn voice(path: &Path, seconds: f64, hz: f64) {
    let rate = 16_000u32;
    let n = (f64::from(rate) * seconds) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + n * 2).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(n * 2).to_le_bytes());
    let silent_from = n.saturating_sub(rate);
    for i in 0..n {
        let phase = (f64::from(i) * hz / f64::from(rate)).fract();
        let v = if i >= silent_from { 0 } else { ((phase * 2.0 - 1.0) * 12_000.0) as i16 };
        b.extend_from_slice(&v.to_le_bytes());
    }
    fs::write(path, b).unwrap();
}

#[test]
fn chunks_parse_with_gaps_where_praat_finds_nothing() {
    let text = "0.005\t--undefined--\t61.2\t512\t1490\t2510\n0.015\t149.8\t70.0\t700\t1200\t2600\nnot a line\n";
    let c = parse_chunk(2, text);
    assert_eq!(c.start_ms, 2.0 * f64::from(CHUNK_MS));
    assert_eq!(c.pitch, vec![None, Some(149.8)]);
    assert_eq!(c.intensity, vec![Some(61.2), Some(70.0)]);
    assert_eq!(c.f3, vec![Some(2510.0), Some(2600.0)]);
}

#[test]
fn the_script_names_each_chunk_and_quotes_paths() {
    let s = analysis_script(Path::new(r#"C:\a "b"\rec"#), Path::new("/c"), &[0, 3]);
    assert!(s.contains(r#"Open long sound file: "C:\a ""b""\rec""#), "{s}");
    assert!(s.contains("t0 = 30\n") && s.contains("3.tsv") && s.contains("0.tsv"), "{s}");
    assert!(s.contains("To Pitch: 0, 75, 600") && s.contains("To Formant (burg): 0, 5, 5500, 0.025, 50"));
}

/// Runs only where Praat is installed (not on CI).
#[test]
fn the_installed_praat_analyses_a_recording_in_cached_chunks() {
    let st = praat::status(&TempDir::new().0);
    let Some(exe) = st.path.filter(|_| st.problem.is_none()).map(PathBuf::from) else {
        eprintln!("no Praat here: {:?}", st.problem);
        return;
    };
    let dir = TempDir::new();
    let mut s = Session::default();
    session::create_project(&mut s, dir.0.join("p").to_str().unwrap(), "prs_Ana").unwrap();
    let file = dir.0.join("voice.wav");
    voice(&file, 12.0, 150.0);
    let source = session::import_media(&mut s, file.to_str().unwrap()).unwrap().id;
    let job = praat_analysis::job(&mut s, &source).unwrap();

    let chunks = praat_analysis::analyse(&exe, &job, &[0, 1]).unwrap();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].pitch.len(), 1000, "10 s of 10 ms frames");
    assert_eq!(chunks[1].pitch.len(), 200, "the last 2 s");
    let voiced: Vec<f32> = chunks[0].pitch.iter().flatten().copied().collect();
    assert!(voiced.len() > 900, "nearly every frame voiced: {}", voiced.len());
    let mean = voiced.iter().sum::<f32>() / voiced.len() as f32;
    assert!((mean - 150.0).abs() < 2.0, "pitch about 150 Hz: {mean}");
    assert!(chunks[1].pitch[150..].iter().all(Option::is_none), "the silent second has no pitch");
    // Intensity needs ~40 ms of signal around a frame, which the recording's first frames lack.
    assert!(chunks[0].intensity[5..].iter().all(Option::is_some));
    assert!(chunks[0].f1.iter().flatten().count() > 900);

    // Cached: the second request does not run Praat (a bogus Praat would fail).
    let again = praat_analysis::analyse(Path::new("/no/such/praat"), &job, &[1, 0]).unwrap();
    assert_eq!(again[0], chunks[1]);
    assert_eq!(again[1], chunks[0]);
    assert!(praat_analysis::analyse(&exe, &job, &[0; MAX_CHUNKS + 1]).is_err());
    // A chunk past the end is empty, not an error.
    assert!(praat_analysis::analyse(&exe, &job, &[5]).unwrap()[0].pitch.is_empty());
}
