// The consonant chart and vowels of the Sounds panel, copied for a paper:
// an HTML table (with tab-separated text beside it) for Word and Google
// Docs, a LaTeX tabular, or columns lined up for plain text, in the format
// chosen for Copy example. Pure, npm-tested.

import { gridOf, MANNER_NAMES, PLACE_NAMES, type Chart } from "./ipa.ts";

export type ChartFormat = "word" | "gb4e" | "expex" | "text";

/** The chart as rows of cells: a header of places, then a row per manner,
 * each cell its voiceless sounds before its voiced ones; then the vowels
 * and any other sounds as lines under it. */
export function chartRows(c: Chart): { rows: string[][]; vowels: string; others: string } {
  const { manners, places } = gridOf(c, false);
  const rows = [["", ...places.map((p) => PLACE_NAMES[p])], ...manners.map((m) => [MANNER_NAMES[m], ...places.map((p) => ((pair) => (pair === undefined ? "" : [...pair[0], ...pair[1]].map((s) => s.sound).join(" ")))(c.consonants.get(m)?.get(p)))])];
  const vowels = [...c.vowels]
    .sort((a, b) => a.at.height - b.at.height || a.at.back - b.at.back || Number(a.at.rounded) - Number(b.at.rounded))
    .map((v) => v.sound.sound)
    .join(" ");
  const others = [...c.affricates.map((s) => s.sound), ...[...c.others.values()].flat().map((s) => s.sound)].join(" ");
  return { rows: manners.length === 0 ? [] : rows, vowels, others };
}

export function chartClipboard(c: Chart, format: ChartFormat): { text: string; html: string | null } {
  const { rows, vowels, others } = chartRows(c);
  const extra = [vowels !== "" ? `Vowels: ${vowels}` : "", others !== "" ? `Also: ${others}` : ""].filter((x) => x !== "");
  if (format === "word") {
    const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    const td = (s: string, head: boolean) => `<${head ? "th" : "td"} style="border:1px solid #000;padding:2px 6px;text-align:center">${esc(s)}</${head ? "th" : "td"}>`;
    const table = rows.length === 0 ? "" : `<table style="border-collapse:collapse">${rows.map((r, i) => `<tr>${r.map((c2, j) => td(c2, i === 0 || j === 0)).join("")}</tr>`).join("")}</table>`;
    const html = table + extra.map((l) => `<p>${esc(l)}</p>`).join("");
    const text = [...rows.map((r) => r.join("\t")), ...extra].join("\n") + "\n";
    return { text, html };
  }
  if (format === "gb4e" || format === "expex") {
    const tex = (s: string) => s.replace(/\\/g, "\\textbackslash{}").replace(/([&%$#_{}])/g, "\\$1");
    const cols = rows[0]?.length ?? 0;
    const lines = rows.length === 0 ? [] : [`\\begin{tabular}{|${"c|".repeat(cols)}}`, "\\hline", ...rows.map((r) => `${r.map(tex).join(" & ")} \\\\ \\hline`), "\\end{tabular}"];
    return { text: [...lines, ...extra.map((l) => `\n${tex(l)}`)].join("\n") + "\n", html: null };
  }
  const units = new Intl.Segmenter(undefined, { granularity: "grapheme" });
  const width = (s: string) => [...units.segment(s)].length;
  const widths = (rows[0] ?? []).map((_, j) => Math.max(...rows.map((r) => width(r[j]!))));
  const text = [...rows.map((r) => r.map((c2, j) => c2 + " ".repeat(widths[j]! - width(c2))).join("  ").trimEnd()), ...extra].join("\n") + "\n";
  return { text, html: null };
}
