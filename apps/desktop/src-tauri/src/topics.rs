//! Topics: sentences filed under a grammar topic ("negation", "relative
//! clauses") while glossing, gathered here for writing a grammar
//! (`wugs_core::topics`). The panel lists each topic with its sentences and
//! copies them as numbered examples.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::session::{project, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicSentence {
    pub text_id: String,
    pub text_title: String,
    pub segment_id: String,
    pub tier_id: String,
    pub content: String,
    /// Its first translation, when it has one.
    pub translation: Option<String>,
    pub start: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Topic {
    pub name: String,
    /// Each sentence once, by text title then place in the text.
    pub sentences: Vec<TopicSentence>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Topics {
    /// By name, capitals aside.
    pub topics: Vec<Topic>,
}

pub fn overview(session: &mut Session) -> Result<Topics, String> {
    let p = project(session)?;
    // Spellings of one topic (two devices, capitals) are one topic, named as most sentences have it.
    let mut groups: BTreeMap<String, (BTreeMap<String, usize>, Vec<(String, String)>)> = BTreeMap::new();
    for u in p.topic_uses()? {
        let g = groups.entry(wugs_model::fold_key(&u.name)).or_default();
        *g.0.entry(u.name).or_default() += 1;
        if !g.1.iter().any(|(_, s)| *s == u.segment) {
            g.1.push((u.text, u.segment));
        }
    }
    let mut titles: BTreeMap<String, String> = BTreeMap::new();
    let mut topics = Vec::new();
    for (_, (names, uses)) in groups {
        let name = names.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|x| x.0.clone()).unwrap_or_default();
        let mut sentences = Vec::new();
        for (text, seg) in uses {
            let title = match titles.get(&text) {
                Some(t) => t.clone(),
                None => {
                    let t = p.proj.field_str(&text, "title")?.unwrap_or_default();
                    titles.insert(text.clone(), t.clone());
                    t
                }
            };
            let translation = crate::interchange::translations_of(p, &seg)?.into_iter().map(|(_, t)| t).find(|t| !t.trim().is_empty());
            sentences.push((
                p.proj.field_str(&seg, "pos")?.unwrap_or_default(),
                TopicSentence { text_id: text, text_title: title, tier_id: p.proj.field_str(&seg, "tier")?.unwrap_or_default(), content: p.proj.field_str(&seg, "content")?.unwrap_or_default(), translation, start: p.proj.field_num(&seg, "start")?, segment_id: seg },
            ));
        }
        sentences.sort_by(|a, b| {
            wugs_model::fold_key(&a.1.text_title)
                .cmp(&wugs_model::fold_key(&b.1.text_title))
                .then(a.1.text_id.cmp(&b.1.text_id))
                .then(a.1.start.unwrap_or(f64::MAX).total_cmp(&b.1.start.unwrap_or(f64::MAX)))
                .then(a.0.cmp(&b.0))
        });
        topics.push(Topic { name, sentences: sentences.into_iter().map(|x| x.1).collect() });
    }
    Ok(Topics { topics })
}

pub fn rename(session: &mut Session, from: &str, to: &str) -> Result<Topics, String> {
    project(session)?.rename_topic(from, to)?;
    overview(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_gather_sentences_across_texts() {
        let dir = std::env::temp_dir().join(format!("wugs-topics-{}", wugs_model::ulid()));
        let mut s: Session = None;
        crate::sample::open_sample(&mut s, &dir, "tester").unwrap();
        let sample = crate::session::list_texts(&mut s).unwrap()[0].id.clone();
        let other = crate::session::create_text(&mut s, "Another").unwrap();
        let b = crate::session::add_segment(&mut s, &other, None, Some(0.0), Some(800.0), "ka").unwrap().id;
        let segs: Vec<String> = crate::session::get_text(&mut s, &sample).unwrap().segments.into_iter().filter(|x| !x.content.is_empty() && x.parent.is_none()).map(|x| x.id).collect();
        crate::gloss::gloss_add_topic(&mut s, &sample, &segs[0], "Numerals").unwrap();
        crate::gloss::gloss_add_topic(&mut s, &other, &b, "numerals").unwrap();
        let v = crate::gloss::gloss_add_topic(&mut s, &sample, &segs[1], "Numerals").unwrap().view;
        assert_eq!(v.topics, ["Numerals"]);
        let o = overview(&mut s).unwrap();
        assert_eq!(o.topics.len(), 1, "capitals aside, one topic");
        let t = &o.topics[0];
        assert_eq!(t.name, "Numerals", "named as most sentences have it");
        assert_eq!(t.sentences.len(), 3);
        assert_eq!(t.sentences[0].text_title, "Another", "texts by title");
        assert!(t.sentences[1..].iter().any(|x| x.translation.is_some()), "the sample's sentences are translated");
        let o = rename(&mut s, "numerals", "Counting").unwrap();
        assert_eq!((o.topics[0].name.as_str(), o.topics[0].sentences.len()), ("Counting", 3));
        let v = crate::gloss::gloss_remove_topic(&mut s, &sample, &segs[0], "counting").unwrap().view;
        assert!(v.topics.is_empty());
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
