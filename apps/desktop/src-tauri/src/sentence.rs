//! The Sentence panel (roadmap 41, SCROLLS.md §1 "Correct where you look"):
//! one sentence's words with lemma, tags, features, head and relation, to
//! draw as a dependency tree and correct cell by cell. The rules and the
//! write are `wugs_conllu::tree`'s.

use serde::Serialize;

use crate::session::{project, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordRow {
    pub token: String,
    pub form: String,
    pub lemma: String,
    pub upos: String,
    pub xpos: String,
    pub feats: String,
    /// Word number of the head (0 = root), or none.
    pub head: Option<usize>,
    pub deprel: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SentenceView {
    pub segment_id: String,
    pub content: String,
    pub words: Vec<WordRow>,
    pub annotated: bool,
    pub empty_nodes: usize,
    pub problems: Vec<String>,
}

fn require(p: &wugs_core::Project, text_id: &str, segment_id: &str) -> Result<(), String> {
    if p.proj.entity_kind(segment_id)?.as_deref() != Some("segment") || p.proj.is_tombstoned(segment_id)? || p.proj.field_str(segment_id, "text")?.as_deref() != Some(text_id) {
        return Err("that sentence is no longer here".into());
    }
    Ok(())
}

fn view(p: &wugs_core::Project, segment_id: &str) -> Result<SentenceView, String> {
    let r = wugs_conllu::tree::rows(p, segment_id)?;
    Ok(SentenceView {
        segment_id: segment_id.to_string(),
        content: p.proj.field_str(segment_id, "content")?.unwrap_or_default(),
        words: r.rows.into_iter().map(|w| WordRow { token: w.token, form: w.form, lemma: w.lemma, upos: w.upos, xpos: w.xpos, feats: w.feats, head: w.head, deprel: w.deprel }).collect(),
        annotated: r.annotated,
        empty_nodes: r.empty_nodes,
        problems: r.problems,
    })
}

pub fn sentence_get(session: &mut Session, text_id: &str, segment_id: &str) -> Result<SentenceView, String> {
    let p = project(session)?;
    require(p, text_id, segment_id)?;
    view(p, segment_id)
}

/// Correct one cell; the whole sentence comes back.
pub fn sentence_set(session: &mut Session, text_id: &str, segment_id: &str, token_id: &str, column: &str, value: &str) -> Result<SentenceView, String> {
    let p = project(session)?;
    require(p, text_id, segment_id)?;
    wugs_conllu::tree::set_cell(p, text_id, segment_id, token_id, column, value)?;
    view(p, segment_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_treebank_sentence_reads_and_corrects() {
        let dir = std::env::temp_dir().join(format!("wugs-sentence-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::session::create_project(&mut s, dir.to_str().unwrap(), "prs_Ana").unwrap();
        let p = project(&mut s).unwrap();
        let tier = crate::session::utterance_tier(p).unwrap();
        let ud = "1\tDogs\tdog\tNOUN\t_\tNumber=Plur\t2\tnsubj\t_\t_\n2\tbark\tbark\tVERB\t_\t_\t0\troot\t_\tSpaceAfter=No\n3\t.\t.\tPUNCT\t_\t_\t2\tpunct\t_\t_\n\n";
        let text = wugs_conllu::import_conllu(p, ud, "dogs", &tier).unwrap().texts[0].clone();
        let seg = p.proj.ordered_children("text", &text, "segment").unwrap()[0].clone();
        let v = sentence_get(&mut s, &text, &seg).unwrap();
        assert_eq!(v.words.len(), 3, "punctuation is a word of the tree");
        assert_eq!((v.words[0].head, v.words[1].head), (Some(2), Some(0)));
        let dogs = v.words[0].token.clone();
        let v = sentence_set(&mut s, &text, &seg, &dogs, "deprel", "obj").unwrap();
        assert_eq!(v.words[0].deprel, "obj");
        assert!(sentence_set(&mut s, &text, &seg, &dogs, "upos", "NOPE").unwrap_err().contains("not a UD"));
        assert!(sentence_get(&mut s, "txt_elsewhere", &seg).is_err());
        drop(s);
        std::fs::remove_dir_all(dir).ok();
    }
}
