// The project's typing shortcuts (`;n` → ŋ) wherever the language is typed:
// the transcription field, the Gloss panel's breakdown, the Lexicon's
// headwords, the Elicit word fields. One table per project, read once and
// replaced when the Transcribe panel saves a new one.
import { typingShortcuts } from "./api";
import { applyShortcuts, parseShortcuts, type Shortcut } from "./transcribe/shortcuts";
import { getState } from "./store";

let loadedFor = "";
let table = "";
let shortcuts: Shortcut[] = [];
let loading: Promise<string> | null = null;

/** Read the project's table (once per project); resolves to the table text. */
export function loadShortcuts(projectId: string): Promise<string> {
  if (projectId === loadedFor && loading !== null) return loading;
  loadedFor = projectId;
  table = "";
  shortcuts = [];
  loading = typingShortcuts().then(
    (t) => {
      if (loadedFor === projectId) rememberShortcuts(t);
      return t;
    },
    () => "",
  );
  return loading;
}

/** A table just saved: in use at once, everywhere. */
export function rememberShortcuts(t: string): void {
  table = t;
  shortcuts = parseShortcuts(t).shortcuts;
}

export const shortcutTable = (): string => table;
export const shortcutCount = (): number => shortcuts.length;

/** Let a field the language is typed in take the shortcuts. Runs before the
 * field's own input handler, so it sees the letter, not the shortcut. */
export function typeWithShortcuts<T extends HTMLInputElement | HTMLTextAreaElement>(field: T): T {
  field.addEventListener(
    "input",
    (ev) => {
      // A field in a panel opened before the Transcribe panel loads the table itself.
      const s = getState();
      if (s.kind === "open" && s.info.projectId !== loadedFor) void loadShortcuts(s.info.projectId);
      if ((ev as InputEvent).isComposing || shortcuts.length === 0) return;
      const end = field.selectionEnd ?? field.value.length;
      const r = applyShortcuts(field.value, end, shortcuts);
      if (r !== null) {
        field.value = r.value;
        field.setSelectionRange(r.caret, r.caret);
      }
    },
    { capture: true },
  );
  return field;
}
