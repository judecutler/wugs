//! Unicode NFC for wugs (PRD §7), behind the zero-dependency core's
//! `Normalizer` seam. An edge crate on purpose: the Unicode tables live
//! here, and a build without them still works, storing text as typed.

use unicode_normalization::{is_nfc, UnicodeNormalization};
use wugs_model::Normalizer;

pub struct NfcNormalizer;

impl Normalizer for NfcNormalizer {
    fn nfc(&self, s: &str) -> String {
        // the common case skips the composition pass: one quick check, one copy
        if is_nfc(s) {
            s.to_string()
        } else {
            s.nfc().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposed_input_composes() {
        // "ç" as c + COMBINING CEDILLA vs precomposed U+00E7
        let decomposed = "c\u{0327}ay";
        let composed = "\u{E7}ay";
        assert_ne!(decomposed, composed, "the two spellings differ byte-wise");
        assert_eq!(NfcNormalizer.nfc(decomposed), composed, "NFC composes them");
        assert_eq!(NfcNormalizer.nfc(composed), composed, "already-NFC text is unchanged");
    }
}
