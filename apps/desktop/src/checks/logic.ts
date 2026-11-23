// The Checks panel's words (pure, so node --test runs them): morphs
// glossed more than one way across the project (src-tauri/src/consistency.rs).

export type Variant = { gloss: string; count: number; textId: string; textTitle: string; tierId: string; segmentId: string };
export type Inconsistent = { surface: string; kind: string; key: string; total: number; variants: Variant[]; slip: boolean };

const n = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`;

/** The line over the list. */
export function summary(rows: readonly Inconsistent[]): string {
  if (rows.length === 0) return "Every morph is glossed one way. Nothing to check.";
  const slips = rows.filter((r) => r.slip).length;
  const head = `${n(rows.length, "morph is", "morphs are")} glossed more than one way.`;
  return slips === 0 ? head : `${head} ${n(slips, "looks", "look")} like a typing slip (the glosses differ only in capitals, spaces or dots).`;
}

/** What a card says under the morph. */
export function cardNote(r: Inconsistent): string {
  if (r.slip) return "Probably a slip: the glosses differ only in capitals, spaces or dots.";
  return "Two meanings, or a slip? If these are different morphs that sound alike, link each to its own lexicon entry and this goes away.";
}

/** The button that makes one gloss the only one, and its armed form. */
export function useLabel(v: Variant, r: Inconsistent, armed: boolean): string {
  const others = r.total - v.count;
  return armed ? `Change ${others}: sure?` : `Use «${v.gloss}» for all ${r.total}`;
}

/** The toast after a change. */
export function unifiedLine(changed: number, gloss: string): string {
  return changed === 0 ? "Nothing needed changing any more." : `${n(changed, "morph now reads", "morphs now read")} «${gloss}».`;
}
