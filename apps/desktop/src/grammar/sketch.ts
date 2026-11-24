// A grammar sketch to start from (roadmap 43, the publishing layer's
// "grammars"): one HTML file, which Word, LibreOffice and Google Docs open,
// with what the project already knows laid out the way a grammar's chapters
// run (the language, its sounds, its spelling, its morphemes, its typology,
// the texts behind it), and a marked place under each heading for the prose
// only the linguist can write. Pure: no DOM, no commands; the Texts panel
// gathers the data and saves the file.

import { chart, gridOf, inventory, MANNER_NAMES, PLACE_NAMES, type Sound } from "../sounds/ipa.ts";
import { gapsNote, positions, shapes, share } from "../sounds/shapes.ts";
import { syllableNote, syllableTypes } from "../sounds/syllables.ts";
import { toneMelodies, tonesNote } from "../sounds/tones.ts";
import { cooccurrence, cooccurrenceNote, standouts } from "../sounds/cooccur.ts";
import { affixLabel, signatures } from "../affixes/signatures.ts";
import { possibleAllophones } from "../sounds/allophones.ts";
import { countsLine, total, verdict, type WordOrder } from "../wordorder/logic.ts";
import type { Grid } from "../orthography/logic.ts";
import type { Answer, Ours } from "../typology/logic.ts";
import { pronounTable, type PronounEntry } from "./pronouns.ts";
import { asHtml, type Example } from "../gloss/example.ts";
import { indexRows, indicesNote, type Indices } from "../affixes/greenberg.ts";
import { abbreviationsIn, abbreviationsOf, sortAbbreviations, type Abbreviation } from "../gloss/abbreviations.ts";

export type Morpheme = { form: string; kind: "prefix" | "suffix" | "infix" | "proclitic" | "enclitic"; glosses: string[]; category: string };
export type Feature = { id: string; question: string; answer: string; note: string };
export type Grapheme = { grapheme: string; ipa: string };
export type SketchText = { title: string; sentences: number; words: number; glossed: number; /** Morphs of the glossed words. */ morphs?: number };

/** Where Glottolog places the language (src-tauri/src/glottolog.rs `place`). */
export type Place = {
  lat: number;
  lon: number;
  family: string;
  macroarea: string;
  neighbours: { glottocode: string; name: string; family: string; lat: number; lon: number; km: number }[];
  /** Natural Earth's land as an SVG path, x = lon + 180, y = 90 − lat. */
  land: string;
};

export type SketchInput = {
  language: { name: string; glottocode: string; iso: string };
  /** Where Glottolog places the language, for a locator map and its neighbours. */
  place?: Place | null;
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
  /** Morph counts over every glossed word, for Greenberg's indices (src/affixes/greenberg.ts). */
  indices?: Indices;
  /** Glosses written more than one way (src/affixes/allomorphs.ts): each
   * gloss, its forms with their neighbours in words, and what may condition them. */
  allomorphs?: { gloss: string; forms: { form: string; uses: number; next: string; vowel: string }[]; note: string }[];
  /** Word orders counted in the parsed sentences (src-tauri/src/word_order.rs). */
  order?: WordOrder;
  /** Loanwords in the lexicon by source language, commonest first (`loanSources`). */
  loans?: { language: string; count: number }[];
  /** Sentences filed under grammar topics (src-tauri/src/topics.rs), as glossed examples, each topic's in order. */
  topics?: { name: string; examples: Example[] }[];
  /** The lexicon's entries with their part of speech and first gloss, for the pronoun table (`src/grammar/pronouns.ts`). */
  pronouns?: PronounEntry[];
  /** The grammatical abbreviations the sketch's glosses use, with their meanings (`sketchAbbreviations`, then `abbreviationList`). */
  abbreviations?: Abbreviation[];
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

const r1 = (n: number) => Math.round(n * 10) / 10;

/** A locator map: the land around the language's place, its neighbours as
 * grey dots and the language as a ringed one, 50° of longitude across. */
export function locatorMap(p: Place, name: string): string {
  const cx = p.lon + 180;
  const cy = 90 - p.lat;
  const w = 50;
  const h = 30;
  const x = r1(cx - w / 2);
  const y = r1(Math.min(Math.max(cy - h / 2, 0), 180 - h));
  // Copies of the land either side, so a place near 180° still has land around it.
  const land = [-360, 0, 360].map((dx) => `<path d="${p.land}" transform="translate(${dx} 0)" fill="#f2ede0" stroke="#b9ad8f" stroke-width="0.08"/>`).join("");
  const near = p.neighbours.map((n) => {
    let nx = n.lon + 180;
    if (nx - cx > 180) nx -= 360;
    if (cx - nx > 180) nx += 360;
    return `<circle cx="${r1(nx)}" cy="${r1(90 - n.lat)}" r="0.35" fill="#777"><title>${esc(n.name)}</title></circle>`;
  });
  return `<figure class="map"><svg xmlns="http://www.w3.org/2000/svg" viewBox="${x} ${y} ${w} ${h}" role="img" aria-label="${esc(`Where ${name} is spoken`)}"><rect x="${x}" y="${y}" width="${w}" height="${h}" fill="#dbeef4"/>${land}${near.join("")}<circle cx="${r1(cx)}" cy="${r1(cy)}" r="0.7" fill="#00718a" stroke="#000" stroke-width="0.25"/></svg><figcaption>Figure 1. Where Glottolog places ${esc(name)} (${placeWords(p)}), with the languages nearest to it in grey. Land from Natural Earth.</figcaption></figure>`;
}

/** "53.00° N, 1.00° W" */
export const placeWords = (p: { lat: number; lon: number }): string => `${Math.abs(p.lat).toFixed(2)}° ${p.lat >= 0 ? "N" : "S"}, ${Math.abs(p.lon).toFixed(2)}° ${p.lon >= 0 ? "E" : "W"}`;

/** The introduction's sentences on family, region and neighbours. */
export function placeLines(p: Place, name: string): string[] {
  const family = p.family === "" ? `Glottolog does not place ${name} in a family` : p.family === "isolate" ? `Glottolog classifies ${name} as an isolate` : `Glottolog classifies ${name} in the ${p.family} family`;
  const lines = [`${family}${p.macroarea ? `, in ${p.macroarea.replace(/;/g, ", ")}` : ""}.`];
  if (p.neighbours.length > 0) {
    const listed = p.neighbours.map((n) => `${n.name} (${n.family === "" ? "unclassified" : n.family}, ${Math.round(n.km).toLocaleString("en")} km)`);
    lines.push(`The languages Glottolog places nearest are ${listed.slice(0, -1).join(", ")}${listed.length > 1 ? " and " : ""}${listed[listed.length - 1]}.`);
  }
  return lines;
}

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

  // A grammar's list of abbreviations comes before its first chapter.
  const abbreviations = d.abbreviations ?? [];
  if (abbreviations.length > 0) {
    parts.push(
      `<h2>Abbreviations</h2>`,
      `<table class="abbr">${abbreviations.map((a) => `<tr><td class="gl">${esc(a.abbr.toLowerCase())}</td><td>${a.meaning === null ? todo("meaning") : esc(a.meaning)}</td></tr>`).join("")}</table>`,
    );
  }

  parts.push(`<h2>1. Introduction</h2>`);
  if (d.place) {
    parts.push(locatorMap(d.place, name), ...placeLines(d.place, name).map((l) => `<p>${esc(l)}</p>`), todo(`Where ${name} is spoken and by whom, which of these neighbours it is in contact with, and the sources of this sketch`));
  } else {
    parts.push(todo(`Where ${name} is spoken, by whom, its family and neighbours, and the sources of this sketch`));
  }
  const glossed = d.texts.reduce((n, t) => n + t.glossed, 0);
  const words = d.texts.reduce((n, t) => n + t.words, 0);
  parts.push(
    `<p>The sketch draws on ${d.texts.length} ${d.texts.length === 1 ? "text" : "texts"} (${words.toLocaleString("en")} words, ${glossed.toLocaleString("en")} of them glossed) and a lexicon of ${d.entries.toLocaleString("en")} ${d.entries === 1 ? "entry" : "entries"}.</p>`,
  );
  // Contact shows in the lexicon: its loanwords by source language.
  const loans = (d.loans ?? []).filter((l) => l.count > 0);
  if (loans.length > 0) {
    const total = loans.reduce((n, l) => n + l.count, 0);
    const pct = d.entries === 0 ? 0 : Math.round((total * 100) / d.entries);
    parts.push(
      `<h3>1.1 Contact and loanwords</h3><p>${total.toLocaleString("en")} ${total === 1 ? "entry is marked as a loanword" : "entries are marked as loanwords"} (${pct}% of the lexicon): ${loans.map((l) => `${esc(l.language)} ${l.count}`).join(", ")}.</p>`,
      todo("When and how these languages came into contact, which semantic fields the loans cluster in, and how loans are adapted to the sound system"),
    );
  }

  parts.push(`<h2>2. Phonology</h2>`);
  const sounds = inventory(d.words);
  if (sounds.length === 0) {
    parts.push(`<p>No transcriptions to read sounds from yet.</p>`);
  } else {
    parts.push(`<p>The sounds below are read from the «${esc(d.soundTrack)}» track as written, so they are phones of the transcription, not yet a phonemic analysis.</p>`);
    parts.push(`<h3>2.1 Consonants</h3>`, consonantTable(sounds));
    // Similar sounds that never share an environment: where the phonemic analysis starts.
    const allophones = possibleAllophones(d.words).slice(0, 12);
    if (allophones.length > 0)
      parts.push(
        `<p>Similar sounds that never occur in the same environment in these words, so may be allophones of one phoneme (a rule to try where one side's neighbours form a natural class):</p><table><tr><th>Sounds</th><th>Uses</th><th>Rule to try</th></tr>${allophones
          .map((x) => `<tr><td class="ipa">${esc(`${x.a} ~ ${x.b}`)}</td><td>${x.countA}, ${x.countB}</td><td class="ipa">${esc(x.rule ?? "")}</td></tr>`)
          .join("")}</table>`,
      );
    parts.push(todo("Which of these contrast, and their allophones"));
    parts.push(`<h3>2.2 Vowels</h3>`, vowelList(sounds));
    // Vowels together in a word: harmony shows as pairs avoided and preferred.
    const together = cooccurrence(d.words, "V");
    if (standouts(together).length > 0) parts.push(`<p>${esc(cooccurrenceNote(together, "V"))} Pairs well below 1 may point to vowel harmony.</p>`);
    parts.push(todo("Vowel contrasts, length, nasality, harmony"));
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
    const syllables = syllableTypes(d.words);
    const total = syllables.reduce((n, x) => n + x.count, 0);
    if (syllables.length > 0)
      parts.push(
      `<p>Syllables, cut so each starts with the longest run of consonants that starts a word: ${esc(syllableNote(syllables))}</p><table><tr><th>Syllable</th><th>Share of syllables</th><th>Example</th></tr>${syllables
        .slice(0, 8)
        .map((x) => `<tr><td>${esc(x.shape)}</td><td>${share(x.count, total)}</td><td class="ipa">${esc(x.example)}</td></tr>`)
        .join("")}</table>`,
    );
  }
  parts.push(todo("Syllable structure (check the cuts above), stress or tone"));
  const tones = toneMelodies(d.words);
  if (tones.marked > 0) {
    parts.push(
      `<h3>2.5 Tone</h3><p>${esc(tonesNote(tones))}</p><table><tr><th>Syllables</th><th>Melody</th><th>Word forms</th><th>Example</th></tr>${tones.melodies
        .filter((m) => m.syllables <= 3)
        .map((m) => `<tr><td>${m.syllables}</td><td class="ipa">${esc(m.melody)}</td><td>${m.words}</td><td class="ipa">${esc(m.example)}</td></tr>`)
        .join("")}</table>`,
      todo("The tones and their contrasts, the melodies words allow, and tone rules (spreading, downstep, floating tones)"),
    );
  }

  parts.push(`<h2>3. Orthography</h2>`);
  if (d.graphemes.length === 0) {
    parts.push(todo("The practical orthography and how it maps to IPA (an orthography profile in the Orthography panel fills this table)"));
  } else {
    parts.push(`<table><tr><th>Letter</th><th>IPA</th></tr>${d.graphemes.map((g) => `<tr><td>${esc(g.grapheme)}</td><td class="ipa">${esc(g.ipa)}</td></tr>`).join("")}</table>`);
  }

  parts.push(`<h2>4. Morphology</h2>`);
  const glossedWords = d.texts.reduce((n, t) => n + t.glossed, 0);
  const morphCount = d.texts.reduce((n, t) => n + (t.morphs ?? 0), 0);
  if (d.indices !== undefined && d.indices.words > 0) {
    // Greenberg's (1960) indices over every word broken into morphs.
    parts.push(
      `<p>${esc(indicesNote(d.indices))}</p>`,
      `<table><tr><th>Index</th><th>Ratio</th><th>Value</th></tr>${indexRows(d.indices).map((r) => `<tr><td>${esc(r.name)}</td><td>${esc(r.ratio)}</td><td>${r.value.toFixed(2)}</td></tr>`).join("")}</table>`,
    );
  } else if (glossedWords > 0 && morphCount > 0) {
    parts.push(`<p>A glossed word in the texts has ${(morphCount / glossedWords).toFixed(1)} morphemes on average (${morphCount.toLocaleString("en")} in ${glossedWords.toLocaleString("en")} words), Greenberg's index of synthesis.</p>`);
  }
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
  const sets = d.allomorphs ?? [];
  if (sets.length > 0) {
    const n = byKind.length + (slots.some((s) => s.affixes.length > 0) ? 2 : 1);
    parts.push(`<h3>4.${n} Allomorphy</h3>`, `<p>Glosses written more than one way on the same side of the root, with the sounds each form stands next to and the stem's nearest vowel.</p>`);
    for (const s of sets) {
      parts.push(
        `<table><tr><th colspan="4" class="gl">${esc(s.gloss)}</th></tr><tr><th>Form</th><th>Uses</th><th>Next to</th><th>Nearest vowel</th></tr>${s.forms.map((f) => `<tr><td class="ipa">${esc(f.form)}</td><td>${f.uses}</td><td class="ipa">${esc(f.next)}</td><td class="ipa">${esc(f.vowel)}</td></tr>`).join("")}</table>`,
        `<p>${esc(s.note)}</p>`,
      );
    }
    parts.push(todo("The rules that choose each form, and the forms these data do not show"));
  }
  // Read from the transcriptions alone, so even an unglossed project has a start here.
  const guessed = signatures(d.words, "suffix").signatures.slice(0, 8);
  if (guessed.length > 0) {
    const n = byKind.length + (slots.some((s) => s.affixes.length > 0) ? 1 : 0) + (sets.length > 0 ? 1 : 0) + 1;
    parts.push(
      `<h3>4.${n} Endings the words suggest</h3><p>Stems that take the same set of endings in the transcriptions (∅ for none), as Goldsmith's Linguistica finds them, before or beside any glossing; some sets are chance.</p><table><tr><th>Endings</th><th>Stems</th><th>Examples</th></tr>${guessed
        .map((g) => `<tr><td class="ipa">${esc(g.affixes.map((a) => affixLabel(a, "suffix")).join(" "))}</td><td>${g.stems.length}</td><td class="ipa">${esc(g.stems.slice(0, 6).map((x) => x.stem).join(", "))}</td></tr>`)
        .join("")}</table>`,
      todo("Which of these are inflection, which derivation, and which chance"),
    );
  }
  const pron = pronounTable(d.pronouns ?? []);
  if (pron !== null) {
    const n = byKind.length + (slots.some((s) => s.affixes.length > 0) ? 1 : 0) + (sets.length > 0 ? 1 : 0) + (guessed.length > 0 ? 1 : 0) + 1;
    const table =
      pron.rows.length === 0
        ? ""
        : `<table><tr><th>Person</th>${pron.columns.map((c) => `<th>${esc(c)}</th>`).join("")}</tr>${pron.rows
            .map((r, i) => `<tr><th>${esc(r)}</th>${pron.cells[i]!.map((c) => `<td class="ipa">${c.length === 0 ? "" : esc(c.join(", "))}</td>`).join("")}</tr>`)
            .join("")}</table>`;
    parts.push(
      `<h3>4.${n} Personal pronouns</h3>`,
      `<p>The lexicon's pronouns, placed by the person and number their glosses give (1SG, 3PL, 1PL.INCL, or an English pronoun).</p>`,
      table,
      pron.unplaced.length === 0 ? "" : `<p>Pronouns whose gloss names no person: ${esc(pron.unplaced.join(", "))}.</p>`,
      todo("Case forms, possessive pronouns, and the gaps in the table"),
    );
  }
  parts.push(todo("Word classes, inflection and derivation"));

  parts.push(`<h2>5. Syntax</h2>`);
  const counted = d.order?.patterns.filter((p) => total(p) > 0) ?? [];
  if (d.order !== undefined && counted.length > 0) {
    parts.push(
      `<h3>5.1 Word order</h3><p>Counted in ${d.order.parsed} parsed sentences; the suggested order is one at least twice as common as any other, as WALS reads it.</p><table><tr><th>Construction</th><th>Counts</th><th>Suggested</th></tr>${counted
        .map((p) => `<tr><td>${esc(p.title)} (WALS ${esc(p.id)})</td><td>${esc(countsLine(p))}</td><td>${esc(verdict(p).label)}</td></tr>`)
        .join("")}</table>`,
      todo("Check these against the examples, then the noun phrase, the clause, questions, negation, complex sentences"),
    );
  } else {
    parts.push(todo("Word order, the noun phrase, the clause, questions, negation, complex sentences"));
  }
  // The examples filed under topics, numbered through, ready to move into their chapters.
  const topics = (d.topics ?? []).filter((t) => t.examples.length > 0);
  if (topics.length > 0) {
    parts.push(`<h3>5.${counted.length > 0 && d.order !== undefined ? 2 : 1} Examples by topic</h3>`, `<p>The sentences filed under each topic in the Gloss panel, glossed. Move each into its section and say what it shows.</p>`);
    let n = 0;
    for (const t of topics) {
      parts.push(`<h4>${esc(t.name)}</h4>`);
      for (const ex of t.examples) {
        n += 1;
        parts.push(`<div class="ex">${asHtml({ ...ex, number: n })}</div>`);
      }
    }
  }
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
.ex{margin:.5em 0}.ex table,.ex td{border:none}.abbr td{border:none;padding:1px 1.2em 1px 0}.abbr .todo{margin:0}
.todo{color:#8a5a00;font-style:italic}.meta{color:#555}.gl{font-variant:small-caps}
.map{margin:1em 0}.map svg{width:100%;max-width:34em;border:1px solid #b9ad8f;display:block}.map figcaption{font-size:.9em;color:#444}`;
  return `<!DOCTYPE html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n<title>A grammar sketch of ${esc(name)}</title>\n<style>${style}</style>\n</head>\n<body>\n${parts.join("\n")}\n</body>\n</html>\n`;
}

/** Every grammatical abbreviation the sketch prints in a gloss: the affix
 * chart's and allomorph sets' glosses, the affixes' glosses in the morphology
 * chapter and the examples by topic; person numbers first, then A to Z. */
export function sketchAbbreviations(d: Pick<SketchInput, "slots" | "allomorphs" | "morphemes" | "topics">): string[] {
  const seen = new Set<string>();
  const add = (gloss: string) => {
    for (const a of abbreviationsIn(gloss)) seen.add(a);
  };
  for (const slot of d.slots ?? []) for (const affix of slot.affixes) add(affix.slice(affix.indexOf(" ") + 1));
  for (const set of d.allomorphs ?? []) add(set.gloss);
  for (const m of d.morphemes) for (const g of m.glosses) add(g);
  for (const t of d.topics ?? []) for (const a of abbreviationsOf(t.examples)) seen.add(a);
  return sortAbbreviations([...seen]);
}
