# Elicitation tracker

*2026-11-09. Status: Decided for v1 (reversible; owner may override). Roadmap item 23. Mirrored to the claude.ai Project doc `product/design/ELICITATION.md`.*

## What a fieldworker needs
Two ways of collecting words dominate early fieldwork:
1. **Word lists**: go through a standard list of concepts (Swadesh 100/200, Leipzig-Jakarta) with a speaker, write down each word, and know at a glance what is still missing.
2. **Rapid word collection by meaning**: take a semantic domain ("Moon"), ask its questions ("What words refer to the phases of the moon?"), and write down every word the speaker offers.

Both must feed the lexicon directly: the answer *is* a lexicon entry, not a separate wordlist that has to be copied over later. Paradigm elicitation (filling a person × number grid for one verb) is the third workflow; it needs inflected `Form`s with features, which the lexicon does not edit yet, so it is a separate roadmap row (50).

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

## Not in v1
Paradigm grids (row 50); session planning (assigning concepts to an elicitation session with speaker and date); audio per answer (record the speaker saying the word, linked to the sense) — natural next steps once the media capture of item 18 has a "record a short clip" path; importing a user's own list (CSV or Concepticon id).
