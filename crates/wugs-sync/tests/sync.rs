use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wugs_core::{Project, SCOPE_LEXICON};
use wugs_model::{new_id, Hlc, Op, Value, SCHEMA_VERSION};
use wugs_sync::{diff_windows, scopes, summary, sync_pair, SyncPolicy};

struct Rig {
    roots: Vec<PathBuf>,
}

impl Rig {
    fn new() -> Rig {
        Rig { roots: Vec::new() }
    }

    fn project(&mut self, actor: &str) -> Project {
        self.project_seeded(actor, &[])
    }

    /// A second device of the same project (the wire protocol refuses to
    /// pair two different project ids).
    fn joined(&mut self, actor: &str, of: &Project) -> Project {
        self.joined_seeded(actor, of, &[])
    }

    fn joined_seeded(&mut self, actor: &str, of: &Project, seed: &[Op]) -> Project {
        let root = std::env::temp_dir().join(format!("wugs-sync-test-{}", wugs_model::ulid()));
        self.roots.push(root.clone());
        if !seed.is_empty() {
            wugs_oplog::LogDir::new(&root).append_all(seed).unwrap();
        }
        Project::open_joined(&root, actor, &of.project_id).unwrap()
    }

    /// Seeds the log before opening so the projection is built by replay, not per-txn ingest.
    fn project_seeded(&mut self, actor: &str, seed: &[Op]) -> Project {
        let root = std::env::temp_dir().join(format!("wugs-sync-test-{}", wugs_model::ulid()));
        self.roots.push(root.clone());
        if !seed.is_empty() {
            wugs_oplog::LogDir::new(&root).append_all(seed).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        Project::open(&root, actor).unwrap()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        for r in &self.roots {
            fs::remove_dir_all(r).ok();
        }
    }
}

fn fields_table(p: &Project) -> Vec<(String, String, String)> {
    let mut s = p
        .proj
        .db
        .prepare("SELECT target, field, value FROM fields ORDER BY target, field")
        .unwrap();
    let mut out = Vec::new();
    while s.step().unwrap() {
        out.push((s.col_text(0), s.col_text(1), s.col_text(2)));
    }
    out
}

fn log_len(p: &Project) -> usize {
    p.log.read_all().unwrap().ops.len()
}

fn copy_log(from: &Project, to: &mut Project) {
    to.ingest(from.log.read_all().unwrap().ops).unwrap();
}

fn all() -> SyncPolicy {
    SyncPolicy::allow_all()
}

#[test]
fn diverged_peers_converge_and_surface_the_concurrent_edit() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "wug").unwrap().0;
    let toks = a.tokenize(&txt, &seg, &["wugs"]).unwrap().0;
    let morphs = a
        .split_morph(&txt, &toks[0], &[("wug", "root"), ("s", "suffix")])
        .unwrap()
        .0;
    copy_log(&a, &mut b);

    a.set_gloss(&txt, &morphs[1], "PL", None).unwrap();
    b.set_gloss(&txt, &morphs[1], "PLU", None).unwrap();
    b.create_lexeme("köpek", "dog").unwrap();
    assert_ne!(fields_table(&a), fields_table(&b), "diverged before sync");

    let r = sync_pair(&mut a, &mut b, &all(), &all()).unwrap();

    assert_eq!(fields_table(&a), fields_table(&b), "fields tables identical after sync");
    assert!(r.a_to_b.ops > 0 && r.b_to_a.ops > 0, "both directions moved ops");
    assert!(a.proj.count("conflicts").unwrap() >= 1, "A recorded the concurrent gloss edit");
    assert!(b.proj.count("conflicts").unwrap() >= 1, "B recorded the concurrent gloss edit");
    let gloss = fields_table(&a)
        .into_iter()
        .find(|(t, f, _)| t == &morphs[1] && f == "gloss")
        .unwrap()
        .2;
    assert!(gloss.contains("PL"), "a single LWW winner holds the field: {gloss}");
}

#[test]
fn second_sync_exchanges_nothing() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    a.create_lexeme("wug", "bird").unwrap();
    b.create_lexeme("gel", "come").unwrap();
    sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    let (la, lb) = (log_len(&a), log_len(&b));

    let r = sync_pair(&mut a, &mut b, &all(), &all()).unwrap();

    assert_eq!((r.a_to_b.ops, r.b_to_a.ops), (0, 0), "no ops exchanged");
    assert_eq!((r.a_to_b.windows, r.b_to_a.windows), (0, 0), "roots equal so no windows");
    assert_eq!((log_len(&a), log_len(&b)), (la, lb), "logs unchanged");
}

#[test]
fn denied_scope_never_reaches_the_peer() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    a.create_lexeme("wug", "bird").unwrap();
    let secret = a.create_text("restricted recording").unwrap();
    a.add_segment(&secret, "tir_utt", 0, None, None, "private").unwrap();

    let r = sync_pair(&mut a, &mut b, &SyncPolicy::deny([secret.clone()]), &all()).unwrap();

    assert!(r.a_to_b.ops > 0, "permitted lexicon scope still synced");
    assert_eq!(r.a_to_b.ops_by_scope.get(&secret), None, "report shows no ops for the denied scope");
    assert!(!scopes(&b.proj).unwrap().contains(&secret), "B has no ops_applied rows for it");
    assert!(b.proj.entity_kind(&secret).unwrap().is_none(), "B has no entity from it");
    assert!(
        b.log.read_all().unwrap().ops.iter().all(|o| o.scope != secret),
        "B's log mirror holds nothing from it"
    );

    let r = sync_pair(&mut a, &mut b, &SyncPolicy::only([SCOPE_LEXICON]), &all()).unwrap();
    assert_eq!(r.a_to_b.ops, 0, "allow-list policy also withholds it on a re-sync");
}

#[test]
fn held_txn_stays_quarantined_and_is_not_reapplied() {
    let mut rig = Rig::new();
    let mut a = rig.project("prs_A");
    let mut b = rig.joined("prs_B", &a);
    let txt = a.create_text("T").unwrap();
    a.add_segment(&txt, "tir_utt", 0, Some(0.0), Some(1000.0), "a").unwrap();
    copy_log(&a, &mut b);

    let good = b.add_segment(&txt, "tir_utt", 1, Some(1000.0), Some(2000.0), "b").unwrap().0;
    let bad = b.add_segment(&txt, "tir_utt", 2, Some(1500.0), Some(2500.0), "c").unwrap().0;

    let r = sync_pair(&mut a, &mut b, &all(), &all()).unwrap();

    assert_eq!(r.b_to_a.ingest.txns_applied, 1, "A applied B's good txn");
    assert_eq!(r.b_to_a.ingest.txns_held, 1, "A held the overlapping txn");
    assert!(a.proj.entity_kind(&good).unwrap().is_some(), "good segment present on A");
    assert!(a.proj.entity_kind(&bad).unwrap().is_none(), "overlapping segment absent on A");
    assert_eq!(a.proj.count("quarantine").unwrap(), 1, "one quarantine row");

    let (ops_applied, log_before) = (a.proj.count("ops_applied").unwrap(), log_len(&a));
    let r = sync_pair(&mut a, &mut b, &all(), &all()).unwrap();

    assert_eq!(r.b_to_a.ingest.txns_applied, 0, "nothing re-applied on re-sync");
    assert_eq!(a.proj.count("quarantine").unwrap(), 1, "still exactly one quarantine row");
    assert_eq!(a.proj.count("ops_applied").unwrap(), ops_applied, "ops_applied unchanged");
    assert_eq!(log_len(&a), log_before, "A's log gained no duplicate lines");
}

fn synthetic_ops(n_txns: usize) -> Vec<Op> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
    let start = now - Duration::from_secs(20 * 3600).as_millis() as u64;
    let device = new_id("dvc");
    let actor = new_id("prs");
    let mut ops = Vec::with_capacity(n_txns * 2);
    for i in 0..n_txns {
        let txn = new_id("txn");
        let lex = new_id("lex");
        for (k, (entity, target, field, value)) in [
            ("lexeme", lex.clone(), None, Value::obj(vec![("lemma", Value::str(&format!("w{i}")))])),
            ("lexeme", lex.clone(), Some("note"), Value::str("n")),
        ]
        .into_iter()
        .enumerate()
        {
            let hlc = Hlc { millis: start + i as u64 * 1500, counter: k as u32, device: device.clone() };
            ops.push(Op {
                id: new_id("op"),
                hlc: hlc.encode(),
                txn: txn.clone(),
                intent: Some("gen".into()),
                actor: actor.clone(),
                device: device.clone(),
                session: None,
                scope: SCOPE_LEXICON.into(),
                schema: SCHEMA_VERSION,
                verb: if field.is_some() { "set" } else { "create" }.into(),
                entity: entity.into(),
                target,
                field: field.map(str::to_string),
                value,
                base: None,
                heads: None,
            });
        }
    }
    ops
}

#[test]
fn diff_over_tens_of_thousands_of_ops_is_fast_and_narrow() {
    let mut rig = Rig::new();
    let ops = synthetic_ops(20_000);
    let mut a = rig.project_seeded("prs_A", &ops);
    let mut b = rig.joined_seeded("prs_B", &a, &ops);
    assert_eq!(a.proj.count("ops_applied").unwrap(), 40_000, "base ops applied on A");
    for i in 0..50 {
        a.create_lexeme(&format!("a{i}"), "x").unwrap();
        b.create_lexeme(&format!("b{i}"), "y").unwrap();
    }

    let t = Instant::now();
    let sa = summary(&a.proj, SCOPE_LEXICON).unwrap();
    let sb = summary(&b.proj, SCOPE_LEXICON).unwrap();
    let diff = diff_windows(&sa, &sb);
    let elapsed = t.elapsed();

    assert!(elapsed < Duration::from_secs(1), "summary+diff took {elapsed:?}");
    assert!(!diff.is_empty() && diff.len() <= 3, "diff is a handful of windows: {diff:?}");
    assert!(sa.windows.len() > 100, "base history spans many windows: {}", sa.windows.len());
    let distinct: BTreeSet<_> = diff.iter().collect();
    assert_eq!(distinct.len(), diff.len(), "no duplicate windows");

    let r = sync_pair(&mut a, &mut b, &all(), &all()).unwrap();
    assert_eq!(r.a_to_b.ops, 100, "only A's 50 new lexemes (2 ops each) crossed, not the 40k base");
    assert_eq!(r.b_to_a.ops, 100, "only B's 50 new lexemes crossed");
}
