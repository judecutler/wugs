//! wugs-elan: ELAN `.eaf` import/export (PRD-TONGUES.md §5).
//!
//! Lossless for the annotation structure ELAN users edit (tiers, hierarchy,
//! aligned and referenced annotations). Media descriptors, controlled
//! vocabularies, locales and lexicon references are not carried in v0.

mod mapping;

pub mod export;
pub mod import;
pub use wugs_xml as xml;

pub use export::export_eaf;
pub use import::{import_eaf, ImportError, ImportReport};
