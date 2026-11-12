//! Compressed recordings decode to WAV and import like any WAV.

use std::fs;
use std::path::{Path, PathBuf};

use wugs_desktop::convert::{self, Converted};
use wugs_desktop::session::{self, Session};
use wugs_model::ulid;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio").join(name)
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let p = std::env::temp_dir().join(format!("wugs-convert-test-{}", ulid()));
        fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn every_compressed_format_decodes_to_a_wav_of_the_same_length() {
    let dir = TempDir::new();
    for (name, rate, channels) in [
        ("tone.m4a", 16000, 1),
        ("tone.mp3", 16000, 1),
        ("tone.flac", 16000, 1),
        ("tone.ogg", 16000, 1),
        ("stereo.m4a", 22050, 2),
        ("tone-alac.m4a", 16000, 1),
        ("tone-mulaw.wav", 16000, 1),
    ] {
        assert_eq!(convert::is_wav(&fixture(name)), name.ends_with(".wav"), "{name}");
        let out = dir.0.join(format!("{name}.wav"));
        let Converted { sample_rate, channels: ch, frames } = convert::to_wav(&fixture(name), &out).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((sample_rate, ch), (rate, channels), "{name}");
        let want = rate as i64 / 2;
        assert!((frames as i64 - want).abs() <= 2048, "{name}: {frames} frames for 0.5 s at {rate} Hz");
        assert!(convert::is_wav(&out), "{name}: the output is a WAV");
        let bytes = fs::read(&out).unwrap();
        assert_eq!(bytes.len() as u64, 44 + frames * 2 * channels as u64, "{name}: header sizes match the data");
        let samples: Vec<i16> = bytes[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
        // ffmpeg's sine source plays at an eighth of full scale (about 4096).
        assert!((2_500..12_000).contains(&peak), "{name}: the tone is there at its level (peak {peak})");
    }
}

#[test]
fn a_file_that_is_not_audio_is_refused_and_leaves_nothing() {
    let dir = TempDir::new();
    let notes = dir.0.join("notes.m4a");
    fs::write(&notes, b"not really audio at all").unwrap();
    let out = dir.0.join("out.wav");
    let err = convert::to_wav(&notes, &out).unwrap_err();
    assert!(err.contains("not audio wugs can read"), "{err}");
    assert!(!out.exists(), "no partial WAV left behind");
}

#[test]
fn a_decoded_recording_imports_under_its_own_name() {
    let dir = TempDir::new();
    let mut s: Session = None;
    session::create_project(&mut s, dir.0.join("p").to_str().unwrap(), "prs_Test").unwrap();
    let wav = dir.0.join("decoded.wav");
    convert::to_wav(&fixture("tone.m4a"), &wav).unwrap();
    let row = session::import_media_named(&mut s, &wav, "voice memo.m4a", None).unwrap();
    assert_eq!(row.label, "voice memo.m4a");
    assert!(row.has_media);
    let listed = session::list_sources(&mut s).unwrap();
    assert!(listed.iter().any(|r| r.id == row.id && r.has_media));
}

#[test]
fn a_wav_the_peaks_reader_refuses_is_decoded_instead() {
    let dir = TempDir::new();
    let mut s: Session = None;
    session::create_project(&mut s, dir.0.join("p").to_str().unwrap(), "prs_Test").unwrap();
    let mulaw = fixture("tone-mulaw.wav");
    // The import command falls back to decoding on exactly this message.
    let err = session::import_media(&mut s, mulaw.to_str().unwrap()).unwrap_err();
    assert!(err.contains("unsupported WAV"), "{err}");
    let wav = dir.0.join("decoded.wav");
    convert::to_wav(&mulaw, &wav).unwrap();
    assert!(session::import_media_named(&mut s, &wav, "tone-mulaw.wav", None).unwrap().has_media);
}
