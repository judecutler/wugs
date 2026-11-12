//! Prototype gate #1 (PRD-TONGUES §10, DATA-MODEL §12): generate a realistic
//! op log (~lexicon + interlinear texts), then measure cold replay into a
//! fresh SQLite projection, plus incremental apply latency.
//!
//! Run: cargo run -p wugs-bench --release -- [N_OPS] [--keep] [--contested]
//! Gates: cold replay of 500k ops ≤ 60 s; steady memory ≤ 300 MB.
//!
//! Corpus gate (SCROLLS.md §2): `-- [N_TOKENS] --corpus` writes a plain-text
//! corpus the way `import_written_text` does (one txn per document of 500
//! sentences, 20 words each from a Zipf vocabulary of 50k types, segments
//! untokenized), replays it, then builds the corpus index (the first read
//! does) and reads a whole-corpus frequency list from it. Gates at 10M
//! tokens: replay ≤ 60 s, index build ≤ 120 s, frequency list ≤ 15 s,
//! peak memory ≤ 300 MB.
//!
//! The default workload is one device's work with no concurrent writes, the
//! shape the gate was set for. `--contested` adds a second device whose
//! glosses cross the first one's: one gloss in 8 gets a concurrent write (a
//! conflict) and one in 16 is then settled by a write naming both, to
//! measure the cost of open heads and conflict rows.
//!
//! Every write sets `base` and `heads` the way `Txn` does: `base` is the
//! field's current winner (none for a field the create left unset, which is
//! every set below), `heads` are left out when they equal `[base]`.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use wugs_model::{new_id, pos_seq, Clock, Op, Value, SCHEMA_VERSION};
use wugs_oplog::LogDir;
use wugs_projection::{ApplyStats, Projection};

const CHUNK: usize = 25_000;

struct Gen<'a> {
    clock: Clock,
    /// A second device whose glosses cross the first one's (DATA-MODEL §5).
    peer: Clock,
    actor: String,
    ops: Vec<Op>,
    log: &'a LogDir,
    written: usize,
}

impl<'a> Gen<'a> {
    fn total(&self) -> usize {
        self.written + self.ops.len()
    }

    /// Keep memory flat: write out full chunks as we go.
    fn flush_if_full(&mut self) {
        if self.ops.len() >= CHUNK {
            self.log.append_all(&self.ops).unwrap();
            self.written += self.ops.len();
            self.ops.clear();
        }
    }

    fn flush(&mut self) {
        self.log.append_all(&self.ops).unwrap();
        self.written += self.ops.len();
        self.ops.clear();
    }

    fn op(
        &mut self,
        verb: &str,
        entity: &str,
        target: &str,
        scope: &str,
        txn: &str,
        intent: &str,
        field: Option<&str>,
        value: Value,
    ) {
        self.ops.push(Op {
            id: new_id("op"),
            hlc: self.clock.now().encode(),
            txn: txn.to_string(),
            intent: Some(intent.to_string()),
            actor: self.actor.clone(),
            device: self.clock.device.clone(),
            session: None,
            scope: scope.to_string(),
            schema: SCHEMA_VERSION,
            verb: verb.to_string(),
            entity: entity.to_string(),
            target: target.to_string(),
            field: field.map(str::to_string),
            value,
            base: None,
            heads: None,
        });
    }

    fn create(
        &mut self,
        entity: &str,
        prefix: &str,
        scope: &str,
        txn: &str,
        intent: &str,
        fields: Vec<(&str, Value)>,
    ) -> String {
        let id = new_id(prefix);
        self.op("create", entity, &id, scope, txn, intent, None, Value::obj(fields));
        id
    }
}

const LEMMAS: [&str; 8] = ["wug", "köpek", "gel", "ev", "su", "ağaç", "kuş", "taş"];

fn generate(target_ops: usize, log: &LogDir, contested: bool) -> usize {
    let mut g = Gen {
        clock: Clock::new(new_id("dvc")),
        peer: Clock::new(new_id("dvc")),
        actor: new_id("prs"),
        ops: Vec::with_capacity(CHUNK + 64),
        log,
        written: 0,
    };

    // lexicon: ~8% of ops
    let lex_budget = target_ops / 12;
    let mut senses: Vec<String> = Vec::new();
    while g.total() < lex_budget {
        let txn = new_id("txn");
        let i = g.total();
        let lemma = format!("{}{}", LEMMAS[i % LEMMAS.len()], i);
        let lex = g.create(
            "lexeme", "lex", "lexicon", &txn, "create-lexeme",
            vec![("lemma", Value::str(&lemma))],
        );
        let sen = g.create(
            "sense", "sen", "lexicon", &txn, "create-lexeme",
            vec![
                ("lexeme", Value::str(&lex)),
                ("gloss", Value::str("gloss")),
                ("pos", Value::str(&pos_seq(0))),
            ],
        );
        senses.push(sen);
        g.flush_if_full();
    }

    // interlinear texts: the rest
    let mut text_i = 0usize;
    while g.total() < target_ops {
        let txn = new_id("txn");
        let txt = g.create(
            "text", "txt", "", &txn, "create-text",
            vec![("title", Value::str(&format!("Text {text_i}")))],
        );
        // scope is the text's own id
        let scope = txt.clone();
        {
            let last = g.ops.last_mut().unwrap();
            last.scope = scope.clone();
        }
        for s in 0..30 {
            if g.total() >= target_ops {
                break;
            }
            let txn = new_id("txn");
            let start = (s * 2000) as f64;
            let seg = g.create(
                "segment", "seg", &scope, &txn, "add-segment",
                vec![
                    ("text", Value::str(&txt)),
                    ("tier", Value::str("tir_utterance")),
                    ("pos", Value::str(&pos_seq(s))),
                    ("start", Value::Num(start)),
                    ("end", Value::Num(start + 1800.0)),
                    ("content", Value::str("köpekler geldi ve gitti")),
                ],
            );
            let txn = new_id("txn");
            for t in 0..4 {
                let tok = g.create(
                    "token", "tok", &scope, &txn, "tokenize",
                    vec![
                        ("segment", Value::str(&seg)),
                        ("pos", Value::str(&pos_seq(t))),
                        ("form", Value::str("köpekler")),
                    ],
                );
                let txn2 = new_id("txn");
                let m1 = g.create(
                    "morph", "mor", &scope, &txn2, "split-morph",
                    vec![
                        ("token", Value::str(&tok)),
                        ("pos", Value::str(&pos_seq(0))),
                        ("surface", Value::str("köpek")),
                        ("type", Value::str("root")),
                    ],
                );
                let m2 = g.create(
                    "morph", "mor", &scope, &txn2, "split-morph",
                    vec![
                        ("token", Value::str(&tok)),
                        ("pos", Value::str(&pos_seq(1))),
                        ("surface", Value::str("ler")),
                        ("type", Value::str("suffix")),
                    ],
                );
                let sense = &senses[(text_i + t) % senses.len()];
                let txn3 = new_id("txn");
                g.op("set", "morph", &m1, &scope, &txn3, "link-sense",
                    Some("sense"), Value::str(sense));
                g.op("set", "morph", &m2, &scope, &txn3, "set-gloss",
                    Some("gloss"), Value::str("PL"));
                // One gloss in 8 also gets a concurrent one from the peer (a
                // conflict); one in 16 is then settled by a write naming both.
                let n = (text_i * 30 + s) * 4 + t;
                if contested && n.is_multiple_of(8) {
                    let pl = g.ops.last().unwrap().hlc.clone();
                    let seen = g.ops[g.ops.len() - 2].hlc.clone();
                    g.peer.observe(&wugs_model::Hlc::parse(&seen).unwrap()).unwrap();
                    // Concurrent with PL: it also found the gloss unset, so no base.
                    g.op("set", "morph", &m2, &scope, &new_id("txn"), "set-gloss", Some("gloss"), Value::str("PLU"));
                    let plu = g.ops.last_mut().unwrap();
                    plu.hlc = g.peer.now().encode();
                    plu.device = g.peer.device.clone();
                    let plu = plu.hlc.clone();
                    if n.is_multiple_of(16) {
                        g.clock.observe(&wugs_model::Hlc::parse(&plu).unwrap()).unwrap();
                        g.op("set", "morph", &m2, &scope, &new_id("txn"), "set-gloss", Some("gloss"), Value::str("PL"));
                        let settle = g.ops.last_mut().unwrap();
                        settle.base = Some(pl.clone().max(plu.clone()));
                        settle.set_heads(vec![pl, plu]);
                    }
                }
            }
        }
        text_i += 1;
        g.flush_if_full();
    }
    if g.total() > target_ops {
        let excess = g.total() - target_ops;
        g.ops.truncate(g.ops.len().saturating_sub(excess));
    }
    g.flush();
    g.written
}

/// A deterministic xorshift generator: the corpus is the same every run.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

const TYPES: usize = 50_000;
const SENTENCE_WORDS: usize = 20;
const DOC_SENTENCES: usize = 500;

/// Invented words from syllables, so no real text is involved.
fn vocabulary() -> Vec<String> {
    const ONSETS: [&str; 12] = ["k", "t", "p", "m", "n", "s", "l", "ŋ", "w", "j", "ʔ", "r"];
    const VOWELS: [&str; 6] = ["a", "e", "i", "o", "u", "ə"];
    (0..TYPES)
        .map(|i| {
            let mut n = i + 1;
            let mut w = String::new();
            while n > 0 {
                w.push_str(ONSETS[n % ONSETS.len()]);
                n /= ONSETS.len();
                w.push_str(VOWELS[n % VOWELS.len()]);
                n /= VOWELS.len();
            }
            w
        })
        .collect()
}

/// Writes about `tokens` running words as plain-text documents; returns ops written.
fn generate_corpus(tokens: usize, log: &LogDir) -> usize {
    let mut g = Gen {
        clock: Clock::new(new_id("dvc")),
        peer: Clock::new(new_id("dvc")),
        actor: new_id("prs"),
        ops: Vec::with_capacity(CHUNK + 1024),
        log,
        written: 0,
    };
    let vocab = vocabulary();
    // Zipf with exponent 1.07, near what English and most corpora show.
    let mut cum = Vec::with_capacity(TYPES);
    let mut total = 0.0;
    for r in 1..=TYPES {
        total += 1.0 / (r as f64).powf(1.07);
        cum.push(total);
    }
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let tier = new_id("tir");
    let cfg = new_id("txn");
    g.create("tier", "tir", "config", &cfg, "add-tier", vec![("name", Value::str("utterance"))]);
    g.ops.last_mut().unwrap().target = tier.clone();
    let mut words = 0usize;
    let mut doc = 0usize;
    while words < tokens {
        let txn = new_id("txn");
        let txt = g.create("text", "txt", "", &txn, "import-written-text", vec![("title", Value::str(&format!("Document {doc}")))]);
        g.ops.last_mut().unwrap().scope = txt.clone();
        for s in 0..DOC_SENTENCES {
            if words >= tokens {
                break;
            }
            let mut line = String::new();
            for w in 0..SENTENCE_WORDS {
                let x = rng.unit() * total;
                let r = cum.partition_point(|c| *c < x).min(TYPES - 1);
                if w > 0 {
                    line.push(' ');
                }
                line.push_str(&vocab[r]);
            }
            line.push('.');
            words += SENTENCE_WORDS;
            g.create(
                "segment", "seg", &txt, &txn, "import-written-text",
                vec![
                    ("text", Value::str(&txt)),
                    ("tier", Value::str(&tier)),
                    ("pos", Value::str(&pos_seq(s))),
                    ("content", Value::str(&line)),
                ],
            );
        }
        g.flush_if_full();
        doc += 1;
    }
    g.flush();
    g.written
}

/// The corpus gate after replay: build the index, then read a frequency list.
fn corpus_gate(proj: &Projection) -> (f64, f64) {
    let tier = proj.entities_of_kind("tier").unwrap();
    let t = Instant::now();
    let rebuilt = proj.refresh_corpus().unwrap();
    let index = t.elapsed();
    println!("corpus index build: {index:.2?} ({rebuilt} segments)");
    let t = Instant::now();
    let mut counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut running = 0u64;
    proj.each_corpus_segment(&tier, None, |_, _, _, words| {
        for w in words {
            running += 1;
            *counts.entry(wugs_model::fold_key(w)).or_insert(0) += 1;
        }
        Ok(())
    })
    .unwrap();
    let freq = t.elapsed();
    println!("frequency list: {freq:.2?} ({running} running words, {} types)", counts.len());
    (index.as_secs_f64(), freq.as_secs_f64())
}

fn vm_hwm_mb() -> f64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<f64>().ok())
        })
        .map(|kb| kb / 1024.0)
        .unwrap_or(f64::NAN)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args
        .iter()
        .find(|a| a.parse::<usize>().is_ok())
        .and_then(|a| a.parse().ok())
        .unwrap_or(500_000);
    let keep = args.iter().any(|a| a == "--keep");
    let contested = args.iter().any(|a| a == "--contested");
    let corpus = args.iter().any(|a| a == "--corpus");

    let root = std::env::temp_dir().join(format!("wugs-bench-{}", wugs_model::ulid()));
    fs::create_dir_all(&root).unwrap();
    println!("bench root: {} ({} {}{})", root.display(), n, if corpus { "corpus tokens" } else { "ops" }, if contested { ", contested" } else { "" });

    // ---- phase 1: generate + write the log (chunked; memory stays flat)
    let log = LogDir::new(&root);
    let t0 = Instant::now();
    let written = if corpus { generate_corpus(n, &log) } else { generate(n, &log, contested) };
    let log_bytes = dir_size(&root.join("log"));
    println!(
        "generate+write: {:>8.2?}  ({} ops, {:.1} MB on disk)",
        t0.elapsed(),
        written,
        log_bytes as f64 / 1e6
    );

    // ---- phase 2: cold replay — STREAMED k-way merge, O(#files) memory
    let db_path = root.join("projection.sqlite");
    let proj = Projection::open(db_path.to_str().unwrap(), None).unwrap();

    let t_replay = Instant::now();
    let mut stream = log.stream_all().unwrap();
    let mut stats = ApplyStats::default();
    let mut applied_targets: Vec<String> = Vec::new(); // sample for phase 3
    let mut i = 0usize;
    proj.begin().unwrap();
    while let Some(op) = stream.next_op().unwrap() {
        if i % 997 == 0 && applied_targets.len() < 1000 {
            applied_targets.push(op.target.clone());
        }
        proj.apply(&op, &mut stats).unwrap();
        i += 1;
    }
    proj.commit().unwrap();
    assert_eq!(stream.corrupt_lines, 0);
    let total = t_replay.elapsed();

    println!(
        "cold replay (stream+apply): {:.2?}  ({:.0} ops/s)",
        total,
        i as f64 / total.as_secs_f64()
    );
    println!(
        "applied={} dupes={} pending={} conflicts={} lww_losses={}",
        stats.applied, stats.skipped_dupes, stats.parked_pending, stats.conflicts, stats.lww_losses
    );
    println!(
        "open conflicts after replay: {} (field_refs rows: {})",
        proj.count("conflicts").unwrap(),
        proj.count("field_refs").unwrap()
    );
    println!(
        "projection size: {:.1} MB",
        fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0) as f64 / 1e6
    );

    let corpus_times = corpus.then(|| corpus_gate(&proj));
    if corpus {
        println!("projection size after the index: {:.1} MB", fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0) as f64 / 1e6);
    }

    // ---- phase 3: incremental apply latency (single-op txns)
    let mut lat_us: Vec<u128> = Vec::with_capacity(1000);
    let mut clock = Clock::new(new_id("dvc"));
    let actor = new_id("prs");
    for i in 0..1000 {
        let target = &applied_targets[i % applied_targets.len()];
        let op = Op {
            id: new_id("op"),
            hlc: clock.now().encode(),
            txn: new_id("txn"),
            intent: Some("bench-set".into()),
            actor: actor.clone(),
            device: clock.device.clone(),
            session: None,
            scope: "lexicon".into(),
            schema: SCHEMA_VERSION,
            verb: "set".into(),
            entity: "any".into(),
            target: target.clone(),
            field: Some("note".into()),
            value: Value::str("incremental"),
            base: proj.field(target, "note").unwrap().map(|(_, h)| h),
            heads: None,
        };
        let t = Instant::now();
        proj.begin().unwrap();
        let mut s = ApplyStats::default();
        proj.apply(&op, &mut s).unwrap();
        proj.commit().unwrap();
        lat_us.push(t.elapsed().as_micros());
    }
    lat_us.sort();
    let p50 = lat_us[499];
    let p95 = lat_us[949];
    println!("incremental apply: p50={p50}µs p95={p95}µs");

    let hwm = vm_hwm_mb();
    println!("peak memory (VmHWM): {hwm:.0} MB");

    // ---- gates
    let gate_time = total.as_secs_f64() <= 60.0;
    let gate_mem = !hwm.is_nan() && hwm <= 300.0;
    let gate_lat = p95 <= 5_000;
    println!("\nGATES (DATA-MODEL §12):");
    println!("  replay ≤ 60 s      : {}", pf(gate_time));
    println!("  memory ≤ 300 MB    : {}", pf(gate_mem));
    println!("  incr p95 ≤ 5 ms    : {}", pf(gate_lat));
    let mut gate_corpus = true;
    if let Some((index, freq)) = corpus_times {
        // Limits are for 10M tokens; smaller runs are held to their share.
        let scale = (n as f64 / 10_000_000.0).max(0.1);
        let (gi, gf) = (index <= 120.0 * scale, freq <= 15.0 * scale);
        println!("  corpus index ≤ {:.0} s : {}", 120.0 * scale, pf(gi));
        println!("  frequency list ≤ {:.1} s : {}", 15.0 * scale, pf(gf));
        gate_corpus = gi && gf;
    }

    if keep {
        println!("kept: {}", root.display());
    } else {
        fs::remove_dir_all(&root).ok();
    }
    if !(gate_time && gate_mem && gate_lat && gate_corpus) {
        std::process::exit(1);
    }
}

fn pf(ok: bool) -> &'static str {
    if ok {
        "PASS"
    } else {
        "FAIL"
    }
}

fn dir_size(p: &PathBuf) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(p) {
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                total += dir_size(&path);
            } else {
                total += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}
