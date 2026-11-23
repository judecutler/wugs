// A grammar sketch to start from (roadmap 43, the publishing layer's
// "grammars"): one HTML file, which Word, LibreOffice and Google Docs open,
// with what the project already knows laid out the way a grammar's chapters
// run (the language, its sounds, its spelling, its morphemes, its typology,
// the texts behind it), and a marked place under each heading for the prose
// only the linguist can write. Pure: no DOM, no commands; the Texts panel
// gathers the data and saves the file.

import { chart, gridOf, inventory, MANNER_NAMES, PLACE_NAMES, type Sound } from "../sounds/ipa.ts";
import { gapsNote, positions, shapes, share } from "../sounds/shapes.ts";
import type { Grid } from "../orthography/logic.ts";
import type { Answer, Ours } from "../typology/logic.ts";

export type Morpheme = { form: string; kind: "prefix" | "suffix" | "infix" | "proclitic" | "enclitic"; glosses: string[]; category: string };
export type Feature = { id: string; question: string; answer: string; note: string };
export type Grapheme = { grapheme: string; ipa: string };
export type SketchText = { title: string; sentences: number; words: number; glossed: number };

export type SketchInput = {
  language: { name: string; glottocode: string; iso: string };
  /** The word types of one transcription track with their counts, as
   * `sound_words` gives them; sounds and word shapes are read from them. */
  words: { word: string; count: number }[];
  /** The track they came from, named in the sketch. */
  soundTrack: string;
  graphemes: Grapheme[];
  morphemes: Morpheme[];
  entries: number;
  features: Feature[];
  /** "Grambank 1.0", "WALS Online v2020.6". */
  featureSource: string;
  texts: SketchText[];
  /** The position-class chart read from the glossed words: each slot's
   * label and its affixes as written with their glosses (`ni- 1SG`). */
  slots?: { label: string; affixes: string[] }[];
  /** The person's own date for the title page, `YYYY-MM-DD`. */
  date: string;
  generator: string;
};

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** What kind of morpheme a lexicon headword is, by its markers (`-ler`,
 * `ni-`, `=ma`, `ma=`, `<um>`), or null for a root. */
export function morphemeKind(lemma: string): Morpheme["kind"] | null {
  const l = lemma.trim();
  if (l.length < 2) return null;
  if (l.startsWith("<") && l.endsWith(">")) return "infix";
  if (l.startsWith("=")) return "enclitic";
  if (l.endsWith("=")) return "proclitic";
  if (l.startsWith("-") && !l.endsWith("-")) return "suffix";
  if (l.endsWith("-") && !l.startsWith("-")) return "prefix";
  return null;
}

/** The lexicon's affixes and clitics, in the order they are listed. */
export function morphemesOf(entries: readonly { lemma: string; glosses: string[]; posOfSpeech: string }[]): Morpheme[] {
  return entries.flatMap((e) => {
    const kind = morphemeKind(e.lemma);
    return kind === null ? [] : [{ form: e.lemma.trim(), kind, glosses: e.glosses.filter((g) => g.trim() !== ""), category: e.posOfSpeech }];
  });
}

/** An orthography profile's graphemes beside its IPA column (else its first
 * output column). */
export function graphemesOf(g: Grid): Grapheme[] {
  const lower = g.header.map((h) => h.trim().toLowerCase());
  const from = lower.indexOf("grapheme");
  if (from < 0) return [];
  const skip = new Set(["grapheme", "frequency", "comment", "comments", "left", "right", "class"]);
  const ipa = lower.indexOf("ipa") >= 0 ? lower.indexOf("ipa") : lower.findIndex((h) => !skip.has(h));
  if (ipa < 0) return [];
  return g.rows.filter((r) => (r[from] ?? "").trim() !== "").map((r) => ({ grapheme: r[from]!.trim(), ipa: (r[ipa] ?? "").trim() }));
}

/** The project's own typology answers in the dataset's order, each in words. */
export function featuresOf(features: readonly Answer[], ours: readonly Ours[]): Feature[] {
  const mine = new Map(ours.map((o) => [o.id, o]));
  return features.flatMap((f) => {
    const o = mine.get(f.id);
    if (o === undefined || (o.value === "" && o.note === "")) return [];
    const answer = o.value === "?" ? "not known" : o.value === "" ? "" : (f.codes.find((c) => c.value === o.value)?.label ?? o.value);
    return [{ id: f.id, question: f.question, answer, note: o.note }];
  });
}

const ORDER: Morpheme["kind"][] = ["prefix", "proclitic", "infix", "suffix", "enclitic"];
const KIND_NAMES: Record<Morpheme["kind"], string> = { prefix: "Prefixes", proclitic: "Proclitics", infix: "Infixes", suffix: "Suffixes", enclitic: "Enclitics" };

const todo = (what: string) => `<p class="todo">[${esc(what)}]</p>`;

function consonantTable(sounds: readonly Sound[]): string {
  const c = chart(sounds);
  const { manners, places } = gridOf(c, false);
  if (manners.length === 0) return "<p>No consonants found yet.</p>";
  const cell = (pair: [Sound[], Sound[]] | undefined) => (pair === undefined ? "" : [...pair[0], ...pair[1]].map((s) => esc(s.sound)).join(" "));
  const head = `<tr><th></th>${places.map((p) => `<th>${esc(PLACE_NAMES[p])}</th>`).join("")}</tr>`;
  const rows = manners.map((m) => `<tr><th>${esc(MANNER_NAMES[m])}</th>${places.map((p) => `<td class="ipa">${cell(c.consonants.get(m)?.get(p))}</td>`).join("")}</tr>`);
  const extra = [...c.affricates.map((s) => s.sound), ...[...c.others.values()].flat().map((s) => s.sound)];
  return `<table class="chart">${head}${rows.join("")}</table>${extra.length > 0 ? `<p>Also: <span class="ipa">${esc(extra.join(" "))}</span>.</p>` : ""}`;
}

function vowelList(sounds: readonly Sound[]): string {
  const c = chart(sounds);
  if (c.vowels.length === 0) return "<p>No vowels found yet.</p>";
  const by = [...c.vowels].sort((a, b) => a.at.height - b.at.height || a.at.back - b.at.back);
  return `<p class="ipa big">${by.map((v) => esc(v.sound.sound)).join(" ")}</p>`;
}

function soundTable(sounds: readonly Sound[]): string {
  const rows = sounds.slice(0, 80).map((s) => `<tr><td class="ipa">${esc(s.sound)}</td><td>${s.count}</td><td class="ipa">${esc(s.example)}</td></tr>`);
  return `<table><tr><th>Sound</th><th>Occurrences</th><th>Example</th></tr>${rows.join("")}</table>`;
}

/** The sketch as one HTML document. */
export function grammarSketch(d: SketchInput): string {
  const name = d.language.name.trim() || "the language";
  const ids = [d.language.glottocode && `Glottocode ${d.language.glottocode}`, d.language.iso && `ISO 639-3 ${d.language.iso}`].filter(Boolean).join(", ");
  const parts: string[] = [];
  parts.push(`<h1>A grammar sketch of ${esc(name)}</h1>`);
  parts.push(`<p class="meta">Draft of ${esc(d.date)}, started from the project's data by ${esc(d.generator)}.${ids ? ` ${esc(ids)}.` : ""} Bracketed passages are places to write.</p>`);

  parts.push(`<h2>1. Introduction</h2>`, todo(`Where ${name} is spoken, by whom, its family and neighbours, and the sources of this sketch`));
  const glossed = d.texts.reduce((n, t) => n + t.glossed, 0);
  const words = d.texts.reduce((n, t) => n + t.words, 0);
  parts.push(
    `<p>The sketch draws on ${d.texts.length} ${d.texts.length === 1 ? "text" : "texts"} (${words.toLocaleString("en")} words, ${glossed.toLocaleString("en")} of them glossed) and a lexicon of ${d.entries.toLocaleString("en")} ${d.entries === 1 ? "entry" : "entries"}.</p>`,
  );

  parts.push(`<h2>2. Phonology</h2>`);
  const sounds = inventory(d.words);
  if (sounds.length === 0) {
    parts.push(`<p>No transcriptions to read sounds from yet.</p>`);
  } else {
    parts.push(`<p>The sounds below are read from the «${esc(d.soundTrack)}» track as written, so they are phones of the transcription, not yet a phonemic analysis.</p>`);
    parts.push(`<h3>2.1 Consonants</h3>`, consonantTable(sounds), todo("Which of these contrast, and their allophones"));
    parts.push(`<h3>2.2 Vowels</h3>`, vowelList(sounds), todo("Vowel contrasts, length, nasality, harmony"));
    parts.push(`<h3>2.3 Frequency</h3>`, soundTable(sounds));
  }
  parts.push(`<h3>2.4 Word shapes, syllables and stress</h3>`);
  const shaped = shapes(d.words);
  if (shaped.length > 0) {
    const running = shaped.reduce((n, x) => n + x.count, 0);
    parts.push(
      `<p>The commonest consonant–vowel shapes of words (C a consonant, V a vowel):</p><table><tr><th>Shape</th><th>Share of words</th><th>Example</th></tr>${shaped
        .slice(0, 12)
        .map((x) => `<tr><td>${esc(x.shape)}</td><td>${share(x.count, running)}</td><td class="ipa">${esc(x.example)}</td></tr>`)
        .join("")}</table>`,
    );
    const gaps = gapsNote(positions(d.words, sounds.map((x) => x.sound)));
    if (gaps !== "") parts.push(`<p>${esc(gaps)}</p>`);
  }
  parts.push(todo("Syllable structure, stress or tone"));

  parts.push(`<h2>3. Orthography</h2>`);
  if (d.graphemes.length === 0) {
    parts.push(todo("The practical orthography and how it maps to IPA (an orthography profile in the Orthography panel fills this table)"));
  } else {
    parts.push(`<table><tr><th>Letter</th><th>IPA</th></tr>${d.graphemes.map((g) => `<tr><td>${esc(g.grapheme)}</td><td class="ipa">${esc(g.ipa)}</td></tr>`).join("")}</table>`);
  }

  parts.push(`<h2>4. Morphology</h2>`);
  const byKind = ORDER.map((k) => [k, d.morphemes.filter((m) => m.kind === k)] as const).filter(([, ms]) => ms.length > 0);
  if (byKind.length === 0) {
    parts.push(todo("Affixes and clitics (lexicon entries written with markers, such as -ler, ni- or =ma, are listed here)"));
  }
  byKind.forEach(([kind, ms], i) => {
    parts.push(`<h3>4.${i + 1} ${KIND_NAMES[kind]}</h3>`);
    parts.push(`<table><tr><th>Form</th><th>Gloss</th><th>Category</th></tr>${ms.map((m) => `<tr><td class="ipa">${esc(m.form)}</td><td class="gl">${esc(m.glosses.join("; "))}</td><td>${esc(m.category)}</td></tr>`).join("")}</table>`);
  });
  const slots = d.slots ?? [];
  if (slots.some((s) => s.affixes.length > 0)) {
    const depth = Math.max(...slots.map((s) => s.affixes.length), 1);
    const rows = Array.from({ length: depth }, (_, i) => `<tr>${slots.map((s) => `<td class="ipa">${s.label === "Root" && i === 0 ? "ROOT" : esc(s.affixes[i] ?? "")}</td>`).join("")}</tr>`);
    parts.push(
      `<h3>4.${byKind.length + 1} Position classes</h3>`,
      `<p>Each affix and clitic in the slot it holds most often in the glossed texts, counted from the root.</p>`,
      `<table class="chart"><tr>${slots.map((s) => `<th>${esc(s.label)}</th>`).join("")}</tr>${rows.join("")}</table>`,
      todo("Which slots are obligatory, and what the order says about the morphology"),
    );
  }
  parts.push(todo("Word classes, inflection and derivation"));

  parts.push(`<h2>5. Syntax</h2>`, todo("Word order, the noun phrase, the clause, questions, negation, complex sentences"));
  if (d.features.length > 0) {
    parts.push(`<h2>6. Typological profile</h2>`, `<p>The project's own answers to ${esc(d.featureSource)}'s questions.</p>`);
    parts.push(
      `<table><tr><th>Feature</th><th>Question</th><th>Answer</th><th>Note</th></tr>${d.features
        .map((f) => `<tr><td>${esc(f.id)}</td><td>${esc(f.question)}</td><td>${esc(f.answer)}</td><td>${esc(f.note)}</td></tr>`)
        .join("")}</table>`,
    );
  }
  if (d.texts.length > 0) {
    parts.push(`<h2>${d.features.length > 0 ? 7 : 6}. Texts</h2>`);
    parts.push(`<table><tr><th>Text</th><th>Sentences</th><th>Words</th><th>Glossed</th></tr>${d.texts.map((t) => `<tr><td>${esc(t.title || "untitled")}</td><td>${t.sentences}</td><td>${t.words}</td><td>${t.glossed}</td></tr>`).join("")}</table>`);
  }

  const style = `body{font-family:"Charis SIL","Doulos SIL","Gentium Plus",serif;max-width:46em;margin:2em auto;line-height:1.45}
table{border-collapse:collapse;margin:.6em 0}th,td{border:1px solid #999;padding:3px 8px;text-align:left;vertical-align:top}
.chart td{min-width:3em;text-align:center}.ipa{font-family:"Charis SIL","Doulos SIL",serif}.big{font-size:1.3em;letter-spacing:.2em}
.todo{color:#8a5a00;font-style:italic}.meta{color:#555}.gl{font-variant:small-caps}`;
  return `<!DOCTYPE html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n<title>A grammar sketch of ${esc(name)}</title>\n<style>${style}</style>\n</head>\n<body>\n${parts.join("\n")}\n</body>\n</html>\n`;
}
