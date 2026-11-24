//! Facsimiles: the page images a text was read from (a manuscript, an
//! inscription, a printed source) and where on them each segment stands.
//! A text's `pages` is an ordered list of image sources (DATA-MODEL §3
//! Source of kind `image`, whose `blob` is the scan); a segment's `region`
//! is one object `{source, x, y, w, h}`: the page and a rectangle on it as
//! fractions of the image's width and height, so it holds at any
//! resolution. Both are single fields, so a page list or an area always
//! changes whole, like a pronunciation's `audio`.

use wugs_model::Value;

use crate::{CommitResult, Project};

/// The most pages a text keeps.
pub const MAX_PAGES: usize = 500;

/// Where a segment stands on a page: fractions of the image, origin top left.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub source: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

fn region_of(v: &Value) -> Option<Region> {
    Some(Region { source: v.get_str("source")?.to_string(), x: v.get("x")?.as_f64()?, y: v.get("y")?.as_f64()?, w: v.get("w")?.as_f64()?, h: v.get("h")?.as_f64()? })
}

fn region_value(r: &Region) -> Value {
    Value::obj(vec![("source", Value::str(&r.source)), ("x", Value::Num(r.x)), ("y", Value::Num(r.y)), ("w", Value::Num(r.w)), ("h", Value::Num(r.h))])
}

impl Project {
    fn require_image(&self, source: &str) -> Result<(), String> {
        self.require(source, "source")?;
        match self.proj.field_str(source, "kind")?.as_deref() {
            Some("image") => Ok(()),
            _ => Err(format!("{source} is not a page image")),
        }
    }

    /// A text's pages in order: the ids its `pages` list names, live image
    /// sources only.
    pub fn text_pages(&self, text: &str) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        if let Some((Value::Arr(items), _)) = self.proj.field(text, "pages")? {
            for id in items.iter().filter_map(Value::as_str) {
                if self.require_image(id).is_ok() && !out.iter().any(|o: &String| o == id) {
                    out.push(id.to_string());
                }
            }
        }
        Ok(out)
    }

    /// Write a text's page list (the live ones, in the order given). A
    /// person's arrangement, so resolving; unchanged writes nothing.
    pub fn set_text_pages(&mut self, text: &str, pages: &[&str]) -> Result<Option<CommitResult>, String> {
        self.require(text, "text")?;
        let mut list: Vec<&str> = Vec::new();
        for p in pages {
            self.require_image(p)?;
            if !list.contains(p) {
                list.push(p);
            }
        }
        if list.len() > MAX_PAGES {
            return Err(format!("A text keeps at most {MAX_PAGES} pages."));
        }
        if self.text_pages(text)? == list {
            return Ok(None);
        }
        let mut t = self.intent("set-text-pages").resolving();
        t.set("text", text, text, "pages", Value::Arr(list.iter().map(|p| Value::str(p)).collect()))?;
        t.commit().map(Some)
    }

    /// Add a page after the text's last.
    pub fn add_text_page(&mut self, text: &str, source: &str) -> Result<Option<CommitResult>, String> {
        let mut pages = self.text_pages(text)?;
        pages.push(source.to_string());
        let refs: Vec<&str> = pages.iter().map(String::as_str).collect();
        self.set_text_pages(text, &refs)
    }

    /// Where a segment stands on a page, when it has been marked.
    pub fn segment_region(&self, segment: &str) -> Result<Option<Region>, String> {
        Ok(self.proj.field(segment, "region")?.and_then(|(v, _)| region_of(&v)))
    }

    /// Mark (or with None clear) where a segment stands on a page: a
    /// rectangle inside the image, of a page of the segment's own text.
    /// Resolving; unchanged writes nothing.
    pub fn set_segment_region(&mut self, text: &str, segment: &str, region: Option<&Region>) -> Result<Option<CommitResult>, String> {
        self.require_in_text(segment, "segment", text)?;
        if let Some(r) = region {
            self.require_image(&r.source)?;
            if !self.text_pages(text)?.iter().any(|p| *p == r.source) {
                return Err("That image is not one of this text's pages.".into());
            }
            let inside = |v: f64| v.is_finite() && (0.0..=1.0).contains(&v);
            if !(inside(r.x) && inside(r.y) && r.w > 0.0 && r.h > 0.0 && inside(r.x + r.w) && inside(r.y + r.h)) {
                return Err("An area must be a rectangle inside the page.".into());
            }
        }
        if self.segment_region(segment)?.as_ref() == region {
            return Ok(None);
        }
        let mut t = self.intent("mark-region").resolving();
        t.set("segment", segment, text, "region", region.map(region_value).unwrap_or(Value::Null))?;
        t.commit().map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::Region;
    use crate::Project;

    #[test]
    fn pages_and_regions_are_kept_and_checked() {
        let root = std::env::temp_dir().join(format!("wugs-facsimile-{}", wugs_model::ulid()));
        let mut p = Project::open(&root, "prs_Tester").unwrap();
        let text = p.create_text("Stele").unwrap();
        let (tier, _) = p.add_tier("transliteration", "time-aligned", None, None).unwrap();
        let (seg, _) = p.add_segment(&text, &tier, 0, None, None, "a-na").unwrap();
        let blob = format!("blake3:{}", "0".repeat(64));
        let one = p.add_source("face A", Some(&blob), "image", None).unwrap();
        let two = p.add_source("face B", Some(&blob), "image", None).unwrap();
        let sound = p.add_source("reading", None, "audio", None).unwrap();

        assert!(p.add_text_page(&text, &sound).unwrap_err().contains("not a page image"));
        p.add_text_page(&text, &one).unwrap();
        p.add_text_page(&text, &two).unwrap();
        assert_eq!(p.text_pages(&text).unwrap(), [one.clone(), two.clone()]);
        assert!(p.add_text_page(&text, &one).unwrap().is_none(), "a page already there is not added twice");
        p.set_text_pages(&text, &[&two, &one]).unwrap();
        assert_eq!(p.text_pages(&text).unwrap(), [two.clone(), one.clone()]);

        let r = Region { source: one.clone(), x: 0.1, y: 0.2, w: 0.5, h: 0.1 };
        p.set_segment_region(&text, &seg, Some(&r)).unwrap();
        assert_eq!(p.segment_region(&seg).unwrap(), Some(r.clone()));
        assert!(p.set_segment_region(&text, &seg, Some(&r)).unwrap().is_none(), "unchanged writes nothing");
        let outside = Region { w: 0.95, ..r.clone() };
        assert!(p.set_segment_region(&text, &seg, Some(&outside)).unwrap_err().contains("inside the page"));
        let elsewhere = p.add_source("another stone", Some(&blob), "image", None).unwrap();
        assert!(p.set_segment_region(&text, &seg, Some(&Region { source: elsewhere, ..r.clone() })).unwrap_err().contains("not one of this text's pages"));
        p.set_segment_region(&text, &seg, None).unwrap();
        assert_eq!(p.segment_region(&seg).unwrap(), None);
        std::fs::remove_dir_all(root).ok();
    }
}
