// The project's language and the CLDF export, as the Texts panel shows them. Pure: no DOM.

export type Language = { name: string; glottocode: string; iso: string };

export type Published = { path: string; entries: number; senses: number; examples: number; lexiconExamples?: number; clips?: number; restrictedLeftOut: number; notes: string[] };

/** A problem with a typed code, in words, or null. Empty is fine: the codes are optional. */
export function glottocodeProblem(code: string): string | null {
  const c = code.trim().toLowerCase();
  if (c === "" || /^[a-z0-9]{4}[0-9]{4}$/u.test(c)) return null;
  return "A Glottocode is four letters and four digits, like stan1293. Find yours at glottolog.org.";
}

export function isoProblem(code: string): string | null {
  const c = code.trim().toLowerCase();
  if (c === "" || /^[a-z]{3}$/u.test(c)) return null;
  return "An ISO 639-3 code is three letters, like tur.";
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** The toast after publishing. */
/** What a dictionary export wrote, in words. */
export function dictionaryLines(r: { path: string; entries: number; senses: number; examples: number; restrictedLeftOut: number }): string[] {
  const lines = [
    `Saved the dictionary as ${r.path}: ${plural(r.entries, "entry", "entries")}, ${plural(r.senses, "sense", "senses")}, ${plural(r.examples, "example", "examples")} from the texts.`,
    "Open it in a browser and print it to PDF, or open it in Word or Google Docs to edit it.",
  ];
  if (r.restrictedLeftOut > 0) lines.push(`${plural(r.restrictedLeftOut, "example was", "examples were")} left out because ${r.restrictedLeftOut === 1 ? "its text is" : "their texts are"} not open.`);
  return lines;
}

/** What a corpus export wrote, in words. */
export function corpusLines(r: { path: string; corpus: string; texts: number; sentences: number; restrictedLeftOut: number; notes: string[] }): string[] {
  const lines = [
    `Saved the corpus ${r.corpus} in ${r.path}: ${plural(r.texts, "text", "texts")}, ${plural(r.sentences, "sentence", "sentences")}.`,
    "Run encode.sh there to load it into CQPweb or cqp, or upload the .vert file to Sketch Engine; the README says how.",
  ];
  if (r.restrictedLeftOut > 0) lines.push(`${plural(r.restrictedLeftOut, "text that is not open was", "texts that are not open were")} left out.`);
  return [...lines, ...r.notes];
}

export function publishedLines(r: Published): string[] {
  return [
    `Saved a CLDF dataset in ${r.path}.`,
    `${plural(r.entries, "entry", "entries")}, ${plural(r.senses, "sense", "senses")} and ${plural(r.examples, "glossed sentence", "glossed sentences")}${(r.lexiconExamples ?? 0) > 0 ? `, with ${plural(r.lexiconExamples ?? 0, "example sentence", "example sentences")} from the lexicon` : ""}, ready for pycldf, Zenodo or a CLLD site. Add a licence before you publish it.`,
    ...((r.clips ?? 0) > 0 ? [`${plural(r.clips ?? 0, "sentence comes", "sentences come")} with ${(r.clips ?? 0) === 1 ? "its" : "their"} sound clip, cut from the recording, in the audio folder.`] : []),
    ...r.notes,
  ];
}
