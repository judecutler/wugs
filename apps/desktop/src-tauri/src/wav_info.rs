//! What a field recorder writes into a WAV file about the recording: the
//! Broadcast Wave `bext` chunk (Zoom, Tascam, Sound Devices, Marantz and most
//! professional recorders: when it was recorded, on what, a description) and
//! the RIFF `LIST`/`INFO` tags (a creation date, an artist, a comment). Read
//! at import, so a recording arrives with its date and equipment as details
//! instead of the person typing them from the recorder's screen.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Text from a fixed-width field: up to the first NUL, trimmed; Latin-1 when not UTF-8.
fn field(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let raw = &bytes[..end];
    let s = match std::str::from_utf8(raw) {
        Ok(s) => s.to_string(),
        Err(_) => raw.iter().map(|&b| b as char).collect(),
    };
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A recorder's date as `YYYY-MM-DD` (bext allows `-`, `_`, `:`, ` ` or `.` between the parts).
fn date(s: &str) -> Option<String> {
    let d: Vec<&str> = s.split(['-', '_', ':', ' ', '.', '/']).filter(|p| !p.is_empty()).collect();
    match d.as_slice() {
        [y, m, d, ..] if y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()) && m.len() <= 2 && d.len() <= 2 => {
            let (m, d): (u32, u32) = (m.parse().ok()?, d.parse().ok()?);
            ((1..=12).contains(&m) && (1..=31).contains(&d) && *y != "0000").then(|| format!("{y}-{m:02}-{d:02}"))
        }
        _ => None,
    }
}

/// A recorder's time as `HH:MM:SS`.
fn time(s: &str) -> Option<String> {
    let t: Vec<u32> = s.split([':', '-', '_', '.', ' ']).filter(|p| !p.is_empty()).map(|p| p.parse().ok()).collect::<Option<_>>()?;
    match t.as_slice() {
        [h, m, sec, ..] if *h < 24 && *m < 60 && *sec < 61 => Some(format!("{h:02}:{m:02}:{sec:02}")),
        _ => None,
    }
}

/// The recording's details its file carries, as (key, value): `date`,
/// `time`, `equipment`, `description`, `recorded_by`. Empty when the file
/// has none or is not a RIFF WAV. Reads only the chunk headers and the two
/// small chunks, never the audio.
pub fn read(path: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let Ok(mut f) = File::open(path) else { return out };
    let mut head = [0u8; 12];
    if f.read_exact(&mut head).is_err() || &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return out;
    }
    let put = |out: &mut Vec<(String, String)>, k: &str, v: String| {
        if !v.is_empty() && !out.iter().any(|(x, _)| x == k) {
            out.push((k.to_string(), v));
        }
    };
    // At most 64 chunks, and only small ones are read whole.
    for _ in 0..64 {
        let mut ch = [0u8; 8];
        if f.read_exact(&mut ch).is_err() {
            break;
        }
        let size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]) as u64;
        let next = f.stream_position().map(|p| p + size + (size & 1)).unwrap_or(u64::MAX);
        match &ch[0..4] {
            b"bext" if size >= 602 && size <= 1 << 20 => {
                let mut b = vec![0u8; size as usize];
                if f.read_exact(&mut b).is_ok() {
                    let description = field(&b[0..256]);
                    put(&mut out, "date", date(&field(&b[320..330])).unwrap_or_default());
                    put(&mut out, "time", time(&field(&b[330..338])).unwrap_or_default());
                    put(&mut out, "equipment", field(&b[256..288]));
                    // Some recorders fill the description with their own settings (`sSPEED=…`); that is not one a person wrote.
                    if !description.contains('=') {
                        put(&mut out, "description", description);
                    }
                }
            }
            b"LIST" if size >= 4 && size <= 1 << 20 => {
                let mut b = vec![0u8; size as usize];
                if f.read_exact(&mut b).is_ok() && &b[0..4] == b"INFO" {
                    let mut at = 4;
                    while at + 8 <= b.len() {
                        let id = &b[at..at + 4];
                        let n = u32::from_le_bytes([b[at + 4], b[at + 5], b[at + 6], b[at + 7]]) as usize;
                        let end = (at + 8 + n).min(b.len());
                        let v = field(&b[at + 8..end]);
                        match id {
                            b"ICRD" => put(&mut out, "date", date(&v).unwrap_or_default()),
                            b"IART" => put(&mut out, "recorded_by", v),
                            b"ICMT" => put(&mut out, "description", v),
                            b"ISRC" | b"IENG" => put(&mut out, "recorded_by", v),
                            _ => {}
                        }
                        at = end + (n & 1);
                    }
                }
            }
            _ => {}
        }
        if next == u64::MAX || f.seek(SeekFrom::Start(next)).is_err() {
            break;
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A short silent WAV with a bext chunk and a LIST/INFO chunk, as a field recorder writes it.
    pub(crate) fn wav_with_tags(path: &Path) {
        let mut bext = vec![0u8; 602];
        bext[..17].copy_from_slice(b"Story by the fire");
        bext[256..265].copy_from_slice(b"ZOOM H4n ");
        bext[320..330].copy_from_slice(b"2025:07:14");
        bext[330..338].copy_from_slice(b"18-42-05");
        let mut info = b"INFO".to_vec();
        for (id, v) in [(b"ICRD", &b"2019-01-01\0"[..]), (b"IART", &b"Jo Doe\0"[..])] {
            info.extend_from_slice(id);
            info.extend_from_slice(&(v.len() as u32).to_le_bytes());
            info.extend_from_slice(v);
            if v.len() % 2 == 1 {
                info.push(0);
            }
        }
        let fmt: Vec<u8> = [&1u16.to_le_bytes()[..], &1u16.to_le_bytes(), &16000u32.to_le_bytes(), &32000u32.to_le_bytes(), &2u16.to_le_bytes(), &16u16.to_le_bytes()].concat();
        let data = vec![0u8; 3200];
        let mut body = b"WAVE".to_vec();
        for (id, c) in [(&b"bext"[..], &bext), (b"fmt ", &fmt), (b"LIST", &info), (b"data", &data)] {
            body.extend_from_slice(id);
            body.extend_from_slice(&(c.len() as u32).to_le_bytes());
            body.extend_from_slice(c);
        }
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        std::fs::write(path, out).unwrap();
    }

    #[test]
    fn a_recorders_tags_become_details() {
        let path = std::env::temp_dir().join(format!("wugs-bext-{}.wav", wugs_model::ulid()));
        wav_with_tags(&path);
        assert_eq!(
            read(&path),
            vec![
                ("date".to_string(), "2025-07-14".to_string()),
                ("time".into(), "18:42:05".into()),
                ("equipment".into(), "ZOOM H4n".into()),
                ("description".into(), "Story by the fire".into()),
                ("recorded_by".into(), "Jo Doe".into()),
            ],
            "the bext date wins over INFO's"
        );
        std::fs::remove_file(&path).ok();
        assert_eq!(date("0000-00-00"), None);
        assert_eq!(date("2024.3.9"), Some("2024-03-09".into()));
        assert!(read(Path::new("/no/such.wav")).is_empty());
    }

    #[test]
    fn an_imported_recording_arrives_with_its_details() {
        let dir = std::env::temp_dir().join(format!("wugs-bext-import-{}", wugs_model::ulid()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("ZOOM0001.WAV");
        wav_with_tags(&wav);
        let mut s: crate::session::Session = None;
        crate::session::create_project(&mut s, dir.join("p").to_str().unwrap(), "prs_Ana").unwrap();
        let row = crate::session::import_media(&mut s, wav.to_str().unwrap()).unwrap();
        let get = |k: &str| row.details.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
        assert_eq!((get("date"), get("equipment"), get("time")), (Some("2025-07-14".into()), Some("ZOOM H4n".into()), Some("18:42:05".into())), "{:?}", row.details);
        std::fs::remove_dir_all(&dir).ok();
    }
}
