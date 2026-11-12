# Elicitation tracker

*2026-11-09. Status: Decided for v1 (reversible; owner may override). Roadmap item 23. Mirrored to the claude.ai Project doc `product/design/ELICITATION.md`.*

## What a fieldworker needs
Two ways of collecting words dominate early fieldwork:
1. **Word lists**: go through a standard list of concepts (Swadesh 100/200, Leipzig-Jakarta) with a speaker, write down each word, and know at a glance what is still missing.
2. **Rapid word collection by meaning**: take a semantic domain ("Moon"), ask its questions ("What words refer to the phases of the moon?"), and write down every word the speaker offers.

Both must feed the lexicon directly: the answer *is* a lexicon entry, not a separate wordlist that has to be copied over later. Paradigm elicitation (filling a person × number grid for one verb) is the third workflow; it was split out as roadmap row 50 and is described under "Paradigms" below.

## Data model
| Entity | Kind, prefix, scope | Fields |
| --- | --- | --- |
| ConceptList | `concept-list` `cpl`, project config | name · source (`concepticon:<list id>`, permanent) · citation · origin |
| Concept | `concept` `cpt`, project config | list `ref` · pos · number · gloss (the list's own wording) · concepticon · concepticon-gloss · skipped `bool?` · skip-note · origin (the Concepticon row id) |
| SenseConcept | `sense-concept` `scn`, lexicon | sense `ref` · concept `ref` |

- **Lists and concepts are the plan**, shared by the team, so they live in the config scope. Which sense answers which concept is data about the language, so the link lives in the lexicon scope, like `sense-domain`.
- **Coverage is derived:** a concept is answered while a live sense (of a live entry) links to it (`concept_answers`, each sense once even when two devices both linked it). Deleting the entry uncovers the concept; nothing has to be kept in step. "Not applicable" is the concept's `skipped` flag, set by a person (a resolving write), with an optional note.
- **Ids come from the published list:** `cpl_` + a 128-bit FNV-1a hash of the list source, `cpt_` + the hash of the source and the row id together (so a user's own list with rows "1", "2" cannot collide with another's), in ULID form. Loading a list twice, or on two devices before they sync, gives one list (the same rule as SIL semantic domains). Two devices that load the same list before syncing create equal entities, which raise no conflict (equal values are not a conflict, DATA-MODEL §5.3).
- **One txn per answer:** `elicit` creates the lexeme, its sense and the link together, in the lexicon scope, under intent `elicit`; `collect_for_domain` does the same with a `sense-domain` link. An optional session id rides on the ops as provenance (the session UI is later work).

## Why Concepticon
Concepticon is the standard catalogue of published concept lists and maps each item to a concept set id. Storing that id on each concept means answers can later be exported as CLDF wordlists and compared across languages (Datasets layer, item 39) without re-mapping. The lists are CC BY 4.0, shipped as separately licensed data in `kits/tongues/concepts/` with attribution in the panel.

## The panel
- **Word lists:** list picker, progress ("37 of 100 answered · 2 not applicable") with a bar, To do / Done / All. Each row: number, concept, answers as chips (× takes an answer back; the entry stays), and on the current row a word field and an optional gloss (the concept's gloss by default). Enter moves to the next concept still to do at once (under Done or All, to the next row shown) and saves in the background, so a fast typist's next word lands on the next concept; Enter on an empty field skips ahead without answering. Unsaved typing survives redraws per concept. Typing a word that already exists offers "Use «su» 'water'", which links the existing entry instead of adding a duplicate. N/A marks a concept as not applicable; text in the row's gloss field is kept as the reason.
- **Semantic domains:** find a domain by word or outline number, see its description, the SIL questions with their example words, and the words already in it; type word and gloss, Enter adds an entry assigned to the domain and keeps focus for the next word.

## Paradigms (roadmap 50)
A paradigm is a grid over one lexeme's inflected forms: each cell is one combination of feature values (1st person singular), and the form in it is what the speaker said for that combination.

**Data.** An inflected form is the spec's `Form` entity (DATA-MODEL §3) with `kind` = `inflected`, `lexeme` and `form` (the surface), in the lexicon scope. The spec's `Form` has no field for features, so each feature is a sibling field `feature:<name>` (`feature:person` = `1`, `feature:number` = `sg`), the same pattern as `gloss:<tag>` on senses and morphs. A plain field rather than `x.<kit>.*`, because grammatical features are core lexicon data that every kit reads, not one kit's extension. One field per feature rather than one `json` field for the set, so per-field LWW merges two devices' answers about different features of one form, and a later dimension (tense on a form that has person and number) is one more field. Feature names are lower-case letters, digits and hyphens; values are the template's lower-case Leipzig abbreviations (`1`, `sg`, `prs`). `environment` and `ws` stay unset.

**Intents** (`wugs-core/src/paradigms.rs`): `add_inflected_form` (plain create), `set_form_surface` and `set_form_features` (resolving: a person's direct edit settles that field's conflicts; `""` clears a feature), `delete_form` (plain tombstone). An empty surface is refused (delete the form instead). All return findings, as in `lexicon.rs`.

**Grid and coverage are derived.** `Project::paradigm(lexeme, dims)` takes the dimensions (feature + values) from the caller and returns one cell per combination, the first dimension outermost. A form sits in a cell when its features are exactly the cell's, no more and no fewer; every other inflected form of the entry is returned as `others`, so the choice of grid never hides a form. Coverage ("4 of 6 cells filled") is counted from the cells on every read, never stored. The engine knows no templates; nothing kit-specific is in core.

**Templates.** One table in the shell, `TEMPLATES` in `apps/desktop/src/lexicon/paradigms.ts`: Person × number (1, 2, 3 × sg, pl), Tense × person (prs, pst, fut × 1, 2, 3) and Singular and plural (sg, pl). The panel sends a template's axes with every paradigm command. The kit's `[templates]` table is not wired to this yet.

**Panel.** A Paradigm section in the Lexicon panel's entry editor, under the senses: a template picker (remembered per machine), the grid with the coverage line, one field per cell (Enter or blur saves: an empty cell adds a form with the cell's features, a filled one retypes it; focus stays in place across the redraw), a two-click × per form (out of the Tab order, so Tab walks from cell to cell), and "Other inflected forms of this entry" for forms outside the grid, each with a two-click Delete. Commands `lexicon_paradigm`, `lexicon_fill_cell` and `lexicon_delete_form` in `src-tauri/src/lexicon.rs` return the whole paradigm view.

**Glossing.** Inflected surfaces go into the same lookup index as allomorphs (`lookup_keys` covers every `Form`), so the Gloss panel's lexicon lookup of `gidiyorum` finds `gitmek` once that form is in its paradigm.

**v1 limits.** Three built-in templates and no editor for custom ones (the engine takes any dimensions); a form's features cannot be changed from the panel (the engine's `set_form_features` can), so a form typed in the wrong cell is deleted and retyped; a cell matches exact feature sets, so a form with an extra feature (tense on a person × number grid) is listed under the grid, not in a cell, and one template cannot show a third fixed feature (person × number in the past tense); two devices filling one empty cell before a sync leave two forms in it, both shown, for the person to delete one; concurrent retypes of one form are an ordinary conflict on its `form` field, settled by the next retype or the Conflicts panel (which names it by field, not by cell); no audio per form, no link from a form to the words in texts that use it, no Elicit-panel view of paradigms across entries, and LIFT and Toolbox export do not carry inflected forms yet.

## Not in v1
Session planning (assigning concepts to an elicitation session with speaker and date); audio per answer (record the speaker saying the word, linked to the sense) — natural next steps once the media capture of item 18 has a "record a short clip" path; importing a user's own list (CSV or Concepticon id).
