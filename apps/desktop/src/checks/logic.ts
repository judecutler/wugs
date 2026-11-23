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

/** A glossed root with no lexicon entry (src-tauri/src/consistency.rs `missing`). */
export type Missing = { surface: string; key: string; total: number; variants: Variant[] };
export type Added = { entries: number; senses: number; linked: number };

/** The line over the roots with no entry. */
export function missingSummary(rows: readonly Missing[]): string {
  if (rows.length === 0) return "Every glossed root has a lexicon entry.";
  const uses = rows.reduce((t, r) => t + r.total, 0);
  return `${n(rows.length, "root is", "roots are")} glossed in the texts (${n(uses, "use", "uses")}) but not in the lexicon yet.`;
}

/** What adding a root makes: an entry, with a sense per gloss. */
export function addLabel(r: Missing): string {
  return r.variants.length > 1 ? `Add with ${r.variants.length} senses` : "Add to lexicon";
}

/** The toast after adding roots. */
export function addedLine(a: Added): string {
  if (a.entries === 0) return "Nothing to add any more.";
  return `Added ${n(a.entries, "entry", "entries")} with ${n(a.senses, "sense", "senses")}, and linked ${n(a.linked, "use", "uses")} in the texts to them.`;
}
