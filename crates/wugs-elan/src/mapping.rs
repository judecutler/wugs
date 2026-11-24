//! The ELAN <-> wugs vocabulary in one place.

/// Entity fields that carry ELAN identity, so that re-import dedupes and
/// export reproduces the original ids.
pub const F_ORIGIN: &str = "origin";
pub const F_LING_TYPE: &str = "x.elan.linguistic_type";
pub const F_CONSTRAINTS: &str = "x.elan.constraints";
pub const F_PARTICIPANT: &str = "x.elan.participant";
/// A tier's ELAN language: its `LANG_REF` goes to the tier's `language`; the
/// header's `<LANGUAGE>` label and definition (an ISO or Glottolog URL) are kept here.
pub const F_LANG_LABEL: &str = "x.elan.lang-label";
pub const F_LANG_DEF: &str = "x.elan.lang-def";
pub const F_TIER_ORDER: &str = "x.elan.order";
/// Tiers are project-scoped but TIER_ID is only unique within one file, so
/// each imported tier records the text it came from.
pub const F_TIER_TEXT: &str = "x.elan.text";

/// HEADER properties written by export and honoured by import.
pub const PROP_TEXT: &str = "wugs.text";
pub const PROP_TITLE: &str = "wugs.title";

/// Linguistic types of the analysis tiers export writes from the glossing
/// (words, morphs, glosses). They are derived from tokens and morphs, so
/// import reads them as already in wugs, never as new tiers of segments.
/// (Only these three: `wugs-association` and `wugs-subdivision` are the
/// default types of ordinary tiers.)
pub const TYPE_WORDS: &str = "wugs-words";
pub const TYPE_MORPHS: &str = "wugs-morphs";
pub const TYPE_GLOSSES: &str = "wugs-glosses";
/// Other tiers on the words or morphs, written from `x.elan.<tier name>`
/// fields an ELAN file's own glossing tiers left on them (`glossing.rs`).
pub const TYPE_WORD_NOTES: &str = "wugs-word-notes";
pub const TYPE_MORPH_NOTES: &str = "wugs-morph-notes";
pub const ANALYSIS_TYPES: [&str; 5] = [TYPE_WORDS, TYPE_MORPHS, TYPE_GLOSSES, TYPE_WORD_NOTES, TYPE_MORPH_NOTES];
/// The prefix of a word's or morph's field that an ELAN tier came from.
pub const F_KEPT_TIER: &str = "x.elan.";

pub const KIND_ALIGNED: &str = "time-aligned";
pub const KIND_SUBDIVISION: &str = "symbolic-subdivision";
pub const KIND_ASSOCIATION: &str = "symbolic-association";

/// ELAN stereotype -> tier kind. Time_Subdivision and Included_In children are
/// time-aligned in wugs; the parent link is kept on the tier, but the
/// segment-level subdivision relationship is not modelled in v0.
const STEREOTYPES: &[(&str, &str)] = &[
    ("Symbolic_Subdivision", KIND_SUBDIVISION),
    ("Symbolic_Association", KIND_ASSOCIATION),
    ("Time_Subdivision", KIND_ALIGNED),
    ("Included_In", KIND_ALIGNED),
];

pub fn kind_for(time_alignable: bool, constraints: Option<&str>) -> &'static str {
    if let Some(c) = constraints {
        if let Some((_, kind)) = STEREOTYPES.iter().find(|(s, _)| *s == c) {
            return kind;
        }
    }
    if time_alignable {
        KIND_ALIGNED
    } else {
        KIND_ASSOCIATION
    }
}

/// Stereotype to emit for a tier that never came from ELAN.
pub fn default_constraints(kind: &str) -> Option<&'static str> {
    match kind {
        KIND_SUBDIVISION => Some("Symbolic_Subdivision"),
        KIND_ASSOCIATION => Some("Symbolic_Association"),
        _ => None,
    }
}

pub fn default_type_id(kind: &str) -> &'static str {
    match kind {
        KIND_SUBDIVISION => "wugs-subdivision",
        KIND_ASSOCIATION => "wugs-association",
        _ => "default-lt",
    }
}

/// The four stereotypes ELAN itself writes into every file.
pub const ELAN_CONSTRAINTS: &[(&str, &str)] = &[
    ("Time subdivision of parent annotation's time interval, no time gaps allowed within this interval", "Time_Subdivision"),
    ("Symbolic subdivision of a parent annotation. Annotations refering to the same parent are ordered", "Symbolic_Subdivision"),
    ("1-1 association with a parent annotation", "Symbolic_Association"),
    ("Time alignable annotations within the parent annotation's time interval, gaps are allowed", "Included_In"),
];
