# Data Model & Op Log Specification (v0.1)

*Implements the architecture decided in `DECISIONS.md`. Scope: the platform core shared by all kits; Tongues-specific usage noted where relevant. Status: ready for prototype gates; expect revision after the FLEx import test.*

---

## 1. Identifiers, clocks, ordering

- **Entity IDs:** ULIDs with a type prefix: `src_01J…`, `seg_01J…`. Prefixes: `prj src trk txt tir seg tok mor lex sen frm dom sdm cpl cpt scn ann prs cns ses ws lng grp gmb fld dvc`. IDs are permanent and never reused; an imported entity keeps a stable mapping to its original ID (`origin` field) so re-imports don't duplicate. Where the source itself has a permanent identifier that every device sees (the GUIDs of SIL Semantic Domains), the id is derived from it (`dom_` + the GUID's 128 bits in ULID form; concept lists: a 128-bit FNV-1a hash of the list source; concepts: of the list source and the row id together), so two devices that load the same list create the same entities instead of two copies.
- **Hybrid logical clock (HLC):** `(wallMillis, counter, deviceId)`, encoded as one lexicographically sortable string, e.g. `018f3c2a7b40-0003-dvc_01J…`. Max tolerated wall-clock drift: 24 h (field laptops have bad clocks; reject beyond that with a "fix your clock" prompt, don't corrupt ordering). On ingest, a txn carrying an HLC more than 24 h ahead of the receiving device's wall clock is held in quarantine before any of it applies (reason `clock drift: …`), and is retried like any held txn, so it applies once the receiver's clock is within range.
- **Total order** = HLC order; ties impossible (deviceId breaks them).
- **List positions** (segments in a text, tokens in a segment, morphs in a token, senses in an entry): **fractional index strings** (base-62, à la Figma). Reordering = one `set` of `pos`. Concurrent inserts at the same spot order by HLC; duplicates impossible.

## 2. The op log

### 2.1 Storage layout
```
project/
  project.json            # project id, schema version, format spec version
  log/<scope>/<device>.jsonl   # append-only; each device writes ONLY its own file
  blobs/<blake3-hash>          # media, keyboard defs, consent scans (never in log)
  cache/                       # SQLite projection, waveform peaks, proxies (disposable)
  recordings-pending/          # local, not archival, never sealed or synced: in-progress or unsaved takes (durable, never auto-deleted)
```
Each device appends only to its own per-scope file. Sync = exchange missing suffixes per `(scope, device)`, verified by the merkle tree (§6). This makes sync idempotent, resumable after power loss, and trivially crash-safe (an interrupted append truncates to the last complete line).

### 2.2 Op envelope (one JSON object per line)
```json
{
  "id": "op_01JD2…",
  "hlc": "018f3c2a7b40-0003-dvc_01JA…",
  "txn": "txn_01JD2…",
  "intent": "split-morph",
  "actor": "prs_01H…",
  "device": "dvc_01JA…",
  "session": "ses_01JC…",
  "scope": "txt_01JB…",
  "schema": 2,
  "op": "set",
  "entity": "morph",
  "target": "mor_01JD…",
  "field": "gloss",
  "value": "PL",
  "base": "018f3c1e2210-0001-dvc_01JX…"
}
```
- **`txn` + `intent`:** the write API (Rust core) exposes *domain intents* (`split-morph`, `merge-entries`, `retime-segment`…); each intent emits 1–n primitive ops sharing a `txn`. The log stores primitives (simple to replay forever); the intent label preserves human-readable provenance. Txns apply atomically — quarantine holds or commits whole txns.
- **`base`:** the HLC of the field value this write overwrote: the winner the writer saw (null if unset).
- **`heads`** (optional array of HLCs): the open versions of the field this write supersedes, like the parents of a merge commit. Absent means `[base]`, or no versions when `base` is null, so ops written before `heads` existed read unchanged, and a writer leaves it out whenever it would equal `[base]`. A write supersedes exactly the versions it names; a version no write names stays open. This is the concurrency detector (§5). A name may refer to an op not yet received; it is recorded, and that op is already superseded when it arrives. **Who names what:** a person's direct edit or pick of a field (typing a gloss, picking a sense, choosing an analysis, retiming a segment, retyping a word's form, editing a lexicon entry or sense) names every open version it was shown, so it settles the field's conflicts; every other write (bulk actions such as apply-to-unanalysed, derived writes such as a retokenize's token list, importers) names only `[base]`, so it never closes a conflict nobody looked at. In `wugs-core` this is `Txn::resolving()` for a whole txn and `Txn::set_resolving` for one field. A field with no resolving writer yet (a text's title, tier fields) keeps its conflicts until the general conflict prompt (roadmap item 24) writes one.
- **Opening a project** folds every HLC in its own log into the clock without the drift check: those ops are already the record, so a clock set back later must not stop the project from opening.
- **`schema`:** 2 since ops gained `heads` (2026-11-09). Schema-1 ops were written when conflicts were detected by comparing actors, so two schema-1 writes by the same actor are never a conflict (§5.3); from schema 2 on, actors play no part.
- **`session`:** optional elicitation-session link — provenance (speaker, place, method) rides on every op made during that session.

### 2.3 Op verbs (closed set)
| Verb | Meaning |
| --- | --- |
| `create` | new entity; `value` = initial field map |
| `set` | one field; per-field LWW by HLC |
| `delete` / `restore` | tombstone flag only — never removes data |
| `text-edit` | collaborative text field; `value` = `{ loro: <base64 update>, resolved: "<full plain text>" }`. `resolved` is authoritative for projection, search and archive; `loro` exists only to merge concurrent edits. Missing/corrupt `loro` ⇒ degrade to LWW on `resolved`. The update is self-contained back to the version the editor had applied. **Cold start:** when a field that already holds plain text (a `set` value, e.g. from an importer) gets its first `text-edit`, the editor first seeds that text as a deterministic *genesis* edit — CRDT peer = a 64-bit hash of the id of the op that wrote the prior value, no wall-clock timestamps — then applies its own edit as its real peer (a 64-bit hash of the device id). Two devices seeding from the same op generate identical genesis ops, which dedupe on merge instead of duplicating the text; devices whose priors came from different ops get distinct genesis peers, so even then the replicas converge, keeping both texts visible. |

There is no `link` verb: **every many-to-many relationship is itself an entity** (§3), so `create`/`delete` cover linking and per-field LWW stays sufficient everywhere.

## 3. Entities

Field types: `str`, `int`, `real`, `bool`, `ms` (milliseconds), `ref:<kind>`, `enum(...)`, `pos` (fractional index), `text` (collaborative), `blob` (hash ref), `json` (small config, LWW as a unit).

### Media & texts
| Entity | Fields |
| --- | --- |
| **Source** `src` | kind `enum(audio,video,image,document)` · label `str` · blob `blob?` (whole-file scan or document; A/V attaches via MediaTrack) · date, place `str` · access `enum(open,restricted,community)` · duration `ms` · origin `str` · notes `text` |
| **MediaTrack** `trk` | source `ref` · blob `blob` · kind `enum(audio,video)` · label `str` · channel `int` |
| **SpeakerRole** `grp`-style link | source `ref` · person `ref` · role `enum(speaker,signer,addressee,researcher,…)` |
| **Text** `txt` | source `ref?` (null for born-written) · title `str` · language `ref` · ws `ref` · kind `enum(narrative,conversation,elicitation,song,written,…)` · access (inherits source if linked) |
| **TierDef** `tir` | project-scoped · name `str` · kind `enum(time-aligned, symbolic-subdivision, symbolic-association)` · parent `ref:tir?` · participant-specific `bool` · role `str?` · language `ref?` — the ELAN tier model. `kind` is the ELAN stereotype; `role` says what the tier is for. Recommended values, aligned with FLEx line names: `transcription`, `translation`, `literal-translation`, `note`; kits may add their own. Core checks only that role and language are non-empty when present and never interprets the vocabulary. `language` lets translation tiers in two languages sit side by side. Glosses are not a tier role: they live on morphs |
| **Segment** `seg` | text `ref` · tier `ref` · pos `pos` · start,end `ms?` · parent `ref:seg?` · participant `ref:prs?` · content `text` · tokens `[ref:tok]?` (ordered membership: a JSON array of token ids, one atomic field, §5.5) |
| **Token** `tok` | segment `ref` · pos `pos` · form `str` · ws `ref?` · morphs `[ref:mor]?` (ordered membership, as `Segment.tokens`) |
| **Morph** `mor` | token `ref` · pos `pos` · surface `str` · type `enum(root,prefix,suffix,infix,clitic,reduplication,…)` · form `ref:frm?` · sense `ref:sen?` · gloss `str` (primary analysis language) · `gloss:<bcp47>` `str` (one sibling field per further language, e.g. `gloss:es`, so per-field LWW merges each language independently). Zero morphs use surface `∅`. Analyses live on each occurrence; reuse is derived: the projection suggests the earlier analyses of a form, ranked by use, and accepting one writes ordinary ops. `retokenize`, `tokenize` and `apply-analysis` diff against the current members in one txn: matched tokens and morphs keep their ids and every field the edit does not name (`form` ref, `x.*`), changed fields are `set`, new entities are created, and the parent's list is written once. Nothing is tombstoned: an entity left out of the winning list is hidden and recoverable (`docs/design/GLOSSING-MODEL.md`, "Merge semantics") |
| **Annotation** `ann` | target `ref:any` · kind `str` · body `text` — standoff notes/comments on anything |

### Lexicon
| Entity | Fields |
| --- | --- |
| **Lexeme** `lex` | lemma `str` · citation `str` · ws `ref` · language `ref` · pos-of-speech `str` · homograph `int?` |
| **Sense** `sen` | lexeme `ref` · pos `pos` · gloss `str` (primary analysis language) · `gloss:<bcp47>` `str` (further languages, as on Morph) · definition `text` · usage `text` |
| **Form** `frm` | lexeme `ref` · kind `enum(allomorph,variant,inflected)` · form `str` · environment `str` · ws `ref` |
| **SemanticDomain** `dom` (kind `domain`, project-config scope) | parent `ref:dom?` · pos `pos` · label `str` · code `str` · description `str?` · origin `str?` (e.g. `semdom:v4:<GUID>`) (tree via parent+pos; acyclicity is a validator, §7) |
| **SenseDomain** `sdm` (kind `sense-domain`, lexicon scope) | sense `ref` · domain `ref` — one live link per pair; unassigning tombstones every live link of the pair |

### Elicitation (`docs/design/ELICITATION.md`)
| Entity | Fields |
| --- | --- |
| **ConceptList** `cpl` (kind `concept-list`, project-config scope) | name `str` · source `str` (`concepticon:<list id>`) · citation `str` · origin `str` |
| **Concept** `cpt` (kind `concept`, project-config scope) | list `ref:cpl` · pos `pos` · number `int` · gloss `str` · concepticon `str?` · concepticon-gloss `str?` · skipped `bool?` · skip-note `str?` · origin `str` |
| **SenseConcept** `scn` (kind `sense-concept`, lexicon scope) | sense `ref` · concept `ref` — a concept is answered while a live sense of a live lexeme links to it |

### People, ethics, context
| Entity | Fields |
| --- | --- |
| **Person** `prs` | names `json` · roles `json` · default-access `enum` |
| **Consent** `cns` | person `ref` · covers `ref:src|prj` · terms `json` (archive-ok, web-ok, community-only, AI-ok…) · date · evidence `blob?` |
| **Session** `ses` | date, place `str` · method `str` · participants via link entities · notes `text` |

### Systems & relations
| Entity | Fields |
| --- | --- |
| **WritingSystem** `ws` | name · script `str` · normalization (always NFC) · charset `str` (validator input) · sort `str` (alphabet units in order, whitespace-separated, variants joined by `/` — e.g. `a/A b/B ch/Ch/CH`; parsed by `wugs_model::sortkey::sort_units`; ICU tailorings are the future edge upgrade) · keyboard `blob?` · fonts `json` |
| **Language** `lng` | name · glottocode · iso639-3 |
| **Group** `grp` + **GroupMember** `gmb` | kind `enum(cognate-set, parallel-passage, …)` · member: group `ref` · target `ref:any` · role `str` — generic sets for Historical/Corpus kits |
| **FieldDef** `fld` | extension fields: entity-kind · name (namespaced `x.<kit>.<name>`) · type · origin kit — kits extend the model without touching core; unknown fields are preserved verbatim on replay (forward compatibility) |

## 4. Scopes & access control

A **scope** is the sync/ACL partition an op belongs to:
- Each **Source** (with its tracks, texts, segments…) is one scope.
- The **lexicon**, **project config** (tiers, writing systems, domains) and **people/consent** are three project-level scopes; people/consent is the most restricted.
- Sync filters by scope *before transmission*: a `restricted` or `community` source's ops are never sent to an unauthorized peer. Enforcement lives in the sync layer, not the UI.
- **The default policy is an allow-list derived from the data** (`policy_from_access`): the lexicon, the project config, and each live source or text whose effective access is `open`. Everything else fails closed — deleted, held, unknown or newly created scopes. A text with no access of its own inherits its linked source's. Rebuild the policy per sync session; it is a snapshot.
- **Access ops travel in the project-config scope**, never in the scope they gate, so a peer that received a text while it was open still learns it became restricted and stops re-sharing it. Copies already sent cannot be recalled. The config scope itself (tiers, writing systems, access levels) is always shared by design.
- **Decided (owner, 2026-11-08): unset access means `open`.** A text or source with no access level and no linked source syncs to enrolled members and appears to guests; simple sharing wins over fail-closed. Access levels remain opt-in machinery for teams that want them (see `DECISIONS.md`, "Sync sharing: simple pairing, flat trust, guest mode").
- Cloud AI (opt-in per project, see `SUITE-CONTEXT.md`) additionally requires `AI-ok` consent terms on the covered sources.

## 5. Merge, concurrency, conflicts

1. Apply ops in HLC order; per-field LWW.
2. An op whose `target` doesn't exist yet (its `create` lives in a log not yet received) parks as `pending-dep` and applies when the dependency arrives.
3. **Concurrency detection:** a field's *open versions* are its LWW winner plus every write that no held write names in its `heads` (§2.2). The set depends only on which writes a replica holds, never on their arrival order or on who wrote them, so one person on two devices is detected like two people. Each open version other than the winner is a **conflict** with the winner, unless both are `text-edit`s whose Loro updates merged (rule 4), or both hold the same value (each write's own value: a text-edit's `resolved`), since there is nothing to choose between (two devices that load one reference list with derived ids create equal entities). The equal version stays open, though: a later *plain* write that differs from it pairs with it (over-reporting, never hiding); a resolving edit closes it. The LWW result applies provisionally **and** the conflict row stands → UI prompt: *"Conflicting edits on this term. Choose version or edit manually."* **Resolution** is a person's direct write of the field made on a device that holds the conflict (§2.2): it names every open version, so the conflict closes on every replica that receives it. A bulk, derived or imported write names only the winner it overwrites and leaves the conflict open. Keeping the winner, choosing the loser, or typing a third value are all one `set`; the losing values stay in the log. A replica keeps a conflict row only while the conflict is open; whether a pair was ever open depends on arrival order, so closed conflicts are not recorded. A schema-1 write is never paired once its own actor wrote the field again under schema 1 (whatever the field holds after that, schema-2 writes included): under the rule those writes were made with, one person's own writes did not conflict, and their writers named only `base`, so pairing them now would flag history nobody can act on. Whether an actor wrote again is a function of the writes held, so this too is order-independent. **Known limit, transient over-reporting:** a replica holding a write and a later write that superseded it only through an intermediate write it has not received yet shows both as open until the intermediate arrives (it over-reports, never hides). The over-report is permanent only if that intermediate write stays held in quarantine forever. Ingest reports and apply statistics count conflict rows *newly opened* by the ops they applied; the `conflicts` table holds the rows open now.
4. `text-edit` fields merge via Loro instead of LWW, and two merged text-edits are never a conflict, however concurrent. The engine is behind a `TextMerger` seam so the core stays zero-dependency (crate `wugs-text` holds the Loro dependency); a replica with no engine installed, or an op whose `loro` payload is corrupt or missing, takes the degradation path: LWW on `resolved`, and conflicts as a `set` would. A degraded text-edit that wins by HLC keeps its own `resolved` value: an older merged edit arriving later updates the stored Loro lineage but never overwrites it. If a field's stored updates stop merging as a set (colliding op ids after a crash), they never merge again, because the set only grows: from then on the field's text-edits conflict like sets, and its value is the HLC winner's own (`resolved` for a text-edit), whatever merged text an earlier arrival order showed. A local edit of such a field cannot be computed against its Loro document, so the writer sends the plain text on the degradation path instead; it still names the open versions, so it settles the conflicts. A `set` and a `text-edit` that are both open conflict. The field's LWW bookkeeping keeps the newest op's HLC either way.
5. **Membership lists are atomic LWW fields with conflict detection (rule 3); member fields still merge per field.** `Segment.tokens` and `Token.morphs` hold the ordered ids of a parent's members. A parent shows exactly the live children its winning list names, in order; one without a list shows its live children by `pos`. Concurrent different lists conflict on that one field (rule 3), so a re-analysis on one device and a re-analysis on another never interleave. An entity a list leaves out is hidden, not tombstoned. A non-empty edit on a hidden entity (a `set`, or a gloss or sense that came with its `create`) is reported as a hidden edit with a restore action, unless the device that made the edit later removed the entity itself. The remover is the list write, found by walking back from the current list through each write's `base`, whose base list still named the entity. All of this is derived from state, so it is replay-deterministic. A conflict on a list offers the losing list whole ("use the other analysis"); choosing it writes that list naming both versions, which closes the conflict.

## 6. Sync pipeline (quarantine)

```
receive batch ──► verify per-device log continuity + merkle diff
      ──► stage txns in quarantine
      ──► replay onto scratch overlay of the projection
      ──► run validators (§7) on affected entities
      ──► pass: append to local log copy + apply to projection
      ──► fail: hold txn, surface reason, offer choose/edit
```
- **Merkle tree per scope** over the op-ids a device *has* — applied or held — bucketed by HLC time windows (Actual Budget's scheme): two devices compare one root per scope, then group hashes (~4.7 h each), then window hashes (~65 s each), and transfer whole txns only for differing windows. Counting held ops means roots converge while a txn sits in quarantine, so peers stop re-offering it; the holder retries it locally whenever new ops arrive (its fix parks as pending-dep and rides the retried txn's savepoint). Detects corruption as well as missing data.
- **Wire protocol v1** (`wugs-sync::wire`, transport-agnostic): `hello` (refuses a different project id; a second device joins via an adopted project id) → `get_roots` → `get_groups` → `get_windows` → `get_ops`, one pull per direction over one connection. The sender-side scope policy is applied to every message, so a request for a denied scope reveals nothing at any level. Transport (`wugs-net`): length-prefixed JSON frames over TCP, peers found via mDNS (`_wugs._tcp`), announced per project id. **No authentication in v1**: same-room trust plus the scope ACL; a pairing code is required before any real deployment.
- Local edits take the same path with validators in warn-only mode (never block the researcher mid-elicitation; flag instead).

## 7. Validators

Configurable per project and kit; each is `warn | hold` (hold applies to incoming sync only).

| # | Check | Default |
| --- | --- | --- |
| 1 | Morph surfaces concatenate to the token form (ignoring `- = ~ < >` markers and zero morphs `∅`; exact case, compared after NFC) | warn |
| 2 | Morph count = gloss count per token | warn |
| 3 | No overlapping intervals within one time-aligned tier | hold |
| 4 | Referenced IDs exist and aren't tombstoned | hold (pending-dep aware) |
| 5 | Characters ∈ the writing system's charset | warn |
| 6 | Semantic-domain tree stays acyclic | hold |
| 7 | Segment intervals within source duration | warn |
| 8 | Duplicate lemma without homograph number | warn |

## 8. SQLite projection (cache/, disposable)

One table per entity kind mirroring §3, plus:
- `ops_applied(op_id, hlc, scope)` — high-water marks per (scope, device)
- `field_state(target, field, hlc, op_id)` — LWW bookkeeping
- `field_refs(target, field, hlc)` — every HLC any write named in `heads`/`base`; `field_heads(target, field, hlc, op_id)` — writes no write names, besides the winner, which is always open (§5.3)
- `conflicts(target, field, winner_op, loser_op)` — one row per open conflict, rewritten from the heads whenever they move
- `text_unmerged(target, field)` — fields whose stored Loro updates do not merge as a set (§5.4); `text_updates` keeps each update's `resolved` text so such a field shows its winner's own
- `legacy_writes(target, field, actor, hlc)` — each actor's newest schema-1 write of each field (§5.3)
- `quarantine(txn, status, reason)`
- `search_fts` — FTS5 over forms, glosses, definitions, segment text (tokenizer: `unicode61` + per-WS custom when sort/charset demands)
- `lookup_keys(target, kind, key)` — a folded key for token forms, lexeme lemmas and allomorph forms (NFC when a normalizer is installed, case-folded with Turkic dotted and dotless i treated as one letter, edge morph markers `- = ~ < >` dropped for lexicon entries), maintained by the projector; one index range scan answers "tokens with form X" and "lexemes for surface Y", which is what analysis suggestions and lexicon lookup run on. A cache built with a different normalizer choice is wiped and replayed
- `members(parent, parent_ref, child, ord)` — a view: a segment's visible tokens and a token's visible morphs in order (§5.5), the one place the membership rule lives; `hidden_edits(segment)` is derived from it
- `text_updates(target, field, hlc, op_id, update)` — the loro payloads seen per collaborative field, in HLC order; the merged text is recomputed from them on each apply and stored as the field value
- Rebuild command: delete `cache/`, replay logs. Rebuilding is the *only* repair path — no manual SQLite surgery, ever.

*(v0 implementation note: the current code uses a generic `entities` + `fields` projection; typed tables and FTS arrive with the first real tool. The projection is disposable, so this carries no migration cost.)*

## 9. Blob store

- Files under `blobs/` keyed by BLAKE3 hash; written once, never modified (sources immutable — annotation-graph heritage).
- Ops and entities reference hashes only, as the string **`blake3:<64 lowercase hex>`**. The ref format is part of the archive format (it appears in sealed logs), so a future hash change means a new prefix, never a reinterpretation.
- Writes are temp-file + rename into `blobs/<hex>`, so a crash never leaves a half-written blob under its final name; a duplicate put leaves the stored bytes untouched (it only refreshes the file's mtime, which shields a just-re-imported blob from a concurrently running sweep).
- Garbage collection = mark-and-sweep with a grace period. **The mark comes from the op log** (`Project::blob_refs`), never from the projection alone: every ref that ever appeared in any log line (torn tails included, scanned as raw text), in a quarantine spill file, or in a parked or held op. A ref the log holds is live forever — tombstoned and overwritten refs included, since the log and sealed bags name them for good — so the sweep only ever reclaims blobs no op ever referenced (aborted imports), and only when they are older than the grace period. Marking from the projection was rejected because the projection can trail the log after a crash, which would have made GC delete referenced media.
- Derived artifacts (waveform peaks, video proxies) live in `cache/`, keyed by `(blob, params)`.

## 10. Schema evolution

- `schema` int on every op; core schema defined in Rust, serialized as JSON Schema into every sealed package (self-describing archives).
- Changes are **reinterpretations at replay** (lens functions old→new), never log rewrites. The log is immutable, forever.
- Kit extension fields via `FieldDef` + `x.<kit>.<name>` namespacing; core replay preserves unknown namespaced fields untouched.

## 11. Worked example — glossing "köpek-ler" (dog-PL)

```json
{"op":"create","entity":"token","target":"tok_A","txn":"txn_1","intent":"tokenize","value":{"segment":"seg_X","pos":"a2","form":"köpekler"}, …}
{"op":"create","entity":"morph","target":"mor_1","txn":"txn_2","intent":"split-morph","value":{"token":"tok_A","pos":"a0","surface":"köpek","type":"root"}, …}
{"op":"create","entity":"morph","target":"mor_2","txn":"txn_2","value":{"token":"tok_A","pos":"a1","surface":"ler","type":"suffix"}, …}
{"op":"set","entity":"morph","target":"mor_1","field":"sense","value":"sen_dog","base":null,"txn":"txn_3","intent":"link-sense", …}
{"op":"set","entity":"morph","target":"mor_2","field":"gloss","value":"PL","base":null,"txn":"txn_3", …}
```
Validator 1 passes (`köpek`+`ler` = `köpekler`); validator 2 passes after txn_3. A peer concurrently setting `mor_2.gloss = "PLU"` with the same null `base` triggers one conflict row and the inline prompt.

## 12. Prototype gate targets (from PRD §10)

On a mid-2010s dual-core laptop: cold replay of a 500k-op log ≤ 60 s · incremental apply ≤ 5 ms p95 · steady-state memory ≤ 300 MB · merkle diff of two 500k-op replicas ≤ 1 s.

## 13. Open questions

1. Finer ACL than source-level (per-segment redaction inside one recording)? Deferred — sealing supports redaction; live model stays source-level until a real case demands more.
2. Gloss abbreviation conventions (Leipzig set + project additions) — project config entity, needed before IGT UI.
3. Per-writing-system FTS tokenizers and ICU sort: prototype during the Tongues search tool, not now.
4. Multi-project references (one lexicon shared by two projects) — out of scope for release 1; revisit with the Corpus kit.
