//! wugs-legacy: the line-based legacy formats field linguists bring with
//! them. SIL Toolbox / Shoebox MDF lexicons and Praat TextGrids, imported
//! into and exported from the shared entities. Whatever the importer reads
//! but does not carry semantically is kept verbatim or counted in the
//! report, so a round trip never loses data silently.

mod common;

pub mod praat;
pub mod sfm;
pub mod textgrid;
pub mod toolbox;

pub use common::{decode, Counts, ImportError};
pub use textgrid::{export_textgrid, import_textgrid, TextGridExport, TextGridReport};
pub use toolbox::{export_toolbox, import_toolbox, ToolboxReport};
