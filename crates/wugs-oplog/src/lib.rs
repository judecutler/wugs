//! Per-device append-only JSONL op logs (DATA-MODEL.md §2.1).
//!
//! Layout: `<root>/log/<scope>/<device>.jsonl`. Each device appends only to
//! its own file; our copies of other devices' files are written verbatim at
//! sync ingestion. A crash mid-append leaves a truncated last line, which the
//! reader tolerates (skipped with a warning count) — everything before it is
//! intact by construction.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::PathBuf;

use wugs_model::Op;

pub struct LogDir {
    root: PathBuf,
}

#[derive(Debug, Default)]
pub struct ReadReport {
    pub ops: Vec<Op>,
    /// count of unparseable lines that were NOT the final line of a file —
    /// these indicate corruption, not a crash, and should be surfaced loudly
    pub corrupt_lines: usize,
    /// truncated trailing lines (normal after a crash); skipped
    pub truncated_tails: usize,
}

impl LogDir {
    pub fn new(root: impl Into<PathBuf>) -> LogDir {
        LogDir { root: root.into() }
    }

    pub fn log_root(&self) -> PathBuf {
        self.root.join("log")
    }

    fn file_for(&self, scope: &str, device: &str) -> PathBuf {
        self.log_root()
            .join(sanitize(scope))
            .join(format!("{}.jsonl", sanitize(device)))
    }

    /// Append a batch, grouping by (scope, device) so each file opens once.
    /// Ops land in their authoring device's file.
    pub fn append_all(&self, ops: &[Op]) -> std::io::Result<()> {
        let mut groups: HashMap<(String, String), Vec<&Op>> = HashMap::new();
        for op in ops {
            groups
                .entry((op.scope.clone(), op.device.clone()))
                .or_default()
                .push(op);
        }
        for ((scope, device), group) in groups {
            let path = self.file_for(&scope, &device);
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            let f = OpenOptions::new().create(true).append(true).open(&path)?;
            let mut w = BufWriter::new(f);
            for op in group {
                w.write_all(op.to_json().as_bytes())?;
                w.write_all(b"\n")?;
            }
            w.flush()?;
            w.into_inner()
                .map_err(|e| e.into_error())?
                .sync_all()?;
        }
        Ok(())
    }

    pub fn append(&self, op: &Op) -> std::io::Result<()> {
        self.append_all(std::slice::from_ref(op))
    }

    /// Read every op from every scope/device file, sorted by (hlc, id).
    /// Convenience over `stream_all`; loads everything into memory — fine for
    /// tests and small projects, use `stream_all` for replay.
    pub fn read_all(&self) -> std::io::Result<ReadReport> {
        let mut stream = self.stream_all()?;
        let mut ops = Vec::new();
        while let Some(op) = stream.next_op()? {
            ops.push(op);
        }
        Ok(ReadReport {
            ops,
            corrupt_lines: stream.corrupt_lines,
            truncated_tails: stream.truncated_tails,
        })
    }

    /// Stream every op in global (hlc, id) order with O(#files) memory.
    /// Each per-device file is append-ordered by that device's clock, so a
    /// k-way merge over files yields the global order without sorting.
    pub fn stream_all(&self) -> std::io::Result<OpStream> {
        self.stream_from(&HashMap::new())
    }

    /// `stream_all` with each file read from a byte offset (`OpStream::ends`
    /// of an earlier read, keyed `<scope dir>/<file>`): what was appended
    /// since. An offset past a file's end reads that file whole.
    pub fn stream_from(&self, starts: &HashMap<String, u64>) -> std::io::Result<OpStream> {
        let mut paths = Vec::new();
        let log_root = self.log_root();
        if log_root.exists() {
            for scope_entry in fs::read_dir(&log_root)? {
                let scope_dir = scope_entry?.path();
                if !scope_dir.is_dir() {
                    continue;
                }
                for file_entry in fs::read_dir(&scope_dir)? {
                    let path = file_entry?.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        let key = format!(
                            "{}/{}",
                            scope_entry_name(&scope_dir),
                            path.file_name().and_then(|n| n.to_str()).unwrap_or("")
                        );
                        let start = starts.get(&key).copied().unwrap_or(0);
                        paths.push((key, path, start));
                    }
                }
            }
        }
        OpStream::open(paths)
    }
}

fn scope_entry_name(dir: &std::path::Path) -> &str {
    dir.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

/// K-way merged streaming reader over per-device log files.
pub struct OpStream {
    readers: Vec<BufReader<File>>,
    keys: Vec<String>,
    /// Per file, the byte offset just past its last newline-ended line read
    /// so far: a torn tail is never counted as read.
    ends: Vec<u64>,
    pending_bad: Vec<bool>,
    /// (hlc, id, reader index) min-heap
    heap: std::collections::BinaryHeap<std::cmp::Reverse<(String, String, usize)>>,
    current: Vec<Option<Op>>,
    pub corrupt_lines: usize,
    pub truncated_tails: usize,
}

impl OpStream {
    fn open(paths: Vec<(String, PathBuf, u64)>) -> std::io::Result<OpStream> {
        let mut s = OpStream {
            readers: Vec::with_capacity(paths.len()),
            keys: Vec::with_capacity(paths.len()),
            ends: Vec::with_capacity(paths.len()),
            pending_bad: vec![false; paths.len()],
            heap: std::collections::BinaryHeap::new(),
            current: Vec::with_capacity(paths.len()),
            corrupt_lines: 0,
            truncated_tails: 0,
        };
        for (key, path, start) in paths {
            let mut f = File::open(path)?;
            let start = if start <= f.metadata()?.len() { start } else { 0 };
            f.seek(SeekFrom::Start(start))?;
            s.readers.push(BufReader::new(f));
            s.keys.push(key);
            s.ends.push(start);
            s.current.push(None);
        }
        for i in 0..s.readers.len() {
            s.refill(i)?;
        }
        Ok(s)
    }

    fn refill(&mut self, idx: usize) -> std::io::Result<()> {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = self.readers[idx].read_until(b'\n', &mut buf)?;
            let line = if n == 0 {
                None
            } else {
                if buf.last() == Some(&b'\n') {
                    self.ends[idx] += n as u64;
                }
                // The line ending is not part of the line. Bytes that are not
                // UTF-8 make a bad line (`None`), never a repaired one: a lossy
                // decode would let changed text through, into the cache and on
                // to peers, where `lines()` used to refuse the whole log.
                let text = buf.strip_suffix(b"\n").unwrap_or(&buf);
                let text = text.strip_suffix(b"\r").unwrap_or(text);
                Some(std::str::from_utf8(text).ok().map(str::to_owned))
            };
            match line {
                None => {
                    if self.pending_bad[idx] {
                        self.truncated_tails += 1;
                        self.pending_bad[idx] = false;
                    }
                    return Ok(());
                }
                Some(line) => {
                    if line.as_deref().is_some_and(|l| l.trim().is_empty()) {
                        continue;
                    }
                    match line.ok_or(()).and_then(|l| Op::from_json(&l).map_err(drop)) {
                        Ok(op) => {
                            if self.pending_bad[idx] {
                                self.corrupt_lines += 1;
                                self.pending_bad[idx] = false;
                            }
                            self.heap.push(std::cmp::Reverse((
                                op.hlc.clone(),
                                op.id.clone(),
                                idx,
                            )));
                            self.current[idx] = Some(op);
                            return Ok(());
                        }
                        Err(_) => {
                            if self.pending_bad[idx] {
                                self.corrupt_lines += 1;
                            }
                            self.pending_bad[idx] = true;
                        }
                    }
                }
            }
        }
    }

    /// Per file (`<scope dir>/<file>`), where the lines read so far end;
    /// complete once `next_op` has returned `None`.
    pub fn ends(&self) -> impl Iterator<Item = (&str, u64)> {
        self.keys.iter().map(String::as_str).zip(self.ends.iter().copied())
    }

    pub fn next_op(&mut self) -> std::io::Result<Option<Op>> {
        let Some(std::cmp::Reverse((_, _, idx))) = self.heap.pop() else {
            return Ok(None);
        };
        let op = self.current[idx].take().expect("heap/current out of sync");
        self.refill(idx)?;
        Ok(Some(op))
    }
}

/// Scope/device ids are prefixed ULIDs or fixed names; keep filenames safe.
/// Public so readers of the log tree (wugs-seal) map ids to the same names.
pub fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wugs_model::{new_id, Clock, Value};

    fn temp_root() -> PathBuf {
        let p = std::env::temp_dir().join(format!("wugs-oplog-test-{}", wugs_model::ulid()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn mk_op(clock: &mut Clock, scope: &str, device: &str) -> Op {
        Op {
            id: new_id("op"),
            hlc: clock.now().encode(),
            txn: new_id("txn"),
            intent: None,
            actor: "prs_01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            device: device.into(),
            session: None,
            scope: scope.into(),
            schema: 1,
            verb: "create".into(),
            entity: "lexeme".into(),
            target: new_id("lex"),
            field: None,
            value: Value::obj(vec![("lemma", Value::str("wug"))]),
            base: None,
            heads: None,
        }
    }

    #[test]
    fn append_and_read_sorted() {
        let root = temp_root();
        let log = LogDir::new(&root);
        let mut c1 = Clock::new("dvc_A".into());
        let mut c2 = Clock::new("dvc_B".into());
        let mut ops = Vec::new();
        for _ in 0..20 {
            ops.push(mk_op(&mut c1, "lexicon", "dvc_A"));
            ops.push(mk_op(&mut c2, "lexicon", "dvc_B"));
        }
        log.append_all(&ops).unwrap();
        let report = log.read_all().unwrap();
        assert_eq!(report.ops.len(), 40);
        assert_eq!(report.corrupt_lines, 0);
        assert_eq!(report.truncated_tails, 0);
        for w in report.ops.windows(2) {
            assert!((w[0].hlc.as_str(), w[0].id.as_str()) <= (w[1].hlc.as_str(), w[1].id.as_str()));
        }
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn stream_from_reads_what_was_appended_since_and_never_a_torn_tail() {
        let root = temp_root();
        let log = LogDir::new(&root);
        let mut clock = Clock::new("dvc_A".into());
        log.append(&mk_op(&mut clock, "lexicon", "dvc_A")).unwrap();
        let mut first = log.stream_all().unwrap();
        while first.next_op().unwrap().is_some() {}
        let ends: HashMap<String, u64> = first.ends().map(|(k, e)| (k.to_string(), e)).collect();
        let path = root.join("log").join("lexicon").join("dvc_A.jsonl");
        assert_eq!(ends["lexicon/dvc_A.jsonl"], fs::metadata(&path).unwrap().len());

        let later = mk_op(&mut clock, "lexicon", "dvc_A");
        log.append(&later).unwrap();
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"id\":\"op_trunc").unwrap();
        drop(f);
        let mut since = log.stream_from(&ends).unwrap();
        assert_eq!(since.next_op().unwrap().map(|o| o.id), Some(later.id));
        assert!(since.next_op().unwrap().is_none());
        assert_eq!(since.truncated_tails, 1);
        let end = since.ends().next().unwrap().1;
        assert_eq!(end, fs::metadata(&path).unwrap().len() - b"{\"id\":\"op_trunc".len() as u64, "the torn tail is not counted as read");

        let past_the_end = HashMap::from([("lexicon/dvc_A.jsonl".to_string(), u64::MAX)]);
        let mut whole = log.stream_from(&past_the_end).unwrap();
        let mut n = 0;
        while whole.next_op().unwrap().is_some() {
            n += 1;
        }
        assert_eq!(n, 2, "an end past the file reads it whole");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn truncated_tail_is_tolerated() {
        let root = temp_root();
        let log = LogDir::new(&root);
        let mut clock = Clock::new("dvc_A".into());
        let op = mk_op(&mut clock, "lexicon", "dvc_A");
        log.append(&op).unwrap();
        // simulate a crash: half a line at EOF
        let path = root.join("log").join("lexicon").join("dvc_A.jsonl");
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"id\":\"op_trunc").unwrap();
        drop(f);
        let report = log.read_all().unwrap();
        assert_eq!(report.ops.len(), 1);
        assert_eq!(report.truncated_tails, 1);
        assert_eq!(report.corrupt_lines, 0);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_line_with_invalid_utf8_is_a_bad_line_never_repaired() {
        let root = temp_root();
        let log = LogDir::new(&root);
        let mut clock = Clock::new("dvc_A".into());
        log.append(&mk_op(&mut clock, "lexicon", "dvc_A")).unwrap();
        // A file re-saved as Latin-1: a valid op whose actor gained the byte
        // 0xF6 («ö» in Latin-1) inside the JSON string.
        let mut bad = mk_op(&mut clock, "lexicon", "dvc_A").to_json().into_bytes();
        let at = bad.windows(9).position(|w| w == b"\"actor\":\"").unwrap() + 9;
        bad.insert(at, 0xF6);
        bad.push(b'\n');
        let path = root.join("log").join("lexicon").join("dvc_A.jsonl");
        OpenOptions::new().append(true).open(&path).unwrap().write_all(&bad).unwrap();
        log.append(&mk_op(&mut clock, "lexicon", "dvc_A")).unwrap();
        let report = log.read_all().unwrap();
        assert_eq!(report.ops.len(), 2, "the good ops on either side still read");
        assert_eq!(report.corrupt_lines, 1, "the invalid line counts as corrupt");
        assert!(report.ops.iter().all(|o| !o.actor.contains('\u{FFFD}')), "nothing was repaired into the stream");
        fs::remove_dir_all(root).ok();
    }
}
