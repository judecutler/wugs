//! Glottolog's languages and dialects (5.4, CC BY 4.0; `datasets/glottolog/`),
//! embedded so the Texts panel's Language row can offer the Glottocode and
//! ISO 639-3 code for a name as it is typed, with nothing sent anywhere.

use serde::Serialize;
use std::sync::OnceLock;

const TABLE: &str = include_str!("../../../../datasets/glottolog/languoids.tsv");

/// The edition the table was built from, for the panel's credit line.
pub const EDITION: &str = "Glottolog 5.4 (CC BY 4.0)";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Languoid {
    pub glottocode: &'static str,
    pub iso: &'static str,
    pub name: &'static str,
    /// A dialect rather than a language.
    pub dialect: bool,
    /// The family's name, `isolate`, or empty (unclassified, sign languages…).
    pub family: &'static str,
    pub macroarea: &'static str,
    /// For a dialect, the language it belongs to.
    pub language: &'static str,
    /// Where Glottolog places it, in degrees, when it does.
    pub lat: Option<f32>,
    pub lon: Option<f32>,
}

struct Row {
    item: Languoid,
    key: String,
}

/// Lower case, Turkic i's together, no diacritics, apostrophes and
/// modifier letters (ʼ ʻ ' ’) dropped, and anything else but letters and
/// digits read as a space, so «Kʼicheʼ», «K'iche'» and «kiche» all meet.
pub fn key(s: &str) -> String {
    use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
    let folded: String = wugs_model::fold_key(s).nfd().filter(|c| !is_combining_mark(*c)).collect();
    let spaced: String = folded
        .chars()
        .filter(|c| !matches!(c, '\'' | '\u{2019}' | '\u{2018}' | '`' | '\u{2B0}'..='\u{2FF}'))
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| {
        TABLE
            .lines()
            .skip(1)
            .filter_map(|line| {
                let mut f = line.split('\t');
                let item = Languoid {
                    glottocode: f.next()?,
                    iso: f.next()?,
                    name: f.next()?,
                    dialect: f.next()? == "D",
                    family: f.next()?,
                    macroarea: f.next()?,
                    language: f.next().unwrap_or(""),
                    lat: f.next().and_then(|v| v.parse().ok()),
                    lon: f.next().and_then(|v| v.parse().ok()),
                };
                Some(Row { key: key(item.name), item })
            })
            .collect()
    })
}

/// How well a row answers the query: lower is better, None is no match.
fn rank(row: &Row, q: &str, code: &str) -> Option<u8> {
    if row.item.glottocode == code || (!row.item.iso.is_empty() && row.item.iso == code) {
        return Some(0);
    }
    if q.is_empty() {
        return None;
    }
    if row.key == q {
        Some(1)
    } else if row.key.starts_with(q) {
        Some(2)
    } else if row.key.split(' ').any(|w| w.starts_with(q)) || row.key.contains(&format!(" {q}")) {
        Some(3)
    } else if q.chars().count() >= 3 && row.key.contains(q) {
        Some(4)
    } else {
        None
    }
}

/// The languoids a typed name or code fits, best first: an exact
/// Glottocode or ISO code, then the whole name, then names starting with
/// it, then a word in the name starting with it, then (three letters or
/// more) anywhere in it; languages before dialects, then shorter names.
pub fn find(query: &str, limit: usize) -> Vec<Languoid> {
    let q = key(query);
    let code = query.trim().to_lowercase();
    if q.chars().count() < 2 && code.chars().count() < 3 {
        return Vec::new();
    }
    let mut hits: Vec<(u8, &Row)> = rows().iter().filter_map(|r| rank(r, &q, &code).map(|k| (k, r))).collect();
    hits.sort_by(|(a, x), (b, y)| a.cmp(b).then(x.item.dialect.cmp(&y.item.dialect)).then(x.item.name.len().cmp(&y.item.name.len())).then(x.item.name.cmp(y.item.name)));
    hits.into_iter().take(limit).map(|(_, r)| r.item.clone()).collect()
}

fn index() -> &'static std::collections::HashMap<&'static str, usize> {
    static INDEX: OnceLock<std::collections::HashMap<&'static str, usize>> = OnceLock::new();
    INDEX.get_or_init(|| rows().iter().enumerate().map(|(i, r)| (r.item.glottocode, i)).collect())
}

/// One languoid by its Glottocode.
pub fn by_glottocode(code: &str) -> Option<Languoid> {
    let code = code.trim().to_lowercase();
    index().get(code.as_str()).map(|i| rows()[*i].item.clone())
}

/// The language (not a dialect) of this name in this family, as a
/// dialect's `language` column names it; None when there is none or several.
pub fn by_name(name: &str, family: &str) -> Option<Languoid> {
    let mut found = rows().iter().filter(|r| !r.item.dialect && r.item.name == name && r.item.family == family);
    let first = found.next()?;
    found.next().is_none().then(|| first.item.clone())
}

/// Every languoid, in Glottocode order.
#[cfg(test)]
pub fn all() -> Vec<Languoid> {
    rows().iter().map(|r| r.item.clone()).collect()
}

/// Great-circle distance in kilometres.
pub fn km(a: (f32, f32), b: (f32, f32)) -> f64 {
    let (la1, lo1, la2, lo2) = (f64::from(a.0).to_radians(), f64::from(a.1).to_radians(), f64::from(b.0).to_radians(), f64::from(b.1).to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * 6371.0 * h.sqrt().asin()
}

/// A language near another, as a grammar's introduction names its neighbours.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Neighbour {
    pub glottocode: &'static str,
    pub name: &'static str,
    pub family: &'static str,
    pub lat: f32,
    pub lon: f32,
    pub km: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Place {
    pub lat: f32,
    pub lon: f32,
    pub family: &'static str,
    pub macroarea: &'static str,
    /// The nearest other languages Glottolog places, nearest first.
    pub neighbours: Vec<Neighbour>,
    /// The land to draw a locator map on (`typology::LAND`).
    pub land: &'static str,
}

/// A locator map as a standalone SVG file: 50° of longitude by 30° of land
/// around the place, copies of the land either side of 180°, the nearest
/// languages as grey dots and the language ringed (the grammar sketch's
/// `locatorMap` draws the same in TypeScript).
pub fn locator_svg(p: &Place, label: &str) -> String {
    let r1 = |v: f64| (v * 10.0).round() / 10.0;
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let cx = f64::from(p.lon) + 180.0;
    let cy = 90.0 - f64::from(p.lat);
    let (w, h) = (50.0, 30.0);
    let x = r1(cx - w / 2.0);
    let y = r1((cy - h / 2.0).clamp(0.0, 180.0 - h));
    let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{x} {y} {w} {h}\" role=\"img\" aria-label=\"{}\"><rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"#dbeef4\"/>", esc(label));
    for dx in [-360, 0, 360] {
        out.push_str(&format!("<path d=\"{}\" transform=\"translate({dx} 0)\" fill=\"#f2ede0\" stroke=\"#b9ad8f\" stroke-width=\"0.08\"/>", p.land.trim()));
    }
    for n in &p.neighbours {
        let mut nx = f64::from(n.lon) + 180.0;
        if nx - cx > 180.0 {
            nx -= 360.0;
        }
        if cx - nx > 180.0 {
            nx += 360.0;
        }
        out.push_str(&format!("<circle cx=\"{}\" cy=\"{}\" r=\"0.35\" fill=\"#777\"><title>{}</title></circle>", r1(nx), r1(90.0 - f64::from(n.lat)), esc(n.name)));
    }
    out.push_str(&format!("<circle cx=\"{}\" cy=\"{}\" r=\"0.7\" fill=\"#00718a\" stroke=\"#000\" stroke-width=\"0.25\"/></svg>\n", r1(cx), r1(cy)));
    out
}

/// Where Glottolog places a languoid (a dialect without a place of its own
/// by its language), and its `n` nearest languages; None when it has no place.
pub fn place(code: &str, n: usize) -> Option<Place> {
    let me = by_glottocode(code)?;
    let at = [Some(me.clone()), if me.dialect { by_name(me.language, me.family) } else { None }].into_iter().flatten().find(|l| l.lat.is_some() && l.lon.is_some())?;
    let here = (at.lat?, at.lon?);
    let mut near: Vec<Neighbour> = rows()
        .iter()
        .map(|r| &r.item)
        .filter(|l| !l.dialect && l.glottocode != me.glottocode && l.glottocode != at.glottocode)
        .filter_map(|l| Some(Neighbour { glottocode: l.glottocode, name: l.name, family: l.family, lat: l.lat?, lon: l.lon?, km: km(here, (l.lat?, l.lon?)) }))
        .collect();
    near.sort_by(|a, b| a.km.total_cmp(&b.km).then(a.glottocode.cmp(b.glottocode)));
    near.truncate(n);
    Some(Place { lat: here.0, lon: here.1, family: me.family, macroarea: me.macroarea, neighbours: near, land: crate::typology::LAND })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_whole_and_well_formed() {
        assert!(rows().len() > 20_000, "{}", rows().len());
        for r in rows() {
            assert!(wugs_core::language::valid_glottocode(r.item.glottocode), "{}", r.item.glottocode);
            assert!(r.item.iso.is_empty() || wugs_core::language::valid_iso639_3(r.item.iso), "{} {}", r.item.glottocode, r.item.iso);
            assert!(!r.item.name.is_empty());
        }
    }

    #[test]
    fn names_and_codes_find_their_language() {
        let tr = find("Turkish", 8);
        assert_eq!(tr[0].glottocode, "nucl1301");
        assert_eq!(tr[0].iso, "tur");
        assert_eq!(tr[0].family, "Turkic");
        assert_eq!(find("tur", 3)[0].name, "Turkish", "an ISO code first");
        assert_eq!(find("stan1293", 3)[0].name, "English", "a Glottocode first");
        assert_eq!(find("  STAN1293 ", 3)[0].name, "English");
        assert!(find("turk", 8).iter().any(|l| l.glottocode == "nucl1301"), "a name's start");
        // Diacritics and case do not matter.
        assert_eq!(find("nahuatl", 30).iter().filter(|l| l.name.to_lowercase().contains("nahuatl") || l.name.contains("Náhuatl")).count(), find("nahuatl", 30).len());
        assert!(find("t", 8).is_empty(), "one letter finds nothing");
        assert!(find("zzzzqqq", 8).is_empty());
    }

    #[test]
    fn languages_come_before_dialects() {
        let hits = find("German", 20);
        let first_dialect = hits.iter().position(|l| l.dialect).unwrap_or(hits.len());
        assert!(hits[..first_dialect].iter().all(|l| !l.dialect));
        assert_eq!(by_glottocode("stan1295").map(|l| l.name), Some("German"));
    }

    #[test]
    fn keys_ignore_marks_and_punctuation() {
        assert_eq!(key("Kʼicheʼ"), key("K'iche'"));
        assert_eq!(key("Kʼicheʼ"), "kiche");
        assert_eq!(key("  Nāhuatl  "), "nahuatl");
        assert_eq!(key("İstanbul Türkçesi"), "istanbul turkcesi");
    }

    #[test]
    fn a_place_and_its_neighbours() {
        let p = place("stan1293", 5).unwrap();
        assert_eq!((p.lat, p.lon), (53.0, -1.0));
        assert_eq!(p.neighbours.len(), 5);
        assert!(p.neighbours.windows(2).all(|w| w[0].km <= w[1].km));
        assert!(p.neighbours.iter().all(|n| n.glottocode != "stan1293" && n.km < 800.0), "{:?}", p.neighbours);
        assert!((km((0.0, 0.0), (0.0, 1.0)) - 111.19).abs() < 0.1);
        assert!(place("nope1234", 5).is_none());
        let svg = locator_svg(&p, "English");
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"154 22 50 30\""), "{}", &svg[..120]);
        assert_eq!(svg.matches("<path ").count(), 3);
        assert_eq!(svg.matches("<title>").count(), 5);
    }
}
