// The only module that talks to Rust. Shapes mirror the structs in
// src-tauri/src/session.rs, which serialize with camelCase field names.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SegmentCheck, SegmentState } from "./timeline/undo";
import type { MorphSpec, Suggestion } from "./gloss/suggest";
import type { Machine } from "./assist/logic";
import type { Kit } from "./kits/logic";
import type { Scope } from "./search/logic";
import type { Available } from "./updates/logic";

export type ProjectInfo = {
  projectId: string;
  path: string;
  texts: number;
  conflicts: number;
  /** Set when this folder is a copy of another one. */
  relocation: { from: string; originalStillPresent: boolean } | null;
};

export type TextRow = {
  id: string;
  title: string;
  kind: string | null;
  segments: number;
};

export type TierRow = {
  id: string;
  name: string;
  kind: string | null;
  /** What the track is for: transcription, translation, note, or a kit's own. */
  role: string | null;
  language: string | null;
  /** Where segments go when no tier is named (Auto-segment with nothing selected). */
  utterance?: boolean;
};

export type SegmentRow = {
  id: string;
  tier: string;
  startMs: number | null;
  endMs: number | null;
  content: string;
  pos: string;
  /** Set while the transcription is still the one a phone model wrote. */
  machine: Machine | null;
};

export type TextDetail = {
  id: string;
  title: string;
  tiers: TierRow[];
  segments: SegmentRow[];
};

export type EditResult = {
  content: string;
  findings: string[];
};

type ProjectArgs = { path: string; researcher: string };

export const openProject = (args: ProjectArgs) => invoke<ProjectInfo>("open_project", args);
export const createProject = (args: ProjectArgs) => invoke<ProjectInfo>("create_project", args);
export type ConcordanceResult = { hits: import("./concordance/logic").Hit[]; total: number };
/** Every occurrence of a word, morph, gloss or translation word, with its segment's words (roadmap 71). */
export const concordance = (args: { query: string; kind: import("./concordance/logic").ConcordanceKind }) => invoke<ConcordanceResult>("concordance", args);
/** The sample project (roadmap 68), built the first time in this computer's app data. */
export const openSample = (args: { researcher: string }) => invoke<ProjectInfo>("open_sample", args);
export const listTexts = () => invoke<TextRow[]>("list_texts");
export const getText = (args: { textId: string }) => invoke<TextDetail>("get_text", args);
export const createText = (args: { title: string }) => invoke<string>("create_text", args);
export const conflictCount = () => invoke<number>("conflict_count");

export type AddedSegment = {
  id: string;
  findings: string[];
  /** The segments as the edit left them (a split: first, then second), for an undo's check. */
  states: SegmentState[];
};

export const addSegment = (args: {
  textId: string;
  tierId: string | null;
  startMs: number | null;
  endMs: number | null;
  content: string;
}) => invoke<AddedSegment>("add_segment", args);

/** With `expect` (a timeline undo or redo), refused when it no longer holds: the segment was edited since. */
export const editSegmentContent = (args: { textId: string; segmentId: string; content: string; expect?: SegmentCheck[] }) =>
  invoke<EditResult>("edit_segment_content", args);

/** Writes only the edges given. Resolves with the commit's findings (warnings; never a block). `expect` as for editSegmentContent. */
export const setSegmentTimes = (args: { textId: string; segmentId: string; startMs?: number; endMs?: number; expect?: SegmentCheck[] }) =>
  invoke<string[]>("set_segment_times", args);

export const deleteSegment = (args: { textId: string; segmentId: string }) =>
  invoke<string[]>("delete_segment", args);

/** `states`: the touched segments after the edit; `before`: as they were (merge only). */
export type SegmentsEdit = { findings: string[]; states: SegmentState[]; before: SegmentState[] };
export type DeletedSegments = { deleted: string[]; kept: string[]; findings: string[] };

/** The timeline's undo and redo, one txn each. Deletes only segments whose
 * check still holds: with `keepChanged`, the edited ones are kept and the
 * rest go; without, any change refuses the whole step. */
export const deleteSegments = (args: { textId: string; expect: SegmentCheck[]; keepChanged: boolean }) => invoke<DeletedSegments>("delete_segments", args);
export const restoreSegments = (args: { textId: string; segmentIds: string[] }) => invoke<SegmentsEdit>("restore_segments", args);

/** Cut at `atMs`; the transcription stays on the first part. Returns the new part. */
export const splitSegment = (args: { textId: string; segmentId: string; atMs: number }) => invoke<AddedSegment>("split_segment", args);
/** Join a segment with the next one on its tier (states: first, then second). `expect` as for editSegmentContent. */
export const mergeSegments = (args: { textId: string; firstId: string; secondId: string; expect?: SegmentCheck[] }) => invoke<SegmentsEdit>("merge_segments", args);
/** Take a merge back, or redo a split, in one txn: restore the second part, move the first's end back, and put its text back when given. */
export const unmergeSegments = (args: {
  textId: string;
  firstId: string;
  secondId: string;
  firstEndMs: number;
  firstContent: string | null;
  expect: SegmentCheck[];
}) => invoke<SegmentsEdit>("unmerge_segments", args);

/** Where speech starts and ends in a recording, `[startMs, endMs]` in time order, per utterance or per word (cached per recording and granularity). */
export const speechSpans = (args: { source: string; granularity: "utterance" | "word" }) => invoke<[number, number][]>("speech_spans", args);

export type AutoSegmented = { ids: string[]; skipped: number; tier: string; findings: string[]; states: SegmentState[] };
/** One machine segment per stretch of speech not already covered on the tier (the utterance tier when null). */
export const autoSegment = (args: { textId: string; source: string; tierId: string | null; granularity: "utterance" | "word" }) => invoke<AutoSegmented>("auto_segment", args);

export type TierTemplate = "transcription" | "translation" | "notes" | "custom";

export const addTier = (args: { template: TierTemplate; name: string | null }) =>
  invoke<TierRow>("add_tier", args);

export type { MorphSpec, Suggestion };

export type MorphView = MorphSpec & {
  id: string;
  /** `lemma – gloss` of the linked sense. */
  senseLabel: string | null;
};

export type WordView = {
  id: string;
  form: string;
  morphs: MorphView[];
  /** Earlier analyses of this form, best first; empty once the word has an analysis. */
  suggestions: Suggestion[];
  /** Other unanalysed tokens spelled like this one, across the project. */
  othersUnanalysed: number;
  /** Validator findings on this word or its morphs. Warnings only. */
  findings: string[];
};

/** Edits from another device on a word or morph the current analysis leaves out. */
export type HiddenEdit = { parent: string; member: string; label: string; fields: string[] };

/** The other side of a conflict on a word's morphs or the segment's words, offered as "use that instead". */
export type Alternative = { parent: string; members: string[]; surfaces: string[]; glosses: string[] };

export type GlossView = { segmentId: string; words: WordView[]; hidden: HiddenEdit[]; alternatives: Alternative[] };
/** Every edit answers with the segment's fresh view, and notes worth a toast. */
export type GlossEdit = { view: GlossView; notes: string[] };
export type ApplyAllResult = { applied: number; texts: number; skippedAnalysed: number; view: GlossView };
export type LexiconMatch = { lexeme: string; sense: string; lemma: string; gloss: string };

type SegmentArgs = { textId: string; segmentId: string };

export const glossGet = (args: SegmentArgs) => invoke<GlossView>("gloss_get", args);
export const glossTokenize = (args: SegmentArgs & { forms: string[] }) => invoke<GlossEdit>("gloss_tokenize", args);
/** Unchanged words keep their analysis; the notes say how many analysed words did not. */
export const glossRetokenize = (args: SegmentArgs & { forms: string[] }) => invoke<GlossEdit>("gloss_retokenize", args);
export const glossApply = (args: SegmentArgs & { tokenId: string; morphs: MorphSpec[] }) => invoke<GlossEdit>("gloss_apply", args);
export const glossApplyAll = (args: SegmentArgs & { form: string; morphs: MorphSpec[] }) => invoke<ApplyAllResult>("gloss_apply_all", args);
/** `lang` null sets the primary gloss, a tag sets `gloss:<tag>`. */
export const glossSet = (args: SegmentArgs & { morphId: string; gloss: string; lang: string | null }) => invoke<GlossEdit>("gloss_set", args);
export const glossPickSense = (args: SegmentArgs & { morphId: string; senseId: string; gloss: string }) => invoke<GlossEdit>("gloss_pick_sense", args);
export const glossAddLexeme = (args: SegmentArgs & { morphId: string; lemma: string; gloss: string }) => invoke<GlossEdit>("gloss_add_lexeme", args);
/** Put a hidden word or morph back into its parent's list. */
export const glossRestore = (args: SegmentArgs & { parentId: string; memberId: string }) => invoke<GlossEdit>("gloss_restore", args);
/** Take the other side of a conflict: the parent's list becomes `members`. */
export const glossChooseList = (args: SegmentArgs & { parentId: string; members: string[] }) => invoke<GlossEdit>("gloss_choose_list", args);
export const glossLexicon = (args: { surface: string }) => invoke<LexiconMatch[]>("gloss_lexicon", args);

export type SourceRow = {
  id: string;
  label: string;
  kind: string | null;
  access: string | null;
  hasMedia: boolean;
};

export type SearchRow = {
  target: string;
  kind: string;
  field: string;
  snippet: string;
  textId: string | null;
  textTitle: string | null;
  tierId: string | null;
  tierName: string | null;
  startMs: number | null;
};

export const listSources = () => invoke<SourceRow[]>("list_sources");
export const search = (args: { query: string; scope: Scope }) => invoke<SearchRow[]>("search", args);

export type PeaksInfo = {
  frames: number;
  sampleRate: number;
  channels: number;
  durationMs: number;
  /** Absolute path of the media file; serve it with convertFileSrc. */
  mediaPath: string;
};

export type PeaksSlice = {
  bucketFrames: number;
  firstFrame: number;
  mins: number[];
  maxs: number[];
};

/** WAV only for now; anything else rejects with an error naming WAV. */
export const importMedia = (args: { path: string }) => invoke<SourceRow>("import_media", args);
export const peaksInfo = (args: { sourceId: string }) => invoke<PeaksInfo>("peaks_info", args);
export const peaksQuery = (args: {
  sourceId: string;
  channel: number;
  startFrame: number;
  endFrame: number;
  pixels: number;
}) => invoke<PeaksSlice>("peaks_query", args);

// Layouts: dockview's toJSON() as a string, one file per kit in app data.
export const saveLayout = (args: { kit: string; layout: string }) => invoke<void>("save_layout", args);
export const loadLayout = (args: { kit: string }) => invoke<string | null>("load_layout", args);

// Kits: src-tauri/src/kits.rs. The bundled kit is read-only; user kits are
// files in app data. Problems in a kit come back as one error, a line each.
export type KitEntry = { kit: Kit; bundled: boolean };
export type KitsView = { active: string; kits: KitEntry[]; broken: { file: string; problems: string[] }[] };
export const kitsList = () => invoke<KitsView>("kits_list");
export const kitCode = (args: { id: string }) => invoke<string>("kit_code", args);
export const kitSave = (args: { id: string; kit: Kit }) => invoke<KitEntry>("kit_save", args);
export const kitSaveCode = (args: { id: string; code: string }) => invoke<KitEntry>("kit_save_code", args);
export const kitDuplicate = (args: { source: string; name: string }) => invoke<KitEntry>("kit_duplicate", args);
export const kitDelete = (args: { id: string }) => invoke<KitsView>("kit_delete", args);
export const kitInstall = (args: { path: string }) => invoke<KitEntry>("kit_install", args);
export const kitSetActive = (args: { id: string }) => invoke<void>("kit_set_active", args);

export type IdentityInfo = {
  publicKey: string;
  fingerprint: string;
  store: "keychain" | "file";
  fallbackReason: string | null;
};

export type Standing = "member" | "guest" | "revoked" | "unknown";

export type DeviceRow = {
  key: string;
  fingerprint: string;
  label: string;
  standing: Standing;
  guest: boolean;
  contested: boolean;
  thisDevice: boolean;
};

export type DevicesView = {
  paired: boolean;
  devices: DeviceRow[];
  copyWarning: string | null;
};

export type PairOffer = { digits: string; port: number; expiresIn: number };
export type JoinResult = { project: ProjectInfo; standing: Standing };
export type ListenInfo = { port: number; discoveryError: string | null };

export type SyncResult = {
  peerLabel: string | null;
  peerGuest: boolean;
  pulledOps: number;
  answered: number;
  txnsHeld: number;
  conflicts: number;
  warnings: string[];
};

export type ServedSession = {
  peerLabel: string | null;
  outcome: string;
  pulledOps: number;
  warnings: string[];
};

export type PairFinished = { kind: "enrolled"; device: DeviceRow } | { kind: "failed"; error: string };

export const deviceIdentity = () => invoke<IdentityInfo>("device_identity");
export const devicesList = () => invoke<DevicesView>("devices_list");
export const deviceMarkLost = (args: { key: string }) => invoke<DevicesView>("device_mark_lost", args);
export const mintDeviceIdentity = () => invoke<DevicesView>("mint_device_identity");
export const relocationAcceptMove = () => invoke<void>("relocation_accept_move");
export const pairStart = (args: { label: string; guest: boolean }) => invoke<PairOffer>("pair_start", args);
export const pairJoin = (args: { path: string; researcher: string; addr: string; code: string; label: string }) =>
  invoke<JoinResult>("pair_join", args);
export const syncNow = (args: { addr: string }) => invoke<SyncResult>("sync_now", args);
export const syncListen = (args: { label: string }) => invoke<ListenInfo>("sync_listen", args);
export const discoverPeers = () => invoke<string[]>("discover_peers");

export const onPairFinished = (f: (e: PairFinished) => void): Promise<UnlistenFn> =>
  listen<PairFinished>("pair-finished", (ev) => f(ev.payload));
export const onSyncServed = (f: (e: ServedSession) => void): Promise<UnlistenFn> =>
  listen<ServedSession>("sync-served", (ev) => f(ev.payload));

/** Commands reject with the Rust error string; anything else is a bug. */
export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

export const captureBegin = (args: { sampleRate: number; channels: number }) => invoke<string>("capture_begin", args);
/** The PCM16 bytes travel as the raw request body; the id rides in a header. */
export const captureChunk = (captureId: string, pcm16: Uint8Array) =>
  invoke<void>("capture_chunk", pcm16, { headers: { "x-capture-id": captureId } });
export const captureFinish = (args: { captureId: string; label: string }) => invoke<SourceRow>("capture_finish", args);
/** Deletes the take. Only ever called after the user confirmed Discard. */
export const captureDiscard = (args: { captureId: string }) => invoke<void>("capture_discard", args);
/** "Keep for later": the file stays in `recordings-pending/` and comes back as a Recovered recording. */
export const captureKeep = (args: { captureId: string }) => invoke<void>("capture_keep", args);
/** A fresh page owns no captures: closes any the old page left, keeping the files. */
export const captureReset = () => invoke<number>("capture_reset");

/** A recording a crash, close or failed import left in `recordings-pending/`. */
export type Recovered = { id: string; bytes: number; durationMs: number; modifiedMs: number };
export const captureRecoveredList = () => invoke<Recovered[]>("capture_recovered_list");
export const captureRecoveredImport = (args: { id: string; label: string }) => invoke<SourceRow>("capture_recovered_import", args);
export const captureRecoveredDiscard = (args: { id: string }) => invoke<void>("capture_recovered_discard", args);

// ---------------------------------------------------------------- lexicon (roadmap 22)

export type EntryRow = { id: string; lemma: string; homograph: number | null; posOfSpeech: string; glosses: string[] };
export type DomainRef = { id: string; code: string; label: string };
export type SenseView = {
  id: string;
  gloss: string;
  glossesByLang: Record<string, string>;
  definition: string;
  usage: string;
  domains: DomainRef[];
  occurrences: number;
};
export type EntryView = {
  id: string;
  lemma: string;
  citation: string;
  posOfSpeech: string;
  homograph: number | null;
  senses: SenseView[];
  /** Other entries with exactly this lemma: [id, homograph]. */
  sameLemma: [string, number | null][];
};
export type EntryEdit = { entry: EntryView | null; notes: string[] };
export type DomainItem = { id: string; code: string; label: string; description: string; depth: number };
export type DomainsLoaded = { created: number; already: number; notes: string[] };
export type LexemeField = "lemma" | "citation" | "pos-of-speech";

export const lexiconList = (args: { query: string }) => invoke<EntryRow[]>("lexicon_list", args);
export const lexiconEntry = (args: { id: string }) => invoke<EntryView>("lexicon_entry", args);
export const lexiconAdd = (args: { lemma: string; gloss: string }) => invoke<EntryEdit>("lexicon_add", args);
export const lexiconSetField = (args: { id: string; field: LexemeField; value: string }) => invoke<EntryEdit>("lexicon_set_field", args);
export const lexiconSetHomograph = (args: { id: string; number: number | null }) => invoke<EntryEdit>("lexicon_set_homograph", args);
export const lexiconNumberHomographs = (args: { id: string }) => invoke<EntryEdit>("lexicon_number_homographs", args);
export const lexiconDelete = (args: { id: string }) => invoke<EntryEdit>("lexicon_delete", args);
export const lexiconAddSense = (args: { id: string; gloss: string }) => invoke<EntryEdit>("lexicon_add_sense", args);
export const lexiconSetSenseGloss = (args: { senseId: string; gloss: string; lang: string | null }) =>
  invoke<EntryEdit>("lexicon_set_sense_gloss", args);
export const lexiconSetSenseText = (args: { senseId: string; field: "definition" | "usage"; text: string }) =>
  invoke<EntryEdit>("lexicon_set_sense_text", args);
export const lexiconDeleteSense = (args: { senseId: string }) => invoke<EntryEdit>("lexicon_delete_sense", args);
export const lexiconAssignDomain = (args: { senseId: string; domainId: string }) => invoke<EntryEdit>("lexicon_assign_domain", args);
export const lexiconUnassignDomain = (args: { senseId: string; domainId: string }) => invoke<EntryEdit>("lexicon_unassign_domain", args);

// Paradigms (roadmap 50): `axes` are a template's features and values (src/lexicon/paradigms.ts).
export type ParadigmAxis = { feature: string; values: string[] };
export type ParadigmForm = { id: string; form: string; features: Record<string, string> };
export type ParadigmCell = { features: Record<string, string>; forms: ParadigmForm[] };
export type ParadigmView = { lexeme: string; cells: ParadigmCell[]; others: ParadigmForm[] };
export type ParadigmEdit = { paradigm: ParadigmView; notes: string[] };
export const lexiconParadigm = (args: { id: string; axes: ParadigmAxis[] }) => invoke<ParadigmView>("lexicon_paradigm", args);
export const lexiconFillCell = (args: { id: string; formId: string | null; features: Record<string, string>; surface: string; axes: ParadigmAxis[] }) =>
  invoke<ParadigmEdit>("lexicon_fill_cell", args);
export const lexiconDeleteForm = (args: { id: string; formId: string; axes: ParadigmAxis[] }) => invoke<ParadigmEdit>("lexicon_delete_form", args);
export const domainsList = () => invoke<DomainItem[]>("domains_list");
export const domainsLoadSil = () => invoke<DomainsLoaded>("domains_load_sil");

// ---------------------------------------------------------------- elicitation (roadmap 23)

export type BundledList = { source: string; name: string; citation: string; count: number; listId: string | null };
export type ListSummary = { id: string; name: string; citation: string; total: number; answered: number; skipped: number };
export type Answer = { senseId: string; lexemeId: string; lemma: string; homograph: number | null; gloss: string };
export type ConceptView = {
  id: string;
  number: number;
  gloss: string;
  concepticonGloss: string;
  skipped: boolean;
  skipNote: string;
  answers: Answer[];
};
export type ListView = { summary: ListSummary; concepts: ConceptView[] };
export type ConceptEdit = { concept: ConceptView; notes: string[] };
export type DomainQuestions = {
  id: string;
  code: string;
  label: string;
  description: string;
  questions: { q: string; words: string }[];
  words: Answer[];
};
export type DomainEdit = { domain: DomainQuestions; notes: string[] };

export const conceptListsBundled = () => invoke<BundledList[]>("concept_lists_bundled");
export const conceptListLoad = (args: { source: string }) => invoke<ListSummary>("concept_list_load", args);
export const conceptLists = () => invoke<ListSummary[]>("concept_lists");
export const conceptListView = (args: { listId: string }) => invoke<ListView>("concept_list_view", args);
export const elicitAnswer = (args: { conceptId: string; form: string; gloss: string }) => invoke<ConceptEdit>("elicit_answer", args);
export const elicitLink = (args: { conceptId: string; senseId: string }) => invoke<ConceptEdit>("elicit_link", args);
export const elicitUnlink = (args: { conceptId: string; senseId: string }) => invoke<ConceptEdit>("elicit_unlink", args);
export const elicitSkip = (args: { conceptId: string; skipped: boolean; note: string }) => invoke<ConceptEdit>("elicit_skip", args);
export const domainQuestions = (args: { domainId: string }) => invoke<DomainQuestions>("domain_questions", args);
export const domainCollect = (args: { domainId: string; form: string; gloss: string }) => invoke<DomainEdit>("domain_collect", args);

// ---------------------------------------------------------------- conflicts (roadmap 24)

export type VersionView = { raw: string; label: string; atMs: number; mine: boolean; winner: boolean };
export type ConflictView = {
  target: string;
  field: string;
  title: string;
  place: string;
  textId: string | null;
  segmentId: string | null;
  editable: boolean;
  /** The field's open versions as listed; sent back with a choice. */
  heads: string[];
  versions: VersionView[];
};
export type ConflictsResolved = { conflicts: ConflictView[]; notes: string[] };

export const conflictsList = () => invoke<ConflictView[]>("conflicts_list");
export const conflictResolve = (args: { target: string; field: string; raw: string; heads: string[] }) => invoke<ConflictsResolved>("conflict_resolve", args);

// Speech assist: src-tauri/src/assist.rs. Models are downloaded on request
// into app data; suggestions are written as machine transcriptions.
export type InstalledFile = { name: string; bytes: number; sha256: string; published: boolean };
export type AssistModel = {
  id: string;
  name: string;
  about: string;
  license: string;
  attribution: string;
  homepage: string;
  capability: string;
  /** Whether this build has the model's runtime adapter. */
  runsHere: boolean;
  approxMb: number | null;
  installed: { files: InstalledFile[] } | null;
  download: { done: number; total: number; error: string | null } | null;
};
export type Suggested = { content: string; model: string; confidence: number; notes: string[] };
export type DownloadEvent = { id: string; done: number; total: number; finished: { Ok: null } | { Err: string } | null };
export const assistModels = () => invoke<AssistModel[]>("assist_models");
export const assistDownload = (args: { id: string }) => invoke<void>("assist_download", args);
export const assistCancel = (args: { id: string }) => invoke<void>("assist_cancel", args);
export const assistDelete = (args: { id: string }) => invoke<void>("assist_delete", args);
/** `expect` is the provider id the caller showed; the backend writes nothing if what powers phone transcription changed since. */
export const assistSuggest = (args: { text: string; segment: string; source: string; expect: string }) => invoke<Suggested>("assist_suggest", args);
// Import and export in other tools' formats: src-tauri/src/interchange.rs.
export type TextFormat = "elan" | "flextext" | "textgrid";
export type LexiconFormat = "lift" | "toolbox";
/** `text`: the text to show afterwards; `lexicon`: the lexicon changed; `lines`: what happened, in words. */
export type Imported = { format: string; text: string | null; lexicon: boolean; lines: string[] };
export type Exported = { path: string; notes: string[] };
export type Sealed = { path: string; files: number; bytes: number; texts: number; lexemes: number; restricted: boolean };
export const importFile = (args: { path: string }) => invoke<Imported>("import_file", args);
// Files dropped onto the window (src-tauri/src/drops.rs): copied in pieces, then imported like a picked file.
export type Dropped = { kind: "recording"; source: SourceRow } | { kind: "file"; imported: Imported };
/** `project`: the folder of the project open when the files were dropped; refused if another one is open now. */
export const dropBegin = (args: { project: string; name: string; size: number }) => invoke<string>("drop_begin", args);
/** The piece travels as the raw request body; the id rides in a header. */
export const dropChunk = (dropId: string, bytes: Uint8Array) => invoke<void>("drop_chunk", bytes, { headers: { "x-drop-id": dropId } });
export const dropFinish = (args: { dropId: string }) => invoke<Dropped>("drop_finish", args);
export const dropCancel = (args: { dropId: string }) => invoke<void>("drop_cancel", args);
export const exportText = (args: { textId: string; format: TextFormat; folder: string }) => invoke<Exported>("export_text", args);
export const exportLexicon = (args: { format: LexiconFormat; folder: string }) => invoke<Exported>("export_lexicon", args);
/** `date`: today on this computer's calendar (YYYY-MM-DD), which names the package folder. */
export const sealProject = (args: { folder: string; date: string }) => invoke<Sealed>("seal_project", args);
// People and consent (roadmap 61): src-tauri/src/people.rs. Every command returns the whole view.
/** A term's answer: true agreed, false refused, null not asked. */
export type PeopleConsent = { id: string; covers: string; terms: [string, boolean | null][]; date: string; evidenceName: string | null; hasEvidence: boolean };
/** `deleted`: the person was deleted on another device while a record was added for them here; the records still count. */
export type PeoplePerson = { id: string; name: string; roles: string[]; consents: PeopleConsent[]; deleted: boolean };
export type Coverable = { id: string; kind: "project" | "recording" | "text"; label: string };
export type PeopleView = { people: PeoplePerson[]; coverable: Coverable[] };
export type PeopleCommand =
  | "people_view" | "person_add" | "person_set_name" | "person_set_roles" | "person_delete"
  | "consent_add" | "consent_set_term" | "consent_set_covers" | "consent_set_date" | "consent_attach" | "consent_detach" | "consent_delete";
export const peopleCommand = (command: PeopleCommand, args: Record<string, unknown> = {}) => invoke<PeopleView>(command, args);
// Corrections export: src-tauri/src/corrections.rs (roadmap 57).
/** `recordedNo`: a consent record says no to AI use; `restricted`: a restricted or community access level. Both start left out; the person decides. */
/** `partial`: why this device may not know every consent record (a guest, records still arriving), or null. */
export type CorrectionsSummary = { ready: number; recordedNo: number; restricted: number; unchecked: number; unusable: number; emptied: number; partial: string | null };
export type CorrectionsExported = { folder: string; written: number; problems: string[]; failed: number };
export const correctionsSummary = () => invoke<CorrectionsSummary>("corrections_summary");
/** `flagged`: also include clips the project's records speak against (a recorded no, a restricted level). */
/** `date`: today on this computer's calendar (YYYY-MM-DD), which names the folder. */
export const correctionsExport = (args: { folder: string; flagged: boolean; date: string }) => invoke<CorrectionsExported>("corrections_export", args);
/** How assist_suggest says the model changed under the caller. */
export const MODEL_CHANGED = "Phone transcription now uses ";

// Capabilities: src-tauri/src/capability.rs (roadmap 53). Feature code asks for a
// job; what powers it is chosen here (this computer's choice, the project's pin).
export type ProviderRow = { id: string; name: string; kind: "model" | "program" | "built-in"; ready: boolean; note: string; score: number | null };
export type CapabilityView = {
  id: string;
  title: string;
  /** This computer's choice: "automatic" or a provider id. */
  machine: string;
  /** The open project's pin. */
  pin: string | null;
  effective: string | null;
  problem: string | null;
  options: ProviderRow[];
};
export const capabilities = () => invoke<CapabilityView[]>("capabilities");
export const capabilityChoose = (args: { capability: string; choice: string }) => invoke<void>("capability_choose", args);
export const capabilityPin = (args: { capability: string; provider: string }) => invoke<void>("capability_pin", args);
export const onAssistDownload = (f: (e: DownloadEvent) => void): Promise<UnlistenFn> => listen<DownloadEvent>("assist-download", (ev) => f(ev.payload));

// Praat handoff: src-tauri/src/praat.rs (roadmap item 46).
export type PraatStatus = { path: string | null; version: string | null; chosen: boolean; problem: string | null };
export type PraatReturn = { retimed: number; relabelled: number; added: number; removed: number; notBroughtBack: [string, number][] };
export const praatStatus = () => invoke<PraatStatus>("praat_status");
export const praatSetPath = (args: { path: string | null }) => invoke<PraatStatus>("praat_set_path", args);
export const praatOpen = (args: { text: string; source: string; fromMs: number; toMs: number }) =>
  invoke<{ script: string; notShown: number }>("praat_open", args);
export const praatPending = (args: { text: string }) => invoke<boolean>("praat_pending", args);
export const praatBringBack = (args: { text: string }) => invoke<PraatReturn>("praat_bring_back", args);
export const praatApply = (args: { text: string }) => invoke<{ changes: number; findings: string[] }>("praat_apply", args);
export const praatDiscard = (args: { text: string }) => invoke<void>("praat_discard", args);
export const acousticAnalysis = (args: { source: string; chunks: number[] }) =>
  invoke<import("./praat/analysis").Chunk[]>("acoustic_analysis", args);

// In-app updates: src-tauri/src/updater.rs. Online.
export type UpdateCheck = { currentVersion: string; available: Available | null };
export const updateCheck = () => invoke<UpdateCheck>("update_check");
export const updateDownload = () => invoke<void>("update_download");
export const updateInstall = () => invoke<void>("update_install");
export const onUpdateProgress = (f: (p: { done: number; total: number | null }) => void): Promise<UnlistenFn> =>
  listen<{ done: number; total: number | null }>("update-progress", (ev) => f(ev.payload));

// Send feedback (roadmap 65). Online; offline it waits in the outbox.
export type FeedbackOutcome = { sent: boolean; waiting: number; problem: string | null; configured: boolean };
export const feedbackDetails = (args: { page: Record<string, unknown> }) => invoke<Record<string, unknown>>("feedback_details", args);
/** Checks and keeps a report; returns its id. */
export const feedbackKeep = (args: { message: string; contact: string; details: Record<string, unknown> | null; attachment: string | null }) =>
  invoke<string>("feedback_keep", args);
export const feedbackDeliver = (args: { id: string }) => invoke<FeedbackOutcome>("feedback_deliver", args);
export const feedbackRetry = () => invoke<FeedbackOutcome>("feedback_retry");
