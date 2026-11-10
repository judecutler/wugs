//! Loro-backed collaborative text for wugs (DATA-MODEL §2.3, §5.4). An edge
//! crate: the zero-dependency core only sees the `TextMerger` trait.

use loro::{ExportMode, LoroDoc, UpdateOptions};
use wugs_model::TextMerger;

const CONTAINER: &str = "t";

pub struct LoroMerger;

fn doc_from(updates: &[Vec<u8>]) -> Result<LoroDoc, String> {
    let doc = LoroDoc::new();
    // Already off by default in loro 1.17; stated so a default change can't
    // put wall-clock time into the genesis change.
    doc.set_record_timestamp(false);
    doc.import_batch(updates).map_err(|e| e.to_string())?;
    Ok(doc)
}

impl TextMerger for LoroMerger {
    fn resolve(&self, updates: &[Vec<u8>]) -> Result<String, String> {
        Ok(doc_from(updates)?.get_text(CONTAINER).to_string())
    }

    fn edit(
        &self,
        updates: &[Vec<u8>],
        prior: Option<(&str, u64)>,
        new_text: &str,
        peer: u64,
    ) -> Result<(Vec<u8>, String), String> {
        let doc = doc_from(updates)?;
        let from = doc.oplog_vv();
        let text = doc.get_text(CONTAINER);
        if let Some((prior, genesis_peer)) = prior.filter(|_| updates.is_empty()) {
            if peer == genesis_peer {
                return Err("editing peer equals the genesis peer".into());
            }
            doc.set_peer_id(genesis_peer).map_err(|e| e.to_string())?;
            text.update(prior, UpdateOptions::default()).map_err(|e| e.to_string())?;
            doc.commit();
        }
        doc.set_peer_id(peer).map_err(|e| e.to_string())?;
        text.update(new_text, UpdateOptions::default()).map_err(|e| e.to_string())?;
        doc.commit();
        let update = doc.export(ExportMode::updates(&from)).map_err(|e| e.to_string())?;
        Ok((update, text.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_is_byte_identical_across_peers() {
        let (a, _) = LoroMerger.edit(&[], Some(("köpek geldi", 7)), "köpek geldi", 11).unwrap();
        let (b, _) = LoroMerger.edit(&[], Some(("köpek geldi", 7)), "köpek geldi", 22).unwrap();
        assert_eq!(a, b, "a genesis-only update does not depend on the editing peer");
    }

    #[test]
    fn independent_genesis_dedupes_on_merge() {
        let (a, ta) = LoroMerger.edit(&[], Some(("köpek geldi", 7)), "büyük köpek geldi", 11).unwrap();
        let (b, tb) = LoroMerger.edit(&[], Some(("köpek geldi", 7)), "köpek geldi dün", 22).unwrap();
        assert_eq!(ta, "büyük köpek geldi");
        assert_eq!(tb, "köpek geldi dün");
        let ab = LoroMerger.resolve(&[a.clone(), b.clone()]).unwrap();
        let ba = LoroMerger.resolve(&[b, a]).unwrap();
        assert_eq!(ab, "büyük köpek geldi dün", "both edits survive and the prior appears once");
        assert_eq!(ab, ba, "import order does not change the merge");
    }

    #[test]
    fn divergent_priors_converge_with_both_texts_visible() {
        // Two devices whose plain priors came from different ops get
        // distinct genesis peers: the merge must keep both, not error.
        let (a, _) = LoroMerger.edit(&[], Some(("köpek geldi", 7)), "büyük köpek geldi", 11).unwrap();
        let (b, _) = LoroMerger.edit(&[], Some(("kedi gitti", 8)), "kedi gitti dün", 22).unwrap();
        let ab = LoroMerger.resolve(&[a.clone(), b.clone()]).unwrap();
        let ba = LoroMerger.resolve(&[b, a]).unwrap();
        assert_eq!(ab, ba, "convergent whichever arrives first");
        for piece in ["büyük köpek geldi", "kedi gitti dün"] {
            assert!(ab.contains(piece), "{piece:?} survives in {ab:?}");
        }
    }

    #[test]
    fn edit_without_prior_has_no_genesis() {
        let (u, t) = LoroMerger.edit(&[], None, "wug", 5).unwrap();
        assert_eq!(t, "wug");
        assert_eq!(LoroMerger.resolve(&[u]).unwrap(), "wug");
    }

    #[test]
    fn update_with_missing_deps_is_not_reflected() {
        let (u1, _) = LoroMerger.edit(&[], None, "a", 5).unwrap();
        let (u2, _) = LoroMerger.edit(std::slice::from_ref(&u1), None, "ab", 5).unwrap();
        assert_eq!(LoroMerger.resolve(std::slice::from_ref(&u2)).unwrap(), "", "u2 waits for u1");
        assert_eq!(LoroMerger.resolve(&[u2, u1]).unwrap(), "ab");
    }

    #[test]
    fn corrupt_bytes_are_an_error() {
        let (good, _) = LoroMerger.edit(&[], None, "a", 5).unwrap();
        assert!(LoroMerger.resolve(&[b"not a loro update".to_vec()]).is_err());
        assert!(LoroMerger.resolve(&[good, b"garbage".to_vec()]).is_err());
        assert!(LoroMerger.edit(&[b"garbage".to_vec()], None, "x", 5).is_err());
    }

    #[test]
    fn editing_peer_equal_to_genesis_peer_is_refused() {
        assert!(LoroMerger.edit(&[], Some(("p", 9)), "x", 9).is_err());
    }
}
