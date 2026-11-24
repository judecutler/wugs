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
export function dictionaryLines(r: { path: string; entries: number; senses: number; examples: number; restrictedLeftOut: number; topics?: number }): string[] {
  const lines = [
    `Saved the dictionary as ${r.path}: ${plural(r.entries, "entry", "entries")}, ${plural(r.senses, "sense", "senses")}, ${plural(r.examples, "example", "examples")} from the texts.`,
    "Open it in a browser and print it to PDF, or open it in Word or Google Docs to edit it.",
  ];
  if ((r.topics ?? 0) > 0) lines.push(`Words by topic lists the entries under ${plural(r.topics!, "semantic domain", "semantic domains")}.`);
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

/** What "Publish as a website" wrote (src-tauri/src/website.rs). */
export type Site = { path: string; texts: number; sentences: number; entries: number; clips: number; restrictedLeftOut: number; clipsMissing: number; headwordClips?: number };

export function siteLines(r: Site): string[] {
  const holds = [
    r.texts > 0 ? `${plural(r.texts, "text", "texts")} (${plural(r.sentences, "sentence", "sentences")})` : null,
    r.entries > 0 ? `a dictionary of ${plural(r.entries, "entry", "entries")}` : null,
  ].filter((x) => x !== null);
  const lines = [`Saved a website in ${r.path}: ${holds.join(" and ")}. Open index.html in a browser, copy the folder to a USB stick, or put it on any web host.`];
  const words = r.headwordClips ?? 0;
  const sentences = r.clips - words;
  if (sentences > 0) lines.push(`${plural(sentences, "sentence plays", "sentences play")} from ${sentences === 1 ? "its" : "their"} recording.`);
  if (words > 0) lines.push(`${plural(words, "dictionary headword plays", "dictionary headwords play")} as ${words === 1 ? "it is" : "they are"} said.`);
  if (r.clipsMissing > 0) lines.push(`${plural(r.clipsMissing, "timed sentence has", "timed sentences have")} no sound: the recording is not on this computer, not linked, or not open.`);
  if (r.restrictedLeftOut > 0) lines.push(`${plural(r.restrictedLeftOut, "text that is not open was", "texts that are not open were")} left out.`);
  lines.push("Notes are never published. Check that the speakers agreed before you share it.");
  return lines;
}

export type Sheet = { path: string; texts: number; withoutRecording: number; notOpen: number };

export function sheetLines(r: Sheet): string[] {
  const lines = [`Saved a catalogue sheet of ${plural(r.texts, "text", "texts")} in ${r.path}: a row each with its details, language, recording and progress, to fill an archive's item forms from.`];
  if (r.withoutRecording > 0) lines.push(`${plural(r.withoutRecording, "text has", "texts have")} no recording linked here, so no file name or length.`);
  if (r.notOpen > 0) lines.push(`${plural(r.notOpen, "text is", "texts are")} not open; the access column says so.`);
  return lines;
}

export type Flashcards = { path: string; cards: number; sounds: number; noGloss: number };

export function flashcardLines(r: Flashcards): string[] {
  const lines = [`Saved ${plural(r.cards, "flashcard", "flashcards")} in ${r.path}: anki.txt for Anki (each word asked both ways) and quizlet.txt for Quizlet. The README there says how to import them.`];
  if (r.sounds > 0) lines.push(`${plural(r.sounds, "card plays", "cards play")} the word as it is said: copy the sounds folder into Anki's media folder first.`);
  else lines.push("No card has a sound yet: give an entry a pronunciation heard in a recording, in the Lexicon.");
  if (r.noGloss > 0) lines.push(`${plural(r.noGloss, "entry with no gloss was", "entries with no gloss were")} left out.`);
  return lines;
}

export type Spelling = { path: string; words: number; fromLexicon: number; once: number; code: string };

/** The toast after a spelling dictionary was saved. */
export function spellingLines(r: Spelling): string[] {
  const lines = [`Saved a spelling dictionary of ${plural(r.words, "word", "words")} in ${r.path}: ${r.code}.dic and ${r.code}.aff for LibreOffice, Firefox and other Hunspell spell checkers, and words.tsv as a plain list. The README there says how to install it.`];
  if (r.code === "und") lines.push("Set the language's ISO 639-3 code in the Language row to name the files after it.");
  if (r.once > 0) lines.push(`${plural(r.once, "word met only once and not in the lexicon was", "words met only once and not in the lexicon were")} left out, since many are slips.`);
  return lines;
}

export function publishedLines(r: Published): string[] {
  return [
    `Saved a CLDF dataset in ${r.path}.`,
    `${plural(r.entries, "entry", "entries")}, ${plural(r.senses, "sense", "senses")} and ${plural(r.examples, "glossed sentence", "glossed sentences")}${(r.lexiconExamples ?? 0) > 0 ? `, with ${plural(r.lexiconExamples ?? 0, "example sentence", "example sentences")} from the lexicon` : ""}, ready for pycldf, Zenodo or a CLLD site. Add a licence before you publish it.`,
    ...((r.clips ?? 0) > 0 ? [`${plural(r.clips ?? 0, "sentence comes", "sentences come")} with ${(r.clips ?? 0) === 1 ? "its" : "their"} sound clip, cut from the recording, in the audio folder.`] : []),
    ...r.notes,
  ];
}

/** A language or dialect in Glottolog. */
export type Languoid = { glottocode: string; iso: string; name: string; dialect: boolean; family: string; macroarea: string; language: string };

/** Whether a typed name is worth looking up: two letters, or a code. */
export function worthLooking(typed: string): boolean {
  return typed.trim().length >= 2;
}

/** A Glottolog match in words, for its button: what it is and where. */
export function languoidLine(l: Languoid): string {
  const kind = l.dialect ? (l.language !== "" ? `a dialect of ${l.language}` : "a dialect") : l.family === "isolate" ? "an isolate" : l.family !== "" ? l.family : "unclassified";
  const area = l.macroarea.split(";").filter((a) => a !== "").join(", ");
  const codes = [l.iso, l.glottocode].filter((c) => c !== "").join(" · ");
  return `${l.name} — ${codes}; ${kind}${area !== "" ? `, ${area}` : ""}`;
}

/** What picking a match writes: Glottolog's codes, and its name unless the
 * person already named the language themselves (their own name stays, such
 * as a name the community uses). */
export function pickedLanguage(now: Language, l: Languoid, typed: string): Language {
  const ownName = now.name.trim() !== "" && typed.trim() === "";
  return { name: ownName ? now.name : l.name, glottocode: l.glottocode, iso: l.iso };
}
