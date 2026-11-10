//! The ELAN <-> wugs vocabulary in one place.

/// Entity fields that carry ELAN identity, so that re-import dedupes and
/// export reproduces the original ids.
pub const F_ORIGIN: &str = "origin";
pub const F_LING_TYPE: &str = "x.elan.linguistic_type";
pub const F_CONSTRAINTS: &str = "x.elan.constraints";
pub const F_PARTICIPANT: &str = "x.elan.participant";
pub const F_TIER_ORDER: &str = "x.elan.order";
/// Tiers are project-scoped but TIER_ID is only unique within one file, so
/// each imported tier records the text it came from.
pub const F_TIER_TEXT: &str = "x.elan.text";

/// HEADER properties written by export and honoured by import.
pub const PROP_TEXT: &str = "wugs.text";
pub const PROP_TITLE: &str = "wugs.title";

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
