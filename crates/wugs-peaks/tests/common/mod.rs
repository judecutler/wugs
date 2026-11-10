#![allow(dead_code)]

use std::path::PathBuf;

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn i16(&mut self) -> i16 {
        (self.next() >> 24) as i16
    }
}

pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        let p = std::env::temp_dir().join(format!("wugs-peaks-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub struct Wav {
    pub tag: u16,
    pub channels: u16,
    pub rate: u32,
    pub bits: u16,
    /// 16, 18 or 40. 40 writes WAVE_FORMAT_EXTENSIBLE with `tag` as the subformat.
    pub fmt_size: u16,
    pub before_data: Vec<(&'static [u8; 4], Vec<u8>)>,
    pub after_data: Vec<(&'static [u8; 4], Vec<u8>)>,
    pub data: Vec<u8>,
    pub declared_len: Option<u32>,
}

impl Wav {
    pub fn pcm(bits: u16, channels: u16, data: Vec<u8>) -> Wav {
        Wav { tag: 1, channels, rate: 44100, bits, fmt_size: 16, before_data: vec![], after_data: vec![], data, declared_len: None }
    }

    pub fn build(&self) -> Vec<u8> {
        let mut out = b"RIFF\0\0\0\0WAVE".to_vec();
        let mut fmt = Vec::new();
        let wire_tag = if self.fmt_size == 40 { 0xFFFE } else { self.tag };
        fmt.extend_from_slice(&wire_tag.to_le_bytes());
        fmt.extend_from_slice(&self.channels.to_le_bytes());
        fmt.extend_from_slice(&self.rate.to_le_bytes());
        let align = self.channels * (self.bits / 8);
        fmt.extend_from_slice(&(self.rate * align as u32).to_le_bytes());
        fmt.extend_from_slice(&align.to_le_bytes());
        fmt.extend_from_slice(&self.bits.to_le_bytes());
        if self.fmt_size >= 18 {
            fmt.extend_from_slice(&(if self.fmt_size == 40 { 22u16 } else { 0 }).to_le_bytes());
        }
        if self.fmt_size == 40 {
            fmt.extend_from_slice(&self.bits.to_le_bytes());
            fmt.extend_from_slice(&3u32.to_le_bytes());
            fmt.extend_from_slice(&self.tag.to_le_bytes());
            fmt.extend_from_slice(&[0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71]);
        }
        assert_eq!(fmt.len(), self.fmt_size as usize);
        let chunk = |out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]| {
            out.extend_from_slice(id);
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(body);
            if body.len() % 2 == 1 {
                out.push(0);
            }
        };
        for (id, body) in self.before_data.iter().take(1) {
            chunk(&mut out, id, body);
        }
        chunk(&mut out, b"fmt ", &fmt);
        for (id, body) in self.before_data.iter().skip(1) {
            chunk(&mut out, id, body);
        }
        out.extend_from_slice(b"data");
        out.extend_from_slice(&self.declared_len.unwrap_or(self.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.data);
        if self.data.len() % 2 == 1 && self.declared_len.is_none() {
            out.push(0);
        }
        for (id, body) in &self.after_data {
            chunk(&mut out, id, body);
        }
        out
    }
}

pub fn enc16(v: &[i16]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}
pub fn enc24(v: &[i32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()[..3].to_vec()).collect()
}
pub fn enc32(v: &[i32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}
pub fn encf32(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// Decodes the whole stream through a reader that hands out one byte per read.
pub struct OneByte<R>(pub R);

impl<R: std::io::Read> std::io::Read for OneByte<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(1);
        self.0.read(&mut buf[..n])
    }
}
