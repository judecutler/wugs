//! wugs-flex: FLEx interchange (PRD-TONGUES.md §5). LIFT lexicons and
//! FlexText interlinear texts, imported into and exported from the shared
//! entities. Every element the importer reads but does not carry is counted
//! in the import report, so a round trip never loses data silently.

mod common;

pub mod flextext;
pub mod lift;

pub use common::{ImportError, MultiText, NotCarried};
pub use flextext::{export_flextext, import_flextext};
pub use lift::{export_lift, import_lift};
