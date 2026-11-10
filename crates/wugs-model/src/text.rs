//! The seam between the zero-dependency core and a CRDT text engine
//! (DATA-MODEL §2.3). The core never sees engine types; an engine never
//! sees the projection. `wugs-text` implements this with Loro.

/// Merges the updates of one collaborative text field. Stateless: callers
/// pass every update the field has, in HLC order, and the engine rebuilds.
/// Updates are opaque bytes (base64 in the op payload and the projection).
pub trait TextMerger {
    /// Plain text after importing every update into a fresh document.
    /// An update whose causal dependencies are missing from the set is
    /// ignored until they arrive; corrupt bytes are an error.
    fn resolve(&self, updates: &[Vec<u8>]) -> Result<String, String>;

    /// One update that changes the text of the document built from
    /// `updates` to `new_text`, as `peer`. When `updates` is empty and the
    /// field already holds plain text (it predates collaborative editing),
    /// `prior` is that text plus a genesis peer id derived from the op that
    /// wrote it, and the update first seeds the text as a deterministic
    /// genesis edit by that peer. Devices seeding from the same op generate
    /// identical genesis ops, which dedupe on merge; devices whose priors
    /// came from different ops get distinct genesis peers, so the merge
    /// keeps both texts visible and still converges, instead of colliding
    /// on op ids and splitting the replicas. `peer` never equals the
    /// genesis peer. Returns the update and the resulting text.
    fn edit(
        &self,
        updates: &[Vec<u8>],
        prior: Option<(&str, u64)>,
        new_text: &str,
        peer: u64,
    ) -> Result<(Vec<u8>, String), String>;
}

/// Unicode normalization seam (PRD §7: NFC at entry, or search silently
/// breaks). The engine stays zero-dependency; `wugs-unicode` implements
/// this with real Unicode tables. Without one installed, text is stored as
/// typed — replay never re-normalizes either way, because ops are the truth
/// as written.
pub trait Normalizer {
    /// The NFC form of `s`.
    fn nfc(&self, s: &str) -> String;
}
