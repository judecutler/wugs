//! Streaming RIFF/WAVE reader. Decodes every supported sample format to
//! interleaved `i16` through a fixed-size buffer and never holds the file.

use std::io::{self, Read};

use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleKind {
    Int,
    Float,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WavFormat {
    pub channels: u16,
    pub sample_rate: u32,
    /// Container width of one sample, in bits (16, 24 or 32).
    pub bits_per_sample: u16,
    pub kind: SampleKind,
}

impl WavFormat {
    pub fn block_align(&self) -> usize {
        self.channels as usize * (self.bits_per_sample as usize / 8)
    }
}

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// Bytes 2..16 of every `KSDATAFORMAT_SUBTYPE_*` GUID built from a format tag.
const SUBFORMAT_TAIL: [u8; 14] = [
    0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
const UNKNOWN_LEN: u32 = 0xFFFF_FFFF;
const MAX_FMT_CHUNK: u64 = 1 << 16;

pub struct WavReader<R> {
    r: R,
    fmt: WavFormat,
    declared_bytes: Option<u64>,
    remaining: Option<u64>,
    truncated: bool,
    done: bool,
    raw: Vec<u8>,
}

impl<R: Read> WavReader<R> {
    /// Parses the header and leaves the reader at the first sample byte.
    pub fn new(mut r: R) -> Result<Self, Error> {
        let mut head = [0u8; 12];
        if read_full(&mut r, &mut head)? < 12 {
            return Err(Error::NotWav("file shorter than a RIFF header".into()));
        }
        match &head[0..4] {
            b"RIFF" => {}
            b"RF64" | b"BW64" => {
                return Err(Error::Unsupported("RF64/BW64 files (over 4 GiB)".into()))
            }
            b"RIFX" => return Err(Error::Unsupported("big-endian RIFX files".into())),
            _ => return Err(Error::NotWav("missing RIFF tag".into())),
        }
        if &head[8..12] != b"WAVE" {
            return Err(Error::NotWav("RIFF file is not WAVE".into()));
        }

        let mut fmt: Option<WavFormat> = None;
        loop {
            let mut ch = [0u8; 8];
            if read_full(&mut r, &mut ch)? < 8 {
                return Err(Error::Malformed("no data chunk before end of file".into()));
            }
            let size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]);
            match &ch[0..4] {
                b"fmt " => {
                    let size = size as u64;
                    if !(16..=MAX_FMT_CHUNK).contains(&size) {
                        return Err(Error::Malformed(format!("fmt chunk of {size} bytes")));
                    }
                    let mut buf = vec![0u8; size as usize];
                    if read_full(&mut r, &mut buf)? < buf.len() {
                        return Err(Error::Malformed("truncated fmt chunk".into()));
                    }
                    fmt = Some(parse_fmt(&buf)?);
                    skip(&mut r, size & 1)?;
                }
                b"data" => {
                    let fmt = fmt.ok_or_else(|| Error::Malformed("data chunk before fmt".into()))?;
                    let declared_bytes = (size != UNKNOWN_LEN).then_some(size as u64);
                    return Ok(WavReader {
                        r,
                        fmt,
                        declared_bytes,
                        remaining: declared_bytes,
                        truncated: false,
                        done: false,
                        raw: Vec::new(),
                    });
                }
                _ => skip(&mut r, size as u64 + (size & 1) as u64)?,
            }
        }
    }

    pub fn format(&self) -> WavFormat {
        self.fmt
    }

    /// A reader over sample frames from the current position of `r`, for a
    /// caller that parsed the header with [`WavReader::new`] and then seeked
    /// inside the data chunk. `remaining_bytes` stops it at the end of the
    /// data chunk (`None` reads to end of file, for an open-ended size).
    pub fn resume(r: R, fmt: WavFormat, remaining_bytes: Option<u64>) -> Self {
        WavReader { r, fmt, declared_bytes: remaining_bytes, remaining: remaining_bytes, truncated: false, done: false, raw: Vec::new() }
    }

    /// Frames the `data` chunk header claims, or `None` for a streaming
    /// writer's open-ended `0xFFFFFFFF` size.
    pub fn declared_frames(&self) -> Option<u64> {
        self.declared_bytes.map(|b| b / self.fmt.block_align() as u64)
    }

    /// True once a read hit end of file before the declared data length.
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Decodes up to `out.len() / channels` whole frames into `out` as
    /// interleaved `i16`. Returns the frames written; 0 means end of data.
    pub fn read_frames(&mut self, out: &mut [i16]) -> Result<usize, Error> {
        let ch = self.fmt.channels as usize;
        let align = self.fmt.block_align();
        let width = self.fmt.bits_per_sample as usize / 8;
        if self.done {
            return Ok(0);
        }
        if out.len() < ch {
            // A too-small buffer must not look like end of data.
            return Err(Error::BadParams(format!(
                "output buffer holds {} samples; one frame needs {ch}",
                out.len()
            )));
        }
        let mut want = (out.len() / ch) * align;
        if let Some(rem) = self.remaining {
            want = want.min(rem as usize);
        }
        if want == 0 {
            self.done = true;
            return Ok(0);
        }
        self.raw.resize(want, 0);
        let got = read_full(&mut self.r, &mut self.raw)?;
        if got < want {
            self.done = true;
            self.truncated = self.declared_bytes.is_some();
        }
        if let Some(rem) = self.remaining.as_mut() {
            *rem -= got as u64;
        }
        let frames = got / align;
        let samples = frames * ch;
        decode(
            self.fmt,
            &self.raw[..samples * width],
            &mut out[..samples],
        );
        Ok(frames)
    }
}

fn decode(fmt: WavFormat, raw: &[u8], out: &mut [i16]) {
    match (fmt.kind, fmt.bits_per_sample) {
        (SampleKind::Int, 16) => {
            for (o, b) in out.iter_mut().zip(raw.chunks_exact(2)) {
                *o = i16::from_le_bytes([b[0], b[1]]);
            }
        }
        (SampleKind::Int, 24) => {
            for (o, b) in out.iter_mut().zip(raw.chunks_exact(3)) {
                let top = i32::from_le_bytes([b[0], b[1], b[2], 0]) << 8;
                *o = (top >> 16) as i16;
            }
        }
        (SampleKind::Int, 32) => {
            for (o, b) in out.iter_mut().zip(raw.chunks_exact(4)) {
                *o = (i32::from_le_bytes([b[0], b[1], b[2], b[3]]) >> 16) as i16;
            }
        }
        (SampleKind::Float, 32) => {
            for (o, b) in out.iter_mut().zip(raw.chunks_exact(4)) {
                let x = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                *o = (x.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            }
        }
        _ => unreachable!("parse_fmt admits only 16/24/32-bit int and 32-bit float"),
    }
}

fn parse_fmt(buf: &[u8]) -> Result<WavFormat, Error> {
    let u16_at = |i: usize| u16::from_le_bytes([buf[i], buf[i + 1]]);
    let mut tag = u16_at(0);
    let channels = u16_at(2);
    let sample_rate = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let block_align = u16_at(12);
    let bits = u16_at(14);

    if tag == WAVE_FORMAT_EXTENSIBLE {
        if buf.len() < 40 || u16_at(16) < 22 {
            return Err(Error::Malformed("extensible fmt chunk shorter than 40 bytes".into()));
        }
        if buf[26..40] != SUBFORMAT_TAIL {
            return Err(Error::Unsupported("extensible WAV with a non-standard subformat GUID".into()));
        }
        tag = u16_at(24);
    }

    let kind = match (tag, bits) {
        (WAVE_FORMAT_PCM, 16 | 24 | 32) => SampleKind::Int,
        (WAVE_FORMAT_FLOAT, 32) => SampleKind::Float,
        (WAVE_FORMAT_PCM, b) => {
            return Err(Error::Unsupported(format!("{b}-bit integer PCM (supported: 16, 24, 32)")))
        }
        (WAVE_FORMAT_FLOAT, b) => {
            return Err(Error::Unsupported(format!("{b}-bit float (supported: 32)")))
        }
        (t, _) => return Err(Error::Unsupported(format!("codec {} (only PCM and IEEE float)", codec_name(t)))),
    };
    if channels == 0 || sample_rate == 0 {
        return Err(Error::Malformed("zero channels or zero sample rate".into()));
    }
    if channels > 256 {
        return Err(Error::Unsupported(format!("{channels} channels (supported: up to 256)")));
    }
    if block_align as usize != channels as usize * (bits as usize / 8) {
        return Err(Error::Malformed("block align disagrees with channels and bit depth".into()));
    }
    Ok(WavFormat { channels, sample_rate, bits_per_sample: bits, kind })
}

fn codec_name(tag: u16) -> String {
    match tag {
        0x0002 => "MS ADPCM".into(),
        0x0006 => "A-law".into(),
        0x0007 => "mu-law".into(),
        0x0011 => "IMA ADPCM".into(),
        0x0055 => "MP3".into(),
        t => format!("format tag 0x{t:04X}"),
    }
}

/// Reads until `buf` is full or the stream ends; returns the bytes read.
fn read_full<R: Read>(r: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

fn skip<R: Read>(r: &mut R, n: u64) -> Result<(), Error> {
    let copied = io::copy(&mut r.by_ref().take(n), &mut io::sink())?;
    if copied < n {
        return Err(Error::Malformed("file ends inside a chunk before the data chunk".into()));
    }
    Ok(())
}
