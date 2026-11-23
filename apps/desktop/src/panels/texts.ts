import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { lexiconList, orthographyGet, projectAffixes, projectProgress, soundWords, typologyAnswers, typologyProfile } from "../api";
import { featuresOf, grammarSketch, graphemesOf, morphemesOf, type Feature } from "../grammar/sketch";
import { defaultTrack } from "../sounds/ipa";
import { toGrid } from "../orthography/logic";
import { SOURCES } from "../typology/logic";
import { marked, slotChart, slotLabel } from "../affixes/logic";
import { abbreviationMeanings, createText, errorText, exportCldf, exportWebsite, exportCorpus, exportDictionary, exportLexicon, exportLexiconTable, exportText, importLexiconTable, glossGet, glottologFind, importFile, projectLanguage, saveDocument, textMetadata, sealProject, setProjectLanguage, type LexiconFormat, type TextDetail, type TextFormat } from "../api";
import { corpusLines, dictionaryLines, glottocodeProblem, isoProblem, languoidLine, pickedLanguage, publishedLines, siteLines, worthLooking, type Language, type Languoid } from "../cldf/logic";
import { glossed, numbering, textDocument } from "../gloss/document";
import { abbreviationList, abbreviationsOf } from "../gloss/abbreviations";
import { EXAMPLE_FORMATS, type Example, type ExampleFormat } from "../gloss/example";
import { translationLines } from "../gloss/translation";
import { detailsSection, importMetadataTable } from "../metadata/section";
import { megabytes } from "../assist/logic";
import { localDay } from "../dates";
import { readerDetails, readerHtml, type ReaderSentence } from "../reader/booklet";
import { emptyState, h } from "../dom";
import { attempt, getState, reloadTexts, selectText, showToast, touchLexicon } from "../store";
import { tableImportLines } from "../lexicon/logic";
import { sliced, type PanelDef } from "./panel";

async function newText(title: string) {
  if (title === "") return;
  await attempt(async () => {
    const textId = await createText({ title });
    await reloadTexts();
    await selectText(textId);
  });
}

const TEXT_FORMATS: [TextFormat, string][] = [
  ["elan", "ELAN (.eaf)"],
  ["flextext", "FLEx (.flextext)"],
  ["textgrid", "Praat (.TextGrid)"],
  ["conllu", "CoNLL-U, Universal Dependencies (.conllu)"],
  ["tei", "TEI P5 (.xml)"],
  ["vertical", "CWB vertical: CQPweb, Sketch Engine (.vert)"],
  ["antconc", "AntConc, words tagged word_TAG (.txt)"],
  ["toolboxtext", "Toolbox interlinear text (.txt)"],
  ["plaintext", "Plain text, a line per segment (.txt)"],
  ["table", "Spreadsheet, a row per sentence (.csv)"],
  ["srt", "Subtitles for a video, SubRip (.srt)"],
  ["vtt", "Subtitles for the web, WebVTT (.vtt)"],
];
const LEXICON_FORMATS: [LexiconFormat | "table", string][] = [
  ["lift", "LIFT (FLEx, WeSay)"],
  ["toolbox", "Toolbox (.txt)"],
  ["table", "Spreadsheet, a row per sense (.csv)"],
];

// Kept across redraws, so a choice survives a text being added elsewhere.
let textFormat: TextFormat = "elan";
let lexiconFormat: LexiconFormat | "table" = "lift";
let busy: string | null = null;
// The project's language, fetched once per project; the CLDF choice resets with it.
let language: { project: string; value: Language } | null = null;
let languageFor: string | null = null;
let includeRestricted = false;
/** Whether a CLDF dataset carries a sound clip per timed example (remembered on this computer). */
const AUDIO_KEY = "wugs.cldf.audio";
let cldfAudio = ((): boolean => {
  try {
    return localStorage.getItem(AUDIO_KEY) !== "off";
  } catch {
    return true;
  }
})();
// What is typed but not saved yet, so a redraw while typing keeps it.
const drafts: Partial<Language> = {};
let exampleFormat: ExampleFormat = "expex";
// Every open copy of the panel, so the one showing when work ends redraws,
// even if the panel was closed and reopened while it ran.
const live = new Set<() => void>();
const redrawAll = () => {
  for (const draw of live) draw();
};

async function chooseFolder(title: string): Promise<string | null> {
  const folder = await pickFile({ directory: true, title });
  return typeof folder === "string" ? folder : null;
}

/** Run one import or export at a time, showing what it is doing on its button. */
async function work(label: string, action: () => Promise<void>) {
  if (busy !== null) return;
  busy = label;
  redrawAll();
  try {
    await action();
  } catch (e) {
    showToast({ tone: "error", lines: [errorText(e)] });
  }
  busy = null;
  redrawAll();
}

async function importOne() {
  const path = await pickFile({
    title: "Import a file from ELAN, FLEx, Praat, Toolbox, CoNLL-U or TEI, or plain text",
    filters: [
      { name: "ELAN, FLEx, Praat, Toolbox, CoNLL-U, TEI, tables of sentences, subtitles, plain text", extensions: ["eaf", "flextext", "lift", "TextGrid", "textgrid", "db", "sfm", "lex", "conllu", "conll", "xml", "tei", "csv", "tsv", "srt", "vtt", "txt"] },
      { name: "All files", extensions: ["*"] },
    ],
  });
  if (typeof path !== "string") return;
  await work("import", async () => {
    const r = await importFile({ path });
    // The report first: a failed refresh below must not hide that the import worked.
    showToast({ tone: "info", lines: [`Imported ${r.format}.`, ...r.lines] });
    if (r.lexicon) touchLexicon();
    await attempt(async () => {
      await reloadTexts();
      if (r.text !== null) await selectText(r.text);
    });
  });
}

async function importDetails() {
  const path = await pickFile({
    title: "Import document details: a table with a row per text (a title or file column, then author, date, genre…)",
    filters: [
      { name: "Tables", extensions: ["csv", "tsv", "txt"] },
      { name: "All files", extensions: ["*"] },
    ],
  });
  if (typeof path !== "string") return;
  await work("details", () => importMetadataTable(path));
}

async function importLexTable() {
  const path = await pickFile({
    title: "Import a lexicon table: a row per sense, with a word (lemma or headword) column and a gloss column; category, definition and glosses in other languages are read too",
    filters: [
      { name: "Tables", extensions: ["csv", "tsv", "txt"] },
      { name: "All files", extensions: ["*"] },
    ],
  });
  if (typeof path !== "string") return;
  await work("lexicon-table", async () => {
    const r = await importLexiconTable({ path });
    touchLexicon();
    showToast({ tone: r.ambiguous.length > 0 ? "warn" : "info", lines: tableImportLines(r) });
  });
}

async function exportOne(textId: string) {
  const folder = await chooseFolder("Choose where the exported text goes");
  if (folder === null) return;
  await work("text", async () => {
    const r = await exportText({ textId, format: textFormat, folder });
    showToast({ tone: r.notes.length > 0 ? "warn" : "info", lines: [`Saved ${r.path}.`, ...r.notes] });
  });
}

async function exportLex() {
  const folder = await chooseFolder("Choose where the exported lexicon goes");
  if (folder === null) return;
  await work("lexicon", async () => {
    if (lexiconFormat === "table") {
      const path = await exportLexiconTable({ folder });
      showToast({ tone: "info", lines: [`Saved ${path}.`, "A row per sense; Excel, LibreOffice and Google Sheets open it, and Import a lexicon table reads it back."] });
      return;
    }
    const r = await exportLexicon({ format: lexiconFormat, folder });
    showToast({ tone: "info", lines: [`Saved ${r.path}.`, ...r.notes] });
  });
}

async function seal() {
  const folder = await chooseFolder("Choose where the archive package goes");
  if (folder === null) return;
  await work("seal", async () => {
    const r = await sealProject({ folder, date: localDay(new Date()) });
    showToast({
      tone: r.restricted ? "warn" : "info",
      lines: [
        `Sealed ${r.texts} text${r.texts === 1 ? "" : "s"} and ${r.lexemes} lexicon entr${r.lexemes === 1 ? "y" : "ies"} into ${r.path} (${r.files} files, ${megabytes(r.bytes)}).`,
        "It holds ELAN, FLEx and LIFT copies, archive metadata, every recording and the full history, with checksums (BagIt), ready to deposit.",
        ...(r.restricted ? ["Some of the project is restricted, and the package holds it too: treat the package as restricted."] : []),
      ],
    });
  });
}

async function loadLanguage(project: string) {
  if (languageFor === project) return;
  languageFor = project;
  includeRestricted = false;
  lookup += 1;
  matchesBox.replaceChildren();
  for (const k of Object.keys(drafts) as (keyof Language)[]) delete drafts[k];
  try {
    language = { project, value: await projectLanguage() };
  } catch {
    language = { project, value: { name: "", glottocode: "", iso: "" } };
  }
  redrawAll();
}

async function saveLanguage(project: string, next: Language) {
  const problem = glottocodeProblem(next.glottocode) ?? isoProblem(next.iso);
  if (problem !== null) {
    showToast({ tone: "warn", lines: [problem] });
    return;
  }
  const now = language?.value;
  if (now !== undefined && now.name === next.name.trim() && now.glottocode === next.glottocode.trim().toLowerCase() && now.iso === next.iso.trim().toLowerCase()) return;
  try {
    const saved = await setProjectLanguage({ language: next });
    if (languageFor === project) language = { project, value: saved };
  } catch (e) {
    showToast({ tone: "error", lines: [errorText(e)] });
  }
  redrawAll();
}

/** The Glottolog matches under the Language row, filled in place as the
 * person types (never by a redraw, so the field keeps its focus). */
let lookup = 0;
const matchesBox = h("div", { className: "glotto-matches" });
matchesBox.setAttribute("role", "list");
matchesBox.setAttribute("aria-label", "Glottolog matches");

function showMatches(project: string, hits: Languoid[], typedName: string) {
  matchesBox.replaceChildren(
    ...hits.map((l) => {
      const b = h("button", { type: "button", className: "glotto-match", textContent: languoidLine(l), title: "Use this language's Glottocode and ISO code" });
      b.setAttribute("role", "listitem");
      // Picking with the mouse keeps focus in the field, so its half-typed
      // text is not saved as the name first.
      b.onmousedown = (ev) => ev.preventDefault();
      b.onclick = () => {
        const now = language?.project === project ? language.value : { name: "", glottocode: "", iso: "" };
        for (const k of Object.keys(drafts) as (keyof Language)[]) delete drafts[k];
        matchesBox.replaceChildren();
        void saveLanguage(project, pickedLanguage(now, l, typedName));
      };
      return b;
    }),
    ...(hits.length > 0 ? [h("p", { className: "note", textContent: "From Glottolog 5.4 (CC BY 4.0), kept in the app: nothing is sent." })] : []),
  );
}

/** Look a typed name or code up in Glottolog, a moment after typing stops. */
function lookUp(project: string, typed: string, fromName: boolean) {
  const mine = ++lookup;
  if (!worthLooking(typed)) {
    matchesBox.replaceChildren();
    return;
  }
  window.setTimeout(async () => {
    if (mine !== lookup) return;
    try {
      const hits = await glottologFind({ query: typed });
      if (mine === lookup) showMatches(project, hits, fromName ? typed : "");
    } catch {
      matchesBox.replaceChildren();
    }
  }, 150);
}

/** Name, Glottocode and ISO 639-3, each saved on Enter or when focus leaves
 * it; typing a name or a code offers Glottolog's matches under the row. */
function languageRow(project: string): HTMLElement {
  const v = language?.project === project ? language.value : { name: "", glottocode: "", iso: "" };
  const field = (key: keyof Language, placeholder: string, title: string, size: number) => {
    const input = h("input", { value: drafts[key] ?? v[key], placeholder, title, size, spellcheck: false });
    input.setAttribute("aria-label", `Language: ${placeholder}`);
    input.oninput = () => {
      drafts[key] = input.value;
      if (key !== "iso") lookUp(project, input.value, key === "name");
    };
    const commit = () => {
      delete drafts[key];
      void saveLanguage(project, { ...v, [key]: input.value });
    };
    input.onchange = commit;
    input.onkeydown = (ev: KeyboardEvent) => {
      if (ev.key === "Enter" && !ev.isComposing) commit();
      if (ev.key === "Escape") matchesBox.replaceChildren();
    };
    return input;
  };
  return h(
    "div",
    { className: "glotto" },
    h(
      "div",
      { className: "share-row" },
      h("span", { className: "share-label", textContent: "Language" }),
      field("name", "Name", "The language this project documents, as you call it; typing it offers Glottolog's matches", 14),
      field("glottocode", "Glottocode", "From glottolog.org, like stan1293: lets the data join other datasets. Typing a name or code here offers matches too", 9),
      field("iso", "ISO 639-3", "Three letters, like tur: archives use it in their catalogue", 9),
    ),
    matchesBox,
  );
}

async function exportDict() {
  const folder = await chooseFolder("Choose where the dictionary goes");
  if (folder === null) return;
  await work("dictionary", async () => {
    const r = await exportDictionary({ folder, date: localDay(new Date()), includeRestricted });
    showToast({ tone: r.restrictedLeftOut > 0 ? "warn" : "info", lines: dictionaryLines(r) });
  });
}

async function exportCwb() {
  const folder = await chooseFolder("Choose where the corpus goes");
  if (folder === null) return;
  await work("cwb", async () => {
    const r = await exportCorpus({ folder, date: localDay(new Date()), includeRestricted });
    showToast({ tone: r.notes.length > 0 || r.restrictedLeftOut > 0 ? "warn" : "info", lines: corpusLines(r) });
  });
}

async function publishCldf() {
  const folder = await chooseFolder("Choose where the CLDF dataset goes");
  if (folder === null) return;
  await work("cldf", async () => {
    const r = await exportCldf({ folder, date: localDay(new Date()), includeRestricted, audio: cldfAudio });
    showToast({ tone: r.notes.length > 0 ? "warn" : "info", lines: publishedLines(r) });
  });
}

async function publishSite() {
  const folder = await chooseFolder("Choose where the website goes");
  if (folder === null) return;
  await work("site", async () => {
    const r = await exportWebsite({ folder, date: localDay(new Date()), includeRestricted, audio: cldfAudio });
    showToast({ tone: r.restrictedLeftOut > 0 || r.clipsMissing > 0 ? "warn" : "info", lines: siteLines(r) });
  });
}

/** A grammar sketch to start from: what the project knows about its sounds,
 * spelling, morphemes, typology and texts, with places to write. */
async function exportSketch() {
  const folder = await chooseFolder("Choose where the grammar sketch goes");
  if (folder === null) return;
  await work("sketch", async () => {
    const [language, progress, entries, ortho] = await Promise.all([projectLanguage(), projectProgress(), lexiconList({ query: "" }), orthographyGet()]);
    const first = await soundWords({ tierId: null, textId: null });
    const track = defaultTrack(first.tracks);
    const words = track === null ? first : await soundWords({ tierId: track, textId: null });
    let features: Feature[] = [];
    let featureSource = "";
    if (language.glottocode !== "") {
      for (const [source] of SOURCES) {
        const ours = await typologyAnswers({ source });
        if (ours.length === 0) continue;
        const profile = await typologyProfile({ glottocode: language.glottocode, source });
        const found = featuresOf(profile.features, ours);
        if (found.length > features.length) {
          features = found;
          featureSource = profile.edition;
        }
      }
    }
    const morphemes = morphemesOf(entries);
    const affixes = await projectAffixes().catch(() => null);
    const slots = affixes === null ? [] : slotChart(affixes.affixes).map((c) => ({ label: slotLabel(c.slot), affixes: c.affixes.map((a) => `${marked(a)} ${a.gloss}`.trim()) }));
    const graphemes = graphemesOf(toGrid(ortho.profile));
    const html = grammarSketch({
      language,
      words: track === null ? [] : words.words,
      soundTrack: first.tracks.find((t) => t.id === track)?.name ?? "",
      graphemes,
      morphemes,
      slots,
      entries: progress.entries,
      features,
      featureSource,
      texts: progress.texts.map((t) => ({ title: t.title, sentences: t.sentences, words: t.words, glossed: t.glossedWords })),
      date: localDay(new Date()),
      generator: "wugs",
    });
    const r = await saveDocument({ folder, title: `${language.name.trim() || "Language"} grammar sketch`, extension: "html", contents: html });
    const missing = [track === null && "sounds (no transcriptions)", graphemes.length === 0 && "an orthography (no profile kept)", morphemes.length === 0 && "affixes (no entries written like -ler or ni-)", features.length === 0 && "typology (no answers in the Typology panel)"].filter((x): x is string => typeof x === "string");
    showToast({
      tone: "info",
      lines: [`Saved a grammar sketch in ${r.path}. Open it in Word or LibreOffice and write in the bracketed places.`, ...(missing.length > 0 ? [`Still to fill: ${missing.join(", ")}.`] : [])],
    });
  });
}

/** The text's glossed sentences as numbered examples, read through the Gloss panel's own view. */
async function examplesOf(text: TextDetail): Promise<{ examples: Example[]; skipped: number }> {
  const sentences = text.tiers.find((t) => t.utterance)?.id ?? text.segments.find((s) => !s.parent)?.tier ?? "";
  const segs = text.segments.filter((s) => s.tier === sentences && !s.parent);
  const numbers = numbering(segs);
  const examples: Example[] = [];
  let skipped = 0;
  for (const seg of [...segs].sort((a, b) => (numbers.get(a.id) ?? 0) - (numbers.get(b.id) ?? 0))) {
    const v = await glossGet({ textId: text.id, segmentId: seg.id });
    const translation = translationLines(seg, text.segments, text.tiers).find((l) => l.segment !== null)?.segment?.content ?? null;
    const ex: Example = { number: numbers.get(seg.id) ?? 0, translation, words: v.words.map((w) => ({ form: w.form, morphs: w.morphs.map((m) => ({ surface: m.surface, type: m.type, gloss: m.gloss })) })) };
    if (glossed(ex)) examples.push(ex);
    else skipped += 1;
  }
  return { examples, skipped };
}

async function exportExamples(text: TextDetail) {
  const folder = await chooseFolder("Choose where the glossed examples go");
  if (folder === null) return;
  await work("examples", async () => {
    const { examples, skipped } = await examplesOf(text);
    if (examples.length === 0) {
      showToast({ tone: "warn", lines: ["No sentence of this text is glossed yet. Break a sentence into morphs in the Gloss panel first."] });
      return;
    }
    const used = abbreviationsOf(examples);
    const meanings = used.length === 0 ? [] : await abbreviationMeanings(used);
    const abbreviations = abbreviationList(used, new Map(used.flatMap((a, i) => (meanings[i] ? [[a, meanings[i]!] as [string, string]] : []))));
    const unexplained = abbreviations.filter((a) => a.source === "none").length;
    const doc = textDocument(text.title || "Examples", examples, exampleFormat, abbreviations);
    const r = await saveDocument({ folder, title: `${text.title || "text"} examples`, extension: doc.extension, contents: doc.contents });
    const label = EXAMPLE_FORMATS.find((f) => f.id === exampleFormat)?.label ?? "";
    showToast({
      tone: "info",
      lines: [
        `Saved ${examples.length} glossed ${examples.length === 1 ? "example" : "examples"} for ${label} in ${r.path}.`,
        ...(skipped > 0 ? [`${skipped} ${skipped === 1 ? "sentence is" : "sentences are"} not glossed yet and left out.`] : []),
        ...(abbreviations.length > 0 ? [`The ${abbreviations.length} ${abbreviations.length === 1 ? "abbreviation" : "abbreviations"} the glosses use are listed after the examples.`] : []),
        ...(unexplained > 0 ? [`${unexplained} of them ${unexplained === 1 ? "has" : "have"} no meaning yet and ${unexplained === 1 ? "shows" : "show"} “?”: write ${unexplained === 1 ? "it" : "them"} under Abbreviations in the Interlinear panel.`] : []),
        ...(doc.extension === "tex" ? ["Compile it with XeLaTeX or LuaLaTeX, which read IPA and every script."] : []),
      ],
    });
  });
}

let readerLayout: "beside" | "under" = "beside";

/** The text read aloud on paper: its sentences in order with their translations, for a reader or a class handout. */
async function exportReader(text: TextDetail) {
  const sentences = text.tiers.find((t) => t.utterance)?.id ?? text.segments.find((s) => !s.parent)?.tier ?? "";
  const segs = text.segments.filter((s) => s.tier === sentences && !s.parent && s.content.trim() !== "");
  if (segs.length === 0) {
    showToast({ tone: "warn", lines: ["This text has no transcribed sentences yet."] });
    return;
  }
  const folder = await chooseFolder("Choose where the reader goes");
  if (folder === null) return;
  await work("reader", async () => {
    const numbers = numbering(segs);
    const rows: ReaderSentence[] = [...segs]
      .sort((a, b) => (numbers.get(a.id) ?? 0) - (numbers.get(b.id) ?? 0))
      .map((seg) => ({
        number: numbers.get(seg.id) ?? 0,
        text: seg.content,
        translation: translationLines(seg, text.segments, text.tiers).find((l) => l.segment !== null)?.segment?.content ?? null,
      }));
    const [language, meta] = await Promise.all([projectLanguage(), textMetadata({ textId: text.id })]);
    const html = readerHtml({
      title: text.title || "Untitled",
      language: language.name,
      details: readerDetails(meta.fields.map((f) => [f.key, f.value] as [string, string])),
      sentences: rows,
      layout: readerLayout,
      date: localDay(new Date()),
    });
    const r = await saveDocument({ folder, title: `${text.title || "text"} reader`, extension: "html", contents: html });
    const missing = rows.filter((x) => x.translation === null || x.translation.trim() === "").length;
    showToast({
      tone: "info",
      lines: [
        `Saved a reader of ${rows.length} ${rows.length === 1 ? "sentence" : "sentences"} in ${r.path}. Open it in a browser or Word to print it.`,
        ...(missing > 0 ? [`${missing} ${missing === 1 ? "sentence has" : "sentences have"} no translation yet and ${missing === 1 ? "prints" : "print"} alone.`] : []),
      ],
    });
  });
}

function select<T extends string>(options: [T, string][], value: T, onchange: (v: T) => void): HTMLSelectElement {
  const el = h("select", {}, ...options.map(([id, label]) => h("option", { value: id, textContent: label, selected: id === value })));
  el.onchange = () => onchange(el.value as T);
  return el;
}

export const textsPanel: PanelDef = {
  title: "Texts",
  create() {
    const el = h("div", { className: "panel panel-texts" });
    const slice = () => {
      const s = getState();
      return s.kind === "open" ? `${s.info.projectId}|${s.text?.id ?? ""}|${JSON.stringify(s.texts)}` : "closed";
    };
    const draw = () => {
      const s = getState();
      if (s.kind === "closed") {
        el.replaceChildren(emptyState("No project open", "Open or create a project from the top bar."));
        return;
      }
      const title = h("input", { placeholder: "New text title" });
      const form = h(
        "form",
        {
          className: "new-text",
          onsubmit: (ev: SubmitEvent) => {
            ev.preventDefault();
            void newText(title.value.trim());
          },
        },
        title,
        h("button", { type: "submit", textContent: "New text" }),
      );
      const list =
        s.texts.length === 0
          ? h("p", { className: "empty", textContent: "No texts yet. Make one above, or import a file from ELAN, FLEx, Praat or Toolbox below." })
          : h(
              "ul",
              { className: "text-list" },
              ...s.texts.map((t) =>
                h(
                  "li",
                  { className: t.id === s.text?.id ? "selected" : "", onclick: () => void selectText(t.id) },
                  h("span", { textContent: t.title || "(untitled)" }),
                  h("span", { className: "count", textContent: String(t.segments) }),
                ),
              ),
            );
      const text = s.text;
      const share = h(
        "section",
        { className: "share" },
        h("h3", { textContent: "Import and export" }),
        languageRow(s.info.projectId),
        h(
          "div",
          { className: "share-row" },
          h("button", {
            type: "button",
            textContent: busy === "import" ? "Importing…" : "Import a file…",
            title: "ELAN .eaf, FLEx .flextext or .lift, Praat .TextGrid, Toolbox .db/.sfm, CoNLL-U .conllu (tags and trees kept), TEI .xml, a .csv/.tsv table of sentences, or plain text .txt (a segment per sentence)",
            disabled: busy !== null,
            onclick: () => void importOne(),
          }),
          h("button", {
            type: "button",
            textContent: busy === "lexicon-table" ? "Importing…" : "Import a lexicon table…",
            title: "A dictionary kept in Excel or Google Sheets (.csv or .tsv): a row per sense, with a word column and a gloss column. Rows already in the lexicon are skipped, so the same table twice adds nothing",
            disabled: busy !== null,
            onclick: () => void importLexTable(),
          }),
          h("button", {
            type: "button",
            textContent: busy === "details" ? "Importing…" : "Import details…",
            title: "A .csv or .tsv with a row per text: a title (or file name) column, then any details, such as author, date, genre or speaker. Corpus searches can then pick texts by them.",
            disabled: busy !== null,
            onclick: () => void importDetails(),
          }),
        ),
        h(
          "div",
          { className: "share-row" },
          h("span", { className: "share-label", textContent: text === null ? "Select a text to export it" : `«${text.title || "untitled"}» as` }),
          text === null ? null : select(TEXT_FORMATS, textFormat, (v) => (textFormat = v)),
          text === null
            ? null
            : h("button", {
                type: "button",
                textContent: busy === "text" ? "Exporting…" : "Export…",
                disabled: busy !== null,
                onclick: () => void exportOne(text.id),
              }),
        ),
        text === null
          ? null
          : h(
              "div",
              { className: "share-row" },
              h("span", { className: "share-label", textContent: "Glossed examples for" }),
              select(EXAMPLE_FORMATS.map((f) => [f.id, f.label] as [ExampleFormat, string]), exampleFormat, (v) => (exampleFormat = v)),
              h("button", {
                type: "button",
                textContent: busy === "examples" ? "Exporting…" : "Export…",
                title: "Every glossed sentence of this text as numbered interlinear examples, in one file for a grammar, a paper or a handout",
                disabled: busy !== null,
                onclick: () => void exportExamples(text),
              }),
            ),
        text === null
          ? null
          : h(
              "div",
              { className: "share-row" },
              h("span", { className: "share-label", textContent: "Reader to print," }),
              select(
                [
                  ["beside", "translation beside"],
                  ["under", "translation under"],
                ],
                readerLayout,
                (v) => (readerLayout = v),
              ),
              h("button", {
                type: "button",
                textContent: busy === "reader" ? "Saving…" : "Save…",
                title: "This text sentence by sentence with its translation, in large type with no glosses: for the people who told it, a literacy class or a community booklet",
                disabled: busy !== null,
                onclick: () => void exportReader(text),
              }),
            ),
        h(
          "div",
          { className: "share-row" },
          h("span", { className: "share-label", textContent: "Lexicon as" }),
          select(LEXICON_FORMATS, lexiconFormat, (v) => (lexiconFormat = v)),
          h("button", {
            type: "button",
            textContent: busy === "lexicon" ? "Exporting…" : "Export…",
            disabled: busy !== null,
            onclick: () => void exportLex(),
          }),
          h("button", {
            type: "button",
            textContent: busy === "dictionary" ? "Making…" : "Dictionary to print…",
            title: "The lexicon as a dictionary in the alphabet's order: senses, glosses, domains and an example from the texts for each sense, with an index of glosses. One HTML file to print to PDF or open in Word",
            disabled: busy !== null,
            onclick: () => void exportDict(),
          }),
          h("button", {
            type: "button",
            textContent: busy === "sketch" ? "Making…" : "Grammar sketch…",
            title: "A grammar to start writing: the sound inventory from the transcriptions, the orthography profile, the lexicon's affixes and clitics, your typology answers and the texts, under a grammar's usual headings with places to write. One HTML file for Word or LibreOffice",
            disabled: busy !== null,
            onclick: () => void exportSketch(),
          }),
        ),
        h(
          "div",
          { className: "share-row" },
          h("button", {
            type: "button",
            textContent: busy === "cldf" ? "Publishing…" : "Publish as CLDF…",
            title: "The lexicon and every glossed sentence as a CLDF Dictionary dataset (CSV files with metadata), what Zenodo, pycldf and CLLD dictionaries read",
            disabled: busy !== null,
            onclick: () => void publishCldf(),
          }),
          h("button", {
            type: "button",
            textContent: busy === "site" ? "Publishing…" : "Publish as a website…",
            title: "Every text sentence by sentence, glossed, with its translation and a button that plays it, and the dictionary: a folder of web pages that open from disk, a USB stick or any web host, for the community",
            disabled: busy !== null,
            onclick: () => void publishSite(),
          }),
          h(
            "label",
            { className: "share-label", title: "A WAV clip of each timed example, cut from its text's recording at the recording's own quality, listed in the dataset's media table" },
            h("input", {
              type: "checkbox",
              checked: cldfAudio,
              onchange: (ev: Event) => {
                cldfAudio = (ev.target as HTMLInputElement).checked;
                try {
                  localStorage.setItem(AUDIO_KEY, cldfAudio ? "on" : "off");
                } catch {
                  /* not remembered */
                }
              },
            }),
            " With sound clips",
          ),
          h("button", {
            type: "button",
            textContent: busy === "cwb" ? "Exporting…" : "Corpus for CQPweb…",
            title: "Every text as one corpus (a CWB vertical file with lemmas, tags and each text's details, plus the commands that encode it) for CQPweb, cqp, NoSketch Engine or Sketch Engine",
            disabled: busy !== null,
            onclick: () => void exportCwb(),
          }),
          h(
            "label",
            { className: "share-label", title: "Sentences from texts marked restricted or community-only. Leave this off unless the speakers agreed to publication." },
            h("input", { type: "checkbox", checked: includeRestricted, onchange: (ev: Event) => (includeRestricted = (ev.target as HTMLInputElement).checked) }),
            " Include texts that are not open",
          ),
        ),
        h(
          "div",
          { className: "share-row" },
          h("button", {
            type: "button",
            textContent: busy === "seal" ? "Sealing…" : "Seal a version for the archive…",
            title: "A self-describing package (BagIt) with every text, the lexicon, recordings and full history, for ELAR, PARADISEC or AILLA",
            disabled: busy !== null,
            onclick: () => void seal(),
          }),
        ),
      );
      void loadLanguage(s.info.projectId);
      // A redraw keeps focus on the details field in use.
      const active = document.activeElement as HTMLElement | null;
      const focusKey = active !== null && el.contains(active) && active.closest(".meta") !== null ? active.dataset.key : undefined;
      el.replaceChildren(form, list, text === null ? h("span") : detailsSection(text, draw), share);
      if (focusKey !== undefined) el.querySelector<HTMLElement>(`.meta [data-key="${CSS.escape(focusKey)}"]`)?.focus();
    };
    const view = sliced(el, slice, draw);
    live.add(draw);
    return {
      ...view,
      dispose: () => {
        live.delete(draw);
        view.dispose?.();
      },
    };
  },
};
