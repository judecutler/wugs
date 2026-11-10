mod common;

use std::io::Cursor;

use common::*;
use wugs_peaks::wav::{SampleKind, WavReader};
use wugs_peaks::{Error, Params, Peaks};

fn read_all(bytes: Vec<u8>) -> (Vec<i16>, WavReader<Cursor<Vec<u8>>>) {
    let mut r = WavReader::new(Cursor::new(bytes)).unwrap();
    let ch = r.format().channels as usize;
    let mut out = Vec::new();
    let mut buf = vec![0i16; 7 * ch];
    loop {
        let n = r.read_frames(&mut buf).unwrap();
        if n == 0 {
            return (out, r);
        }
        out.extend_from_slice(&buf[..n * ch]);
    }
}

fn err_text(bytes: Vec<u8>) -> String {
    match WavReader::new(Cursor::new(bytes)) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected an error"),
    }
}

#[test]
fn pcm16_mono_and_stereo() {
    let v = [0i16, 1, -1, i16::MAX, i16::MIN, 1234];
    let (mono, r) = read_all(Wav::pcm(16, 1, enc16(&v)).build());
    assert_eq!(mono, v);
    assert_eq!(r.format().channels, 1);
    assert_eq!(r.declared_frames(), Some(6));
    let (stereo, r) = read_all(Wav::pcm(16, 2, enc16(&v)).build());
    assert_eq!(stereo, v);
    assert_eq!(r.format().channels, 2);
    assert_eq!(r.declared_frames(), Some(3));
    assert!(!r.truncated());
}

#[test]
fn pcm24_sign_and_scaling() {
    let v = [0, 255, 256, -1, -256, -257, 8_388_607, -8_388_608, 0x123456, -0x123456];
    let (got, _) = read_all(Wav::pcm(24, 1, enc24(&v)).build());
    let want: Vec<i16> = v.iter().map(|&x| (x >> 8) as i16).collect();
    assert_eq!(got, want);
    assert_eq!(&got[6..8], &[i16::MAX, i16::MIN]);
    assert_eq!(got[3], -1);
}

#[test]
fn pcm32_sign_and_scaling() {
    let v = [0, 65535, 65536, -1, -65536, i32::MAX, i32::MIN];
    let (got, _) = read_all(Wav::pcm(32, 1, enc32(&v)).build());
    assert_eq!(got, [0, 0, 1, -1, -1, i16::MAX, i16::MIN]);
}

#[test]
fn float32_scaling_and_clamp() {
    let v = [0.0f32, 1.0, -1.0, 0.5, -0.5, 2.0, -3.0, f32::NAN];
    let mut w = Wav::pcm(32, 1, encf32(&v));
    w.tag = 3;
    let (got, r) = read_all(w.build());
    assert_eq!(r.format().kind, SampleKind::Float);
    assert_eq!(got, [0, 32767, -32767, 16384, -16384, 32767, -32767, 0]);
}

#[test]
fn fmt_chunk_sizes_16_18_40() {
    let v = [5i16, -6, 7, -8];
    for size in [16, 18, 40] {
        let mut w = Wav::pcm(16, 2, enc16(&v));
        w.fmt_size = size;
        let (got, r) = read_all(w.build());
        assert_eq!(got, v, "fmt size {size}");
        assert_eq!(r.format().sample_rate, 44100);
    }
}

#[test]
fn extensible_float_and_24bit() {
    let mut w = Wav::pcm(32, 2, encf32(&[1.0, -1.0]));
    w.tag = 3;
    w.fmt_size = 40;
    let (got, r) = read_all(w.build());
    assert_eq!(r.format().kind, SampleKind::Float);
    assert_eq!(got, [32767, -32767]);

    let mut w = Wav::pcm(24, 1, enc24(&[-8_388_608, 8_388_607]));
    w.fmt_size = 40;
    assert_eq!(read_all(w.build()).0, [i16::MIN, i16::MAX]);
}

#[test]
fn any_sample_rate_is_reported() {
    let mut w = Wav::pcm(16, 1, enc16(&[1, 2]));
    w.rate = 8000;
    assert_eq!(read_all(w.build()).1.format().sample_rate, 8000);
    w.rate = 192000;
    assert_eq!(read_all(w.build()).1.format().sample_rate, 192000);
}

#[test]
fn unknown_chunks_are_skipped_including_odd_padded_ones() {
    let v = [10i16, 20, 30, 40];
    let mut w = Wav::pcm(16, 2, enc16(&v));
    w.before_data = vec![(b"JUNK", vec![1, 2, 3]), (b"LIST", vec![9; 5])];
    w.after_data = vec![(b"id3 ", vec![7; 11])];
    let (got, r) = read_all(w.build());
    assert_eq!(got, v);
    assert!(!r.truncated());
}

#[test]
fn truncated_data_chunk_reads_what_is_there() {
    let v: Vec<i16> = (0..300).map(|i| i as i16).collect();
    let mut data = enc16(&v);
    data.push(0xAB);
    let mut w = Wav::pcm(16, 1, data);
    w.declared_len = Some(2000);
    let bytes = w.build();
    let (got, r) = read_all(bytes.clone());
    assert_eq!(got, v);
    assert!(r.truncated());
    assert_eq!(r.declared_frames(), Some(1000));

    let p = Peaks::compute(Cursor::new(bytes), Params::default(), None).unwrap();
    assert!(p.info().truncated);
    assert_eq!(p.info().frames, 300);
}

#[test]
fn open_ended_data_size_reads_to_eof_without_flagging_truncation() {
    let v = [1i16, 2, 3];
    let mut w = Wav::pcm(16, 1, enc16(&v));
    w.declared_len = Some(0xFFFF_FFFF);
    let (got, r) = read_all(w.build());
    assert_eq!(got, v);
    assert!(!r.truncated());
    assert_eq!(r.declared_frames(), None);
}

#[test]
fn short_reads_do_not_change_the_result() {
    let v: Vec<i16> = (0..50).map(|i| (i * 700) as i16).collect();
    let bytes = Wav::pcm(16, 2, enc16(&v)).build();
    let mut r = WavReader::new(OneByte(Cursor::new(bytes))).unwrap();
    let mut buf = vec![0i16; 100];
    let n = r.read_frames(&mut buf).unwrap();
    assert_eq!(n, 25);
    assert_eq!(&buf[..50], &v[..]);
}

#[test]
fn rejects_other_codecs_with_a_clear_error() {
    let mut w = Wav::pcm(4, 1, vec![0; 4]);
    w.tag = 2;
    w.fmt_size = 16;
    let msg = err_text(w.build());
    assert!(msg.contains("MS ADPCM"), "{msg}");

    let msg = err_text(Wav::pcm(8, 1, vec![0; 4]).build());
    assert!(msg.contains("8-bit"), "{msg}");

    let mut w = Wav::pcm(64, 1, vec![0; 8]);
    w.tag = 3;
    assert!(err_text(w.build()).contains("64-bit float"));
}

#[test]
fn rejects_non_wav_and_malformed_headers() {
    assert!(err_text(b"OggS....WAVE".to_vec()).contains("RIFF"));
    assert!(err_text(b"RIFF\0\0\0\0AVI ".to_vec()).contains("not WAVE"));
    assert!(err_text(b"RIFX\0\0\0\0WAVE".to_vec()).contains("big-endian"));
    assert!(err_text(b"RF64\0\0\0\0WAVE".to_vec()).contains("4 GiB"));
    assert!(err_text(b"short".to_vec()).contains("shorter"));

    let mut head_only = Wav::pcm(16, 1, vec![]).build();
    head_only.truncate(head_only.len() - 8);
    assert!(err_text(head_only).contains("no data chunk"));

    let mut data_first = b"RIFF\0\0\0\0WAVEdata".to_vec();
    data_first.extend_from_slice(&0u32.to_le_bytes());
    assert!(err_text(data_first).contains("before fmt"));
}

#[test]
fn compute_surfaces_wav_errors() {
    let mut w = Wav::pcm(16, 1, vec![0; 4]);
    w.tag = 7;
    match Peaks::compute(Cursor::new(w.build()), Params::default(), None) {
        Err(Error::Unsupported(m)) => assert!(m.contains("mu-law")),
        other => panic!("{other:?}"),
    }
}
