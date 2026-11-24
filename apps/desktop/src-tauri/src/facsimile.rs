//! The Facsimile panel's commands: a text's page images (scans or photos
//! of a manuscript, an inscription, a printed source) and the area of a
//! page each segment stands on (`wugs_core::facsimile`). A page is an image
//! source whose file is in the blob store, shown through the asset protocol
//! like a sense's picture.

use std::path::Path;

use serde::{Deserialize, Serialize};
use wugs_core::facsimile::Region;

use crate::lexicon::{PICTURE_TYPES, PICTURE_MAX_BYTES};
use crate::session::{project, Session};

/// A page scan may be larger than a dictionary picture.
pub const PAGE_MAX_BYTES: u64 = 4 * PICTURE_MAX_BYTES;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageView {
    pub source: String,
    pub label: String,
    /// The image's file here, for `convertFileSrc`; None when the file is
    /// not on this computer.
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Area {
    pub source: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Marked {
    pub segment: String,
    pub tier: String,
    pub content: String,
    pub area: Area,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacsimileView {
    pub pages: Vec<PageView>,
    /// Every live segment of the text with an area on one of its pages.
    pub marked: Vec<Marked>,
}

pub fn view(session: &mut Session, text: &str) -> Result<FacsimileView, String> {
    let p = project(session)?;
    let mut pages = Vec::new();
    for source in p.text_pages(text)? {
        let label = p.proj.field_str(&source, "label")?.unwrap_or_default();
        let path = p
            .proj
            .field_str(&source, "blob")?
            .and_then(|b| wugs_blob::parse_ref(&b).map(|hex| p.root.join("blobs").join(hex)))
            .filter(|f| f.is_file())
            .map(|f| f.display().to_string());
        pages.push(PageView { source, label, path });
    }
    let mut marked = Vec::new();
    for seg in p.proj.ordered_children("text", text, "segment")? {
        if let Some(r) = p.segment_region(&seg)? {
            if pages.iter().any(|g| g.source == r.source) {
                marked.push(Marked {
                    tier: p.proj.field_str(&seg, "tier")?.unwrap_or_default(),
                    content: p.proj.field_str(&seg, "content")?.unwrap_or_default(),
                    area: Area { source: r.source, x: r.x, y: r.y, w: r.w, h: r.h },
                    segment: seg,
                });
            }
        }
    }
    Ok(FacsimileView { pages, marked })
}

/// A page picture's file name, once it is a kind and size wugs takes.
fn checked_picture(file: &Path, path: &str) -> Result<String, String> {
    let name = file.file_name().and_then(|n| n.to_str()).ok_or_else(|| format!("{path} has no usable file name"))?.to_string();
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if !PICTURE_TYPES.contains(&ext.as_str()) {
        return Err(format!("{name} is not a picture wugs can show: choose a PNG, JPEG, GIF or WebP file (save a TIFF scan as PNG first)."));
    }
    let size = std::fs::metadata(file).map_err(|e| format!("{name}: {e}"))?.len();
    if size > PAGE_MAX_BYTES {
        return Err(format!("{name} is {} MB; a page can be at most {} MB. Save a smaller copy and choose that.", size / (1024 * 1024), PAGE_MAX_BYTES / (1024 * 1024)));
    }
    Ok(name)
}

/// Copy a picture into the blob store as an image source and add it as
/// the text's last page. The page takes the text's sharing level, as its
/// recording does, by being reached only through the text.
pub fn add_page(session: &mut Session, text: &str, path: &str) -> Result<FacsimileView, String> {
    let file = Path::new(path);
    let name = checked_picture(file, path)?;
    let p = project(session)?;
    if p.proj.entity_kind(text)?.as_deref() != Some("text") {
        return Err(format!("no such text: {text}"));
    }
    let store = wugs_blob::BlobStore::open(&p.root).map_err(|e| format!("open blob store: {e}"))?;
    let blob = store.put_file(file).map_err(|e| format!("store {name}: {e}"))?;
    let source = p.add_source(&name, Some(&blob), "image", None)?;
    p.add_text_page(text, &source)?;
    view(session, text)
}

/// Give a page that has no picture here (one a TEI file named) its image.
pub fn set_page_image(session: &mut Session, text: &str, source: &str, path: &str) -> Result<FacsimileView, String> {
    let file = Path::new(path);
    let name = checked_picture(file, path)?;
    let p = project(session)?;
    if !p.text_pages(text)?.iter().any(|s| s == source) {
        return Err("That image is not one of this text's pages.".into());
    }
    let store = wugs_blob::BlobStore::open(&p.root).map_err(|e| format!("open blob store: {e}"))?;
    let blob = store.put_file(file).map_err(|e| format!("store {name}: {e}"))?;
    p.set_page_image(source, &blob)?;
    view(session, text)
}

/// Take a page off the text (the image stays in the project); the areas
/// marked on it are cleared in the same go.
pub fn remove_page(session: &mut Session, text: &str, source: &str) -> Result<FacsimileView, String> {
    let p = project(session)?;
    let pages: Vec<String> = p.text_pages(text)?.into_iter().filter(|s| s != source).collect();
    for seg in p.proj.ordered_children("text", text, "segment")? {
        if p.segment_region(&seg)?.is_some_and(|r| r.source == source) {
            p.set_segment_region(text, &seg, None)?;
        }
    }
    let refs: Vec<&str> = pages.iter().map(String::as_str).collect();
    p.set_text_pages(text, &refs)?;
    view(session, text)
}

/// Move a page one place earlier (`-1`) or later (`1`).
pub fn move_page(session: &mut Session, text: &str, source: &str, by: i32) -> Result<FacsimileView, String> {
    let p = project(session)?;
    let mut pages = p.text_pages(text)?;
    if let Some(i) = pages.iter().position(|s| s == source) {
        let j = (i as i64 + by as i64).clamp(0, pages.len() as i64 - 1) as usize;
        let page = pages.remove(i);
        pages.insert(j, page);
        let refs: Vec<&str> = pages.iter().map(String::as_str).collect();
        p.set_text_pages(text, &refs)?;
    }
    view(session, text)
}

/// Mark or clear where a segment stands on a page.
pub fn set_area(session: &mut Session, text: &str, segment: &str, area: Option<Area>) -> Result<FacsimileView, String> {
    let p = project(session)?;
    let region = area.map(|a| Region { source: a.source, x: a.x, y: a.y, w: a.w, h: a.h });
    p.set_segment_region(text, segment, region.as_ref())?;
    view(session, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-pixel PNG, enough to stand for a scan.
    const PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0, 5, 0, 1, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn pages_are_added_marked_moved_and_taken_off() {
        let dir = std::env::temp_dir().join(format!("wugs-facsimile-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir.join("sample"), "prs_Ana").unwrap();
        let text = crate::session::list_texts(&mut s).unwrap()[0].id.clone();
        let seg = crate::session::get_text(&mut s, &text).unwrap().segments.into_iter().find(|g| g.content.starts_with("wʌn")).unwrap().id;
        std::fs::write(dir.join("page 1.png"), PNG).unwrap();
        std::fs::write(dir.join("page 2.png"), PNG).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a picture").unwrap();
        assert!(add_page(&mut s, &text, dir.join("notes.txt").to_str().unwrap()).unwrap_err().contains("not a picture"));
        add_page(&mut s, &text, dir.join("page 1.png").to_str().unwrap()).unwrap();
        let v = add_page(&mut s, &text, dir.join("page 2.png").to_str().unwrap()).unwrap();
        assert_eq!(v.pages.iter().map(|g| g.label.as_str()).collect::<Vec<_>>(), ["page 1.png", "page 2.png"]);
        assert!(v.pages.iter().all(|g| g.path.is_some()), "the images are in the blob store");
        let second = v.pages[1].source.clone();

        let area = Area { source: second.clone(), x: 0.1, y: 0.1, w: 0.4, h: 0.05 };
        let v = set_area(&mut s, &text, &seg, Some(area.clone())).unwrap();
        assert_eq!(v.marked.len(), 1);
        assert_eq!(v.marked[0].area, area);
        assert!(v.marked[0].content.starts_with("wʌn"));

        // A page named without its picture gets one.
        let bare = crate::session::project(&mut s).unwrap().add_source("folio 3", None, "image", None).unwrap();
        crate::session::project(&mut s).unwrap().add_text_page(&text, &bare).unwrap();
        assert!(view(&mut s, &text).unwrap().pages[2].path.is_none());
        let v = set_page_image(&mut s, &text, &bare, dir.join("page 1.png").to_str().unwrap()).unwrap();
        assert!(v.pages[2].path.is_some());
        let v = move_page(&mut s, &text, &second, -1).unwrap();
        assert_eq!(v.pages[0].source, second);
        let v = remove_page(&mut s, &text, &second).unwrap();
        assert_eq!(v.pages.len(), 2);
        assert!(v.marked.is_empty(), "an area on a page taken off is cleared");
        drop(s);
        std::fs::remove_dir_all(&dir).ok();
    }
}
