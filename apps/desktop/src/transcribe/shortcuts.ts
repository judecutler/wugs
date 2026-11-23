// Typing shortcuts for a project's own letters, as Keyman users make them:
// `;n` becomes ŋ, `b'` becomes ɓ, as soon as the last character is typed.
// The table is a project setting, so every device and every person on the
// project types the same way. Pure, npm-tested.

export type Shortcut = { from: string; to: string };

/** At most this many characters in what is typed or what it becomes. */
export const MAX_SHORTCUT = 8;
export const MAX_SHORTCUTS = 300;

const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });
const length = (s: string): number => [...graphemes.segment(s)].length;

/** A table as written: a line each, what to type, then a space or tab, then
 * what it becomes; `#` starts a comment. Problems are named by line. */
export function parseShortcuts(text: string): { shortcuts: Shortcut[]; problems: string[] } {
  const shortcuts: Shortcut[] = [];
  const problems: string[] = [];
  const seen = new Set<string>();
  for (const [i, raw] of text.split(/\r?\n/u).entries()) {
    const line = raw.trim();
    if (line === "" || line.startsWith("#")) continue;
    const m = /^(\S+)\s+(\S+)$/u.exec(line.normalize("NFC"));
    if (m === null) {
      problems.push(`Line ${i + 1}: write what you type, a space, and what it becomes, such as ";n ŋ".`);
      continue;
    }
    const [, from, to] = m as unknown as [string, string, string];
    if (length(from) > MAX_SHORTCUT || length(to) > MAX_SHORTCUT) {
      problems.push(`Line ${i + 1}: at most ${MAX_SHORTCUT} characters on each side.`);
      continue;
    }
    if (from === to) continue;
    if (seen.has(from)) {
      problems.push(`Line ${i + 1}: «${from}» is already in the table.`);
      continue;
    }
    seen.add(from);
    shortcuts.push({ from, to });
    if (shortcuts.length > MAX_SHORTCUTS) {
      problems.push(`Only the first ${MAX_SHORTCUTS} shortcuts are used.`);
      shortcuts.pop();
      break;
    }
  }
  // The longest match wins, so `;ng` can stand beside `;n`.
  shortcuts.sort((a, b) => b.from.length - a.from.length);
  return { shortcuts, problems };
}

/** The field after typing reached `caret`: when the text just before the
 * caret ends with a shortcut, it is replaced and the caret moves after what
 * it became. Null when nothing changes. */
export function applyShortcuts(value: string, caret: number, shortcuts: readonly Shortcut[]): { value: string; caret: number } | null {
  const before = value.slice(0, caret);
  for (const s of shortcuts) {
    if (before.endsWith(s.from)) {
      const head = before.slice(0, before.length - s.from.length) + s.to;
      return { value: (head + value.slice(caret)).normalize("NFC"), caret: head.normalize("NFC").length };
    }
  }
  return null;
}

/** A starting table to edit, for the letters people most often lack a key for. */
export const EXAMPLE_TABLE = `# What you type, a space, and what it becomes.
;n ŋ
;e ɛ
;o ɔ
;i ɨ
;? ʔ
b' ɓ
d' ɗ
`;

/** What the toolbar says about the table in use. */
export function shortcutsLine(n: number): string {
  return n === 0 ? "No typing shortcuts yet." : `${n} typing ${n === 1 ? "shortcut" : "shortcuts"} on, for everyone on this project.`;
}
