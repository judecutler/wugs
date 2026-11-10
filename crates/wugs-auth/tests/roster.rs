use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use wugs_auth::{Body, DeviceKey, MergeError, PublicKey, RecordHash, Roster, RosterFile, SignedRecord, Standing, MAX_PEER_RECORDS};

const P: &str = "prj_test";
const KEYS: u8 = 5;

fn key(i: u8) -> DeviceKey {
    DeviceKey::from_seed([i + 1; 32])
}

fn pk(i: u8) -> PublicKey {
    key(i).public()
}

#[derive(Debug, Clone)]
enum Kind {
    Enrol { key: u8, guest: bool, label: u8 },
    /// `pick` 0 revokes everything; otherwise it picks a record of the target's chain.
    Revoke { key: u8, pick: u8 },
    Attest { key: u8, dvc: u8 },
}

/// One record to sign: by `by`, at the tip of its chain, or, with `fork`,
/// at an earlier point of it (equivocation).
#[derive(Debug, Clone)]
struct Step {
    by: u8,
    kind: Kind,
    fork: Option<u8>,
}

fn arb_kind() -> impl Strategy<Value = Kind> {
    prop_oneof![
        (0..KEYS, any::<bool>(), 0..3u8).prop_map(|(key, guest, label)| Kind::Enrol { key, guest, label }),
        (0..KEYS, 0..6u8).prop_map(|(key, pick)| Kind::Revoke { key, pick }),
        (0..KEYS, 0..2u8).prop_map(|(key, dvc)| Kind::Attest { key, dvc }),
    ]
}

/// Arbitrary steps, authorized or not: any of five keys signs any body.
/// Key 0 is the genesis device. With `forks`, some steps equivocate.
fn arb_steps(forks: bool) -> impl Strategy<Value = Vec<Step>> {
    let fork = prop::option::weighted(0.15, any::<u8>()).prop_map(move |f| f.filter(|_| forks));
    prop::collection::vec((0..KEYS, arb_kind(), fork).prop_map(|(by, kind, fork)| Step { by, kind, fork }), 0..24)
}

fn genesis() -> Roster {
    Roster::genesis(P, &key(0), "founder")
}

/// Sign the steps in order, chaining each signer's records. Deterministic,
/// so a longer step list signs the same records first.
fn build(steps: &[Step]) -> Vec<SignedRecord> {
    let mut line: Vec<Vec<RecordHash>> = vec![Vec::new(); KEYS as usize];
    line[0].push(genesis().records()[0].hash());
    let mut out = Vec::new();
    for step in steps {
        let mine = &line[step.by as usize];
        let tip = mine.last().copied();
        let prev = match step.fork {
            Some(f) if !mine.is_empty() => {
                let i = f as usize % mine.len();
                i.checked_sub(1).map(|j| mine[j])
            }
            _ => tip,
        };
        let body = match step.kind {
            Kind::Enrol { key, guest, label } => Body::Enrol { key: pk(key), label: format!("d{label}"), guest },
            Kind::Revoke { key, pick } => {
                let theirs = &line[key as usize];
                let upto = (pick as usize).checked_sub(1).and_then(|p| theirs.get(p % theirs.len().max(1)).copied());
                Body::Revoke { key: pk(key), upto }
            }
            Kind::Attest { key, dvc } => Body::Attest { key: pk(key), dvc: format!("dvc_{dvc}") },
        };
        let r = SignedRecord::sign(&key(step.by), P, prev, body);
        if prev == tip {
            line[step.by as usize].push(r.hash());
        }
        out.push(r);
    }
    out
}

fn roster_of(records: &[SignedRecord]) -> Roster {
    let mut r = genesis();
    let lines: Vec<String> = records.iter().map(SignedRecord::to_line).collect();
    r.merge_lines(lines.iter().map(String::as_str));
    r
}

/// `hashes` and every record before them on their chains.
fn with_ancestors(records: &[SignedRecord], hashes: impl IntoIterator<Item = RecordHash>) -> BTreeSet<RecordHash> {
    let prev: BTreeMap<RecordHash, Option<RecordHash>> = records.iter().map(|r| (r.hash(), r.record().prev)).collect();
    let mut out = BTreeSet::new();
    for mut at in hashes.into_iter().map(Some) {
        while let Some(h) = at.filter(|h| out.insert(*h)) {
            at = prev.get(&h).copied().flatten();
        }
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn arrival_order_does_not_matter(shuffled in arb_steps(true).prop_map(|s| build(&s)).prop_shuffle()) {
        let mut sorted = shuffled.clone();
        sorted.sort_by_key(SignedRecord::to_line);
        prop_assert_eq!(roster_of(&sorted).state(), roster_of(&shuffled).state());
    }

    #[test]
    fn replayed_records_are_idempotent(steps in arb_steps(true)) {
        let records = build(&steps);
        let mut r = roster_of(&records);
        let before = r.state();
        let lines = r.lines();
        let again = r.merge_lines(lines.iter().map(String::as_str));
        prop_assert!(again.added.is_empty());
        let doubled: Vec<SignedRecord> = records.iter().chain(records.iter()).cloned().collect();
        prop_assert_eq!(&before, &r.state());
        prop_assert_eq!(&before, &roster_of(&doubled).state());
    }

    #[test]
    /// Enrolling a revoked member again changes nothing. (A revoked key
    /// that was never a member is different: an enrol activates its records
    /// before the cutoff, as for any device. A merge never stores records of
    /// a non-member, so a revoker cannot have kept such a record in its
    /// cutoff outside a crafted record set.)
    fn revocation_is_terminal(steps in arb_steps(true)) {
        let st = roster_of(&build(&steps)).state();
        for k in st.revoked.keys().filter(|k| st.devices.get(k).is_some_and(|d| !d.guest)) {
            let k = (0..KEYS).find(|i| pk(*i) == *k).unwrap();
            let mut more = steps.clone();
            more.extend((0..KEYS).map(|by| Step { by, kind: Kind::Enrol { key: k, guest: false, label: 9 }, fork: None }));
            prop_assert_eq!(roster_of(&build(&more)).state().standing(&pk(k)), Standing::Revoked);
        }
    }

    #[test]
    /// A record past its signer's cutoff has no effect at all, except as the
    /// anchor a revoke names, or as evidence in a paradox, which fails
    /// closed and is reported as `contested` (pinned by
    /// `mutual_revokes_fail_closed`).
    fn records_after_a_revocation_are_void(steps in arb_steps(false)) {
        let records = build(&steps);
        let st = roster_of(&records).state();
        if !st.contested.is_empty() {
            return Ok(());
        }
        let anchors = records.iter().filter_map(|r| match r.record().body {
            Body::Revoke { upto, .. } => upto,
            _ => None,
        });
        let standing = with_ancestors(&records, st.revoked.values().flatten().copied().chain(anchors));
        let kept: Vec<SignedRecord> = records
            .iter()
            .filter(|r| !st.revoked.contains_key(&r.record().signer) || standing.contains(&r.hash()))
            .cloned()
            .collect();
        prop_assert_eq!(st, roster_of(&kept).state());
    }

    #[test]
    /// Outside a paradox (see above), nothing a guest signs has any effect.
    fn guests_cannot_sign_anything(steps in arb_steps(false)) {
        let records = build(&steps);
        let st = roster_of(&records).state();
        if !st.contested.is_empty() {
            return Ok(());
        }
        let kept: Vec<SignedRecord> = records
            .iter()
            .filter(|r| st.standing(&r.record().signer) != Standing::Guest)
            .cloned()
            .collect();
        prop_assert_eq!(st, roster_of(&kept).state());
    }

    #[test]
    /// An attest names its signer's current id, so a key holds at most one.
    fn each_key_has_one_current_device_id(steps in arb_steps(true)) {
        let st = roster_of(&build(&steps)).state();
        let mut seen = BTreeSet::new();
        for k in st.attested.values().flatten() {
            prop_assert!(seen.insert(*k), "{k:?} is current for two ids");
        }
    }

    #[test]
    /// Equivocation is detected whatever order the forks arrive in.
    fn a_forked_signer_is_contested_and_revoked(steps in arb_steps(true).prop_shuffle()) {
        let records = build(&steps);
        let st = roster_of(&records).state();
        let mut first: BTreeMap<_, RecordHash> = BTreeMap::new();
        for r in std::iter::once(&genesis().records()[0]).chain(&records) {
            let rec = r.record();
            if *first.entry((rec.signer, rec.prev)).or_insert(r.hash()) != r.hash() {
                prop_assert!(st.contested.contains(&rec.signer));
                prop_assert_eq!(st.standing(&rec.signer), Standing::Revoked);
            }
        }
    }
}

#[test]
fn a_revoked_device_signs_nothing_new_and_a_late_revoke_voids_what_it_did() {
    let (a, b, c, d) = (key(0), key(1), key(2), key(3));
    let mut r = Roster::genesis(P, &a, "A");
    r.enrol(&a, b.public(), "B", false).unwrap();
    r.enrol(&a, c.public(), "C", false).unwrap();
    // A revokes C before seeing anything C signed.
    let mut at_a = r.clone();
    at_a.revoke(&a, c.public()).unwrap();
    // Meanwhile C, elsewhere, enrols D.
    let mut at_c = r.clone();
    at_c.enrol(&c, d.public(), "D", false).unwrap();
    assert_eq!(at_c.state().standing(&d.public()), Standing::Member);

    at_a.merge_lines(at_c.lines().iter().map(String::as_str));
    let st = at_a.state();
    assert_eq!(st.standing(&c.public()), Standing::Revoked);
    assert_eq!(st.standing(&d.public()), Standing::Unknown, "C's enrol came after A's cutoff");
    assert!(at_a.enrol(&c, key(4).public(), "E", false).is_err(), "a revoked device cannot append");
}

#[test]
fn devices_enrolled_before_a_revocation_stay_enrolled() {
    let (a, c, d) = (key(0), key(2), key(3));
    let mut r = Roster::genesis(P, &a, "A");
    r.enrol(&a, c.public(), "C", false).unwrap();
    r.enrol(&c, d.public(), "D", false).unwrap();
    r.revoke(&a, c.public()).unwrap();
    let st = r.state();
    assert_eq!(st.standing(&c.public()), Standing::Revoked);
    assert_eq!(st.standing(&d.public()), Standing::Member, "C enrolled D before its cutoff");
    assert!(r.enrol(&a, c.public(), "C again", false).is_ok());
    assert_eq!(r.state().standing(&c.public()), Standing::Revoked, "re-enrolment does not undo a revoke");
}

#[test]
fn a_stolen_genesis_key_cannot_slip_under_a_revoke() {
    let (a, b, t) = (key(0), key(1), key(2));
    let mut r = Roster::genesis(P, &a, "A");
    let enrol_b = r.enrol(&a, b.public(), "B", false).unwrap();
    r.append(&a, Body::Attest { key: a.public(), dvc: "dvc_a".into() }).unwrap();
    r.append(&b, Body::Attest { key: b.public(), dvc: "dvc_b".into() }).unwrap();
    // A's laptop is stolen; B marks it lost, having seen A's records so far.
    let revoke = r.revoke(&b, a.public()).unwrap();
    let a_tip = r.records().iter().rfind(|x| x.record().signer == a.public()).unwrap().hash();
    assert!(matches!(revoke.record().body, Body::Revoke { upto: Some(h), .. } if h == a_tip));

    // The thief signs with A's key at an old position and at the tip, and
    // its own key then tries to throw B out and enrol a helper.
    let under = SignedRecord::sign(&a, P, Some(enrol_b.hash()), Body::Enrol { key: t.public(), label: "T".into(), guest: false });
    let past = SignedRecord::sign(&a, P, Some(a_tip), Body::Enrol { key: t.public(), label: "T".into(), guest: false });
    let oust = SignedRecord::sign(&t, P, None, Body::Revoke { key: b.public(), upto: None });
    let helper = SignedRecord::sign(&t, P, Some(oust.hash()), Body::Enrol { key: key(3).public(), label: "H".into(), guest: false });
    let lines: Vec<String> = [under, past, oust, helper].iter().map(SignedRecord::to_line).collect();
    r.merge_lines(lines.iter().map(String::as_str));

    let st = r.state();
    assert_eq!(st.standing(&b.public()), Standing::Member);
    assert_eq!(st.standing(&a.public()), Standing::Revoked);
    assert_eq!(st.standing(&t.public()), Standing::Unknown, "the thief's enrol is void");
    assert_eq!(st.standing(&key(3).public()), Standing::Unknown);
    assert!(st.contested.contains(&a.public()), "the equivocation is reported");
    assert!(st.attested["dvc_b"].contains(&b.public()));
    assert!(r.enrol(&b, key(4).public(), "E", false).is_ok(), "B still runs the project");
}

#[test]
fn two_records_on_one_prev_fail_closed() {
    let (a, b) = (key(0), key(1));
    let mut r = Roster::genesis(P, &a, "A");
    r.enrol(&a, b.public(), "B", false).unwrap();
    let before = r.append(&b, Body::Attest { key: b.public(), dvc: "dvc_b".into() }).unwrap();
    let left = SignedRecord::sign(&b, P, Some(before.hash()), Body::Enrol { key: key(2).public(), label: "C".into(), guest: false });
    let right = SignedRecord::sign(&b, P, Some(before.hash()), Body::Enrol { key: key(3).public(), label: "D".into(), guest: false });
    r.merge_lines([right.to_line().as_str(), left.to_line().as_str()]);

    let st = r.state();
    assert_eq!(st.standing(&b.public()), Standing::Revoked);
    assert_eq!(st.contested, BTreeSet::from([b.public()]));
    assert_eq!(st.revoked[&b.public()], Some(before.hash()), "B's records up to the fork stand");
    assert!(st.attested["dvc_b"].contains(&b.public()));
    assert_eq!(st.standing(&key(2).public()), Standing::Unknown, "both branches are void");
    assert_eq!(st.standing(&key(3).public()), Standing::Unknown);
    assert_eq!(st.standing(&a.public()), Standing::Member);
    assert!(r.enrol(&b, key(4).public(), "E", false).is_err(), "a forked key signs nothing more");
}

#[test]
fn a_fork_before_a_devices_enrol_takes_that_enrol_with_it() {
    let (a, b) = (key(0), key(1));
    let mut r = Roster::genesis(P, &a, "A");
    r.enrol(&a, b.public(), "B", false).unwrap();
    r.revoke(&b, a.public()).unwrap();
    // A second record with A's key at the root: nothing A signed after its
    // genesis can be attributed any more, B's enrol included.
    let root = SignedRecord::sign(&a, P, None, Body::Attest { key: a.public(), dvc: "dvc_x".into() });
    r.merge_lines([root.to_line().as_str()]);
    let st = r.state();
    assert_eq!(st.standing(&a.public()), Standing::Revoked);
    assert!(st.contested.contains(&a.public()));
    assert_eq!(st.standing(&b.public()), Standing::Unknown, "fails closed: B must be enrolled again by a member");
}

#[test]
fn guests_cannot_append_and_genesis_is_pinned() {
    let (a, g) = (key(0), key(1));
    let mut r = Roster::genesis(P, &a, "A");
    r.enrol(&a, g.public(), "guest", true).unwrap();
    assert_eq!(r.state().standing(&g.public()), Standing::Guest);
    assert!(r.enrol(&g, key(2).public(), "x", false).is_err());

    let other = Roster::genesis(P, &key(3), "rival");
    assert_eq!(r.merge_peer(&other.lines()).unwrap_err(), MergeError::ForeignGenesis);
    assert!(Roster::from_lines(P, std::iter::empty()).is_err(), "a roster needs its genesis");
}

#[test]
fn a_member_enrolled_as_a_guest_stays_a_member() {
    let (a, b) = (key(0), key(1));
    let mut r = Roster::genesis(P, &a, "A");
    r.enrol(&a, b.public(), "B", false).unwrap();
    r.enrol(&a, b.public(), "B", true).unwrap();
    assert_eq!(r.state().standing(&b.public()), Standing::Member, "revoke is the only way down");
}

#[test]
fn merge_peer_keeps_only_records_of_enrolled_members() {
    let (a, b, g) = (key(0), key(1), key(2));
    let mut ours = Roster::genesis(P, &a, "A");
    ours.enrol(&a, g.public(), "visitor", true).unwrap();
    let mut theirs = ours.clone();
    let legit = theirs.enrol(&a, b.public(), "B", false).unwrap();
    let by_b = theirs.append(&b, Body::Attest { key: b.public(), dvc: "dvc_b".into() }).unwrap();
    let mut lines = theirs.lines();
    lines.push(SignedRecord::sign(&g, P, None, Body::Attest { key: g.public(), dvc: "dvc_g".into() }).to_line());
    for i in 0..50u8 {
        let stranger = DeviceKey::from_seed([100 + i; 32]);
        lines.push(SignedRecord::sign(&stranger, P, None, Body::Enrol { key: stranger.public(), label: "x".into(), guest: false }).to_line());
    }

    let report = ours.merge_peer(&lines).unwrap();
    assert_eq!(report.added, vec![legit, by_b], "B's enrol and B's own record, signed once B is a member");
    assert_eq!(report.dropped, 51, "the guest's and the strangers' records are not stored");
    assert_eq!(ours.records().len(), 4);
    assert_eq!(ours.state().standing(&b.public()), Standing::Member);
}

#[test]
fn an_oversized_peer_roster_is_refused_whole() {
    let a = key(0);
    let mut ours = Roster::genesis(P, &a, "A");
    let mut lines = vec![ours.genesis_line(); MAX_PEER_RECORDS];
    assert!(ours.merge_peer(&lines).is_ok(), "the limit itself is accepted");
    lines.push(ours.genesis_line());
    assert_eq!(ours.merge_peer(&lines).unwrap_err(), MergeError::TooManyRecords { records: MAX_PEER_RECORDS + 1, limit: MAX_PEER_RECORDS });
}

#[test]
fn tampered_lines_are_rejected() {
    let a = key(0);
    let mut r = Roster::genesis(P, &a, "A");
    let rec = r.enrol(&a, key(1).public(), "B", false).unwrap();
    let forged = rec.to_line().replace("\\\"B\\\"", "\\\"Mallory\\\"");
    assert_ne!(forged, rec.to_line());
    assert!(SignedRecord::from_line(&forged).is_err());
    let foreign = SignedRecord::sign(&a, "prj_other", None, Body::Revoke { key: key(1).public(), upto: None });
    let report = r.merge_lines([forged.as_str(), foreign.to_line().as_str()]);
    assert_eq!((report.added.len(), report.rejected), (0, 2));
}

#[test]
fn roster_file_survives_a_torn_tail() {
    let dir = std::env::temp_dir().join(format!("wugs-auth-test-{}", wugs_model::ulid()));
    let f = RosterFile::in_project(&dir);
    let a = key(0);
    let mut r = Roster::genesis(P, &a, "A");
    f.create(&r).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(f.path())
        .and_then(|mut h| std::io::Write::write_all(&mut h, b"{\"rec\":\"{\\\"v\\\":2"))
        .unwrap();
    let rec = r.enrol(&a, key(1).public(), "B", false).unwrap();
    f.append(&[rec]).unwrap();
    let loaded = f.load(P).unwrap();
    assert_eq!(loaded.state(), r.state());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn mutual_revokes_fail_closed() {
    let (a, b) = (key(0), key(1));
    let mut r = Roster::genesis(P, &a, "A");
    r.enrol(&a, b.public(), "B", false).unwrap();
    // Each revokes the other before seeing the other's revoke: each voids
    // the record that would void it.
    let mut at_a = r.clone();
    at_a.revoke(&a, b.public()).unwrap();
    let mut at_b = r.clone();
    at_b.revoke(&b, a.public()).unwrap();
    at_a.merge_lines(at_b.lines().iter().map(String::as_str));
    let st = at_a.state();
    assert_eq!(st.standing(&a.public()), Standing::Revoked);
    assert_eq!(st.standing(&b.public()), Standing::Revoked);
    assert_eq!(st.contested, BTreeSet::from([a.public(), b.public()]));

    let mut r = Roster::genesis(P, &key(1), "B");
    r.revoke(&key(1), key(1).public()).unwrap();
    let st = r.state();
    assert_eq!(st.standing(&key(1).public()), Standing::Revoked, "the API's self-revoke is not a paradox");
    assert!(st.contested.is_empty());
}

#[test]
fn a_keys_newest_attest_supersedes_its_older_ones() {
    let (a, b) = (key(0), key(1));
    let mut r = Roster::genesis(P, &a, "A");
    r.enrol(&a, b.public(), "B", false).unwrap();
    r.append(&a, Body::Attest { key: a.public(), dvc: "dvc_1".into() }).unwrap();
    // B's folder was copied from A's: it claims A's id too.
    r.append(&b, Body::Attest { key: b.public(), dvc: "dvc_1".into() }).unwrap();
    assert_eq!(r.state().dvc_claimed_by_other("dvc_1", &a.public()), Some(b.public()));

    r.append(&b, Body::Attest { key: b.public(), dvc: "dvc_2".into() }).unwrap();
    let st = r.state();
    assert_eq!(st.attested["dvc_1"], BTreeSet::from([a.public()]), "B moved on to dvc_2");
    assert_eq!(st.attested["dvc_2"], BTreeSet::from([b.public()]));
    assert_eq!(st.dvc_claimed_by_other("dvc_1", &a.public()), None, "A's warning clears");
    assert_eq!(st.dvc_claimed_by_other("dvc_2", &b.public()), None);
}

#[test]
fn an_attest_past_a_revoke_cutoff_leaves_the_previous_one_current() {
    let (a, b) = (key(0), key(1));
    let mut r = Roster::genesis(P, &a, "A");
    r.enrol(&a, b.public(), "B", false).unwrap();
    r.append(&b, Body::Attest { key: b.public(), dvc: "dvc_1".into() }).unwrap();
    // A marks B lost having seen B's first attest; B, elsewhere, attests again.
    let mut at_a = r.clone();
    at_a.revoke(&a, b.public()).unwrap();
    let mut at_b = r.clone();
    at_b.append(&b, Body::Attest { key: b.public(), dvc: "dvc_2".into() }).unwrap();
    at_a.merge_lines(at_b.lines().iter().map(String::as_str));

    let st = at_a.state();
    assert_eq!(st.standing(&b.public()), Standing::Revoked);
    assert_eq!(st.attested.get("dvc_1"), Some(&BTreeSet::from([b.public()])), "the attest inside the cutoff stays current");
    assert_eq!(st.attested.get("dvc_2"), None, "the attest past the cutoff is void");
}
