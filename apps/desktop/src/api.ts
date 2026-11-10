// The only module that talks to Rust. Shapes mirror the structs in
// src-tauri/src/session.rs, which serialize with camelCase field names.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { MorphSpec, Suggestion } from "./gloss/suggest";
import type { Machine } from "./assist/logic";
import type { Kit } from "./kits/logic";
import type { Scope } from "./search/logic";

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
export const listTexts = () => invoke<TextRow[]>("list_texts");
export const getText = (args: { textId: string }) => invoke<TextDetail>("get_text", args);
export const createText = (args: { title: string }) => invoke<string>("create_text", args);
export const conflictCount = () => invoke<number>("conflict_count");

export type AddedSegment = {
  id: string;
  findings: string[];
};

export const addSegment = (args: {
  textId: string;
  tierId: string | null;
  startMs: number | null;
  endMs: number | null;
  content: string;
}) => invoke<AddedSegment>("add_segment", args);

export const editSegmentContent = (args: { textId: string; segmentId: string; content: string }) =>
  invoke<EditResult>("edit_segment_content", args);

/** Writes only the edges given. Resolves with the commit's findings (warnings; never a block). */
export const setSegmentTimes = (args: { textId: string; segmentId: string; startMs?: number; endMs?: number }) =>
  invoke<string[]>("set_segment_times", args);

export const deleteSegment = (args: { textId: string; segmentId: string }) =>
  invoke<string[]>("delete_segment", args);

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
/** How assist_suggest says the model changed under the caller. */
export const MODEL_CHANGED = "Phone transcription now uses ";

// Capabilities: src-tauri/src/capability.rs (roadmap 53). Feature code asks for a
// job; what powers it is chosen here (this computer's choice, the project's pin).
export type ProviderRow = { id: string; name: string; kind: "model" | "program"; ready: boolean; note: string; score: number | null };
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
