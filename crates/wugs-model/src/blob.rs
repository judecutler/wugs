//! The blob ref format (DATA-MODEL §9): `blake3:` + 64 lowercase hex.
//! Part of the archive format; this is the one implementation.

use std::collections::BTreeSet;

pub const BLOB_REF_PREFIX: &str = "blake3:";

/// Conformance vectors for the ref format: `(text, is a ref)`. Every
/// validator of the format asserts against this one table.
pub const REF_VECTORS: &[(&str, bool)] = &[
    ("blake3:af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", true),
    ("blake3:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", true),
    ("", false),
    ("blake3:", false),
    ("blake3:abc", false),
    ("sha256:af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", false),
    ("BLAKE3:af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", false),
    ("blake3:AF1349B9F5F9A1A6A0404DEA36DCC9499BCB25C9ADC112B7CC9A93CAE41F3262", false),
    ("blake3:af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f32620", false),
    ("blake3:af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f326g", false),
    ("af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", false),
];

fn is_hex(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

/// Validate a whole string as a blob ref and return its hex digest.
pub fn parse_blob_ref(r: &str) -> Option<&str> {
    let hex = r.strip_prefix(BLOB_REF_PREFIX)?;
    (hex.len() == 64 && hex.bytes().all(is_hex)).then_some(hex)
}

/// Collect every blob ref embedded in arbitrary text (an op line, torn or
/// not). A hex run longer than 64 is not a ref, so two refs written back to
/// back with no separator are out of contract: JSON always separates them.
pub fn scan_blob_refs(text: &str, out: &mut BTreeSet<String>) {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(i) = text[from..].find(BLOB_REF_PREFIX) {
        let start = from + i + BLOB_REF_PREFIX.len();
        let run = bytes[start..].iter().take_while(|&&b| is_hex(b)).count();
        if run == 64 {
            out.insert(text[start - BLOB_REF_PREFIX.len()..start + 64].to_string());
        }
        from = start;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(text: &str) -> Vec<String> {
        let mut out = BTreeSet::new();
        scan_blob_refs(text, &mut out);
        out.into_iter().collect()
    }

    #[test]
    fn scan_takes_exact_refs_only() {
        let good = format!("blake3:{}", "ab01".repeat(16));
        let text = format!(
            "{{\"a\":\"{good}\",\"b\":\"blake3:{}\",\"c\":\"blake3:{}\",\"d\":\"blake3:short\"}}",
            "a".repeat(65),
            "A".repeat(64)
        );
        assert_eq!(scan(&text), vec![good]);
    }

    #[test]
    fn parse_and_scan_agree_with_the_shared_vectors() {
        for &(text, ok) in REF_VECTORS {
            assert_eq!(parse_blob_ref(text).is_some(), ok, "parse {text:?}");
            let want = if ok { vec![text.to_string()] } else { Vec::new() };
            assert_eq!(scan(text), want, "scan {text:?}");
        }
    }

    #[test]
    fn scan_edge_cases() {
        let hex = "0123456789abcdef".repeat(4);
        let r = format!("blake3:{hex}");
        assert_eq!(scan(&r), vec![r.clone()], "ref at the very end of the text");
        assert_eq!(scan(&format!("blake3:{r}")), vec![r.clone()], "doubled prefix still yields the inner ref");
        assert_eq!(
            scan(&format!("{{\"label\":\"köpekler 🎙 geldi\",\"blob\":\"{r}\",\"note\":\"ğış\"}}")),
            vec![r.clone()],
            "multibyte text around the ref"
        );
        assert_eq!(scan(&format!("{{\"blob\":\"{r}")), vec![r.clone()], "torn line ending right after the ref");
        assert_eq!(scan(&r[..r.len() - 1]), Vec::<String>::new(), "torn inside the ref");
        assert_eq!(scan(""), Vec::<String>::new());
    }
}
