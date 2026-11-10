use std::fs;
use std::path::PathBuf;

use wugs_core::Project;
use wugs_model::{base64, ulid, Hlc, Op, Value};
use wugs_text::LoroMerger;

struct Device {
    root: PathBuf,
    p: Project,
}

impl Device {
    fn open(actor: &str, merger: bool) -> Device {
        let root = std::env::temp_dir().join(format!("wugs-text-test-{}", ulid()));
        let p = if merger {
            Project::open_with(&root, actor, wugs_core::Engines { merger: Some(Box::new(LoroMerger)), ..Default::default() }).unwrap()
        } else {
            Project::open(&root, actor).unwrap()
        };
        Device { root, p }
    }

    fn own_ops(&self) -> Vec<Op> {
        let me = self.p.device().to_string();
        self.p.log.read_all().unwrap().ops.into_iter().filter(|o| o.device == me).collect()
    }

    fn pull(&mut self, from: &Device) -> wugs_core::IngestReport {
        let r = self.p.ingest(from.own_ops()).unwrap();
        assert_eq!(r.txns_held, 0, "nothing held: {:?}", r.held);
        r
    }

    fn content(&self, seg: &str) -> String {
        self.p.proj.field_str(seg, "content").unwrap().unwrap()
    }

    fn reopen_without_cache(mut self, actor: &str) -> Device {
        let root = std::mem::take(&mut self.root);
        drop(self);
        fs::remove_dir_all(root.join("cache")).unwrap();
        let p = Project::open_with(&root, actor, wugs_core::Engines { merger: Some(Box::new(LoroMerger)), ..Default::default() }).unwrap();
        Device { root, p }
    }

    fn conflicts(&self) -> i64 {
        self.p.proj.count("conflicts").unwrap()
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        if !self.root.as_os_str().is_empty() {
            fs::remove_dir_all(&self.root).ok();
        }
    }
}

fn shared_segment(bob_merger: bool, content: &str) -> (Device, Device, String, String) {
    let mut alice = Device::open("prs_Alice", true);
    let mut bob = Device::open("prs_Bob", bob_merger);
    let txt = alice.p.create_text("Shared").unwrap();
    let seg = alice.p.add_segment(&txt, "tir_utt", 0, None, None, content).unwrap().0;
    bob.pull(&alice);
    assert_eq!(bob.content(&seg), content, "bob starts from alice's plain text");
    (alice, bob, txt, seg)
}

fn concurrent_merge(alice: &mut Device, bob: &mut Device, txt: &str, seg: &str) {
    alice.p.edit_content(txt, seg, "büyük köpek geldi").unwrap();
    bob.p.edit_content(txt, seg, "köpek geldi dün").unwrap();
    alice.pull(bob);
    bob.pull(alice);
}

#[test]
fn concurrent_edits_both_survive() {
    let (mut alice, mut bob, txt, seg) = shared_segment(true, "köpek geldi");
    concurrent_merge(&mut alice, &mut bob, &txt, &seg);

    let merged = alice.content(&seg);
    assert_eq!(merged, "büyük köpek geldi dün", "alice holds both edits");
    assert_eq!(bob.content(&seg), merged, "bob converges to the same text");
    assert_eq!(merged.matches("köpek geldi").count(), 1, "the genesis text is not duplicated");
    assert_eq!(alice.conflicts(), 0, "no conflict row on alice");
    assert_eq!(bob.conflicts(), 0, "no conflict row on bob");
}

#[test]
fn one_person_on_two_devices_merges_text_edits_without_a_conflict() {
    let mut a = Device::open("prs_Alice", true);
    let mut b = Device::open("prs_Alice", true);
    let txt = a.p.create_text("Shared").unwrap();
    let seg = a.p.add_segment(&txt, "tir_utt", 0, None, None, "köpek geldi").unwrap().0;
    b.pull(&a);
    concurrent_merge(&mut a, &mut b, &txt, &seg);
    assert_eq!(a.content(&seg), "büyük köpek geldi dün");
    assert_eq!(b.content(&seg), a.content(&seg));
    assert_eq!((a.conflicts(), b.conflicts()), (0, 0), "merged text-edits are never a conflict");
    assert_eq!(a.p.proj.heads(&seg, "content").unwrap().len(), 2, "both edits stay open versions");

    // The next edit names both versions, so it carries them in `heads`.
    a.p.edit_content(&txt, &seg, "büyük köpek geldi").unwrap();
    let last = a.own_ops().pop().unwrap();
    assert_eq!(last.heads.map(|h| h.len()), Some(2));
    b.pull(&a);
    assert_eq!(b.p.proj.heads(&seg, "content").unwrap().len(), 1);
}

#[test]
fn sequential_edits_behave_like_plain_editing() {
    let (mut alice, mut bob, txt, seg) = shared_segment(true, "köpek geldi");
    alice.p.edit_content(&txt, &seg, "köpekler geldi").unwrap();
    bob.pull(&alice);
    assert_eq!(bob.content(&seg), "köpekler geldi", "bob sees alice's edit");
    bob.p.edit_content(&txt, &seg, "köpekler gitti").unwrap();
    alice.pull(&bob);

    assert_eq!(alice.content(&seg), "köpekler gitti", "alice ends on bob's version");
    assert_eq!(bob.content(&seg), "köpekler gitti");
    assert_eq!(alice.conflicts(), 0);
    assert_eq!(bob.conflicts(), 0);
}

#[test]
fn device_without_merger_degrades_to_lww() {
    let (mut alice, mut bob, txt, seg) = shared_segment(false, "köpek geldi");
    concurrent_merge(&mut alice, &mut bob, &txt, &seg);

    let (a, b) = (alice.content(&seg), bob.content(&seg));
    assert!(!a.is_empty() && !b.is_empty(), "both sides keep text: alice={a:?} bob={b:?}");
    assert!(
        b == "büyük köpek geldi" || b == "köpek geldi dün",
        "bob keeps one side by LWW, got {b:?}"
    );
    assert_eq!(a, b, "a resolved-only edit carries no CRDT update, so alice also lands on the LWW winner");
    assert_eq!(bob.conflicts(), 1, "bob surfaces the concurrent edit");
    assert_eq!(alice.conflicts(), 1, "alice surfaces the concurrent edit");
}

#[test]
fn merged_text_survives_replay() {
    let (mut alice, mut bob, txt, seg) = shared_segment(true, "köpek geldi");
    concurrent_merge(&mut alice, &mut bob, &txt, &seg);
    let before = alice.content(&seg);

    let alice = alice.reopen_without_cache("prs_Alice");

    assert_eq!(alice.content(&seg), before, "replay rebuilds the merged text byte for byte");
    assert_eq!(alice.conflicts(), 0, "replay raises no conflicts");
}

#[test]
fn corrupt_payload_degrades_then_merging_resumes() {
    let (mut alice, mut bob, txt, seg) = shared_segment(true, "köpek geldi");
    concurrent_merge(&mut alice, &mut bob, &txt, &seg);

    let template = alice.own_ops().into_iter().rev().find(|o| o.verb == "text-edit").unwrap();
    let (_, cur_hlc) = alice.p.proj.field(&seg, "content").unwrap().unwrap();
    let mut h = Hlc::parse(&cur_hlc).unwrap();
    h.millis += 1;
    h.device = "dvc_mallory".into();
    let mut bad = template.clone();
    bad.id = format!("op_{}", ulid());
    bad.txn = format!("txn_{}", ulid());
    bad.device = h.device.clone();
    bad.hlc = h.encode();
    bad.base = Some(cur_hlc);
    bad.value = Value::obj(vec![
        ("loro", Value::str(&base64::encode(b"definitely not a loro update"))),
        ("resolved", Value::str("bozuk satır")),
    ]);
    for d in [&mut alice, &mut bob] {
        let r = d.p.ingest(vec![bad.clone()]).unwrap();
        assert_eq!(r.txns_applied, 1, "corrupt payload applies instead of erroring: {:?}", r.held);
        assert_eq!(d.content(&seg), "bozuk satır", "content degrades to the op's resolved text");
    }

    alice.p.edit_content(&txt, &seg, "büyük köpek geldi dün akşam").unwrap();
    bob.p.edit_content(&txt, &seg, "çok büyük köpek geldi dün").unwrap();
    alice.pull(&bob);
    bob.pull(&alice);

    let merged = alice.content(&seg);
    assert_eq!(merged, "çok büyük köpek geldi dün akşam", "later good edits merge again");
    assert_eq!(bob.content(&seg), merged, "both sides converge after the corrupt op");
}

#[test]
fn multibyte_edits_merge() {
    let base = "köpek şişe 🐕 geldi";
    let (mut alice, mut bob, txt, seg) = shared_segment(true, base);
    alice.p.edit_content(&txt, &seg, "köpek şişe 🐕🐈 geldi").unwrap();
    bob.p.edit_content(&txt, &seg, "köpek şişeş 🐕 geldiş").unwrap();
    alice.pull(&bob);
    bob.pull(&alice);

    let merged = alice.content(&seg);
    assert_eq!(merged, "köpek şişeş 🐕🐈 geldiş", "both edits around multi-byte chars survive");
    assert_eq!(bob.content(&seg), merged);
    assert_eq!(alice.conflicts(), 0);
}

impl Device {
    /// Close and reopen the same project (same cache, same device).
    fn reopen(mut self, actor: &str) -> Device {
        let root = std::mem::take(&mut self.root);
        drop(self);
        let p = Project::open_with(&root, actor, wugs_core::Engines { merger: Some(Box::new(LoroMerger)), ..Default::default() }).unwrap();
        Device { root, p }
    }
}

/// Reviewer finding 1: a plain `set` newer than a text-edit must win on
/// every replica whatever the arrival order, match a cache rebuild, and
/// surface a conflict row for the crossing.
#[test]
fn newer_plain_set_beats_older_text_edit_in_any_order() {
    let (mut alice, mut bob, txt, seg) = shared_segment(true, "köpek geldi");
    alice.p.edit_content(&txt, &seg, "köpekler geldi").unwrap();
    // Bob writes the field with a plain set, later, without seeing Alice.
    let mut t = bob.p.intent("retype-content");
    t.set("segment", &seg, &txt, "content", Value::str("kedi gitti")).unwrap();
    t.commit().unwrap();

    alice.pull(&bob);
    bob.pull(&alice);
    assert_eq!(alice.content(&seg), "kedi gitti", "newer set wins where the text-edit landed first");
    assert_eq!(bob.content(&seg), "kedi gitti", "newer set wins where it landed first");
    assert_eq!(alice.conflicts(), 1, "the crossing is surfaced on alice");
    assert_eq!(bob.conflicts(), 1, "the crossing is surfaced on bob");

    // a later text-edit on top of the set wins again, and is exactly what
    // the editor typed
    bob.p.edit_content(&txt, &seg, "kedi gitti dün").unwrap();
    assert_eq!(bob.content(&seg), "kedi gitti dün", "editing over the set applies verbatim");

    let alice = alice.reopen_without_cache("prs_Alice");
    assert_eq!(alice.content(&seg), "kedi gitti", "rebuild matches live state");
}

/// Reviewer finding 2: a crash between the log append and the savepoint
/// release leaves the projection behind the log. The next session's edit
/// must not corrupt the lineage; full exchange plus a rebuild converge.
#[test]
fn crash_between_log_and_projection_does_not_corrupt() {
    let (mut alice, mut bob, txt, seg) = shared_segment(true, "köpek geldi");
    alice.p.edit_content(&txt, &seg, "büyük köpek geldi").unwrap();
    bob.pull(&alice);

    // crash simulation: snapshot the cache (db + wal), edit, then restore
    // the snapshot: the log keeps the edit, the projection forgets it,
    // like a power cut after append and before release
    copy_dir(&alice.root.join("cache"), &alice.root.join("cache-saved"));
    alice.p.edit_content(&txt, &seg, "büyük köpek geldi dün").unwrap();
    let root = std::mem::take(&mut alice.root);
    drop(alice);
    fs::remove_dir_all(root.join("cache")).unwrap();
    fs::rename(root.join("cache-saved"), root.join("cache")).unwrap();
    let mut alice = Device {
        p: Project::open_with(&root, "prs_Alice", wugs_core::Engines { merger: Some(Box::new(LoroMerger)), ..Default::default() }).unwrap(),
        root,
    };

    // the post-crash session edits from its stale view; the per-open peer
    // keeps these ops distinct from the forgotten edit's
    alice.p.edit_content(&txt, &seg, "çok büyük köpek geldi").unwrap();

    bob.p.ingest(alice.p.log.read_all().unwrap().ops).unwrap();
    let rebuilt = alice.reopen_without_cache("prs_Alice");
    assert_eq!(rebuilt.content(&seg), bob.content(&seg), "replicas converge after full exchange");
    let got = rebuilt.content(&seg);
    assert!(got.contains("çok büyük köpek geldi"), "the newest edit survives in {got:?}");
    assert!(!got.contains("gemi"), "no garbled interleaving in {got:?}");
}

/// Two updates that collide on op ids (same peer, same counters, the
/// crash shape) must converge across replicas and arrival orders, and a
/// rebuild must agree.
#[test]
fn colliding_updates_degrade_without_forking() {
    use wugs_model::TextMerger;
    // the crash shape: two different edits continue from the same base
    // with the same peer, so they claim the same op ids
    let (base, _) = LoroMerger.edit(&[], None, "köpek geldi", 5).unwrap();
    let (u1, _) = LoroMerger.edit(&[base.clone()], None, "büyük köpek geldi", 9).unwrap();
    let (u2, _) = LoroMerger.edit(&[base.clone()], None, "köpek geldi dün", 9).unwrap();
    // loro either rejects the set or skips the second claim of the ids;
    // both are fine as long as every replica lands on the same text,
    // because text_updates feeds resolve in canonical hlc order

    let (mut alice, mut bob, txt, seg) = shared_segment(true, "x");
    alice.p.edit_content(&txt, &seg, "y").unwrap();
    bob.pull(&alice);
    let template = alice
        .p
        .log
        .read_all()
        .unwrap()
        .ops
        .into_iter()
        .rev()
        .find(|o| o.verb == "text-edit")
        .unwrap();
    let millis = u64::from_str_radix(&template.hlc[..12], 16).unwrap();
    let mk = |n: u64, update: &[u8], resolved: &str| {
        let mut op = template.clone();
        op.id = format!("op_{}", ulid());
        op.txn = format!("txn_{}", ulid());
        op.hlc = format!("{:012x}-{:04x}-dvc_01HANDMADEDEVICE0000000000", millis + 60_000 + n, 0u32);
        op.base = Some(template.hlc.clone());
        op.value = Value::obj(vec![
            ("loro", Value::str(&base64::encode(update))),
            ("resolved", Value::str(resolved)),
        ]);
        op
    };
    let ob = mk(0, &base, "köpek geldi");
    let o1 = mk(1, &u1, "büyük köpek geldi");
    let o2 = mk(2, &u2, "köpek geldi dün");
    for op in [&ob, &o1, &o2] {
        alice.p.ingest(vec![op.clone()]).unwrap();
    }
    for op in [&ob, &o2, &o1] {
        bob.p.ingest(vec![op.clone()]).unwrap();
    }
    assert_eq!(alice.content(&seg), bob.content(&seg), "both replicas degrade to the same text");
    let a2 = alice.reopen_without_cache("prs_Alice");
    assert_eq!(a2.content(&seg), bob.content(&seg), "rebuild agrees too");
}

#[test]
fn second_text_edit_on_a_field_in_one_txn_is_refused() {
    let (mut alice, _bob, txt, seg) = shared_segment(true, "x");
    let mut t = alice.p.intent("edit-content");
    t.text_edit("segment", &seg, &txt, "content", "y").unwrap();
    let second = t.text_edit("segment", &seg, &txt, "content", "z");
    assert!(second.is_err(), "second edit of one field in a txn must be refused");
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()));
        } else {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
