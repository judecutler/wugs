// Word order from the parsed sentences, read as WALS reads it: for each
// feature the orders counted (src-tauri/src/word_order.rs), the dominant
// one by WALS's rule (at least twice as common as the next), and the
// project's WALS answer it suggests. Pure, npm-tested.

export type Example = { textId: string; tierId: string; segmentId: string; sentence: string };
export type Order = { label: string; code: string; count: number; example: Example | null };
export type Pattern = { id: string; title: string; orders: Order[] };
export type WordOrder = { parsed: number; patterns: Pattern[] };

/** Fewer counted than this and nothing is suggested. */
export const MIN = 5;

/** WALS's code for "no dominant order" in each feature ("Mixed" where it has no such code). */
const NO_DOMINANT: Record<string, { label: string; code: string } | null> = {
  "81A": { label: "No dominant order", code: "7" },
  "82A": { label: "No dominant order", code: "3" },
  "83A": { label: "No dominant order", code: "3" },
  "85A": { label: "No dominant order", code: "4" },
  "86A": { label: "No dominant order", code: "3" },
  "87A": { label: "No dominant order", code: "3" },
  "88A": { label: "Mixed", code: "6" },
  "89A": { label: "No dominant order", code: "3" },
  "90A": { label: "Mixed", code: "7" },
  "143A": null,
};

export type Verdict = { label: string; code: string | null; why: string };

export const total = (p: Pattern): number => p.orders.reduce((n, o) => n + o.count, 0);

/** The order WALS would call dominant: one at least twice as common as any other. */
export function verdict(p: Pattern): Verdict {
  const n = total(p);
  if (n === 0) return { label: "Not found", code: null, why: "No sentence shows this construction." };
  if (n < MIN) return { label: "Too few to tell", code: null, why: `Only ${n} counted; at least ${MIN} are needed.` };
  const [first, second] = [...p.orders].sort((a, b) => b.count - a.count);
  if (second === undefined || first!.count >= 2 * second.count) return { label: first!.label, code: first!.code, why: `${first!.label} is at least twice as common as any other order.` };
  const none = NO_DOMINANT[p.id] ?? null;
  return { label: none?.label ?? "No dominant order", code: none?.code ?? null, why: `${first!.label} (${first!.count}) is not twice as common as ${second.label} (${second.count}).` };
}

/** The counts in words: "SVO 40, SOV 3". */
export const countsLine = (p: Pattern): string =>
  p.orders
    .filter((o) => o.count > 0)
    .map((o) => `${o.label} ${o.count}`)
    .join(", ");

/** The note written with a WALS answer taken from the data. */
export const answerNote = (p: Pattern, parsed: number): string => `Counted in ${parsed} parsed ${parsed === 1 ? "sentence" : "sentences"}: ${countsLine(p)}.`;

/** The line over the table. */
export function orderLine(w: WordOrder): string {
  if (w.parsed === 0) return "No sentence has a dependency parse yet. Import CoNLL-U or TEI, or tag and parse a text from the Sentence panel, and the orders are counted here.";
  const n = w.patterns.filter((p) => verdict(p).code !== null).length;
  return `Counted in ${w.parsed.toLocaleString()} parsed ${w.parsed === 1 ? "sentence" : "sentences"}; ${n} of ${w.patterns.length} features have enough to suggest an answer. Subjects and objects count only when they are nouns, as WALS counts them. A tagger's parse can be wrong: open the examples before you rely on a count.`;
}

/** Every feature as a table for a spreadsheet. */
export function orderTsv(w: WordOrder): string {
  const rows = w.patterns.map((p) => {
    const v = verdict(p);
    return [p.id, p.title, countsLine(p), v.label, v.code ?? ""].join("\t");
  });
  return [["WALS", "Feature", "Counts", "Suggested", "WALS code"].join("\t"), ...rows].join("\n") + "\n";
}

/** Grambank's questions a WALS order answers, by WALS code: Grambank asks
 * orders as one feature with "both" (GB024, GB025, GB065, GB130, GB193) or,
 * for the clause, as verb-initial / medial / final present or absent. */
const GRAMBANK: Record<string, Record<string, [string, string][]>> = {
  "81A": {
    "1": [["GB131", "0"], ["GB132", "0"], ["GB133", "1"]],
    "2": [["GB131", "0"], ["GB132", "1"], ["GB133", "0"]],
    "3": [["GB131", "1"], ["GB132", "0"], ["GB133", "0"]],
    "4": [["GB131", "1"], ["GB132", "0"], ["GB133", "0"]],
    "5": [["GB131", "0"], ["GB132", "1"], ["GB133", "0"]],
    "6": [["GB131", "0"], ["GB132", "0"], ["GB133", "1"]],
  },
  "82A": { "1": [["GB130", "1"]], "2": [["GB130", "2"]], "3": [["GB130", "3"]] },
  "86A": { "1": [["GB065", "1"]], "2": [["GB065", "2"]], "3": [["GB065", "3"]] },
  "87A": { "1": [["GB193", "1"]], "2": [["GB193", "2"]], "3": [["GB193", "3"]] },
  "88A": { "1": [["GB025", "1"]], "2": [["GB025", "2"]], "6": [["GB025", "3"]] },
  "89A": { "1": [["GB024", "1"]], "2": [["GB024", "2"]], "3": [["GB024", "3"]] },
};

/** The Grambank answers a feature's counts suggest: from its verdict, or for
 * adpositions from whether each kind is found at all (GB074 prepositions,
 * GB075 postpositions: present when counted twice or more, absent when never
 * counted among `MIN` or more adpositions). */
export function grambankAnswers(p: Pattern): [string, string][] {
  if (p.id === "85A") {
    const n = total(p);
    const of = (label: string) => p.orders.find((o) => o.label === label)?.count ?? 0;
    const say = (id: string, c: number): [string, string][] => (c >= 2 ? [[id, "1"]] : c === 0 && n >= MIN ? [[id, "0"]] : []);
    return [...say("GB074", of("Prepositions")), ...say("GB075", of("Postpositions"))];
  }
  const v = verdict(p);
  return v.code === null ? [] : (GRAMBANK[p.id]?.[v.code] ?? []);
}
