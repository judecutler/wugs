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

/** Words glossed in the texts unlike the lexicon sense they link to
 * (src-tauri/src/consistency.rs `sense_drift`). */
export type Drift = { senseId: string; entryId: string; headword: string; homograph: number | null; senseGloss: string; textGloss: string; count: number; first: Variant };

/** The line over the glosses that differ from their sense. */
export function driftSummary(rows: readonly Drift[]): string {
  if (rows.length === 0) return "Every word linked to the lexicon is glossed as its sense is.";
  const uses = rows.reduce((t, r) => t + r.count, 0);
  return `${n(uses, "linked word is", "linked words are")} glossed in the texts unlike the lexicon sense ${uses === 1 ? "it links" : "they link"} to (${n(rows.length, "gloss", "glosses")}). Keep one of the two for each.`;
}

/** What settling a drift did. */
export function settledLine(changed: number, useLexicon: boolean, gloss: string): string {
  if (!useLexicon) return `The lexicon now glosses this sense «${gloss}».`;
  return changed === 0 ? "Nothing needed changing any more." : `${n(changed, "word now reads", "words now read")} «${gloss}», as in the lexicon.`;
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

/** The Example sentences line: how many senses lack one, how many the texts can give one. */
export function exampleGapsLine(g: { without: number; fillable: number }): string {
  const senses = g.without === 1 ? "1 sense has" : `${g.without} senses have`;
  if (g.fillable === 0) return `${senses} no example sentence, and none is glossed in a text that is open yet.`;
  const can = g.fillable === g.without ? (g.without === 1 ? "it is" : "all are") : `${g.fillable} of them ${g.fillable === 1 ? "is" : "are"}`;
  return `${senses} no example sentence; ${can} glossed in a text that is open, so wugs can take one from there.`;
}

/** The line over the dependency trees with a problem. */
export function treesLine(total: number, annotated: number, shown: number): string {
  if (total === 0) return `All ${annotated === 1 ? "1 parsed sentence has" : `${annotated} parsed sentences have`} a well-formed tree: one root, no cycles, every head a word of the sentence.`;
  const of = `${total} of ${annotated} parsed ${annotated === 1 ? "sentence" : "sentences"} ${total === 1 ? "has" : "have"}`;
  const more = total > shown ? ` The first ${shown} are listed.` : "";
  return `${of} a tree problem. Click one to correct it in the Sentence panel.${more}`;
}

/** The toast after Add from the texts. */
export const examplesAddedLine = (n: number): string =>
  n === 0 ? "No sentence was added." : `Added ${n === 1 ? "an example sentence" : `${n} example sentences`} from the texts, each with its translation where it has one. Edit them in the Lexicon.`;

/** What the consent records say about a text (src-tauri/src/people.rs `consent_gaps`). */
export type TextConsent = { textId: string; title: string; records: number; no: string[]; communityOnly: boolean };
export type ConsentGaps = { texts: TextConsent[]; total: number; records: number };

/** The line over the texts: records inform, they never block. */
export function consentLine(g: ConsentGaps): string {
  if (g.total === 0) return "";
  const none = g.texts.filter((t) => t.records === 0).length;
  if (g.records === 0) return `No consent records yet. The People panel keeps who agreed to what for each recording or text; nothing in wugs waits on them.`;
  if (g.texts.length === 0) return `Every text is covered by a consent record that limits nothing.`;
  const parts = [none > 0 ? n(none, "text has no consent record covering it", "texts have no consent record covering them") : "", g.texts.length - none > 0 ? n(g.texts.length - none, "text is covered by a record that limits it", "texts are covered by records that limit them") : ""].filter((x) => x !== "");
  return `${parts.join("; ")}. Records inform what you share; they never block your work.`;
}

/** One text's card note, the terms in words through `termLabel`. */
export function consentNote(t: TextConsent, termLabel: (term: string) => string): string {
  if (t.records === 0) return "No consent record covers this text, its recording or the whole project.";
  const limits = [...t.no.map((x) => `no to «${termLabel(x)}»`), ...(t.communityOnly ? ["shared within the community only"] : [])];
  return `${t.records === 1 ? "A record" : "Its records"}: ${limits.join(", ")}.`;
}

/** Segment timing (src-tauri/src/timing.rs). */
export type TimingIssue = { kind: "overlap" | "short" | "long"; textId: string; textTitle: string; tierId: string; segmentId: string; sentence: string; startMs: number; endMs: number; other: string | null; overlapMs: number | null };
export type Timing = { timed: number; overlaps: number; short: number; long: number; issues: TimingIssue[] };

const secs = (ms: number): string => `${(ms / 1000).toFixed(ms < 10_000 ? 2 : 1)} s`;

/** The line over the list. */
export function timingLine(t: Timing): string {
  if (t.timed === 0) return "No segment has times yet.";
  const parts = [t.overlaps > 0 ? `${t.overlaps} ${t.overlaps === 1 ? "overlaps" : "overlap"} the one before it on its track` : "", t.short > 0 ? `${t.short} ${t.short === 1 ? "is" : "are"} under 0.15 s` : "", t.long > 0 ? `${t.long} ${t.long === 1 ? "is" : "are"} over 30 s` : ""].filter((x) => x !== "");
  if (parts.length === 0) return `All ${t.timed} timed segments are fine: none overlap on a track, and none is under 0.15 s or over 30 s.`;
  return `Of ${t.timed} timed segments, ${parts.join(", ")}. Overlaps on one track confuse ELAN and Praat; very short ones are often stray clicks; speech tools take at most 30 s.`;
}

/** One issue in words. */
export function timingNote(i: TimingIssue): string {
  const span = `${secs(i.startMs)}–${secs(i.endMs)}`;
  if (i.kind === "overlap") return `Starts ${secs(i.overlapMs ?? 0)} before the segment before it ends (${span})`;
  if (i.kind === "short") return `Only ${secs(i.endMs - i.startMs)} long (${span})`;
  return `${secs(i.endMs - i.startMs)} long (${span}): split it for speech tools`;
}

/** Archive readiness (src-tauri/src/archive_sheet.rs `readiness`). */
export type TextReadiness = { textId: string; title: string; missing: string[]; sentences: number; transcribed: number; translated: number };
export type Readiness = { total: number; ready: number; texts: TextReadiness[]; language: string[] };

const LACKS: Record<string, string> = {
  date: "date",
  place: "place",
  speakers: "speakers",
  genre: "genre",
  description: "description",
  recording: "recording",
  consent: "consent record",
};

/** The line over the list. */
export function readinessLine(r: Readiness): string {
  if (r.total === 0) return "";
  const head = r.ready === r.total ? `Every text has what an archive's item form asks.` : `${r.ready} of ${n(r.total, "text has", "texts have")} what an archive's item form asks.`;
  const language = r.language.length === 0 ? "" : ` The project's language has no ${r.language.map((x) => (x === "iso639_3" ? "ISO 639-3 code" : x === "glottocode" ? "Glottocode" : "name")).join(" or ")} yet (Texts panel, Language).`;
  return `${head}${language} Nothing here stops a deposit; it is what ELAR, PARADISEC and AILLA will ask for.`;
}

/** What one text lacks, in words: details first, then the work left. */
export function readinessNote(t: TextReadiness): string {
  const lacks = t.missing.filter((m) => m in LACKS).map((m) => LACKS[m]!);
  const parts: string[] = [];
  if (lacks.length > 0) parts.push(`missing: ${lacks.length === 1 ? lacks[0] : `${lacks.slice(0, -1).join(", ")} and ${lacks[lacks.length - 1]}`}`);
  if (t.missing.includes("transcription")) parts.push(t.sentences === 0 ? "nothing transcribed yet" : `${t.sentences - t.transcribed} of ${n(t.sentences, "sentence", "sentences")} still to transcribe`);
  if (t.missing.includes("translation")) parts.push(`${t.transcribed - t.translated} transcribed ${t.transcribed - t.translated === 1 ? "sentence" : "sentences"} without a translation`);
  const out = parts.join("; ");
  return out === "" ? "" : `${out.charAt(0).toUpperCase()}${out.slice(1)}.`;
}

export type SentenceCopy = { textId: string; textTitle: string; tierId: string; segmentId: string };
export type Twice = { sentence: string; copies: SentenceCopy[] };
export type Duplicates = { listed: Twice[]; groups: number; extra: number; read: number };

/** The line over the sentences written twice. */
export function twiceLine(d: Duplicates): string {
  if (d.read === 0) return "No sentences of four words or more to compare yet.";
  if (d.groups === 0) return `None of the ${d.read} sentences of four words or more is written twice.`;
  return `${d.groups} ${d.groups === 1 ? "sentence is" : "sentences are"} written more than once (the same words in the same order), ${d.extra} ${d.extra === 1 ? "copy" : "copies"} beyond the first, which word counts take as uses of their own: a text imported twice, or a sentence pasted again. Sentences under four words are left out, since short replies repeat.`;
}

/** Where one sentence's copies are, in words. */
export function twiceNote(t: Twice): string {
  const texts = new Set(t.copies.map((c) => c.textId)).size;
  return texts === 1 ? `${t.copies.length} times in «${t.copies[0]!.textTitle}»` : `${t.copies.length} times in ${texts} texts`;
}

/** One way a word is broken into morphs (src-tauri/src/consistency.rs `segmentations`). */
export type Breakdown = { breakdown: string; glosses: string; count: number; tokenId: string; textId: string; textTitle: string; tierId: string; segmentId: string };
export type Segmented = { form: string; key: string; total: number; variants: Breakdown[] };

/** The line over the words broken up more than one way. */
export function breakdownsLine(list: readonly Segmented[]): string {
  if (list.length === 0) return "Every analysed word is broken into morphs the same way each time it occurs.";
  return `${n(list.length, "word is", "words are")} broken into morphs more than one way. One may be a slip, or two words that are spelled alike (then keep both).`;
}

/** What tells a form's breakdowns apart, in words. */
export function breakdownNote(s: Segmented): string {
  const whole = s.variants.find((v) => !v.breakdown.includes("-"));
  if (whole !== undefined && s.variants.length === 2) return `Analysed as one morph ${whole.count === 1 ? "once" : `${whole.count} times`} and in parts elsewhere.`;
  const glosses = new Set(s.variants.map((v) => v.glosses));
  return glosses.size === 1 ? "Cut in different places with the same glosses." : "Cut differently, with different glosses.";
}

/** The button that makes one breakdown everyone's. */
export function useBreakdownLabel(v: Breakdown, s: Segmented, armed: boolean): string {
  return armed ? `Change ${s.total - v.count}: sure?` : `Use «${v.breakdown}» for all ${s.total}`;
}

/** The toast after it. */
export function breakdownUsedLine(changed: number, breakdown: string): string {
  return changed === 0 ? "Nothing needed changing any more." : `${n(changed, "word is", "words are")} now «${breakdown}».`;
}
