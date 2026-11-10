//! Project settings (DATA-MODEL §3, `Setting`): one entity per key in the
//! project config scope, so a setting syncs to every member and two devices
//! that set it at once get the ordinary conflict card. The entity's id is
//! derived from its key, so a first write on two devices before they sync
//! names the same entity. Today's keys are capability pins
//! (`capability:<id>`, value `automatic` or a package id; docs/design/PACKAGES.md).

use wugs_model::Value;

use crate::lexicon::id_from_key;
use crate::{CommitResult, Project, SCOPE_CONFIG};

const MAX_KEY: usize = 128;
const MAX_VALUE: usize = 1024;

/// The entity id a setting key lives on.
pub fn setting_id(key: &str) -> String {
    id_from_key("stg", key)
}

impl Project {
    /// The stored value of `key`, if anyone set it.
    pub fn setting(&self, key: &str) -> Result<Option<String>, String> {
        let id = setting_id(key);
        if self.proj.entity_kind(&id)?.as_deref() != Some("setting")
            || self.proj.is_tombstoned(&id)?
        {
            return Ok(None);
        }
        self.proj.field_str(&id, "value")
    }

    /// Set `key` to `value`: a person's choice, so it names every open
    /// version and settles a conflict on the setting.
    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<CommitResult, String> {
        if key.is_empty() || key.len() > MAX_KEY || !key.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(format!("{key:?} is not a setting key"));
        }
        if value.len() > MAX_VALUE {
            return Err("a setting's value is at most 1 kB".into());
        }
        let id = setting_id(key);
        let exists = match self.proj.entity_kind(&id)?.as_deref() {
            Some("setting") => true,
            None => false,
            Some(other) => return Err(format!("{id} is a {other}, not a setting")),
        };
        let mut t = self.intent("set-setting").resolving();
        if exists {
            t.set("setting", &id, SCOPE_CONFIG, "value", Value::str(value))?;
        } else {
            t.push_create(
                "setting",
                &id,
                SCOPE_CONFIG,
                vec![("key", Value::str(key)), ("value", Value::str(value))],
            );
        }
        t.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Op;

    fn open(actor: &str) -> (std::path::PathBuf, Project) {
        let root = std::env::temp_dir().join(format!("wugs-setting-test-{}", wugs_model::ulid()));
        let p = Project::open(&root, actor).unwrap();
        (root, p)
    }

    fn theirs(p: &Project) -> Vec<Op> {
        p.log
            .read_all()
            .unwrap()
            .ops
            .into_iter()
            .filter(|o| o.device == p.device())
            .collect()
    }

    #[test]
    fn a_setting_survives_replay_and_reaches_the_other_device() {
        let (ra, mut a) = open("prs_Ana");
        let (rb, mut b) = open("prs_Bo");
        assert_eq!(a.setting("capability:phone-transcription").unwrap(), None);
        a.set_setting("capability:phone-transcription", "wav2vec2-espeak")
            .unwrap();
        a.set_setting("capability:phone-transcription", "zipa-small")
            .unwrap();
        assert_eq!(
            a.setting("capability:phone-transcription")
                .unwrap()
                .as_deref(),
            Some("zipa-small")
        );
        b.ingest(theirs(&a)).unwrap();
        assert_eq!(
            b.setting("capability:phone-transcription")
                .unwrap()
                .as_deref(),
            Some("zipa-small")
        );
        drop(a);
        std::fs::remove_dir_all(ra.join("cache")).unwrap();
        let a = Project::open(&ra, "prs_Ana").unwrap();
        assert_eq!(
            a.setting("capability:phone-transcription")
                .unwrap()
                .as_deref(),
            Some("zipa-small")
        );
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }

    #[test]
    fn two_first_pins_name_one_entity_and_a_later_pick_settles_them() {
        let (ra, mut a) = open("prs_Ana");
        let (rb, mut b) = open("prs_Bo");
        a.set_setting("capability:phone-transcription", "wav2vec2-espeak")
            .unwrap();
        b.set_setting("capability:phone-transcription", "zipa-small")
            .unwrap();
        a.ingest(theirs(&b)).unwrap();
        b.ingest(theirs(&a)).unwrap();
        assert_eq!(
            a.setting("capability:phone-transcription").unwrap(),
            b.setting("capability:phone-transcription").unwrap()
        );
        assert_eq!(a.proj.open_conflict_count().unwrap(), 1);
        a.set_setting("capability:phone-transcription", "automatic")
            .unwrap();
        b.ingest(theirs(&a)).unwrap();
        assert_eq!(a.proj.open_conflict_count().unwrap(), 0);
        assert_eq!(b.proj.open_conflict_count().unwrap(), 0);
        assert_eq!(
            b.setting("capability:phone-transcription")
                .unwrap()
                .as_deref(),
            Some("automatic")
        );
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }

    #[test]
    fn the_same_first_value_on_two_devices_is_not_a_conflict() {
        let (ra, mut a) = open("prs_Ana");
        let (rb, mut b) = open("prs_Bo");
        a.set_setting("capability:phone-transcription", "automatic")
            .unwrap();
        b.set_setting("capability:phone-transcription", "automatic")
            .unwrap();
        a.ingest(theirs(&b)).unwrap();
        assert_eq!(a.proj.open_conflict_count().unwrap(), 0);
        std::fs::remove_dir_all(ra).ok();
        std::fs::remove_dir_all(rb).ok();
    }

    #[test]
    fn bad_keys_are_refused() {
        let (ra, mut a) = open("prs_Ana");
        assert!(a.set_setting("", "x").is_err());
        assert!(a.set_setting("a b", "x").is_err());
        assert!(a.set_setting("k", &"x".repeat(2000)).is_err());
        std::fs::remove_dir_all(ra).ok();
    }
}
