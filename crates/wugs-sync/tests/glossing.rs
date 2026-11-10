//! Concurrent glossing by two linguists (GLOSSING-MODEL.md, "Merge
//! semantics"). Each test edits offline on both devices, syncs, and checks
//! that no analysis work vanished without a trace. Conflicts come from write
//! heads, not from who wrote, so some tests run one person on two laptops.

use std::fs;
use std::path::PathBuf;

use wugs_core::{MorphSpec, Project};
use wugs_sync::{sync_pair, SyncPolicy};

struct Rig {
    roots: Vec<PathBuf>,
}

impl Rig {
    fn root(&mut self) -> PathBuf {
        let root = std::env::temp_dir().join(format!("wugs-gloss-sync-{}", wugs_model::ulid()));
        self.roots.push(root.clone());
        root
    }

    /// Two linguists, one device each.
    fn pair(&mut self) -> (Project, Project) {
        self.devices("prs_Ana", "prs_Bo")
    }

    /// One linguist on two laptops.
    fn laptops(&mut self) -> (Project, Project) {
        self.devices("prs_Ana", "prs_Ana")
    }

    fn devices(&mut self, first: &str, second: &str) -> (Project, Project) {
        let a = Project::open(self.root(), first).unwrap();
        let b = Project::open_joined(self.root(), second, &a.project_id).unwrap();
        (a, b)
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        for r in &self.roots {
            fs::remove_dir_all(r).ok();
        }
    }
}

fn sync(a: &mut Project, b: &mut Project) {
    let all = SyncPolicy::allow_all();
    sync_pair(a, b, &all, &all).unwrap();
}

fn tokens(p: &Project, seg: &str) -> Vec<String> {
    p.proj.members(seg).unwrap()
}

fn morphs(p: &Project, tok: &str) -> Vec<String> {
    p.proj.members(tok).unwrap()
}

/// What the linguist sees for a segment: each word with its morphs as
/// `surface:gloss`.
fn igt(p: &Project, seg: &str) -> Vec<(String, Vec<String>)> {
    let f = |id: &str, k: &str| p.proj.field_str(id, k).unwrap().unwrap_or_default();
    tokens(p, seg)
        .iter()
        .map(|t| (f(t, "form"), morphs(p, t).iter().map(|m| format!("{}:{}", f(m, "surface"), f(m, "gloss"))).collect()))
        .collect()
}

fn conflicts_on(p: &Project, target: &str, field: &str) -> i64 {
    let mut s = p.proj.db.prepare("SELECT COUNT(*) FROM conflicts WHERE target=?1 AND field=?2").unwrap();
    s.bind_text(1, target).unwrap();
    s.bind_text(2, field).unwrap();
    s.step().unwrap();
    s.col_i64(0)
}

/// Is the other list of a conflict on `target.field` still on offer?
fn alternative_open(p: &Project, target: &str, field: &str) -> bool {
    p.proj.conflict_loser(target, field).unwrap().is_some()
}

fn spec(surface: &str, ty: &str, gloss: &str) -> MorphSpec {
    MorphSpec { surface: surface.into(), morph_type: ty.into(), gloss: Some(gloss.into()), ..Default::default() }
}

/// "köpekler geldi" with köpekler split into köpek + ler and nothing glossed,
/// on both devices.
struct Shared {
    txt: String,
    seg: String,
    toks: Vec<String>,
    morphs: Vec<String>,
}

fn shared(a: &mut Project, b: &mut Project) -> Shared {
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "köpekler geldi").unwrap().0;
    let toks = a.tokenize(&txt, &seg, &["köpekler", "geldi"]).unwrap().0;
    let morphs = a.split_morph(&txt, &toks[0], &[("köpek", "root"), ("ler", "suffix")]).unwrap().0;
    sync(a, b);
    Shared { txt, seg, toks, morphs }
}

#[test]
fn a_gloss_survives_a_concurrent_retokenize_of_another_word() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.pair();
    let s = shared(&mut a, &mut b);

    a.retokenize(&s.txt, &s.seg, &["köpekler", "gitti"]).unwrap();
    b.set_gloss(&s.txt, &s.morphs[1], "PL", None).unwrap();
    sync(&mut a, &mut b);

    for p in [&a, &b] {
        let view = igt(p, &s.seg);
        assert_eq!(view[0], ("köpekler".to_string(), vec!["köpek:".to_string(), "ler:PL".to_string()]), "{view:?}");
        assert_eq!(view[1].0, "gitti");
    }
    assert_eq!(igt(&a, &s.seg), igt(&b, &s.seg), "both devices show the same analysis");
}

#[test]
fn a_gloss_on_a_kept_morph_survives_a_concurrent_reanalysis_of_the_word() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.pair();
    let s = shared(&mut a, &mut b);

    // A re-edits the suffix; B glosses the root it did not touch.
    a.apply_analysis(&s.txt, &s.toks[0], &[MorphSpec { surface: "köpek".into(), morph_type: "root".into(), ..Default::default() }, spec("ler", "suffix", "PL")]).unwrap();
    b.set_gloss(&s.txt, &s.morphs[0], "dog", None).unwrap();
    sync(&mut a, &mut b);

    for p in [&a, &b] {
        assert_eq!(igt(p, &s.seg)[0].1, ["köpek:dog", "ler:PL"], "both edits kept");
    }
}

#[test]
fn concurrent_analyses_of_one_word_leave_one_winner_and_a_conflict() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.laptops();
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "köpekler").unwrap().0;
    let tok = a.tokenize(&txt, &seg, &["köpekler"]).unwrap().0.remove(0);
    sync(&mut a, &mut b);

    a.apply_analysis(&txt, &tok, &[spec("köpek", "root", "dog"), spec("ler", "suffix", "PL")]).unwrap();
    b.apply_analysis(&txt, &tok, &[spec("köpekler", "root", "dogs")]).unwrap();
    sync(&mut a, &mut b);

    let view = igt(&a, &seg);
    assert_eq!(view, igt(&b, &seg), "both devices agree");
    let winner = &view[0].1;
    assert!(
        *winner == ["köpek:dog", "ler:PL"] || *winner == ["köpekler:dogs"],
        "one analysis wins whole, never an interleaving: {winner:?}"
    );
    assert_eq!(morphs(&a, &tok), morphs(&b, &tok), "the same morph ids on both");
    for p in [&a, &b] {
        assert_eq!(conflicts_on(p, &tok, "morphs"), 1, "the concurrent analysis is a conflict to choose from");
        let got = p.analysis_suggestions("köpekler").unwrap();
        assert_eq!(got.len(), 1, "only the winning analysis is a suggestion: {got:?}");
    }
}

#[test]
fn apply_to_unanalysed_against_a_concurrent_analysis_leaves_one_winner() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.pair();
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "wugs wugs").unwrap().0;
    let toks = a.tokenize(&txt, &seg, &["wugs", "wugs"]).unwrap().0;
    sync(&mut a, &mut b);

    let r = a.apply_analysis_to_unanalysed("wugs", &[spec("wug", "root", "wug"), spec("s", "suffix", "PL")]).unwrap();
    assert_eq!(r.applied, 2);
    b.apply_analysis(&txt, &toks[1], &[spec("wugs", "root", "wugs")]).unwrap();
    sync(&mut a, &mut b);

    let view = igt(&a, &seg);
    assert_eq!(view, igt(&b, &seg), "both devices agree");
    assert_eq!(view[0].1, ["wug:wug", "s:PL"], "the uncontested token took the batch analysis");
    assert!(view[1].1 == ["wug:wug", "s:PL"] || view[1].1 == ["wugs:wugs"], "one analysis, not both: {:?}", view[1].1);
    for p in [&a, &b] {
        assert_eq!(conflicts_on(p, &toks[1], "morphs"), 1);
        assert_eq!(conflicts_on(p, &toks[0], "morphs"), 0);
    }
}

#[test]
fn concurrent_first_tokenize_leaves_one_token_list_and_a_conflict() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.laptops();
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "köpekler geldi").unwrap().0;
    sync(&mut a, &mut b);

    a.tokenize(&txt, &seg, &["köpekler", "geldi"]).unwrap();
    b.tokenize(&txt, &seg, &["köpekler", "geldi"]).unwrap();
    sync(&mut a, &mut b);

    for p in [&a, &b] {
        let forms: Vec<String> = igt(p, &seg).into_iter().map(|(f, _)| f).collect();
        assert_eq!(forms, ["köpekler", "geldi"], "no doubled words");
        assert_eq!(conflicts_on(p, &seg, "tokens"), 1);
    }
    assert_eq!(tokens(&a, &seg), tokens(&b, &seg), "the same token ids on both");
}

/// Everything a replay must reproduce: what each segment shows and which
/// field writes conflicted.
fn state(p: &Project, seg: &str) -> (Vec<(String, Vec<String>)>, Vec<(String, String, String, String)>) {
    let mut s = p.proj.db.prepare("SELECT target, field, winner_op, loser_op FROM conflicts ORDER BY target, field, winner_op").unwrap();
    let mut rows = Vec::new();
    while s.step().unwrap() {
        rows.push((s.col_text(0), s.col_text(1), s.col_text(2), s.col_text(3)));
    }
    (igt(p, seg), rows)
}

fn rebuilt(p: Project) -> Project {
    let (root, actor) = (p.root.clone(), p.actor.clone());
    drop(p);
    fs::remove_file(root.join("cache").join("projection.sqlite")).unwrap();
    for side in ["projection.sqlite-wal", "projection.sqlite-shm"] {
        fs::remove_file(root.join("cache").join(side)).ok();
    }
    Project::open(&root, &actor).unwrap()
}

#[test]
fn replay_rebuilds_the_same_membership_and_conflicts() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.pair();
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "köpekler geldi").unwrap().0;
    sync(&mut a, &mut b);
    a.tokenize(&txt, &seg, &["köpekler", "geldi"]).unwrap();
    b.tokenize(&txt, &seg, &["köpekler", "geldi"]).unwrap();
    sync(&mut a, &mut b);
    let toks = tokens(&a, &seg);
    a.apply_analysis(&txt, &toks[0], &[spec("köpek", "root", "dog"), spec("ler", "suffix", "PL")]).unwrap();
    b.apply_analysis(&txt, &toks[0], &[spec("köpekler", "root", "dogs")]).unwrap();
    b.retokenize(&txt, &seg, &["köpekler", "gitti"]).unwrap();
    sync(&mut a, &mut b);

    let before = state(&a, &seg);
    assert_eq!(before, state(&b, &seg), "converged");
    let mut fields: Vec<&str> = before.1.iter().map(|r| r.1.as_str()).collect();
    fields.sort();
    // Retokenizing is a derived write (it follows the transcription), so it
    // names only the list it overwrote: the concurrent first tokenize stays
    // an open alternative until a person chooses (DATA-MODEL §2.2).
    assert_eq!(fields, ["morphs", "tokens"], "B's retokenize leaves the token-list conflict open");
    assert!(alternative_open(&a, &toks[0], "morphs"), "the morph lists still conflict");
    assert!(alternative_open(&a, &seg, "tokens"), "the other first tokenize is still on offer");
    assert_eq!(state(&rebuilt(a), &seg), before, "replay on A");
    assert_eq!(state(&rebuilt(b), &seg), before, "replay on B");
}

fn hidden(p: &Project, seg: &str) -> Vec<(String, String, String, String)> {
    p.proj.hidden_edits(seg).unwrap().into_iter().map(|h| (h.parent, h.member, h.target, h.field)).collect()
}

#[test]
fn a_gloss_on_a_morph_another_device_dropped_is_reported_on_both_and_restorable() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.pair();
    let s = shared(&mut a, &mut b);

    // A decides köpekler is one morph; B glosses the suffix A just dropped.
    a.apply_analysis(&s.txt, &s.toks[0], &[MorphSpec { surface: "köpekler".into(), morph_type: "root".into(), ..Default::default() }]).unwrap();
    b.set_gloss(&s.txt, &s.morphs[1], "PL", None).unwrap();
    sync(&mut a, &mut b);

    let want = vec![(s.toks[0].clone(), s.morphs[1].clone(), s.morphs[1].clone(), "gloss".to_string())];
    assert_eq!(hidden(&a, &s.seg), want, "A sees that B's gloss is hidden");
    assert_eq!(hidden(&b, &s.seg), want, "and so does B");
    assert!(hidden(&a, &s.seg).iter().all(|(_, m, _, _)| !a.proj.is_tombstoned(m).unwrap()));

    b.restore_member(&s.txt, &s.toks[0], &s.morphs[1]).unwrap();
    sync(&mut a, &mut b);
    for p in [&a, &b] {
        assert_eq!(igt(p, &s.seg)[0].1, ["köpekler:", "ler:PL"], "the gloss is back in view");
        assert!(hidden(p, &s.seg).is_empty());
    }
}

#[test]
fn a_gloss_on_a_word_another_device_retyped_is_reported_on_both() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.pair();
    let s = shared(&mut a, &mut b);

    a.retokenize(&s.txt, &s.seg, &["kediler", "geldi"]).unwrap();
    b.set_gloss(&s.txt, &s.morphs[0], "dog", None).unwrap();
    sync(&mut a, &mut b);

    let want = vec![(s.seg.clone(), s.toks[0].clone(), s.morphs[0].clone(), "gloss".to_string())];
    assert_eq!(hidden(&a, &s.seg), want);
    assert_eq!(hidden(&b, &s.seg), want);
    assert!(hidden(&rebuilt(a), &s.seg) == want, "replay reports it too");
}

#[test]
fn dropping_your_own_work_or_unedited_morphs_reports_nothing() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.pair();
    let s = shared(&mut a, &mut b);
    a.set_gloss(&s.txt, &s.morphs[1], "PL", None).unwrap();
    a.apply_analysis(&s.txt, &s.toks[0], &[spec("köpekler", "root", "dogs")]).unwrap();
    a.retokenize(&s.txt, &s.seg, &["köpekler", "gitti"]).unwrap();
    sync(&mut a, &mut b);
    assert!(hidden(&a, &s.seg).is_empty() && hidden(&b, &s.seg).is_empty(), "a device's own drops and bare morphs are not flagged");
}

#[test]
fn a_losing_whole_analysis_is_reported_on_both_devices() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.pair();
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "köpekler").unwrap().0;
    let tok = a.tokenize(&txt, &seg, &["köpekler"]).unwrap().0.remove(0);
    sync(&mut a, &mut b);

    a.apply_analysis(&txt, &tok, &[spec("köpek", "root", "dog"), spec("ler", "suffix", "PL")]).unwrap();
    b.apply_analysis(&txt, &tok, &[spec("köpekler", "root", "dogs")]).unwrap();
    sync(&mut a, &mut b);

    let shown = igt(&a, &seg)[0].1.clone();
    let lost: Vec<&str> = if shown == ["köpekler:dogs"] { vec!["köpek", "ler"] } else { vec!["köpekler"] };
    for p in [&a, &b] {
        let mut got: Vec<String> = p.proj.hidden_edits(&seg).unwrap().iter().map(|h| p.proj.field_str(&h.member, "surface").unwrap().unwrap()).collect();
        got.dedup();
        assert_eq!(got, lost, "the losing analysis's glosses are reported (winner {shown:?})");
    }
}

#[test]
fn a_hidden_gloss_stays_reported_after_its_device_restores_a_sibling() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.pair();
    let s = shared(&mut a, &mut b);

    b.set_gloss(&s.txt, &s.morphs[0], "dog", None).unwrap();
    b.set_gloss(&s.txt, &s.morphs[1], "PL", None).unwrap();
    sync(&mut a, &mut b);
    a.apply_analysis(&s.txt, &s.toks[0], &[MorphSpec { surface: "köpekler".into(), morph_type: "root".into(), ..Default::default() }]).unwrap();
    sync(&mut a, &mut b);
    assert_eq!(hidden(&b, &s.seg).len(), 2, "both of B's glosses are hidden by A");

    b.restore_member(&s.txt, &s.toks[0], &s.morphs[1]).unwrap();
    sync(&mut a, &mut b);
    let want = vec![(s.toks[0].clone(), s.morphs[0].clone(), s.morphs[0].clone(), "gloss".to_string())];
    assert_eq!(hidden(&a, &s.seg), want, "B's dog gloss is still reported on A");
    assert_eq!(hidden(&b, &s.seg), want, "and on B");
    assert_eq!(hidden(&rebuilt(b), &s.seg), want, "and after a replay");
}

fn loser(p: &Project, parent: &str, field: &str) -> Vec<String> {
    let v = p.proj.conflict_loser(parent, field).unwrap().expect("an open conflict");
    let wugs_model::Value::Arr(ids) = v else { panic!("a list") };
    ids.iter().map(|i| i.as_str().unwrap().to_string()).collect()
}

#[test]
fn choosing_the_other_analysis_converges_and_settles_the_conflict() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.laptops();
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "köpekler").unwrap().0;
    let tok = a.tokenize(&txt, &seg, &["köpekler"]).unwrap().0.remove(0);
    sync(&mut a, &mut b);
    a.apply_analysis(&txt, &tok, &[spec("köpek", "root", "dog"), spec("ler", "suffix", "PL")]).unwrap();
    b.apply_analysis(&txt, &tok, &[spec("köpekler", "root", "dogs")]).unwrap();
    sync(&mut a, &mut b);

    let shown = igt(&a, &seg)[0].1.clone();
    let other = loser(&a, &tok, "morphs");
    assert_eq!(other, loser(&b, &tok, "morphs"), "both devices offer the same other analysis");
    a.choose_member_list(&txt, &tok, &other).unwrap();
    sync(&mut a, &mut b);

    for p in [&a, &b] {
        assert_eq!(morphs(p, &tok), other, "the other analysis shows");
        assert_ne!(igt(p, &seg)[0].1, shown);
        assert!(!alternative_open(p, &tok, "morphs"), "the chosen list is the winner, so nothing is left to choose");
        assert_eq!(conflicts_on(p, &tok, "morphs"), 0, "the choice named both lists, so the conflict closed");
        assert!(!p.proj.hidden_edits(&seg).unwrap().is_empty(), "the analysis set aside stays reported");
    }
    assert_eq!(igt(&a, &seg), igt(&b, &seg));
}

#[test]
fn choosing_the_other_word_breakdown_converges_and_settles_the_conflict() {
    let mut rig = Rig { roots: Vec::new() };
    let (mut a, mut b) = rig.pair();
    let txt = a.create_text("T").unwrap();
    let seg = a.add_segment(&txt, "tir_utt", 0, None, None, "köpekler geldi").unwrap().0;
    sync(&mut a, &mut b);
    a.tokenize(&txt, &seg, &["köpekler", "geldi"]).unwrap();
    b.tokenize(&txt, &seg, &["köpek", "ler", "geldi"]).unwrap();
    sync(&mut a, &mut b);

    let other = loser(&b, &seg, "tokens");
    b.choose_member_list(&txt, &seg, &other).unwrap();
    sync(&mut a, &mut b);
    for p in [&a, &b] {
        assert_eq!(tokens(p, &seg), other);
        assert!(!alternative_open(p, &seg, "tokens"));
    }
}
