//! Compressed recordings (phone voice memos in `.m4a`, AAC or Apple
//! Lossless, `.mp3`, `.flac`, `.ogg`, `.aiff`) and WAV encodings the peaks
//! reader does not take (8-bit, A-law, μ-law, ADPCM) become WAV before import, because the blob store, the
//! peaks, the timeline, speech assist and Praat all read WAV. Decoding is
//! Symphonia (pure Rust, MPL-2.0). The decoded audio keeps the file's own
//! sample rate; one or two channels stay as they are, more are mixed to one.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// The WAV data chunk's size field is 32-bit.
const MAX_DATA: u64 = u32::MAX as u64 - 64;

/// Remove decoded files a crash left in `dir` (older than an hour, so an
/// import running now keeps its own).
pub fn sweep(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > 3600);
        if old && e.path().extension().is_some_and(|x| x == "wav") {
            std::fs::remove_file(e.path()).ok();
        }
    }
}

/// Whether the file is already a WAV (RIFF … WAVE), from its first bytes.
pub fn is_wav(path: &Path) -> bool {
    let mut head = [0u8; 12];
    match File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)) {
        Ok(()) => (&head[0..4] == b"RIFF" || &head[0..4] == b"RF64") && &head[8..12] == b"WAVE",
        Err(_) => false,
    }
}

#[derive(Debug, PartialEq)]
pub struct Converted {
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
}

/// Decode `input` into a new 16-bit PCM WAV at `output`. On failure the
/// partial output is removed.
pub fn to_wav(input: &Path, output: &Path) -> Result<Converted, String> {
    let name = input.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let result = decode(input, output, &name);
    if result.is_err() {
        std::fs::remove_file(output).ok();
    }
    result
}

fn decode(input: &Path, output: &Path, name: &str) -> Result<Converted, String> {
    let file = File::open(input).map_err(|e| format!("{name}: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = input.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    const KINDS: &str = "WAV, M4A (AAC or Apple Lossless), MP3, FLAC, Ogg Vorbis and AIFF import";
    let unreadable = |e: Error| format!("{name} is not audio wugs can read ({e}); {KINDS}");
    let undecodable = |e: Error| format!("{name} uses an audio encoding wugs cannot decode yet ({e}); {KINDS}");
    let probed = symphonia::default::get_probe()
        // Gapless: encoder priming and padding are trimmed, so times match
        // what other players show for the same file.
        .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
        .map_err(unreadable)?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| format!("{name} has no audio track"))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(undecodable)?;

    let out = File::create(output).map_err(|e| format!("{}: {e}", output.display()))?;
    let mut w = BufWriter::new(out);
    w.write_all(&[0u8; 44]).map_err(|e| e.to_string())?;
    let mut spec_seen: Option<(u32, u16, usize)> = None; // (rate, channels written, channels decoded)
    let mut data_bytes: u64 = 0;
    let mut samples: Option<SampleBuffer<i16>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            // A chained stream (a new Ogg section) may change its format.
            Err(Error::ResetRequired) => return Err(format!("{name} changes its format part way through; split it first")),
            Err(e) => return Err(format!("{name}: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // A damaged packet costs its few milliseconds, not the recording.
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("{name}: {e}")),
        };
        let spec = *decoded.spec();
        let in_ch = spec.channels.count();
        if in_ch == 0 {
            continue;
        }
        let out_ch: u16 = if in_ch <= 2 { in_ch as u16 } else { 1 };
        match spec_seen {
            None => spec_seen = Some((spec.rate, out_ch, in_ch)),
            Some((rate, _, ch)) if rate != spec.rate || ch != in_ch => {
                return Err(format!("{name} changes its sample rate or channels part way through"))
            }
            Some(_) => {}
        }
        let buf = match &mut samples {
            Some(b) if b.capacity() >= decoded.capacity() * in_ch => b,
            _ => samples.insert(SampleBuffer::<i16>::new(decoded.capacity() as u64, spec)),
        };
        buf.copy_interleaved_ref(decoded);
        let chunk = buf.samples();
        let mut bytes = Vec::with_capacity(chunk.len() * 2);
        if in_ch <= 2 {
            for s in chunk {
                bytes.extend_from_slice(&s.to_le_bytes());
            }
        } else {
            for frame in chunk.chunks_exact(in_ch) {
                let sum: i32 = frame.iter().map(|&s| s as i32).sum();
                bytes.extend_from_slice(&((sum / in_ch as i32) as i16).to_le_bytes());
            }
        }
        if data_bytes + bytes.len() as u64 > MAX_DATA {
            return Err(format!("{name} is too long to hold as one WAV (over 4 GiB of audio); split it first"));
        }
        w.write_all(&bytes).map_err(|e| format!("{}: {e}", output.display()))?;
        data_bytes += bytes.len() as u64;
    }
    let (rate, channels, _) = spec_seen.ok_or_else(|| format!("{name} holds no audio"))?;
    if data_bytes == 0 {
        return Err(format!("{name} holds no audio"));
    }
    w.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    w.write_all(&wav_header(rate, channels, data_bytes as u32)).map_err(|e| e.to_string())?;
    let file = w.into_inner().map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(Converted { sample_rate: rate, channels, frames: data_bytes / (2 * channels as u64) })
}

fn wav_header(rate: u32, channels: u16, data: u32) -> [u8; 44] {
    let block = channels * 2;
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data).to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes());
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&rate.to_le_bytes());
    h[28..32].copy_from_slice(&(rate * block as u32).to_le_bytes());
    h[32..34].copy_from_slice(&block.to_le_bytes());
    h[34..36].copy_from_slice(&16u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}
