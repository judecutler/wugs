//! A text as a Pangloss Collection XML document (https://pangloss.cnrs.fr),
//! the LACITO archive format the Pangloss and Cocoon archives take: `TEXT`
//! with a `HEADER` (title, sound file), an `S` per sentence with its `AUDIO`
//! span in seconds, `FORM`, each translation as `TRANSL xml:lang`, and its
//! words as `W` (form, glosses joined at Leipzig's markers) with an `M` per
//! morph (surface and gloss). Export only.

use wugs_core::Project;

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

fn seconds(ms: f64) -> String {
    let s = format!("{:.3}", ms / 1000.0);
    let s = s.trim_end_matches('0');
    s.strip_suffix('.').map(|t| format!("{t}.0")).unwrap_or_else(|| s.to_string())
}

/// The document, and how many sentences have morphs.
pub fn export(p: &Project, text_id: &str, title: &str, language: Option<&str>, sound: Option<&str>, segments: &[String]) -> Result<(String, usize), String> {
    let lang = language.unwrap_or("und");
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let id: String = text_id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
    out.push_str(&format!("<TEXT id=\"wugs-{id}\" xml:lang=\"{}\">\n  <HEADER>\n    <TITLE xml:lang=\"{}\">{}</TITLE>\n", esc(lang), esc(lang), esc(title)));
    if let Some(sound) = sound {
        out.push_str(&format!("    <SOUNDFILE href=\"{}\"/>\n", esc(sound)));
    }
    out.push_str("  </HEADER>\n");
    let mut glossed = 0;
    for (i, seg) in segments.iter().enumerate() {
        out.push_str(&format!("  <S id=\"S{}\">\n", i + 1));
        if let (Some(start), Some(end)) = (p.proj.field_num(seg, "start")?, p.proj.field_num(seg, "end")?) {
            if end > start {
                out.push_str(&format!("    <AUDIO start=\"{}\" end=\"{}\"/>\n", seconds(start), seconds(end)));
            }
        }
        let content = p.proj.field_str(seg, "content")?.unwrap_or_default();
        out.push_str(&format!("    <FORM>{}</FORM>\n", esc(content.trim())));
        for (tl, text) in crate::interchange::translations_of(p, seg)? {
            out.push_str(&format!("    <TRANSL xml:lang=\"{}\">{}</TRANSL>\n", esc(tl.as_deref().filter(|t| !t.is_empty()).unwrap_or("und")), esc(text.trim())));
        }
        let mut any = false;
        for tok in p.proj.members(seg)? {
            if p.proj.is_punct_token(&tok) {
                continue;
            }
            let form = p.proj.field_str(&tok, "form")?.unwrap_or_default();
            out.push_str(&format!("    <W>\n      <FORM>{}</FORM>\n", esc(&form)));
            if let Some((_, glosses)) = crate::website::word_lines(p, &tok)? {
                any = true;
                out.push_str(&format!("      <TRANSL xml:lang=\"en\">{}</TRANSL>\n", esc(&glosses)));
                for m in p.proj.members(&tok)? {
                    let surface = p.proj.field_str(&m, "surface")?.unwrap_or_default();
                    let gloss = p.proj.field_str(&m, "gloss")?.unwrap_or_default();
                    out.push_str(&format!("      <M>\n        <FORM>{}</FORM>\n", esc(surface.trim())));
                    if !gloss.trim().is_empty() {
                        out.push_str(&format!("        <TRANSL xml:lang=\"en\">{}</TRANSL>\n", esc(gloss.trim())));
                    }
                    out.push_str("      </M>\n");
                }
            }
            out.push_str("    </W>\n");
        }
        glossed += usize::from(any);
        out.push_str("  </S>\n");
    }
    out.push_str("</TEXT>\n");
    Ok((out, glossed))
}

#[cfg(test)]
mod tests {
    #[test]
    fn seconds_are_written_short() {
        assert_eq!(super::seconds(1200.0), "1.2");
        assert_eq!(super::seconds(3000.0), "3.0");
        assert_eq!(super::seconds(1234.5), "1.234");
    }

    #[test]
    fn the_tutorial_text_goes_out_as_pangloss_xml() {
        let dir = std::env::temp_dir().join(format!("wugs-pangloss-{}", std::process::id()));
        let out = std::env::temp_dir().join(format!("wugs-pangloss-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let mut session: crate::session::Session = None;
        crate::sample::open_sample(&mut session, &dir, "tester").unwrap();
        let text = crate::session::list_texts(&mut session).unwrap().into_iter().next().unwrap().id;
        let done = crate::interchange::export_text(&mut session, &text, crate::interchange::Format::Pangloss, &out).unwrap();
        let xml = std::fs::read_to_string(&done.path).unwrap();
        assert!(done.path.ends_with(".xml"), "{}", done.path);
        assert!(xml.contains("<SOUNDFILE href=\"Counting to six (tutorial).wav\"/>"), "{xml}");
        assert!(xml.contains("<FORM>wʌn tuː θɹiː</FORM>"), "{xml}");
        assert!(xml.contains("<TRANSL xml:lang=\"en\">one two three</TRANSL>") || xml.contains("<TRANSL xml:lang=\"und\">"), "{xml}");
        assert!(xml.contains("<AUDIO start=\""), "{xml}");
        assert!(xml.contains("<M>\n        <FORM>wʌn</FORM>\n        <TRANSL xml:lang=\"en\">one</TRANSL>"), "{xml}");
        assert!(done.notes.iter().any(|n| n.contains(".wav")), "{:?}", done.notes);
        // Well-formed: the strict reader the importers use takes it.
        wugs_xml::parse(&xml).unwrap();
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&out);
    }
}
