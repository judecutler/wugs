// App state every panel reads, and the actions that change it. Panels
// subscribe and re-render; only this module assigns `state`.
import { capabilities, conflictCount, errorText, getText, listTexts, type AssistModel, type CapabilityView, type KitsView, type ProjectInfo, type TextDetail, type TextRow } from "./api";
import type { Kit } from "./kits/logic";
import { rememberProject } from "./last-project";

export type AppState =
  | { kind: "closed" }
  | { kind: "open"; info: ProjectInfo; texts: TextRow[]; text: TextDetail | null };

export type Toast = { tone: "info" | "warn" | "error"; lines: string[] };

/** The segment the user is working on, shared by the timeline and the
 * transcription panel so each follows the other. */
export type Selection = { textId: string; tierId: string; segmentId: string };

const RESEARCHER_KEY = "wugs.researcher";

let state: AppState = { kind: "closed" };
let toast: Toast | null = null;
// Toasts have their own listeners: a toast must never re-render a panel,
// which would throw away an edit in progress.
const listeners = new Set<() => void>();
const toastListeners = new Set<() => void>();
// Selection has its own listeners too: panels that only follow it must not
// redraw on every state change, and state listeners need not hear selection.
let selection: Selection | null = null;
const selectionListeners = new Set<() => void>();

export const getSelection = (): Selection | null => selection;

export function subscribeSelection(f: () => void): () => void {
  selectionListeners.add(f);
  return () => selectionListeners.delete(f);
}

export function selectSegment(next: Selection | null) {
  if (next?.segmentId === selection?.segmentId && next?.tierId === selection?.tierId && next?.textId === selection?.textId) return;
  selection = next;
  for (const f of selectionListeners) f();
}

export const getState = (): AppState => state;
export const getToast = (): Toast | null => toast;

export function subscribe(f: () => void): () => void {
  listeners.add(f);
  return () => listeners.delete(f);
}

export function subscribeToast(f: () => void): () => void {
  toastListeners.add(f);
  return () => toastListeners.delete(f);
}

function setState(next: AppState) {
  state = next;
  for (const f of listeners) f();
}

export function showToast(next: Toast | null) {
  toast = next;
  for (const f of toastListeners) f();
}

export const researcher = (): string => localStorage.getItem(RESEARCHER_KEY) ?? "";
export const setResearcher = (id: string) => localStorage.setItem(RESEARCHER_KEY, id.trim());

/** Run an action, turning a rejection into an error toast. */
export async function attempt(action: () => Promise<void>) {
  try {
    await action();
  } catch (e) {
    const message = errorText(e);
    // The session thread closes the project after a panic; the view must
    // not keep pretending one is open.
    if (message.includes("no project is open") && state.kind === "open") setState({ kind: "closed" });
    showToast({ tone: "error", lines: [message] });
  }
}

export async function enterProject(info: ProjectInfo) {
  showToast(null);
  selectSegment(null);
  rememberProject(localStorage, info.path);
  setState({ kind: "open", info, texts: await listTexts(), text: null });
  // The project's pins change what runs; a failure here only leaves the old view.
  void refreshCapabilities().catch(() => {});
}

/** The folder's relocation was resolved (accepted as a move, or given a fresh device id).
 * Only the project the action started on is cleared: another may be shown by now. */
export function clearRelocation(projectId: string) {
  if (state.kind === "open" && state.info.projectId === projectId) setState({ ...state, info: { ...state.info, relocation: null } });
}

let selectSeq = 0;

/** A user selection: always wins over anything still in flight. */
export async function selectText(textId: string) {
  selectSeq += 1;
  await fetchText(textId, selectSeq);
}

/** A refresh of the selected text: loses to any newer selection. */
export async function refreshText() {
  if (state.kind !== "open" || state.text === null) return;
  await fetchText(state.text.id, selectSeq, true);
}

async function fetchText(textId: string, seq: number, refresh = false) {
  await attempt(async () => {
    const text = await getText({ textId });
    if (state.kind !== "open" || seq !== selectSeq) return;
    // a refresh must not displace a selection that landed while it ran
    if (refresh && state.text !== null && state.text.id !== textId) return;
    setState({ ...state, text });
    if (selection !== null && selection.textId !== text.id) selectSegment(null);
  });
}

export async function reloadTexts() {
  if (state.kind !== "open") return;
  const texts = await listTexts();
  // state may have moved while awaiting; never resurrect the old copy
  if (state.kind !== "open") return;
  setState({ ...state, texts });
}

export async function refreshConflicts() {
  if (state.kind !== "open") return;
  const conflicts = await conflictCount();
  if (state.kind !== "open") return;
  setState({ ...state, info: { ...state.info, conflicts } });
}

let generation = 0;
let lexiconRev = 0;
let media: string | null = null;
let models: AssistModel[] = [];

/** The recording the Timeline has loaded, which the Transcribe panel plays and speech assist hears. */
export const mediaSource = (): string | null => media;

export function setMediaSource(id: string | null) {
  if (id === media) return;
  media = id;
  for (const f of listeners) f();
}

/** Speech-assist models and whether each is downloaded. */
export const assistModelList = (): AssistModel[] => models;

export function setAssistModels(next: AssistModel[]) {
  models = next;
  for (const f of listeners) f();
}

let caps: CapabilityView[] = [];

/** What powers each capability here (this computer's choice, the open project's pin). */
export const capabilityList = (): CapabilityView[] => caps;

/** One capability's view, if loaded. Feature code checks `effective` and never names a model. */
export const capability = (id: string): CapabilityView | null => caps.find((c) => c.id === id) ?? null;

/** Reload after anything that changes them: a download, a choice, a pin, another project. */
export async function refreshCapabilities() {
  caps = await capabilities();
  for (const f of listeners) f();
}

let kits: KitsView | null = null;

/** Installed kits, or null until the first load. */
export const getKits = (): KitsView | null => kits;

/** The kit in use: what the brand, the panel menu and Reset layout follow. */
export function activeKit(): Kit | null {
  return kits?.kits.find((k) => k.kit.id === kits?.active)?.kit ?? null;
}

export function setKits(next: KitsView) {
  kits = next;
  for (const f of listeners) f();
}

/** Bumped when another panel changed the lexicon (the Gloss panel's add-to-lexicon),
 * so the Lexicon panel refetches. */
export const lexiconRevision = (): number => lexiconRev;

export function touchLexicon() {
  lexiconRev += 1;
  for (const f of listeners) f();
}

/** Bumped whenever other devices' ops may have arrived, for panels that
 * fetch their own data (the media bin) to know when to fetch again. */
export const dataGeneration = (): number => generation;

/** After anything that may have brought in other devices' ops. */
export async function refreshAll() {
  generation += 1;
  await attempt(async () => {
    await reloadTexts();
    await refreshConflicts();
    await refreshText();
    // Synced ops can carry a project pin.
    await refreshCapabilities();
  });
}
