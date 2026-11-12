// The command palette (roadmap 70): Ctrl+K (Cmd+K) finds any action or panel
// by name, the convention of VS Code, Figma and Premiere. Pure: no DOM.
// Panels register their own actions while they are open, so a kit's tools
// (Scrolls next) bring their commands with them.

export type Command = {
  /** Stable id, unique across the app. */
  id: string;
  /** What it does, as a button would say it: "Split at the playhead". */
  title: string;
  /** Where it belongs, shown beside it: "Timeline", "Panels", "Colours". */
  group: string;
  /** Its key, when it has one: "S", "Ctrl+K". */
  keys?: string;
  /** Other words people may type for it. */
  words?: string[];
  /** Why it cannot run now, or null. Shown, never hidden: a newcomer learns it exists. */
  disabled?: () => string | null;
  run: () => void;
};

/** How well `query` finds `text`: 0 is no match. Every query word must
 * start a word of the text or appear in it; starts weigh more, and an
 * exact word more again. Case and accents are ignored. */
export function score(query: string, text: string): number {
  const words = fold(query).split(/\s+/u).filter((w) => w !== "");
  if (words.length === 0) return 1;
  const hay = fold(text);
  const starts = hay.split(/[^\p{L}\p{N}]+/u).filter((w) => w !== "");
  let total = 0;
  for (const w of words) {
    if (starts.includes(w)) total += 4;
    else if (starts.some((s) => s.startsWith(w))) total += 3;
    else if (hay.includes(w)) total += 1;
    else return 0;
  }
  return total;
}

export const fold = (s: string): string => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

/** The commands to list for a query: best first, then by group and title.
 * With no query, the order they were given (most useful first). */
export function rank(query: string, commands: readonly Command[]): Command[] {
  if (query.trim() === "") return [...commands];
  return commands
    .map((c, i) => ({ c, i, s: Math.max(score(query, c.title) * 2, score(query, [c.title, c.group, ...(c.words ?? [])].join(" "))) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s || a.i - b.i)
    .map((x) => x.c);
}

/** Arrow keys move through the list and wrap at either end. */
export const moveIndex = (at: number, by: number, length: number): number => (length === 0 ? 0 : (((at + by) % length) + length) % length);

/** "Ctrl" on Windows and Linux reads "⌘" on a Mac. */
export function keyLabel(keys: string, mac: boolean): string {
  return mac ? keys.replace(/\bCtrl\+/gu, "⌘").replace(/\bAlt\+/gu, "⌥").replace(/\bShift\+/gu, "⇧") : keys;
}

/** The palette opens on Ctrl+K or Cmd+K, wherever focus is. */
export const opensPalette = (e: { key: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }): boolean =>
  (e.key === "k" || e.key === "K") && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey;

/** `?` opens the shortcuts sheet, except while typing (where it is a question mark). */
export const opensShortcuts = (e: { key: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean }, typing: boolean): boolean =>
  e.key === "?" && !typing && !e.ctrlKey && !e.metaKey && !e.altKey;

/** Every key the app answers to, by where it works. The one table the sheet
 * and the docs are written from; a panel's keys work while it has focus. */
export const SHORTCUTS: { where: string; keys: [string, string][] }[] = [
  {
    where: "Anywhere",
    keys: [
      ["Ctrl+K", "Find any action or panel"],
      ["?", "Show these shortcuts"],
      ["Escape", "Close a sheet, menu or form"],
    ],
  },
  {
    where: "Timeline",
    keys: [
      ["Space", "Play or pause"],
      ["J / K / L", "Back two seconds / stop / play"],
      ["Home / End", "Go to the start / end"],
      ["+ / -", "Zoom in / out"],
      ["Shift+Z", "Fit the whole recording"],
      ["S", "Split the selected segment at the playhead"],
      ["M", "Merge the selected segment with the next"],
      ["[ / ]", "Move the start edge 10 ms earlier / later"],
      ["Shift+[ / Shift+]", "Move the end edge 10 ms earlier / later"],
      ["Delete", "Delete the selected segment (asks first)"],
      ["Ctrl+Z", "Undo"],
      ["Ctrl+Shift+Z or Ctrl+Y", "Redo"],
      ["Alt + drag", "Place an edge without snapping"],
    ],
  },
  {
    where: "Transcribe",
    keys: [
      ["Enter", "Save and go to the next segment"],
      ["Shift+Enter", "Save and go back"],
      ["Tab", "Play the segment again"],
      ["Escape", "Undo what you typed in this row"],
    ],
  },
  {
    where: "Gloss",
    keys: [
      ["Enter", "Save the field (on an empty breakdown: accept the suggestion)"],
      ["Tab", "Next field"],
    ],
  },
];
