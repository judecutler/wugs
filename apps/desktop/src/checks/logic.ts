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

/** One character the project uses (src-tauri/src/characters.rs). */
export type Place = { textId: string; textTitle: string; tierId: string; segmentId: string; context: string };
export type CharRow = {
  ch: string;
  code: string;
  name: string;
  script: string;
  kind: "letter" | "mark" | "digit" | "space" | "invisible" | "other";
  inTexts: number;
  inLexicon: number;
  place: Place | null;
  entry: [string, string] | null;
  flags: string[];
};
export type Inventory = { rows: CharRow[]; mainScript: string; notNfc: number };

/** How a character is shown: a combining mark on a dotted circle, an
 * invisible or space character by its code, since it would show as nothing. */
export function glyph(r: Pick<CharRow, "ch" | "kind" | "code">): string {
  if (r.kind === "mark") return `◌${r.ch}`;
  if (r.kind === "invisible" || r.kind === "space") return r.code;
  return r.ch;
}

/** Whether the Concordance can look a character up as `*c*`: a letter or
 * mark that is not itself one of the pattern's wildcards. */
export function findable(r: Pick<CharRow, "ch" | "kind">): boolean {
  return (r.kind === "letter" || r.kind === "mark" || r.kind === "digit") && r.ch !== "*" && r.ch !== "?";
}

/** The line over the character inventory. */
export function inventoryNote(inv: Inventory): string {
  if (inv.rows.length === 0) return "No transcriptions or lexicon entries yet.";
  const letters = inv.rows.filter((r) => r.kind === "letter").length;
  const marks = inv.rows.filter((r) => r.kind === "mark").length;
  const odd = inv.rows.filter((r) => r.flags.length > 0).length;
  const script = inv.mainScript === "" ? "" : `, mostly ${inv.mainScript}`;
  const parts = [`${n(inv.rows.length, "character", "characters")} in use: ${n(letters, "letter", "letters")}${script}${marks > 0 ? `, ${n(marks, "combining mark", "combining marks")}` : ""}.`];
  parts.push(odd === 0 ? "Nothing looks like a slip." : `${n(odd, "is", "are")} worth a look:`);
  if (inv.notNfc > 0) parts.push(`${n(inv.notNfc, "transcription or headword is", "transcriptions or headwords are")} stored decomposed (from an older import); they still match, but other tools may count them apart.`);
  return parts.join(" ");
}
