//! Topics on a sentence: the grammar writer's collections ("negation",
//! "relative clause", "evidentials"), so the examples for a chapter can be
//! gathered while glossing and pulled out later as numbered examples. Each
//! topic on a sentence is an Annotation (DATA-MODEL §3) of kind `topic`
//! whose `body` is the topic's name, in the sentence's text's scope like a
//! note. One annotation per topic per sentence, so two devices adding
//! different topics to one sentence both keep theirs, and taking one off
//! is a tombstone that never touches the others.

use wugs_model::Value;

use crate::{CommitResult, Project};

/// The longest topic name kept (characters).
pub const MAX_TOPIC: usize = 80;
pub const TOPIC_KIND: &str = "topic";

/// One sentence filed under a topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicUse {
    pub annotation: String,
    pub name: String,
    pub segment: String,
    pub text: String,
}

/// A topic's name as kept: NFC, spaces collapsed.
fn clean(p: &Project, name: &str) -> Result<String, String> {
    let name = p.proj.nfc(&name.split_whitespace().collect::<Vec<_>>().join(" "));
    if name.is_empty() {
        return Err("A topic needs a name.".into());
    }
    if name.chars().count() > MAX_TOPIC {
        return Err(format!("A topic's name is at most {MAX_TOPIC} characters."));
    }
    Ok(name)
}

/// Two names for one topic: capitals aside.
pub fn same_topic(a: &str, b: &str) -> bool {
    wugs_model::fold_key(a) == wugs_model::fold_key(b)
}

impl Project {
    /// The topics a segment is filed under, by name: `(annotation id, name)`.
    pub fn segment_topics(&self, segment: &str) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        for id in self.proj.children_by_ref("target", segment, "annotation")? {
            if self.proj.field_str(&id, "kind")?.as_deref() == Some(TOPIC_KIND) {
                out.push((id.clone(), self.proj.field_str(&id, "body")?.unwrap_or_default()));
            }
        }
        out.sort_by(|a, b| wugs_model::fold_key(&a.1).cmp(&wugs_model::fold_key(&b.1)).then(a.0.cmp(&b.0)));
        Ok(out)
    }

    /// File a segment of `text` under a topic. A topic it already has
    /// (capitals aside) writes nothing and returns None.
    pub fn add_segment_topic(&mut self, text: &str, segment: &str, name: &str) -> Result<Option<(String, CommitResult)>, String> {
        self.require_in_text(segment, "segment", text)?;
        let name = clean(self, name)?;
        if self.segment_topics(segment)?.iter().any(|(_, n)| same_topic(n, &name)) {
            return Ok(None);
        }
        let mut t = self.intent("add-topic");
        let id = t.create("annotation", "ann", text, vec![("target", Value::str(segment)), ("kind", Value::str(TOPIC_KIND)), ("body", Value::str(&name))]);
        Ok(Some((id, t.commit()?)))
    }

    /// Take a segment off a topic: every live annotation of that name on it
    /// (two devices may each have filed it), so it is off everywhere.
    pub fn remove_segment_topic(&mut self, text: &str, segment: &str, name: &str) -> Result<Option<CommitResult>, String> {
        self.require_in_text(segment, "segment", text)?;
        let gone: Vec<String> = self.segment_topics(segment)?.into_iter().filter(|(_, n)| same_topic(n, name)).map(|(id, _)| id).collect();
        if gone.is_empty() {
            return Ok(None);
        }
        let mut t = self.intent("remove-topic");
        for id in &gone {
            t.delete("annotation", id, text);
        }
        t.commit().map(Some)
    }

    /// Every live sentence filed under a topic, in a live text: one row per
    /// annotation (a sentence filed twice under one name by two devices
    /// shows twice; callers count sentences).
    pub fn topic_uses(&self) -> Result<Vec<TopicUse>, String> {
        let mut out = Vec::new();
        for id in self.proj.entities_of_kind("annotation")? {
            if self.proj.field_str(&id, "kind")?.as_deref() != Some(TOPIC_KIND) {
                continue;
            }
            let Some(segment) = self.proj.field_str(&id, "target")? else { continue };
            if self.proj.entity_kind(&segment)?.as_deref() != Some("segment") || self.proj.is_tombstoned(&segment)? {
                continue;
            }
            let Some(text) = self.proj.field_str(&segment, "text")? else { continue };
            if self.proj.is_tombstoned(&text)? {
                continue;
            }
            out.push(TopicUse { name: self.proj.field_str(&id, "body")?.unwrap_or_default(), annotation: id, segment, text });
        }
        Ok(out)
    }

    /// Rename a topic everywhere (or fold it into another): each sentence
    /// under `from` gets `to` once, and its `from` annotations go. Per text
    /// one txn. Returns how many sentences moved.
    pub fn rename_topic(&mut self, from: &str, to: &str) -> Result<usize, String> {
        let to = clean(self, to)?;
        let mut by_text: std::collections::BTreeMap<String, Vec<TopicUse>> = Default::default();
        for u in self.topic_uses()? {
            if same_topic(&u.name, from) && u.name != to {
                by_text.entry(u.text.clone()).or_default().push(u);
            }
        }
        let mut moved = 0;
        for (text, uses) in by_text {
            let mut done = std::collections::HashSet::new();
            let mut has: std::collections::HashMap<String, bool> = Default::default();
            for u in &uses {
                let already = self.segment_topics(&u.segment)?.iter().any(|(id, n)| n == &to && *id != u.annotation);
                has.insert(u.segment.clone(), already);
            }
            let mut t = self.intent("rename-topic");
            for u in &uses {
                t.delete("annotation", &u.annotation, &text);
                if !has[&u.segment] && done.insert(u.segment.clone()) {
                    t.create("annotation", "ann", &text, vec![("target", Value::str(&u.segment)), ("kind", Value::str(TOPIC_KIND)), ("body", Value::str(&to))]);
                    moved += 1;
                }
            }
            t.commit()?;
        }
        Ok(moved)
    }
}

#[cfg(test)]
mod tests {
    use crate::Project;

    #[test]
    fn sentences_are_filed_under_topics() {
        let root = std::env::temp_dir().join(format!("wugs-topics-{}", wugs_model::ulid()));
        let mut p = Project::open(&root, "prs_Tester").unwrap();
        let text = p.create_text("Story").unwrap();
        let (tier, _) = p.add_tier("utterance", "time-aligned", Some("transcription"), None).unwrap();
        let (a, _) = p.add_segment(&text, &tier, 0, Some(0.0), Some(900.0), "ka ni").unwrap();
        let (b, _) = p.add_segment(&text, &tier, 1, Some(900.0), Some(1800.0), "te ka").unwrap();
        assert!(p.add_segment_topic(&text, &a, "  ").is_err());
        p.add_segment_topic(&text, &a, " Negation ").unwrap().unwrap();
        p.add_segment_topic(&text, &a, "evidentials").unwrap().unwrap();
        assert!(p.add_segment_topic(&text, &a, "negation").unwrap().is_none(), "capitals aside, once");
        p.add_segment_topic(&text, &b, "Negation").unwrap().unwrap();
        let names: Vec<String> = p.segment_topics(&a).unwrap().into_iter().map(|x| x.1).collect();
        assert_eq!(names, ["evidentials", "Negation"]);
        assert_eq!(p.topic_uses().unwrap().len(), 3);
        // Renamed into another spelling, then folded into an existing topic.
        assert_eq!(p.rename_topic("negation", "Clausal negation").unwrap(), 2);
        assert_eq!(p.topic_uses().unwrap().iter().filter(|u| u.name == "Clausal negation").count(), 2);
        p.add_segment_topic(&text, &a, "Polarity").unwrap().unwrap();
        assert_eq!(p.rename_topic("clausal negation", "Polarity").unwrap(), 1, "a sentence already under Polarity is not filed twice");
        assert_eq!(p.topic_uses().unwrap().iter().filter(|u| u.name == "Polarity").count(), 2);
        p.remove_segment_topic(&text, &a, "POLARITY").unwrap().unwrap();
        assert!(p.remove_segment_topic(&text, &a, "polarity").unwrap().is_none());
        assert_eq!(p.segment_topics(&a).unwrap().into_iter().map(|x| x.1).collect::<Vec<_>>(), ["evidentials"]);
        // A deleted sentence's topics drop out.
        p.delete_segment(&text, &b).unwrap();
        assert!(p.topic_uses().unwrap().iter().all(|u| u.segment == a));
        let _ = std::fs::remove_dir_all(root);
    }
}
