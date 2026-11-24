// Words around a search, position by position (Sketch Engine's and
// LancsBox's frequency by position): for the lines shown, the commonest
// words one, two and three words before the match and after it. Pure.

import { nearKey } from "./near.ts";
import type { Hit } from "./logic.ts";

/** How far either side is counted, and how many words each column lists. */
export const SPAN = 3;
export const TOP = 8;

export type PositionColumn = { offset: number; label: string; rows: { word: string; count: number }[]; lines: number };

/** A column per position, L3 … L1 then R1 … R3, each word counted once per line, case and edge punctuation aside. */
export function positions(hits: readonly Hit[], span = SPAN, top = TOP): PositionColumn[] {
  const offsets = [...Array.from({ length: span }, (_, i) => -(span - i)), ...Array.from({ length: span }, (_, i) => i + 1)];
  return offsets.map((offset) => {
    const counts = new Map<string, { n: number; spellings: Map<string, number> }>();
    let lines = 0;
    for (const h of hits) {
      const i = offset < 0 ? h.at + offset : h.at + (h.len ?? 1) - 1 + offset;
      const word = h.words[i];
      if (word === undefined) continue;
      const key = nearKey(word);
      if (key === "") continue;
      lines += 1;
      const c = counts.get(key) ?? { n: 0, spellings: new Map() };
      c.n += 1;
      const spelled = word.normalize("NFC").replace(/^[\p{P}\p{S}]+|[\p{P}\p{S}]+$/gu, "");
      c.spellings.set(spelled, (c.spellings.get(spelled) ?? 0) + 1);
      counts.set(key, c);
    }
    const rows = [...counts.values()]
      .map((c) => ({ word: [...c.spellings].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))[0]![0], count: c.n }))
      .sort((a, b) => b.count - a.count || a.word.localeCompare(b.word))
      .slice(0, top);
    return { offset, label: offset < 0 ? `${-offset} before` : `${offset} after`, rows, lines };
  });
}

/** The columns as a table to paste: a row per rank, word and count per position. */
export function positionsTsv(cols: readonly PositionColumn[]): string {
  const depth = Math.max(0, ...cols.map((c) => c.rows.length));
  const head = ["rank", ...cols.flatMap((c) => [c.label, "n"])].join("\t");
  const rows = Array.from({ length: depth }, (_, r) => [String(r + 1), ...cols.flatMap((c) => (c.rows[r] === undefined ? ["", ""] : [c.rows[r]!.word, String(c.rows[r]!.count)]))].join("\t"));
  return [head, ...rows].join("\n") + "\n";
}
