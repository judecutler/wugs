//! SQLite projection of the op log (DATA-MODEL.md §5, §8).
//!
//! Disposable by contract: the only repair path is delete + replay. v0 uses a
//! generic entities+fields layout; typed per-entity tables and FTS arrive with
//! the first real tool (see CLAUDE.md "Projection note").
//!
//! Apply semantics implemented here:
//! - idempotent (an op already applied is a no-op)
//! - per-field LWW by HLC
//! - open heads per field (`heads`/`base` refs) → `conflicts` rows
//! - ops on not-yet-created targets park in `pending` and drain on create

use wugs_model::{json, Op, Value};
use wugs_sqlite::Db;

/// Bump when the cache schema changes shape; an old cache is wiped and
/// `wiped` tells the caller to replay (the cache is disposable by design).
const CACHE_SCHEMA: i64 = 14;

pub struct Projection {
    /// Collaborative-text engine (DATA-MODEL §2.3). None = the spec's
    /// degradation path: text-edit falls back to LWW on `resolved`.
    pub merger: Option<Box<dyn wugs_model::TextMerger>>,
    /// Unicode NFC (PRD §7). Installed at open, before any replay: lookup keys
    /// and validator #1 compare text through it, so stored text that predates
    /// NFC at entry (or came from a peer without it) still matches. None =
    /// text compares as stored.
    pub normalizer: Option<Box<dyn wugs_model::Normalizer>>,
    /// True when open() discarded an old-schema cache: the caller must
    /// replay the log.
    pub wiped: bool,
    pub db: Db,
}

struct FieldMeta {
    hlc: String,
    op_id: String,
    verb: String,
    legacy_actor: Option<String>,
    /// The field's current value, as JSON.
    value: String,
}

/// What a write itself says the field is: a create's value for the field,
/// a text-edit's `resolved`, a set's value.
fn own_value<'a>(op: &'a Op, field: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    match op.verb.as_str() {
        "create" => op.value.get(field).unwrap_or(&NULL),
        "text-edit" => op.value.get("resolved").unwrap_or(&op.value),
        _ => &op.value,
    }
}

/// The actor of a write made before ops carried `heads` (schema 1), None
/// for a heads-aware write. A schema-1 write is never paired once its actor
/// wrote the field again under schema 1: one person's writes did not
/// conflict under the rule they were made with, and their writers named only
/// `base`, so pairing them now would flag history nobody can act on.
fn legacy_actor(op: &Op) -> Option<&str> {
    (op.schema < 2).then_some(op.actor.as_str())
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub target: String,
    pub kind: String,
    pub field: String,
    /// The matched text with [brackets] around hits, elided with …
    pub snippet: String,
}

/// An edit that sits on a token or morph the winning membership list leaves
/// out, written by another device than the one whose list left it out: work
/// a linguist did that the view no longer shows (GLOSSING-MODEL, "Merge
/// semantics"). Derived from current state, so every device that holds the
/// same ops reports the same rows, and a replay reports them too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenEdit {
    /// The visible segment or token whose list leaves `member` out.
    pub parent: String,
    /// The hidden token or morph.
    pub member: String,
    /// The edited entity: `member`, or a morph of a hidden token.
    pub target: String,
    pub field: String,
    pub op_id: String,
}

/// One version of a field in an open conflict.
#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    pub op_id: String,
    pub hlc: String,
    /// What this write itself set (a text-edit's `resolved`).
    pub value: Value,
}

/// A field with open versions besides its winner (DATA-MODEL §5.3).
#[derive(Debug, Clone, PartialEq)]
pub struct OpenConflict {
    pub target: String,
    pub kind: String,
    pub field: String,
    /// The winner; its `value` is what the field shows now (for merged text,
    /// the merged text rather than the winning edit's own).
    pub winner: Version,
    /// The open versions paired with the winner, oldest first.
    pub others: Vec<Version>,
    /// The winner is a text-edit: a resolution is written as one.
    pub text: bool,
}

#[derive(Debug, Default, Clone)]
pub struct ApplyStats {
    pub applied: usize,
    pub skipped_dupes: usize,
    pub parked_pending: usize,
    /// Conflict rows newly opened by the ops applied with these stats (a row
    /// re-derived while already open is not counted again; closed rows are
    /// not subtracted). The `conflicts` table holds the rows open now.
    pub conflicts: usize,
    pub lww_losses: usize,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS entities(
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  tombstone INTEGER NOT NULL DEFAULT 0,
  created_hlc TEXT
);
CREATE INDEX IF NOT EXISTS entities_kind ON entities(kind);
CREATE TABLE IF NOT EXISTS fields(
  target TEXT NOT NULL,
  field TEXT NOT NULL,
  value TEXT NOT NULL,
  hlc TEXT NOT NULL,
  op_id TEXT NOT NULL,
  verb TEXT NOT NULL DEFAULT 'set',
  legacy_actor TEXT,
  PRIMARY KEY(target, field)
);
CREATE INDEX IF NOT EXISTS fields_lookup ON fields(field, value);
CREATE TABLE IF NOT EXISTS ops_applied(
  op_id TEXT PRIMARY KEY,
  hlc TEXT NOT NULL,
  scope TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS ops_hlc ON ops_applied(scope, hlc);
-- One row per open head of a field that is not its winner, unless both
-- are text-edits Loro merged, or the head is a schema-1 write whose actor
-- later wrote the field again under schema 1 (DATA-MODEL §5.3). Recomputed from the heads, so
-- a row exists exactly while its conflict is open.
CREATE TABLE IF NOT EXISTS conflicts(
  target TEXT NOT NULL,
  field TEXT NOT NULL,
  winner_op TEXT NOT NULL,
  loser_op TEXT NOT NULL,
  PRIMARY KEY(target, field, loser_op)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS pending(
  op_id TEXT PRIMARY KEY,
  missing_target TEXT NOT NULL,
  hlc TEXT NOT NULL DEFAULT '',
  scope TEXT NOT NULL DEFAULT '',
  op_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS pending_scope ON pending(scope);
CREATE INDEX IF NOT EXISTS pending_target ON pending(missing_target);
CREATE TABLE IF NOT EXISTS text_updates(
  target TEXT NOT NULL,
  field TEXT NOT NULL,
  hlc TEXT NOT NULL,
  op_id TEXT NOT NULL,
  update_b64 TEXT NOT NULL,
  resolved TEXT NOT NULL DEFAULT 'null',
  PRIMARY KEY(target, field, op_id)
);
CREATE INDEX IF NOT EXISTS fields_by_value ON fields(field, value);
-- Fields whose stored loro updates do not merge as a set (e.g. colliding op
-- ids after a crash). Updates only accumulate, so once the full set fails
-- it keeps failing: the field's text-edits take the LWW path, conflict like
-- plain writes, and the value is the HLC winner's own (DATA-MODEL §5.4).
CREATE TABLE IF NOT EXISTS text_unmerged(
  target TEXT NOT NULL,
  field TEXT NOT NULL,
  PRIMARY KEY(target, field)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS quarantine(
  txn TEXT PRIMARY KEY,
  status TEXT NOT NULL,
  reason TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS quarantine_ops(
  op_id TEXT PRIMARY KEY,
  txn TEXT NOT NULL,
  hlc TEXT NOT NULL,
  scope TEXT NOT NULL,
  op_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS quarantine_ops_txn ON quarantine_ops(txn);
CREATE INDEX IF NOT EXISTS quarantine_ops_scope ON quarantine_ops(scope);
CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(
  kind UNINDEXED,
  field UNINDEXED,
  body,
  tokenize='unicode61 remove_diacritics 2'
);
CREATE TABLE IF NOT EXISTS lookup_keys(
  target TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  key TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS lookup_keys_by_key ON lookup_keys(kind, key);
-- A segment's tokens and a token's morphs, in order (DATA-MODEL §3, §5).
-- A live child (by its `segment` / `token` ref) is shown when its parent has
-- no membership list (imported or older data), ordered by `pos` then id, or
-- when the parent's list (`Segment.tokens`, `Token.morphs`: a JSON array of
-- ids) names it, ordered by list index. Every reader of tokens or morphs goes
-- through this view. One SELECT, no UNION, so SQLite flattens it into outer
-- and correlated queries; `parent_ref` is the parent id as the child's ref
-- field stores it (JSON text), which makes `parent_ref = ?` an index seek.
CREATE VIEW IF NOT EXISTS members AS
SELECT json_extract(r.value, '$') AS parent, r.value AS parent_ref, c.id AS child,
       CASE WHEN l.value IS NULL THEN COALESCE(json_extract(p.value, '$'), '')
            ELSE (SELECT MIN(j.key) FROM json_each(l.value) j WHERE j.value = c.id) END AS ord
  FROM fields r
  JOIN entities c ON c.id = r.target AND c.tombstone = 0
   AND c.kind = CASE r.field WHEN 'segment' THEN 'token' ELSE 'morph' END
  LEFT JOIN fields l ON l.target = json_extract(r.value, '$')
   AND l.field = CASE r.field WHEN 'segment' THEN 'tokens' ELSE 'morphs' END
   AND json_type(l.value) = 'array'
  LEFT JOIN fields p ON p.target = c.id AND p.field = 'pos'
 WHERE r.field IN ('segment', 'token')
   AND (l.value IS NULL OR EXISTS (SELECT 1 FROM json_each(l.value) j WHERE j.value = c.id));
-- Every membership list ever written (`Segment.tokens`, `Token.morphs`),
-- winners and losers, so a hidden member's remover can be found by walking
-- the current list's `base` chain (`Projection::hidden_edits`).
CREATE TABLE IF NOT EXISTS list_writes(
  target TEXT NOT NULL,
  field TEXT NOT NULL,
  hlc TEXT NOT NULL,
  op_id TEXT NOT NULL,
  base TEXT,
  value TEXT NOT NULL,
  PRIMARY KEY(target, field, hlc)
);
-- Open versions of each field (DATA-MODEL §5): every hlc any write named
-- in its `heads` (or `base`), and the writes no write names other than the
-- field's winner (the `fields` row, always open). Both are functions of the
-- set of writes held, so arrival order cannot change them.
CREATE TABLE IF NOT EXISTS field_refs(
  target TEXT NOT NULL,
  field TEXT NOT NULL,
  hlc TEXT NOT NULL,
  PRIMARY KEY(target, field, hlc)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS field_heads(
  target TEXT NOT NULL,
  field TEXT NOT NULL,
  hlc TEXT NOT NULL,
  op_id TEXT NOT NULL,
  legacy_actor TEXT,
  -- The write's own value as JSON (a text-edit's `resolved`), so two open
  -- versions holding the same value are not a conflict (DATA-MODEL §5.3).
  value TEXT,
  PRIMARY KEY(target, field, hlc)
) WITHOUT ROWID;
-- The newest schema-1 write of each field by each actor: a schema-1 head is
-- closed by a newer schema-1 write of its own actor (the actor rule those
-- writes were made under), whatever else the field holds.
CREATE TABLE IF NOT EXISTS legacy_writes(
  target TEXT NOT NULL,
  field TEXT NOT NULL,
  actor TEXT NOT NULL,
  hlc TEXT NOT NULL,
  PRIMARY KEY(target, field, actor)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS fts_map(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  target TEXT NOT NULL,
  field TEXT NOT NULL,
  UNIQUE(target, field)
);
";

/// The device part of an encoded HLC (`{millis}-{counter}-{device}`).
fn hlc_device(hlc: &str) -> &str {
    hlc.splitn(3, '-').nth(2).unwrap_or("")
}

impl Projection {
    /// Open the cache file. Its lookup keys depend on whether text is
    /// compared through a normalizer, so a cache built with the other choice
    /// is wiped like an old-schema one and `wiped` asks for a replay.
    pub fn open(path: &str, normalizer: Option<Box<dyn wugs_model::Normalizer>>) -> Result<Projection, String> {
        let db = Db::open(path)?;
        db.exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let wiped = Projection::migrate(&db, CACHE_SCHEMA * 2 + i64::from(normalizer.is_some()))?;
        db.exec(SCHEMA)?;
        Ok(Projection { merger: None, normalizer, wiped, db })
    }

    fn migrate(db: &Db, want: i64) -> Result<bool, String> {
        let mut s = db.prepare("PRAGMA user_version")?;
        let version = if s.step()? { s.col_i64(0) } else { 0 };
        drop(s);
        if version == want {
            return Ok(false);
        }
        for t in [
            "entities",
            "fields",
            "ops_applied",
            "conflicts",
            "pending",
            "quarantine",
            "quarantine_ops",
            "text_updates",
            "search_fts",
            "fts_map",
            "lookup_keys",
            "list_writes",
            "field_refs",
            "field_heads",
        ] {
            db.exec(&format!("DROP TABLE IF EXISTS {t}"))?;
        }
        db.exec("DROP VIEW IF EXISTS members")?;
        db.exec(&format!("PRAGMA user_version = {want}"))?;
        Ok(true)
    }

    pub fn open_memory() -> Result<Projection, String> {
        let db = Db::open_memory()?;
        db.exec(SCHEMA)?;
        Ok(Projection { merger: None, normalizer: None, wiped: true, db })
    }

    pub fn begin(&self) -> Result<(), String> {
        self.db.exec("BEGIN")
    }
    pub fn commit(&self) -> Result<(), String> {
        self.db.exec("COMMIT")
    }
    pub fn rollback(&self) -> Result<(), String> {
        self.db.exec("ROLLBACK")
    }
    pub fn savepoint(&self, name: &str) -> Result<(), String> {
        self.db.exec(&format!("SAVEPOINT {name}"))
    }
    pub fn release(&self, name: &str) -> Result<(), String> {
        self.db.exec(&format!("RELEASE {name}"))
    }
    pub fn rollback_to(&self, name: &str) -> Result<(), String> {
        self.db
            .exec(&format!("ROLLBACK TO {name}; RELEASE {name}"))
    }

    pub fn already_applied(&self, op_id: &str) -> Result<bool, String> {
        let mut s = self
            .db
            .prepare("SELECT 1 FROM ops_applied WHERE op_id=?1")?;
        s.bind_text(1, op_id)?;
        s.step()
    }

    /// Apply one op. Caller is responsible for transaction boundaries.
    pub fn apply(&self, op: &Op, stats: &mut ApplyStats) -> Result<(), String> {
        if self.already_applied(&op.id)? {
            stats.skipped_dupes += 1;
            return Ok(());
        }
        let parked = match op.verb.as_str() {
            "create" => {
                self.apply_create(op, stats)?;
                false
            }
            "set" | "text-edit" => self.apply_set(op, stats)?,
            "delete" => self.apply_tombstone(op, 1, stats)?,
            "restore" => self.apply_tombstone(op, 0, stats)?,
            other => return Err(format!("unknown verb {other}")),
        };
        // A parked op is not yet applied: it must survive the dedupe check so
        // drain_pending can run it for real once its target exists.
        if !parked {
            self.mark_applied(op)?;
        }
        Ok(())
    }

    fn mark_applied(&self, op: &Op) -> Result<(), String> {
        let mut s = self
            .db
            .prepare("INSERT OR IGNORE INTO ops_applied(op_id,hlc,scope) VALUES(?1,?2,?3)")?;
        s.bind_text(1, &op.id)?;
        s.bind_text(2, &op.hlc)?;
        s.bind_text(3, &op.scope)?;
        s.step()?;
        Ok(())
    }

    fn entity_known(&self, id: &str) -> Result<bool, String> {
        let mut s = self.db.prepare("SELECT 1 FROM entities WHERE id=?1")?;
        s.bind_text(1, id)?;
        s.step()
    }

    fn apply_create(&self, op: &Op, stats: &mut ApplyStats) -> Result<(), String> {
        {
            let mut s = self.db.prepare(
                "INSERT OR IGNORE INTO entities(id,kind,tombstone,created_hlc) VALUES(?1,?2,0,?3)",
            )?;
            s.bind_text(1, &op.target)?;
            s.bind_text(2, &op.entity)?;
            s.bind_text(3, &op.hlc)?;
            s.step()?;
        }
        if let Some(fields) = op.value.as_obj() {
            for (k, v) in fields {
                self.lww_write(&op.target, k, v, op, None, stats)?;
            }
        }
        stats.applied += 1;
        self.drain_pending(&op.target, stats)?;
        Ok(())
    }

    /// Returns true if the op was parked pending its target's create.
    fn apply_set(&self, op: &Op, stats: &mut ApplyStats) -> Result<bool, String> {
        let Some(field) = op.field.clone() else {
            return Err(format!("set op {} without field", op.id));
        };
        if !self.entity_known(&op.target)? {
            self.park_pending(op)?;
            stats.parked_pending += 1;
            return Ok(true);
        }
        if op.verb == "text-edit" {
            self.apply_text_edit(op, &field, stats)?;
        } else {
            self.lww_write(&op.target, &field, &op.value, op, op.base.as_deref(), stats)?;
        }
        stats.applied += 1;
        Ok(false)
    }

    /// A text-edit op (DATA-MODEL §2.3, §5.4). With a merger and a sound
    /// loro payload, the field becomes the merged text of every update seen
    /// so far and concurrent edits raise no conflict row. Otherwise this is
    /// the spec's degradation path: LWW on `resolved`, conflicts included.
    fn apply_text_edit(&self, op: &Op, field: &str, stats: &mut ApplyStats) -> Result<(), String> {
        let resolved = match op.value.get("resolved") {
            Some(v) => v.clone(),
            None => op.value.clone(),
        };
        let update = op.value.get_str("loro").map(wugs_model::base64::decode);
        if let (Some(m), Some(Ok(bytes))) = (self.merger.as_deref(), update) {
            if m.resolve(std::slice::from_ref(&bytes)).is_err() {
                // the bytes themselves are corrupt: degrade, keep nothing
                return self.lww_write(&op.target, field, &resolved, op, op.base.as_deref(), stats);
            }
            self.store_update(op, field, &bytes, &resolved)?;
            match m.resolve(&self.text_updates(&op.target, field)?) {
                Ok(text) => {
                    self.mark_unmerged(&op.target, field, false)?;
                    self.merge_write(&op.target, field, &text, op, stats)?;
                    return Ok(());
                }
                Err(_) => {
                    self.mark_unmerged(&op.target, field, true)?;
                    self.lww_write(&op.target, field, &resolved, op, op.base.as_deref(), stats)?;
                    // Earlier arrivals may have left merged text from a
                    // subset of the updates that did merge. Now that the set
                    // does not, the value is the winner's own, as in every
                    // other arrival order.
                    return self.unmerged_value(&op.target, field);
                    // The update is sound alone but the set will not merge
                    // (e.g. colliding op ids after a crash). Keep it: every
                    // replica that holds the same set degrades the same way,
                    // so they stay convergent; deleting it would fork the
                    // lineage silently.
                }
            }
        }
        self.lww_write(&op.target, field, &resolved, op, op.base.as_deref(), stats)
    }

    fn mark_unmerged(&self, target: &str, field: &str, unmerged: bool) -> Result<(), String> {
        let sql = if unmerged {
            "INSERT OR IGNORE INTO text_unmerged(target,field) VALUES(?1,?2)"
        } else {
            "DELETE FROM text_unmerged WHERE target=?1 AND field=?2"
        };
        let mut s = self.db.prepare(sql)?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        s.step()?;
        Ok(())
    }

    /// The value of a field whose updates do not merge: the HLC winner's own.
    /// A winning text-edit's `resolved` is read from its stored update row;
    /// a set or a degraded text-edit already wrote its own value by LWW.
    fn unmerged_value(&self, target: &str, field: &str) -> Result<(), String> {
        let mut s = self.db.prepare(
            "SELECT u.resolved, e.kind FROM fields f JOIN text_updates u ON u.target=f.target AND u.field=f.field AND u.op_id=f.op_id
               JOIN entities e ON e.id=f.target WHERE f.target=?1 AND f.field=?2",
        )?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        if !s.step()? {
            return Ok(());
        }
        let (resolved, kind) = (s.col_text(0), s.col_text(1));
        drop(s);
        let value = json::parse(&resolved).map_err(|e| format!("stored resolved text: {e:?}"))?;
        let mut s = self.db.prepare("UPDATE fields SET value=?1 WHERE target=?2 AND field=?3")?;
        s.bind_text(1, &value.to_json())?;
        s.bind_text(2, target)?;
        s.bind_text(3, field)?;
        s.step()?;
        drop(s);
        self.fts_write(target, &kind, field, &value)
    }

    /// Did this op's loro update go into the field's merged text?
    fn merged_op(&self, target: &str, field: &str, op_id: &str) -> Result<bool, String> {
        let mut s = self.db.prepare("SELECT 1 FROM text_updates WHERE target=?1 AND field=?2 AND op_id=?3")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        s.bind_text(3, op_id)?;
        s.step()
    }

    fn store_update(&self, op: &Op, field: &str, bytes: &[u8], resolved: &Value) -> Result<(), String> {
        let mut s = self.db.prepare(
            "INSERT OR IGNORE INTO text_updates(target,field,hlc,op_id,update_b64,resolved) VALUES(?1,?2,?3,?4,?5,?6)",
        )?;
        s.bind_text(1, &op.target)?;
        s.bind_text(2, field)?;
        s.bind_text(3, &op.hlc)?;
        s.bind_text(4, &op.id)?;
        s.bind_text(5, &wugs_model::base64::encode(bytes))?;
        s.bind_text(6, &resolved.to_json())?;
        s.step()?;
        Ok(())
    }

    /// Every stored update of a field, in HLC order.
    pub fn text_updates(&self, target: &str, field: &str) -> Result<Vec<Vec<u8>>, String> {
        let mut s = self.db.prepare(
            "SELECT update_b64 FROM text_updates WHERE target=?1 AND field=?2 ORDER BY hlc, op_id",
        )?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        let mut out = Vec::new();
        while s.step()? {
            out.push(wugs_model::base64::decode(&s.col_text(0))?);
        }
        Ok(out)
    }

    /// Write a merged text value. Deterministic whatever the arrival order
    /// (replay in HLC order gives the same state): the newest op on the
    /// field decides the kind. If it is a merged text-edit, the merged text
    /// is the value; if it is a plain set or a degraded text-edit (no loro
    /// update in the merge), its own value stands and the merge only updates
    /// the stored lineage.
    fn merge_write(&self, target: &str, field: &str, text: &str, op: &Op, stats: &mut ApplyStats) -> Result<(), String> {
        let cur = self.field_meta(target, field)?;
        let winner_merged = match &cur {
            Some(c) if op.hlc <= c.hlc => c.verb == "text-edit" && self.merged_op(target, field, &c.op_id)?,
            _ => false,
        };
        match &cur {
            Some(c) if op.hlc <= c.hlc && !winner_merged => {}
            Some(c) if op.hlc <= c.hlc => {
                let mut s = self.db.prepare("UPDATE fields SET value=?1 WHERE target=?2 AND field=?3")?;
                s.bind_text(1, &Value::str(text).to_json())?;
                s.bind_text(2, target)?;
                s.bind_text(3, field)?;
                s.step()?;
                drop(s);
                self.fts_write(target, &op.entity, field, &Value::str(text))?;
            }
            _ => self.upsert_field(target, field, &Value::str(text), op)?,
        }
        self.track_heads(target, field, op, cur.as_ref(), stats)
    }

    /// Returns true if the op was parked pending its target's create.
    fn apply_tombstone(&self, op: &Op, flag: i64, stats: &mut ApplyStats) -> Result<bool, String> {
        if !self.entity_known(&op.target)? {
            self.park_pending(op)?;
            stats.parked_pending += 1;
            return Ok(true);
        }
        let mut s = self
            .db
            .prepare("UPDATE entities SET tombstone=?1 WHERE id=?2")?;
        s.bind_i64(1, flag)?;
        s.bind_text(2, &op.target)?;
        s.step()?;
        stats.applied += 1;
        Ok(false)
    }

    /// Per-field LWW by HLC (DATA-MODEL §5); conflicts come from the heads.
    fn lww_write(
        &self,
        target: &str,
        field: &str,
        value: &Value,
        op: &Op,
        base: Option<&str>,
        stats: &mut ApplyStats,
    ) -> Result<(), String> {
        if matches!((op.entity.as_str(), field), ("segment", "tokens") | ("token", "morphs")) {
            let mut s = self.db.prepare("INSERT OR IGNORE INTO list_writes(target,field,hlc,op_id,base,value) VALUES(?1,?2,?3,?4,?5,?6)")?;
            s.bind_text(1, target)?;
            s.bind_text(2, field)?;
            s.bind_text(3, &op.hlc)?;
            s.bind_text(4, &op.id)?;
            match base {
                Some(b) => s.bind_text(5, b)?,
                None => s.bind_null(5)?,
            }
            s.bind_text(6, &value.to_json())?;
            s.step()?;
        }
        let current = self.field_meta(target, field)?;
        if current.as_ref().is_some_and(|c| op.hlc <= c.hlc) {
            stats.lww_losses += 1;
        } else {
            self.upsert_field(target, field, value, op)?;
        }
        self.track_heads(target, field, op, current.as_ref(), stats)
    }

    /// Keep `field_refs` and `field_heads` in step with one write, given the
    /// field's winner before it. The write's refs close the versions they
    /// name. The newer of the write and the old winner becomes the winner;
    /// the other stays open unless some write already named it. Then the
    /// field's conflict rows are derived again if its heads may have moved.
    fn track_heads(&self, target: &str, field: &str, op: &Op, prev: Option<&FieldMeta>, stats: &mut ApplyStats) -> Result<(), String> {
        let refs = op.refs();
        let mut moved = false;
        if let Some(actor) = legacy_actor(op) {
            let mut s = self.db.prepare(
                "INSERT INTO legacy_writes(target,field,actor,hlc) VALUES(?1,?2,?3,?4)
                 ON CONFLICT(target,field,actor) DO UPDATE SET hlc=MAX(hlc, ?4)",
            )?;
            s.bind_text(1, target)?;
            s.bind_text(2, field)?;
            s.bind_text(3, actor)?;
            s.bind_text(4, &op.hlc)?;
            s.step()?;
            // If this closes an older head of the same actor, the field has
            // side heads, so the rows are derived again below.
        }
        for r in &refs {
            let mut s = self.db.prepare("INSERT OR IGNORE INTO field_refs(target,field,hlc) VALUES(?1,?2,?3)")?;
            s.bind_text(1, target)?;
            s.bind_text(2, field)?;
            s.bind_text(3, r)?;
            s.step()?;
            let mut s = self.db.prepare("DELETE FROM field_heads WHERE target=?1 AND field=?2 AND hlc=?3 RETURNING 1")?;
            s.bind_text(1, target)?;
            s.bind_text(2, field)?;
            s.bind_text(3, r)?;
            moved |= s.step()?;
        }
        // A field's first write has nothing to stay open beside it.
        let Some(prev) = prev else { return Ok(()) };
        let (hlc, op_id, legacy) = if op.hlc > prev.hlc {
            (&prev.hlc, &prev.op_id, prev.legacy_actor.as_deref())
        } else {
            (&op.hlc, &op.id, legacy_actor(op))
        };
        if !refs.contains(&hlc.as_str()) && !self.referenced(target, field, hlc)? {
            // The side head's own value: never the merged text it may show,
            // which depends on which other updates had arrived.
            let own = if op.hlc > prev.hlc {
                match (prev.verb.as_str(), self.stored_resolved(target, field, &prev.op_id)?) {
                    ("text-edit", Some(r)) => r,
                    _ => prev.value.clone(),
                }
            } else {
                own_value(op, field).to_json()
            };
            let mut s = self.db.prepare("INSERT OR IGNORE INTO field_heads(target,field,hlc,op_id,legacy_actor,value) VALUES(?1,?2,?3,?4,?5,?6)")?;
            s.bind_text(1, target)?;
            s.bind_text(2, field)?;
            s.bind_text(3, hlc)?;
            s.bind_text(4, op_id)?;
            match legacy {
                Some(a) => s.bind_text(5, a)?,
                None => s.bind_null(5)?,
            }
            s.bind_text(6, &own)?;
            s.step()?;
            moved = true;
        }
        if moved || self.has_side_heads(target, field)? {
            self.derive_conflicts(target, field, stats)?;
        }
        Ok(())
    }

    /// A merged text-edit's own `resolved` text, as JSON.
    fn stored_resolved(&self, target: &str, field: &str, op_id: &str) -> Result<Option<String>, String> {
        let mut s = self.db.prepare("SELECT resolved FROM text_updates WHERE target=?1 AND field=?2 AND op_id=?3")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        s.bind_text(3, op_id)?;
        Ok(if s.step()? { Some(s.col_text(0)) } else { None })
    }

    fn has_side_heads(&self, target: &str, field: &str) -> Result<bool, String> {
        let mut s = self.db.prepare("SELECT 1 FROM field_heads WHERE target=?1 AND field=?2 LIMIT 1")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        s.step()
    }

    /// Rewrite a field's conflict rows from its heads: one per open head
    /// that is not the winner, except where both are text-edits whose loro
    /// updates merged (§5.4: both have a stored update and the field's set
    /// of updates merges), or the head is a schema-1 write and its actor
    /// wrote the field again later under schema 1 (§5.3), or the two writes
    /// hold the same value (§5.3: nothing to choose between; this is what
    /// two devices loading one list with derived ids produce).
    /// `stats.conflicts` counts rows that are new.
    fn derive_conflicts(&self, target: &str, field: &str, stats: &mut ApplyStats) -> Result<(), String> {
        let mut s = self.db.prepare("DELETE FROM conflicts WHERE target=?1 AND field=?2 RETURNING loser_op")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        let mut before = Vec::new();
        while s.step()? {
            before.push(s.col_text(0));
        }
        drop(s);
        let mut s = self.db.prepare(
            "INSERT INTO conflicts(target,field,winner_op,loser_op)
             SELECT f.target, f.field, f.op_id, h.op_id FROM fields f
               JOIN field_heads h ON h.target=f.target AND h.field=f.field
              WHERE f.target=?1 AND f.field=?2
                AND NOT (h.legacy_actor IS NOT NULL AND EXISTS (SELECT 1 FROM legacy_writes w
                         WHERE w.target=h.target AND w.field=h.field AND w.actor=h.legacy_actor AND w.hlc > h.hlc))
                AND NOT (NOT EXISTS (SELECT 1 FROM text_unmerged x WHERE x.target=f.target AND x.field=f.field)
                     AND EXISTS (SELECT 1 FROM text_updates u WHERE u.target=f.target AND u.field=f.field AND u.op_id=f.op_id)
                     AND EXISTS (SELECT 1 FROM text_updates u WHERE u.target=f.target AND u.field=f.field AND u.op_id=h.op_id))
                AND h.value IS NOT COALESCE(
                     (SELECT u.resolved FROM text_updates u WHERE f.verb='text-edit' AND u.target=f.target AND u.field=f.field AND u.op_id=f.op_id),
                     f.value)
             RETURNING loser_op",
        )?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        while s.step()? {
            if !before.contains(&s.col_text(0)) {
                stats.conflicts += 1;
            }
        }
        Ok(())
    }

    fn referenced(&self, target: &str, field: &str, hlc: &str) -> Result<bool, String> {
        let mut s = self.db.prepare("SELECT 1 FROM field_refs WHERE target=?1 AND field=?2 AND hlc=?3")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        s.bind_text(3, hlc)?;
        s.step()
    }

    /// The open versions of a field, by hlc: its winner plus every write no
    /// write has named in its heads. What a write of this field supersedes.
    pub fn heads(&self, target: &str, field: &str) -> Result<Vec<String>, String> {
        let mut s = self.db.prepare(
            "SELECT hlc FROM fields WHERE target=?1 AND field=?2
             UNION SELECT hlc FROM field_heads WHERE target=?1 AND field=?2 ORDER BY hlc",
        )?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        let mut out = Vec::new();
        while s.step()? {
            out.push(s.col_text(0));
        }
        Ok(out)
    }

    fn searchable(field: &str) -> bool {
        wugs_model::is_language_field(field)
    }

    /// Keep the FTS index in step with a field write. One row per
    /// (target, field), keyed through fts_map so replacement is by rowid,
    /// never a scan. Non-string values just clear the row.
    fn fts_write(&self, target: &str, kind: &str, field: &str, value: &Value) -> Result<(), String> {
        if !Projection::searchable(field) {
            return Ok(());
        }
        let mut s = self
            .db
            .prepare("INSERT OR IGNORE INTO fts_map(target, field) VALUES(?1, ?2)")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        s.step()?;
        let mut s = self
            .db
            .prepare("SELECT id FROM fts_map WHERE target=?1 AND field=?2")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        if !s.step()? {
            return Err("fts_map row vanished".into());
        }
        let id = s.col_i64(0);
        drop(s);
        let mut s = self.db.prepare("DELETE FROM search_fts WHERE rowid=?1")?;
        s.bind_i64(1, id)?;
        s.step()?;
        if let Some(text) = value.as_str() {
            if !text.is_empty() {
                let mut s = self.db.prepare(
                    "INSERT INTO search_fts(rowid, kind, field, body) VALUES(?1, ?2, ?3, ?4)",
                )?;
                s.bind_i64(1, id)?;
                s.bind_text(2, kind)?;
                s.bind_text(3, field)?;
                s.bind_text(4, text)?;
                s.step()?;
            }
        }
        Ok(())
    }

    fn upsert_field(&self, target: &str, field: &str, value: &Value, op: &Op) -> Result<(), String> {
        let mut s = self.db.prepare(
            "INSERT INTO fields(target,field,value,hlc,op_id,verb,legacy_actor) VALUES(?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(target,field) DO UPDATE SET value=?3, hlc=?4, op_id=?5, verb=?6, legacy_actor=?7",
        )?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        s.bind_text(3, &value.to_json())?;
        s.bind_text(4, &op.hlc)?;
        s.bind_text(5, &op.id)?;
        s.bind_text(6, &op.verb)?;
        match legacy_actor(op) {
            Some(a) => s.bind_text(7, a)?,
            None => s.bind_null(7)?,
        }
        s.step()?;
        drop(s);
        self.lookup_key_write(target, &op.entity, field, value)?;
        self.fts_write(target, &op.entity, field, value)
    }

    /// Keep the case-folded lookup index in step with the one field per kind
    /// that people look entries up by: a token's `form`, a lexeme's `lemma`,
    /// an allomorph's `form`. The projector is the only writer.
    fn lookup_key_write(&self, target: &str, kind: &str, field: &str, value: &Value) -> Result<(), String> {
        let fold: fn(&str) -> String = match (kind, field) {
            ("token", "form") => wugs_model::fold_key,
            ("lexeme", "lemma") | ("form", "form") => wugs_model::lexicon_key,
            _ => return Ok(()),
        };
        let Some(text) = value.as_str() else {
            let mut s = self.db.prepare("DELETE FROM lookup_keys WHERE target=?1")?;
            s.bind_text(1, target)?;
            s.step()?;
            return Ok(());
        };
        let key = fold(&self.nfc(text));
        let mut s = self.db.prepare(
            "INSERT INTO lookup_keys(target,kind,key) VALUES(?1,?2,?3)
             ON CONFLICT(target) DO UPDATE SET key=?3",
        )?;
        s.bind_text(1, target)?;
        s.bind_text(2, kind)?;
        s.bind_text(3, &key)?;
        s.step()?;
        Ok(())
    }

    /// `s` in NFC, or as given when no normalizer is installed.
    pub fn nfc(&self, s: &str) -> String {
        match &self.normalizer {
            Some(n) => n.nfc(s),
            None => s.to_string(),
        }
    }

    /// Live entities of `kind` (`token`, `lexeme` or `form`) whose folded
    /// lookup key equals `key` (already folded by `wugs_model::fold_key` or
    /// `lexicon_key`), ordered by id. One index range scan, so the cost
    /// tracks the number of matches, not the size of the project.
    pub fn by_lookup_key(&self, kind: &str, key: &str) -> Result<Vec<String>, String> {
        let mut s = self.db.prepare(
            "SELECT k.target FROM lookup_keys k JOIN entities e ON e.id=k.target
             WHERE k.kind=?1 AND k.key=?2 AND e.tombstone=0 ORDER BY k.target",
        )?;
        s.bind_text(1, kind)?;
        s.bind_text(2, key)?;
        let mut out = Vec::new();
        while s.step()? {
            out.push(s.col_text(0));
        }
        Ok(out)
    }

    /// The visible tokens whose folded form is one of `keys`, in live segments
    /// of live texts, as (key, text, token, analysed), ordered by key then
    /// token id. Visible = listed by its segment (the `members` view), and
    /// analysed = it has at least one visible morph. One query, whatever the
    /// number of keys or matches.
    pub fn tokens_by_keys(&self, keys: &[String]) -> Result<Vec<(String, String, String, bool)>, String> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let marks: Vec<String> = (1..=keys.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT k.key, json_extract(ft.value, '$'), t.id,
                    EXISTS (SELECT 1 FROM members m WHERE m.parent_ref = json_quote(t.id))
               FROM lookup_keys k
               JOIN entities t ON t.id = k.target AND t.tombstone = 0
               JOIN fields fs ON fs.target = t.id AND fs.field = 'segment'
               JOIN entities sg ON sg.id = json_extract(fs.value, '$') AND sg.tombstone = 0
               JOIN fields ft ON ft.target = sg.id AND ft.field = 'text'
               JOIN entities tx ON tx.id = json_extract(ft.value, '$') AND tx.tombstone = 0
              WHERE k.kind = 'token' AND k.key IN ({})
                AND EXISTS (SELECT 1 FROM members m WHERE m.parent_ref = fs.value AND m.child = t.id)
              ORDER BY k.key, t.id",
            marks.join(",")
        );
        let mut s = self.db.prepare(&sql)?;
        for (i, k) in keys.iter().enumerate() {
            s.bind_text(i + 1, k)?;
        }
        let mut out = Vec::new();
        while s.step()? {
            out.push((s.col_text(0), s.col_text(1), s.col_text(2), s.col_i64(3) != 0));
        }
        Ok(out)
    }

    /// Every field of an entity, by name: what a copy needs to carry over.
    pub fn fields_of(&self, target: &str) -> Result<Vec<(String, Value)>, String> {
        let mut s = self.db.prepare("SELECT field, value FROM fields WHERE target=?1 ORDER BY field")?;
        s.bind_text(1, target)?;
        let mut out = Vec::new();
        while s.step()? {
            out.push((s.col_text(0), json::parse(&s.col_text(1)).map_err(|e| e.to_string())?));
        }
        Ok(out)
    }

    fn park_pending(&self, op: &Op) -> Result<(), String> {
        let mut s = self.db.prepare(
            "INSERT OR IGNORE INTO pending(op_id,missing_target,hlc,scope,op_json) VALUES(?1,?2,?3,?4,?5)",
        )?;
        s.bind_text(1, &op.id)?;
        s.bind_text(2, &op.target)?;
        s.bind_text(3, &op.hlc)?;
        s.bind_text(4, &op.scope)?;
        s.bind_text(5, &op.to_json())?;
        s.step()?;
        Ok(())
    }

    /// Is this op parked waiting for its target's create? Parked ops are
    /// already in the log, so sync must treat them as "had".
    pub fn is_pending(&self, op_id: &str) -> Result<bool, String> {
        let mut s = self.db.prepare("SELECT 1 FROM pending WHERE op_id=?1")?;
        s.bind_text(1, op_id)?;
        s.step()
    }

    /// Parked op ids with hlc per scope, for sync summaries.
    pub fn pending_op_ids(&self, scope: &str) -> Result<Vec<(String, String)>, String> {
        let mut s = self.db.prepare("SELECT op_id, hlc FROM pending WHERE scope=?1")?;
        s.bind_text(1, scope)?;
        let mut out = Vec::new();
        while s.step()? {
            out.push((s.col_text(0), s.col_text(1)));
        }
        Ok(out)
    }

    /// Parked ops of one scope, for serving to peers.
    pub fn pending_ops(&self, scope: &str) -> Result<Vec<Op>, String> {
        let mut s = self.db.prepare("SELECT op_json FROM pending WHERE scope=?1 ORDER BY hlc, op_id")?;
        s.bind_text(1, scope)?;
        let mut out = Vec::new();
        while s.step()? {
            out.push(Op::from_json(&s.col_text(0))?);
        }
        Ok(out)
    }

    /// Held ops of one scope, for serving to peers.
    pub fn held_ops_of_scope(&self, scope: &str) -> Result<Vec<Op>, String> {
        let mut s = self
            .db
            .prepare("SELECT op_json FROM quarantine_ops WHERE scope=?1 ORDER BY hlc, op_id")?;
        s.bind_text(1, scope)?;
        let mut out = Vec::new();
        while s.step()? {
            out.push(Op::from_json(&s.col_text(0))?);
        }
        Ok(out)
    }

    fn drain_pending(&self, created: &str, stats: &mut ApplyStats) -> Result<(), String> {
        let parked: Vec<String> = {
            let mut s = self
                .db
                .prepare("SELECT op_json FROM pending WHERE missing_target=?1")?;
            s.bind_text(1, created)?;
            let mut out = Vec::new();
            while s.step()? {
                out.push(s.col_text(0));
            }
            out
        };
        if parked.is_empty() {
            return Ok(());
        }
        {
            let mut s = self.db.prepare("DELETE FROM pending WHERE missing_target=?1")?;
            s.bind_text(1, created)?;
            s.step()?;
        }
        for line in parked {
            let op = Op::from_json(&line)?;
            // un-mark so apply() doesn't dedupe... pending ops were never marked applied
            self.apply(&op, stats)?;
        }
        Ok(())
    }

    // ---- read helpers (used by validators, wugs-core, tools) ----

    fn field_meta(&self, target: &str, field: &str) -> Result<Option<FieldMeta>, String> {
        let mut s = self
            .db
            .prepare("SELECT hlc, op_id, verb, legacy_actor, value FROM fields WHERE target=?1 AND field=?2")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        if s.step()? {
            Ok(Some(FieldMeta {
                hlc: s.col_text(0),
                op_id: s.col_text(1),
                verb: s.col_text(2),
                legacy_actor: s.col_opt_text(3),
                value: s.col_text(4),
            }))
        } else {
            Ok(None)
        }
    }

    /// Raw JSON value, hlc and writing op of a field.
    pub fn field_with_op(&self, target: &str, field: &str) -> Result<Option<(Value, String, String)>, String> {
        let mut s = self
            .db
            .prepare("SELECT value, hlc, op_id FROM fields WHERE target=?1 AND field=?2")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        if s.step()? {
            let v = json::parse(&s.col_text(0)).map_err(|e| e.to_string())?;
            Ok(Some((v, s.col_text(1), s.col_text(2))))
        } else {
            Ok(None)
        }
    }

    /// The field's value with its winning op id and verb.
    pub fn field_with_verb(&self, target: &str, field: &str) -> Result<Option<(Value, String, String)>, String> {
        let mut s = self.db.prepare("SELECT value, op_id, verb FROM fields WHERE target=?1 AND field=?2")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        if s.step()? {
            Ok(Some((json::parse(&s.col_text(0)).map_err(|e| e.to_string())?, s.col_text(1), s.col_text(2))))
        } else {
            Ok(None)
        }
    }

    /// Raw JSON value + hlc of a field.
    pub fn field(&self, target: &str, field: &str) -> Result<Option<(Value, String)>, String> {
        let mut s = self
            .db
            .prepare("SELECT value, hlc FROM fields WHERE target=?1 AND field=?2")?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        if s.step()? {
            let v = json::parse(&s.col_text(0)).map_err(|e| e.to_string())?;
            Ok(Some((v, s.col_text(1))))
        } else {
            Ok(None)
        }
    }

    pub fn field_str(&self, target: &str, field: &str) -> Result<Option<String>, String> {
        Ok(self
            .field(target, field)?
            .map(|(v, _)| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_json())))
    }

    pub fn field_num(&self, target: &str, field: &str) -> Result<Option<f64>, String> {
        Ok(self.field(target, field)?.and_then(|(v, _)| v.as_f64()))
    }

    pub fn entity_kind(&self, id: &str) -> Result<Option<String>, String> {
        let mut s = self.db.prepare("SELECT kind FROM entities WHERE id=?1")?;
        s.bind_text(1, id)?;
        if s.step()? {
            Ok(Some(s.col_text(0)))
        } else {
            Ok(None)
        }
    }

    pub fn is_tombstoned(&self, id: &str) -> Result<bool, String> {
        let mut s = self
            .db
            .prepare("SELECT tombstone FROM entities WHERE id=?1")?;
        s.bind_text(1, id)?;
        if s.step()? {
            Ok(s.col_i64(0) != 0)
        } else {
            Ok(false)
        }
    }

    /// Live entities of `kind` whose ref-field `field` equals `ref_id`
    /// (e.g. morphs of a token: children_by_ref("token", tok_id, "morph")).
    pub fn children_by_ref(
        &self,
        field: &str,
        ref_id: &str,
        kind: &str,
    ) -> Result<Vec<String>, String> {
        let quoted = Value::str(ref_id).to_json();
        let mut s = self.db.prepare(
            "SELECT f.target FROM fields f JOIN entities e ON e.id=f.target
             WHERE f.field=?1 AND f.value=?2 AND e.kind=?3 AND e.tombstone=0",
        )?;
        s.bind_text(1, field)?;
        s.bind_text(2, &quoted)?;
        s.bind_text(3, kind)?;
        let mut out = Vec::new();
        while s.step()? {
            out.push(s.col_text(0));
        }
        Ok(out)
    }

    /// Store a held txn's ops so sync summaries can count them and a later
    /// ingest can retry them without a network re-offer (DATA-MODEL §6).
    pub fn quarantine_store(&self, op: &Op) -> Result<(), String> {
        let mut s = self.db.prepare(
            "INSERT OR IGNORE INTO quarantine_ops(op_id,txn,hlc,scope,op_json) VALUES(?1,?2,?3,?4,?5)",
        )?;
        s.bind_text(1, &op.id)?;
        s.bind_text(2, &op.txn)?;
        s.bind_text(3, &op.hlc)?;
        s.bind_text(4, &op.scope)?;
        s.bind_text(5, &op.to_json())?;
        s.step()?;
        Ok(())
    }

    /// Held txns touching `scope`, each with its stored ops in HLC order.
    pub fn held_txns(&self, scope: Option<&str>) -> Result<Vec<(String, Vec<Op>)>, String> {
        let mut q = match scope {
            Some(_) => self.db.prepare(
                "SELECT DISTINCT txn FROM quarantine_ops WHERE scope=?1 ORDER BY txn",
            )?,
            None => self.db.prepare("SELECT DISTINCT txn FROM quarantine_ops ORDER BY txn")?,
        };
        if let Some(sc) = scope {
            q.bind_text(1, sc)?;
        }
        let mut txns = Vec::new();
        while q.step()? {
            txns.push(q.col_text(0));
        }
        drop(q);
        let mut out = Vec::new();
        for txn in txns {
            let mut s = self
                .db
                .prepare("SELECT op_json FROM quarantine_ops WHERE txn=?1 ORDER BY hlc, op_id")?;
            s.bind_text(1, &txn)?;
            let mut ops = Vec::new();
            while s.step()? {
                ops.push(Op::from_json(&s.col_text(0))?);
            }
            out.push((txn, ops));
        }
        Ok(out)
    }

    pub fn quarantine_release(&self, txn: &str) -> Result<(), String> {
        for sql in [
            "DELETE FROM quarantine_ops WHERE txn=?1",
            "DELETE FROM quarantine WHERE txn=?1",
        ] {
            let mut s = self.db.prepare(sql)?;
            s.bind_text(1, txn)?;
            s.step()?;
        }
        Ok(())
    }

    /// Held op ids with hlc per scope, for sync summaries.
    pub fn held_op_ids(&self, scope: &str) -> Result<Vec<(String, String)>, String> {
        let mut s = self
            .db
            .prepare("SELECT op_id, hlc FROM quarantine_ops WHERE scope=?1")?;
        s.bind_text(1, scope)?;
        let mut out = Vec::new();
        while s.step()? {
            out.push((s.col_text(0), s.col_text(1)));
        }
        Ok(out)
    }

    /// Live entities of one kind, ordered by id (stable, creation-ordered
    /// for ULIDs).
    pub fn entities_of_kind(&self, kind: &str) -> Result<Vec<String>, String> {
        let mut s = self
            .db
            .prepare("SELECT id FROM entities WHERE kind=?1 AND tombstone=0 ORDER BY id")?;
        s.bind_text(1, kind)?;
        let mut out = Vec::new();
        while s.step()? {
            out.push(s.col_text(0));
        }
        Ok(out)
    }

    /// Full-text search over the language-bearing fields. The query is
    /// user text, never FTS syntax: each whitespace token becomes a quoted
    /// prefix term, so punctuation and operators cannot error or widen the
    /// search. Tombstoned entities are filtered at query time. Diacritics
    /// fold both ways (unicode61 remove_diacritics 2): "kopek" finds
    /// "köpek". `kinds` limits hits to those entity kinds; empty means all.
    pub fn search(&self, query: &str, kinds: &[&str], limit: usize) -> Result<Vec<SearchHit>, String> {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
            .collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let match_expr = terms.join(" ");
        let sql = format!(
            "SELECT m.target, f.kind, f.field,
                    snippet(search_fts, 2, '[', ']', '…', 10),
                    bm25(search_fts)
             FROM search_fts f
             JOIN fts_map m ON m.id = f.rowid
             JOIN entities e ON e.id = m.target
             WHERE search_fts MATCH ?1 AND e.tombstone = 0 {}
             ORDER BY bm25(search_fts), m.target
             LIMIT {}",
            if kinds.is_empty() {
                String::new()
            } else {
                let marks: Vec<String> = (2..2 + kinds.len()).map(|i| format!("?{i}")).collect();
                format!("AND f.kind IN ({})", marks.join(","))
            },
            limit.min(1000)
        );
        let mut s = self.db.prepare(&sql)?;
        s.bind_text(1, &match_expr)?;
        for (i, k) in kinds.iter().enumerate() {
            s.bind_text(i + 2, k)?;
        }
        let mut out = Vec::new();
        while s.step()? {
            out.push(SearchHit {
                target: s.col_text(0),
                kind: s.col_text(1),
                field: s.col_text(2),
                snippet: s.col_text(3),
            });
        }
        Ok(out)
    }

    /// A segment's tokens or a token's morphs, in order: its live children
    /// that its membership list names, in list order, or all of them by `pos`
    /// when it has no list (the `members` view). Ids a winning list leaves out are hidden, not
    /// deleted: they stay in the log and come back if a list names them.
    pub fn members(&self, parent: &str) -> Result<Vec<String>, String> {
        let mut s = self
            .db
            .prepare("SELECT child FROM members WHERE parent_ref=?1 ORDER BY ord, child")?;
        s.bind_text(1, &Value::str(parent).to_json())?;
        let mut out = Vec::new();
        while s.step()? {
            out.push(s.col_text(0));
        }
        Ok(out)
    }

    /// Hidden edits under one segment: on tokens its list leaves out, and on
    /// morphs that the lists of its visible tokens leave out. An edit is a
    /// non-empty `set` or `text-edit` value, or a non-empty `gloss`,
    /// `gloss:*` or `sense` that came with the member's `create` (an
    /// accepted analysis). It is reported unless the device that made it
    /// later removed the member itself: dropping your own work is a choice,
    /// losing it to another device's list is not (GLOSSING-MODEL, "Merge
    /// semantics").
    pub fn hidden_edits(&self, segment: &str) -> Result<Vec<HiddenEdit>, String> {
        let mut out = Vec::new();
        let tokens = self.members(segment)?;
        for (parent, list_field, ref_field, kind) in std::iter::once((segment, "tokens", "segment", "token"))
            .chain(tokens.iter().map(|t| (t.as_str(), "morphs", "token", "morph")))
        {
            if self.field(parent, list_field)?.is_none() {
                continue;
            }
            let shown = if kind == "token" { tokens.clone() } else { self.members(parent)? };
            for member in self.children_by_ref(ref_field, parent, kind)? {
                if shown.contains(&member) {
                    continue;
                }
                let remover = self.remover(parent, list_field, &member)?;
                let mut edited = vec![member.clone()];
                if kind == "token" {
                    edited.extend(self.members(&member)?);
                }
                for target in edited {
                    let mut s = self.db.prepare(
                        "SELECT field, hlc, op_id FROM fields WHERE target=?1 AND value NOT IN ('\"\"', 'null', '[]')
                           AND (verb IN ('set','text-edit') OR field IN ('gloss','sense') OR field LIKE 'gloss:%')
                         ORDER BY field",
                    )?;
                    s.bind_text(1, &target)?;
                    while s.step()? {
                        let hlc = s.col_text(1);
                        let dropped_by_its_maker = remover.as_deref().is_some_and(|r| hlc_device(r) == hlc_device(&hlc) && hlc.as_str() < r);
                        if !dropped_by_its_maker {
                            out.push(HiddenEdit {
                                parent: parent.to_string(),
                                member: member.clone(),
                                target: target.clone(),
                                field: s.col_text(0),
                                op_id: s.col_text(2),
                            });
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    /// The HLC of the list write that took `member` out of `parent`'s list:
    /// walking back from the current list through each write's `base`, the
    /// first write whose base list still named it. None when no list this
    /// replica holds on that chain ever named it (it was only in a list that
    /// lost the race).
    fn remover(&self, parent: &str, field: &str, member: &str) -> Result<Option<String>, String> {
        let Some((_, mut hlc)) = self.field(parent, field)? else { return Ok(None) };
        loop {
            let mut s = self.db.prepare(
                "SELECT b.hlc, EXISTS(SELECT 1 FROM json_each(b.value) j WHERE j.value=?4)
                   FROM list_writes w JOIN list_writes b ON b.target=w.target AND b.field=w.field AND b.hlc=w.base
                  WHERE w.target=?1 AND w.field=?2 AND w.hlc=?3",
            )?;
            s.bind_text(1, parent)?;
            s.bind_text(2, field)?;
            s.bind_text(3, &hlc)?;
            s.bind_text(4, member)?;
            if !s.step()? {
                return Ok(None);
            }
            if s.col_i64(1) == 1 {
                return Ok(Some(hlc));
            }
            hlc = s.col_text(0);
        }
    }

    /// Every open conflict on a live entity, one entry per field: the
    /// winner, with the value the field shows (merged text included), and
    /// each open version paired with it, with its own value (a text-edit's
    /// `resolved`). Ordered by the newest version involved, newest first.
    /// A conflict on a deleted entity stays in the table (it reopens with a
    /// restore) but is not listed: there is nothing left to show it on.
    pub fn open_conflicts(&self) -> Result<Vec<OpenConflict>, String> {
        let mut s = self.db.prepare(
            "SELECT c.target, c.field, e.kind, f.op_id, f.hlc, f.verb, f.value,
                    h.op_id, h.hlc, COALESCE(h.value, 'null')
               FROM conflicts c
               JOIN entities e ON e.id=c.target AND e.tombstone=0
               JOIN fields f ON f.target=c.target AND f.field=c.field
               JOIN field_heads h ON h.target=c.target AND h.field=c.field AND h.op_id=c.loser_op
              ORDER BY c.target, c.field, h.hlc",
        )?;
        let mut out: Vec<OpenConflict> = Vec::new();
        while s.step()? {
            let (target, field) = (s.col_text(0), s.col_text(1));
            let loser = Version { op_id: s.col_text(7), hlc: s.col_text(8), value: json::parse(&s.col_text(9)).map_err(|e| e.to_string())? };
            match out.last_mut() {
                Some(c) if c.target == target && c.field == field => c.others.push(loser),
                _ => out.push(OpenConflict {
                    kind: s.col_text(2),
                    winner: Version { op_id: s.col_text(3), hlc: s.col_text(4), value: json::parse(&s.col_text(6)).map_err(|e| e.to_string())? },
                    text: s.col_text(5) == "text-edit",
                    others: vec![loser],
                    target,
                    field,
                }),
            }
        }
        out.sort_by(|a, b| {
            let newest = |c: &OpenConflict| c.others.iter().map(|v| v.hlc.clone()).chain([c.winner.hlc.clone()]).max();
            newest(b).cmp(&newest(a)).then(a.target.cmp(&b.target)).then(a.field.cmp(&b.field))
        });
        Ok(out)
    }

    /// The scope an applied op was written in.
    pub fn op_scope(&self, op_id: &str) -> Result<Option<String>, String> {
        let mut s = self.db.prepare("SELECT scope FROM ops_applied WHERE op_id=?1")?;
        s.bind_text(1, op_id)?;
        Ok(if s.step()? { Some(s.col_text(0)) } else { None })
    }

    /// How many fields `open_conflicts` lists: the badge count.
    pub fn open_conflict_count(&self) -> Result<i64, String> {
        let mut s = self.db.prepare(
            "SELECT COUNT(DISTINCT c.target || char(0) || c.field) FROM conflicts c
               JOIN entities e ON e.id=c.target AND e.tombstone=0",
        )?;
        s.step()?;
        Ok(s.col_i64(0))
    }

    /// The membership list the field's winner conflicts with: the newest
    /// open losing list. None once a write names it in its heads. Only
    /// `Segment.tokens` and `Token.morphs` keep their write history.
    pub fn conflict_loser(&self, target: &str, field: &str) -> Result<Option<Value>, String> {
        let mut s = self.db.prepare(
            "SELECT w.value FROM conflicts c
               JOIN list_writes w ON w.target=c.target AND w.field=c.field AND w.op_id=c.loser_op
              WHERE c.target=?1 AND c.field=?2
              ORDER BY w.hlc DESC LIMIT 1",
        )?;
        s.bind_text(1, target)?;
        s.bind_text(2, field)?;
        if s.step()? {
            Ok(Some(json::parse(&s.col_text(0)).map_err(|e| e.to_string())?))
        } else {
            Ok(None)
        }
    }

    /// `children_by_ref`, ordered by their `pos` field then id.
    pub fn ordered_children(&self, field: &str, parent: &str, kind: &str) -> Result<Vec<String>, String> {
        let mut v: Vec<(String, String)> = Vec::new();
        for id in self.children_by_ref(field, parent, kind)? {
            v.push((self.field_str(&id, "pos")?.unwrap_or_default(), id));
        }
        v.sort();
        Ok(v.into_iter().map(|(_, id)| id).collect())
    }

    pub fn max_hlc(&self) -> Result<Option<String>, String> {
        let mut s = self.db.prepare("SELECT MAX(hlc) FROM ops_applied")?;
        if s.step()? {
            Ok(s.col_opt_text(0))
        } else {
            Ok(None)
        }
    }

    pub fn count(&self, table: &str) -> Result<i64, String> {
        // internal use only; table names are compile-time constants at call sites
        let mut s = self.db.prepare(&format!("SELECT COUNT(*) FROM {table}"))?;
        s.step()?;
        Ok(s.col_i64(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wugs_model::{new_id, Clock};

    #[allow(clippy::too_many_arguments)] // one positional arg per op envelope field
    fn op(
        clock: &mut Clock,
        actor: &str,
        verb: &str,
        entity: &str,
        target: &str,
        field: Option<&str>,
        value: Value,
        base: Option<&str>,
    ) -> Op {
        Op {
            id: new_id("op"),
            hlc: clock.now().encode(),
            txn: new_id("txn"),
            intent: None,
            actor: actor.into(),
            device: clock.device.clone(),
            session: None,
            scope: "lexicon".into(),
            schema: 1,
            verb: verb.into(),
            entity: entity.into(),
            target: target.into(),
            field: field.map(str::to_string),
            value,
            base: base.map(str::to_string),
            heads: None,
        }
    }

    #[test]
    fn create_set_lww_and_idempotence() {
        let p = Projection::open_memory().unwrap();
        let mut stats = ApplyStats::default();
        let mut clock = Clock::new("dvc_A".into());
        let lex = new_id("lex");
        let c = op(&mut clock, "prs_A", "create", "lexeme", &lex, None,
            Value::obj(vec![("lemma", Value::str("wug"))]), None);
        p.apply(&c, &mut stats).unwrap();
        assert_eq!(p.field_str(&lex, "lemma").unwrap().unwrap(), "wug");

        let (_, hlc1) = p.field(&lex, "lemma").unwrap().unwrap();
        let s1 = op(&mut clock, "prs_A", "set", "lexeme", &lex, Some("lemma"),
            Value::str("wugs"), Some(&hlc1));
        p.apply(&s1, &mut stats).unwrap();
        assert_eq!(p.field_str(&lex, "lemma").unwrap().unwrap(), "wugs");
        assert_eq!(stats.conflicts, 0);

        // replaying the same op is a no-op
        p.apply(&s1, &mut stats).unwrap();
        assert_eq!(stats.skipped_dupes, 1);
    }

    #[test]
    fn concurrent_edits_conflict_and_lww() {
        let p = Projection::open_memory().unwrap();
        let mut stats = ApplyStats::default();
        let mut ca = Clock::new("dvc_A".into());
        let mut cb = Clock::new("dvc_B".into());
        let mor = new_id("mor");
        let c = op(&mut ca, "prs_A", "create", "morph", &mor, None,
            Value::obj(vec![("gloss", Value::str(""))]), None);
        p.apply(&c, &mut stats).unwrap();
        let (_, base) = p.field(&mor, "gloss").unwrap().unwrap();

        // both researchers edit from the same base, concurrently
        let a = op(&mut ca, "prs_A", "set", "morph", &mor, Some("gloss"),
            Value::str("PL"), Some(&base));
        let b = op(&mut cb, "prs_B", "set", "morph", &mor, Some("gloss"),
            Value::str("PLU"), Some(&base));
        p.apply(&a, &mut stats).unwrap();
        assert_eq!(stats.conflicts, 0); // a's base matches: causal, no conflict
        p.apply(&b, &mut stats).unwrap();
        assert_eq!(stats.conflicts, 1, "b did not see a => concurrent");
        // LWW: whichever hlc is larger holds the field; conflict row exists either way
        assert_eq!(p.count("conflicts").unwrap(), 1);
    }

    #[test]
    fn pending_dep_parks_and_drains() {
        let p = Projection::open_memory().unwrap();
        let mut stats = ApplyStats::default();
        let mut clock = Clock::new("dvc_A".into());
        let tok = new_id("tok");
        // set arrives before its create (other device's log not here yet)
        let s = op(&mut clock, "prs_A", "set", "token", &tok, Some("form"),
            Value::str("köpekler"), None);
        p.apply(&s, &mut stats).unwrap();
        assert_eq!(stats.parked_pending, 1);
        assert!(p.field(&tok, "form").unwrap().is_none());

        let c = op(&mut clock, "prs_A", "create", "token", &tok, None,
            Value::obj(vec![]), None);
        p.apply(&c, &mut stats).unwrap();
        assert_eq!(p.field_str(&tok, "form").unwrap().unwrap(), "köpekler");
        assert_eq!(p.count("pending").unwrap(), 0);
    }

    #[test]
    fn tombstone_delete_restore() {
        let p = Projection::open_memory().unwrap();
        let mut stats = ApplyStats::default();
        let mut clock = Clock::new("dvc_A".into());
        let sen = new_id("sen");
        let c = op(&mut clock, "prs_A", "create", "sense", &sen, None,
            Value::obj(vec![]), None);
        p.apply(&c, &mut stats).unwrap();
        let d = op(&mut clock, "prs_A", "delete", "sense", &sen, None, Value::Null, None);
        p.apply(&d, &mut stats).unwrap();
        assert!(p.is_tombstoned(&sen).unwrap());
        let r = op(&mut clock, "prs_A", "restore", "sense", &sen, None, Value::Null, None);
        p.apply(&r, &mut stats).unwrap();
        assert!(!p.is_tombstoned(&sen).unwrap());
    }

    #[test]
    fn text_edit_projects_resolved() {
        let p = Projection::open_memory().unwrap();
        let mut stats = ApplyStats::default();
        let mut clock = Clock::new("dvc_A".into());
        let seg = new_id("seg");
        let c = op(&mut clock, "prs_A", "create", "segment", &seg, None,
            Value::obj(vec![]), None);
        p.apply(&c, &mut stats).unwrap();
        let t = op(&mut clock, "prs_A", "text-edit", "segment", &seg, Some("content"),
            Value::obj(vec![("resolved", Value::str("köpekler geldi"))]), None);
        p.apply(&t, &mut stats).unwrap();
        assert_eq!(
            p.field_str(&seg, "content").unwrap().unwrap(),
            "köpekler geldi"
        );
    }

    #[test]
    fn lookup_keys_fold_case_follow_edits_and_hide_tombstones() {
        let p = Projection::open_memory().unwrap();
        let mut stats = ApplyStats::default();
        let mut clock = Clock::new("dvc_A".into());
        let mk = |clock: &mut Clock, entity: &str, id: &str, field: &str, v: &str| {
            op(clock, "prs_A", "create", entity, id, None, Value::obj(vec![(field, Value::str(v))]), None)
        };
        let (t1, t2, lex) = (new_id("tok"), new_id("tok"), new_id("lex"));
        for o in [
            mk(&mut clock, "token", &t1, "form", "Köpek"),
            mk(&mut clock, "token", &t2, "form", "köpek"),
            mk(&mut clock, "lexeme", &lex, "lemma", "-ler"),
        ] {
            p.apply(&o, &mut stats).unwrap();
        }
        assert_eq!(p.by_lookup_key("token", "köpek").unwrap().len(), 2, "case folds");
        assert!(p.by_lookup_key("lexeme", "köpek").unwrap().is_empty(), "kinds do not mix");
        assert_eq!(p.by_lookup_key("lexeme", "ler").unwrap(), vec![lex], "an affix lemma matches without its marker");

        let (_, hlc) = p.field(&t1, "form").unwrap().unwrap();
        let edit = op(&mut clock, "prs_A", "set", "token", &t1, Some("form"), Value::str("Kedi"), Some(&hlc));
        p.apply(&edit, &mut stats).unwrap();
        assert_eq!(p.by_lookup_key("token", "köpek").unwrap(), vec![t2.clone()], "an edit moves the key");
        assert_eq!(p.by_lookup_key("token", "kedi").unwrap(), vec![t1.clone()]);

        let del = op(&mut clock, "prs_A", "delete", "token", &t2, None, Value::Null, None);
        p.apply(&del, &mut stats).unwrap();
        assert!(p.by_lookup_key("token", "köpek").unwrap().is_empty(), "tombstoned tokens are not offered");
        let fields = p.fields_of(&t1).unwrap();
        assert_eq!(fields, vec![("form".to_string(), Value::str("Kedi"))]);

        let (_, hlc) = p.field(&t1, "form").unwrap().unwrap();
        p.apply(&op(&mut clock, "prs_A", "set", "token", &t1, Some("form"), Value::Null, Some(&hlc)), &mut stats).unwrap();
        assert!(p.by_lookup_key("token", "kedi").unwrap().is_empty(), "a non-string form clears the key");
    }

    #[test]
    fn members_follow_the_list_and_fall_back_to_pos_without_one() {
        let p = Projection::open_memory().unwrap();
        let mut stats = ApplyStats::default();
        let mut c = Clock::new("dvc_A".into());
        let tok = new_id("tok");
        p.apply(&op(&mut c, "prs_A", "create", "token", &tok, None, Value::obj(vec![("form", Value::str("ab"))]), None), &mut stats).unwrap();
        let morph = |c: &mut Clock, pos: &str| {
            let id = new_id("mor");
            let v = Value::obj(vec![("token", Value::str(&tok)), ("pos", Value::str(pos))]);
            (id.clone(), op(c, "prs_A", "create", "morph", &id, None, v, None))
        };
        let (m1, c1) = morph(&mut c, "1");
        let (m2, c2) = morph(&mut c, "0");
        p.apply(&c1, &mut stats).unwrap();
        p.apply(&c2, &mut stats).unwrap();
        assert_eq!(p.members(&tok).unwrap(), [m2.clone(), m1.clone()], "no list: live children by pos");

        let (m3, c3) = morph(&mut c, "2");
        let list = Value::Arr(vec![Value::str(&m3), Value::str(&m1)]);
        p.apply(&op(&mut c, "prs_A", "set", "token", &tok, Some("morphs"), list, None), &mut stats).unwrap();
        assert_eq!(p.members(&tok).unwrap(), [m1.clone()], "a listed id that does not exist yet is skipped");
        p.apply(&c3, &mut stats).unwrap();
        assert_eq!(p.members(&tok).unwrap(), [m3.clone(), m1.clone()], "list order, m2 hidden");
        assert!(!p.is_tombstoned(&m2).unwrap(), "hidden is not deleted");

        p.apply(&op(&mut c, "prs_A", "delete", "morph", &m1, None, Value::Null, None), &mut stats).unwrap();
        assert_eq!(p.members(&tok).unwrap(), [m3.clone()], "a tombstoned member drops out");
        p.apply(&op(&mut c, "prs_A", "set", "token", &tok, Some("morphs"), Value::str("junk"), None), &mut stats).unwrap();
        assert_eq!(p.members(&tok).unwrap(), [m2.clone(), m3.clone()], "a list field that is not an array falls back to pos");

        let mut plan = p.db.prepare("EXPLAIN QUERY PLAN SELECT child FROM members WHERE parent_ref=?1 ORDER BY ord, child").unwrap();
        let mut steps = Vec::new();
        while plan.step().unwrap() {
            steps.push(plan.col_text(3));
        }
        assert!(!steps.iter().any(|d| d.starts_with("SCAN f") || d.starts_with("SCAN r") || d.contains("entities_kind")), "both arms seek by index: {steps:?}");
    }

    #[allow(clippy::too_many_arguments)] // one positional arg per op envelope field
    fn at(millis: u64, device: &str, verb: &str, entity: &str, target: &str, field: Option<&str>, value: Value, base: Option<u64>) -> Op {
        let hlc = |m: u64, d: &str| format!("{m:012x}-0000-{d}");
        Op {
            id: format!("op_{millis}"),
            hlc: hlc(millis, device),
            txn: format!("txn_{millis}"),
            intent: None,
            actor: "prs_Ana".into(),
            device: device.into(),
            session: None,
            scope: "txt_1".into(),
            schema: 1,
            verb: verb.into(),
            entity: entity.into(),
            target: target.into(),
            field: field.map(str::to_string),
            value,
            base: base.map(|m| hlc(m, "dvc_A")),
            heads: None,
        }
    }

    #[test]
    fn heads_are_the_versions_no_write_names() {
        let p = Projection::open_memory().unwrap();
        let mut st = ApplyStats::default();
        let hlc = |m: u64, d: &str| format!("{m:012x}-0000-{d}");
        let mut merge = at(5, "dvc_A", "set", "morph", "m", Some("gloss"), Value::str("PL"), Some(4));
        merge.set_heads(vec![hlc(4, "dvc_A"), hlc(3, "dvc_B")]);
        let ops = [
            at(1, "dvc_A", "create", "morph", "m", None, Value::obj(vec![("gloss", Value::str(""))]), None),
            at(4, "dvc_A", "set", "morph", "m", Some("gloss"), Value::str("PL"), Some(1)),
            at(3, "dvc_B", "set", "morph", "m", Some("gloss"), Value::str("PLU"), Some(1)),
            merge,
        ];
        for op in &ops[..3] {
            p.apply(op, &mut st).unwrap();
        }
        assert_eq!(p.heads("m", "gloss").unwrap(), [hlc(3, "dvc_B"), hlc(4, "dvc_A")]);
        p.apply(&ops[3], &mut st).unwrap();
        assert_eq!(p.heads("m", "gloss").unwrap(), [hlc(5, "dvc_A")]);
    }

    #[test]
    fn ops_written_before_heads_supersede_their_base() {
        // Log lines from before `heads` existed: B's set saw only the
        // create, A's last set saw B's.
        let lines = [
            r#"{"id":"op_1","hlc":"000000000001-0000-dvc_A","txn":"txn_1","actor":"prs_A","device":"dvc_A","scope":"s","schema":1,"op":"create","entity":"morph","target":"m","value":{"gloss":""}}"#,
            r#"{"id":"op_2","hlc":"000000000002-0000-dvc_A","txn":"txn_2","actor":"prs_A","device":"dvc_A","scope":"s","schema":1,"op":"set","entity":"morph","target":"m","field":"gloss","value":"PL","base":"000000000001-0000-dvc_A"}"#,
            r#"{"id":"op_3","hlc":"000000000003-0000-dvc_B","txn":"txn_3","actor":"prs_B","device":"dvc_B","scope":"s","schema":1,"op":"set","entity":"morph","target":"m","field":"gloss","value":"PLU","base":"000000000001-0000-dvc_A"}"#,
            r#"{"id":"op_4","hlc":"000000000004-0000-dvc_A","txn":"txn_4","actor":"prs_A","device":"dvc_A","scope":"s","schema":1,"op":"set","entity":"morph","target":"m","field":"gloss","value":"PLURAL","base":"000000000003-0000-dvc_B"}"#,
        ];
        let p = Projection::open_memory().unwrap();
        let mut st = ApplyStats::default();
        for (i, line) in lines.iter().enumerate() {
            let op = Op::from_json(line).unwrap();
            assert_eq!(op.heads, None);
            p.apply(&op, &mut st).unwrap();
            if i == 2 {
                assert_eq!(p.count("conflicts").unwrap(), 1, "PL and PLU both build on the create");
            }
        }
        assert_eq!(p.heads("m", "gloss").unwrap(), ["000000000002-0000-dvc_A", "000000000004-0000-dvc_A"], "PLURAL closes only its base");
        // PL and PLURAL stay open heads, but both are prs_A's schema-1
        // writes: under the rule they were written with, one actor never
        // conflicts with itself (here PLURAL even came from the same device).
        assert_eq!(p.count("conflicts").unwrap(), 0);
    }

    /// Two concurrent writes of one field, each on its own device, both
    /// naming only the create: (schema, actor of the first, actor of the second).
    fn concurrent_pair(schema: u32, a: &str, b: &str) -> Projection {
        let p = Projection::open_memory().unwrap();
        let mut st = ApplyStats::default();
        let mut ops = [
            at(1, "dvc_A", "create", "morph", "m", None, Value::obj(vec![("gloss", Value::str(""))]), None),
            at(2, "dvc_A", "set", "morph", "m", Some("gloss"), Value::str("PL"), Some(1)),
            at(3, "dvc_B", "set", "morph", "m", Some("gloss"), Value::str("PLU"), Some(1)),
        ];
        for op in &mut ops {
            op.schema = schema;
        }
        ops[1].actor = a.into();
        ops[2].actor = b.into();
        for op in &ops {
            p.apply(op, &mut st).unwrap();
        }
        p
    }

    #[test]
    fn schema_1_writes_by_one_actor_on_two_devices_raise_no_conflict() {
        assert_eq!(concurrent_pair(1, "prs_Ana", "prs_Ana").count("conflicts").unwrap(), 0);
    }

    #[test]
    fn a_later_schema_2_write_does_not_revive_a_schema_1_same_actor_pair() {
        // Ana's PL (dvc_A) and PLU (dvc_B) predate heads; then a plain
        // schema-2 write overwrites PLU. PL is still Ana's own superseded write.
        let p = concurrent_pair(1, "prs_Ana", "prs_Ana");
        let mut next = at(4, "dvc_B", "set", "morph", "m", Some("gloss"), Value::str("PLURAL"), None);
        next.schema = 2;
        next.base = Some("000000000003-0000-dvc_B".into());
        p.apply(&next, &mut ApplyStats::default()).unwrap();
        assert_eq!(p.count("conflicts").unwrap(), 0);
        assert_eq!(p.heads("m", "gloss").unwrap().len(), 2, "PL stays an open version; it is just not a conflict");
    }

    #[test]
    fn schema_1_writes_by_two_actors_conflict() {
        assert_eq!(concurrent_pair(1, "prs_Ana", "prs_Bo").count("conflicts").unwrap(), 1);
    }

    #[test]
    fn schema_2_writes_by_one_actor_on_two_devices_conflict() {
        assert_eq!(concurrent_pair(2, "prs_Ana", "prs_Ana").count("conflicts").unwrap(), 1);
    }

    /// Takes each update's bytes as its text and joins them sorted: a
    /// function of the set, like Loro.
    struct JoinMerger;
    impl wugs_model::TextMerger for JoinMerger {
        fn resolve(&self, updates: &[Vec<u8>]) -> Result<String, String> {
            let mut t: Vec<String> = updates.iter().map(|u| String::from_utf8_lossy(u).into_owned()).collect();
            t.sort();
            Ok(t.join("+"))
        }
        fn edit(&self, _: &[Vec<u8>], _: Option<(&str, u64)>, _: &str, _: u64) -> Result<(Vec<u8>, String), String> {
            unreachable!()
        }
    }

    #[test]
    fn a_degraded_text_edit_that_wins_is_not_overwritten_by_an_older_merged_one() {
        // C creates; A (t2) is a merged edit, B (t3) is a degraded edit (no
        // loro payload) that saw neither. B wins by HLC in every order.
        let text = |m: u64, d: &str, t: &str, loro: bool| {
            let mut v = vec![("resolved", Value::str(t))];
            if loro {
                v.push(("loro", Value::str(&wugs_model::base64::encode(t.as_bytes()))));
            }
            at(m, d, "text-edit", "segment", "m", Some("content"), Value::obj(v), Some(1))
        };
        let mut c = at(1, "dvc_A", "create", "segment", "m", None, Value::obj(vec![("content", Value::str(""))]), None);
        c.schema = 2;
        let mut a = text(2, "dvc_A", "aaa", true);
        let mut b = text(3, "dvc_B", "bbb", false);
        a.schema = 2;
        b.schema = 2;
        let mut seen = Vec::new();
        for order in [[&c, &a, &b], [&c, &b, &a]] {
            let mut p = Projection::open_memory().unwrap();
            p.merger = Some(Box::new(JoinMerger));
            let mut st = ApplyStats::default();
            for op in order {
                p.apply(op, &mut st).unwrap();
            }
            seen.push((p.field_str("m", "content").unwrap(), p.count("conflicts").unwrap()));
        }
        assert_eq!(seen[0], (Some("bbb".into()), 1), "the degraded winner stands and conflicts with A");
        assert_eq!(seen[0], seen[1]);
    }

    #[test]
    fn a_hidden_gloss_is_charged_to_the_list_that_removed_it_not_to_hlc_order() {
        // A's re-analysis (t6) drops m1 and m2. B glosses m1 at t8 without
        // having seen it, then restores m2 (t9, base t6). B wrote the newest
        // list, and the first list after the gloss in HLC order, but A's
        // list removed m1, so the gloss is reported.
        let p = Projection::open_memory().unwrap();
        let mut st = ApplyStats::default();
        let list = |ids: &[&str]| Value::Arr(ids.iter().map(|i| Value::str(i)).collect());
        let ops = [
            at(1, "dvc_A", "create", "segment", "seg", None, Value::obj(vec![]), None),
            at(2, "dvc_A", "create", "morph", "m1", None, Value::obj(vec![("token", Value::str("tok"))]), None),
            at(3, "dvc_A", "create", "morph", "m2", None, Value::obj(vec![("token", Value::str("tok"))]), None),
            at(4, "dvc_A", "create", "morph", "m3", None, Value::obj(vec![("token", Value::str("tok"))]), None),
            at(5, "dvc_A", "create", "token", "tok", None, Value::obj(vec![("segment", Value::str("seg")), ("morphs", list(&["m1", "m2"]))]), None),
            at(6, "dvc_A", "set", "token", "tok", Some("morphs"), list(&["m3"]), Some(5)),
            at(8, "dvc_B", "set", "morph", "m1", Some("gloss"), Value::str("dog"), None),
            at(9, "dvc_B", "set", "token", "tok", Some("morphs"), list(&["m3", "m2"]), Some(6)),
        ];
        for op in &ops {
            p.apply(op, &mut st).unwrap();
        }
        let got: Vec<(String, String)> = p.hidden_edits("seg").unwrap().into_iter().map(|h| (h.member, h.field)).collect();
        assert_eq!(got, [("m1".to_string(), "gloss".to_string())]);
    }

    #[test]
    fn a_device_following_up_on_its_own_edit_does_not_hide_the_other_persons_value() {
        // Ana sets PL, Bo (never having seen it) sets PLU, then PLURAL on top of
        // his own PLU. Ana's PL is still a write nobody who overwrote it saw.
        // Explicit HLCs: two live clocks can stamp PLU below PL, which made
        // this test pass or fail by timing.
        let p = Projection::open_memory().unwrap();
        let mut stats = ApplyStats::default();
        let hlc = |m: u64, d: &str| format!("{m:012x}-0000-{d}");
        let mut ops = [
            at(1, "dvc_A", "create", "morph", "m", None, Value::obj(vec![("gloss", Value::str(""))]), None),
            at(2, "dvc_A", "set", "morph", "m", Some("gloss"), Value::str("PL"), Some(1)),
            at(3, "dvc_B", "set", "morph", "m", Some("gloss"), Value::str("PLU"), Some(1)),
            at(4, "dvc_B", "set", "morph", "m", Some("gloss"), Value::str("PLURAL"), None),
        ];
        ops[2].actor = "prs_Bo".into();
        ops[3].actor = "prs_Bo".into();
        ops[3].base = Some(hlc(3, "dvc_B"));
        for op in &ops {
            p.apply(op, &mut stats).unwrap();
        }
        assert_eq!(p.count("conflicts").unwrap(), 1, "Ana's PL against Bo's PLU stays on record");
        assert_eq!(p.field_str("m", "gloss").unwrap().as_deref(), Some("PLURAL"));
    }
}
