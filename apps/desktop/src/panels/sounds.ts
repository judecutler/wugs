// Sounds: the project's sound inventory on the IPA chart. A transcription
// track's words (src-tauri/src/sounds.rs) are cut into sounds and placed as
// the chart places them (../sounds/ipa.ts): pulmonic consonants by place and
// manner, voiceless left of voiced; vowels on the trapezoid, unrounded left
// of rounded; affricates, clicks, implosives and other consonants under it;
// and what the IPA does not use (capitals, other letters) listed apart, since
// those are usually spelling slips. Each sound opens the Concordance on the
// words that hold it.
import { writeHtml, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, phoibleCompare, projectLanguage, soundWords } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { clustersNote, consonantClusters, gapsNote, positions, shapes, share, type Cluster, type Shape, type Where } from "../sounds/shapes";
import { againstRelatives, checkInventory, phoibleLine, shareOf, type PhoibleCompared } from "../sounds/phoible";
import { chartClipboard } from "../sounds/chartcopy";
import { EXAMPLE_FORMATS, FORMAT_KEY, parseFormat } from "../gloss/example";
import { syllableNote, syllableTypes, type SyllableType } from "../sounds/syllables";
import { distributionNote, envLabel, environments, environmentsTsv, type NeighbourRow } from "../sounds/environments";
import { contrastLabel, contrastsOf, minimalPairs, pairsNote, pairsTable, WORDS_READ, type Contrast } from "../sounds/pairs";
import { chart, defaultTrack, gridOf, HEIGHT_NAMES, inventory, MANNER_NAMES, PLACE_NAMES, placeOf, soundsNote, vowelXY, type Chart, type Sound } from "../sounds/ipa";
import { dataGeneration, getState, showToast } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

const WHOLE_KEY = "wugs.sounds.whole";
const SVG = "http://www.w3.org/2000/svg";
const VW = 340;
const VH = 220;
const PAD = 26;
/** Room on the left for unrounded vowels with several spellings (i ĩ iː). */
const PADX = 72;

type Loaded = { tracks: { id: string; name: string; words: number }[]; sounds: Sound[]; chart: Chart; running: number; omitted: number; types: number; pairs: Contrast[]; where: Where[]; shapes: Shape[]; clusters: Cluster[]; clustersNote: string; words: { word: string; count: number }[]; syllables: SyllableType[] };

const clustersOf = (words: readonly { word: string; count: number }[]) => {
  const clusters = consonantClusters(words);
  return { clusters, clustersNote: clustersNote(words, clusters) };
};

export const soundsPanel: PanelDef = {
  title: "Sounds",
  create(): PanelView {
    const el = h("div", { className: "panel panel-sounds" });
    let whole = localStorage.getItem(WHOLE_KEY) === "on";
    /** "" reads every transcription track; null until the first load picks one. */
    let track: string | null = null;
    let onlyThisText = false;
    let loaded: Loaded | null = null;
    let problem: string | null = null;
    let key = "";
    let project = "";
    let seq = 0;
    /** The sound whose contrasts are listed (null: every sound), and the contrast opened. */
    let pairSound: string | null = null;
    let openContrast: string | null = null;
    /** The two sounds whose environments are compared (null: the two commonest). */
    let envA: string | null = null;
    let envB: string | null = null;
    /** PHOIBLE, once asked for: its comparison, or why there is none. */
    let phoible: PhoibleCompared | null = null;
    let phoibleNote = "";
    let phoibleBusy = false;
    let phoibleInv = 0;

    const shownText = (): { id: string; title: string } | null => {
      const s = getState();
      return s.kind === "open" && s.text !== null ? { id: s.text.id, title: s.text.title } : null;
    };

    const load = async () => {
      const mine = ++seq;
      const text = onlyThisText ? shownText() : null;
      try {
        let r = await soundWords({ tierId: track === "" ? null : track, textId: text?.id ?? null });
        if (track === null) {
          track = defaultTrack(r.tracks) ?? "";
          if (track !== "") r = await soundWords({ tierId: track, textId: text?.id ?? null });
        }
        if (mine !== seq) return;
        const sounds = inventory(r.words);
        loaded = { tracks: r.tracks, sounds, chart: chart(sounds), running: r.running, omitted: r.omitted, types: r.words.length, pairs: minimalPairs(r.words), where: positions(r.words, sounds.map((x) => x.sound)), shapes: shapes(r.words), ...clustersOf(r.words), words: r.words, syllables: syllableTypes(r.words) };
        problem = null;
      } catch (e) {
        if (mine !== seq) return;
        problem = errorText(e);
      }
      paint();
    };

    const soundButton = (s: Sound): HTMLElement => {
      const b = h("button", { type: "button", className: "sd-sound", title: `«${s.sound}» ${s.count.toLocaleString()} ${s.count === 1 ? "time" : "times"} in ${s.words} ${s.words === 1 ? "word" : "words"}, e.g. «${s.example}». Click for every word with it.` }, h("span", { className: "sd-glyph", textContent: s.sound }), h("span", { className: "sd-n", textContent: s.count.toLocaleString() }));
      b.onclick = () => askConcordance(`*${s.sound}*`, "word", onlyThisText ? shownText() : null, false);
      return b;
    };

    const consonantTable = (c: Chart): HTMLElement | null => {
      const grid = gridOf(c, whole);
      if (grid.manners.length === 0) return null;
      const head = h("tr", {}, h("th", {}), ...grid.places.map((p) => h("th", { scope: "col", textContent: PLACE_NAMES[p] })));
      const rows = grid.manners.map((m) =>
        h(
          "tr",
          {},
          h("th", { scope: "row", textContent: MANNER_NAMES[m] }),
          ...grid.places.map((p) => {
            const cell = c.consonants.get(m)?.get(p);
            return h("td", {}, h("div", { className: "sd-cell" }, h("div", { className: "sd-side" }, ...(cell?.[0] ?? []).map(soundButton)), h("div", { className: "sd-side sd-voiced" }, ...(cell?.[1] ?? []).map(soundButton))));
          }),
        ),
      );
      return h("div", { className: "sd-scroll" }, h("table", { className: "sd-consonants" }, h("thead", {}, head), h("tbody", {}, ...rows)));
    };

    const vowelChart = (c: Chart): HTMLElement | null => {
      if (c.vowels.length === 0) return null;
      const box = h("div", { className: "sd-vowels" });
      box.setAttribute("style", `width:${VW + PADX + PAD}px;height:${VH + 2 * PAD}px`);
      const lines = document.createElementNS(SVG, "svg");
      lines.setAttribute("viewBox", `0 0 ${VW + PADX + PAD} ${VH + 2 * PAD}`);
      lines.setAttribute("class", "sd-trapezoid");
      lines.setAttribute("aria-hidden", "true");
      const at = (height: number, back: number) => {
        const p = vowelXY({ height: height as 0, back, rounded: false }, VW, VH);
        return `${p.x + 11 + PADX},${p.y + PAD}`;
      };
      const path = (d: string) => {
        const e = document.createElementNS(SVG, "path");
        e.setAttribute("d", d);
        lines.append(e);
      };
      path(`M${at(0, 0)} L${at(0, 2)} L${at(6, 2)} L${at(6, 0)} Z`);
      path(`M${at(0, 1)} L${at(6, 1)}`);
      for (const height of [2, 4]) path(`M${at(height, 0)} L${at(height, 2)}`);
      box.append(lines);
      // One group per base vowel, anchored at its point.
      const groups = new Map<string, { x: number; y: number; rounded: boolean; sounds: Sound[] }>();
      for (const v of c.vowels) {
        const key = `${v.at.height}|${v.at.back}|${v.at.rounded}`;
        const g = groups.get(key) ?? { ...vowelXY(v.at, VW, VH), rounded: v.at.rounded, sounds: [] };
        g.sounds.push(v.sound);
        groups.set(key, g);
      }
      for (const g of groups.values()) {
        const group = h("div", { className: `sd-vgroup ${g.rounded ? "sd-rounded" : "sd-unrounded"}` }, ...g.sounds.map(soundButton));
        group.setAttribute("style", `top:${g.y + PAD}px;${g.rounded ? `left:${g.x + PADX - 9}px` : `right:${VW + PAD - g.x - 9}px`}`);
        box.append(group);
      }
      return h("div", { className: "sd-scroll" }, box);
    };

    const listed = (title: string, sounds: Sound[], note?: string): HTMLElement[] =>
      sounds.length === 0 ? [] : [h("h4", { className: "sd-h", textContent: title }), ...(note === undefined ? [] : [h("p", { className: "note", textContent: note })]), h("div", { className: "sd-list" }, ...sounds.map(soundButton))];

    const wordButton = (word: string, count: number): HTMLElement => {
      const b = h("button", { type: "button", className: "sd-word", textContent: word, title: `«${word}», ${count.toLocaleString()} ${count === 1 ? "time" : "times"}. Click for its lines.` });
      b.onclick = () => askConcordance(word, "word", onlyThisText ? shownText() : null, true);
      return b;
    };

    const clusterSection = (l: Loaded): HTMLElement[] => {
      const cell = (n: number) => h("td", { className: `num${n === 0 ? " sd-gap" : ""}`, textContent: n === 0 ? "–" : n.toLocaleString() });
      const rows = l.clusters.slice(0, 40).map((c) => {
        const find = h("button", { type: "button", className: "sd-sound", textContent: c.cluster, title: `e.g. «${c.example}», in ${c.words} ${c.words === 1 ? "word" : "words"}: open every word with it in the Concordance` });
        find.onclick = () => askConcordance(`*${c.cluster}*`, "word", null, false);
        return h("tr", {}, h("td", {}, find), cell(c.initial), cell(c.medial), cell(c.final), h("td", { className: "ipa", textContent: c.example }));
      });
      return [
        h("h4", { className: "sd-h", textContent: "Consonant clusters" }),
        h("p", { className: "note", textContent: l.clustersNote }),
        ...(rows.length === 0
          ? []
          : [h("div", { className: "sd-scroll" }, h("table", { className: "fq-table sd-where" }, h("thead", {}, h("tr", {}, ...["Cluster", "Word-initial", "Inside", "Word-final", "Example"].map((t) => h("th", { textContent: t })))), h("tbody", {}, ...rows)))]),
      ];
    };

    const syllableSection = (l: Loaded): HTMLElement[] => {
      const total = l.syllables.reduce((n, x) => n + x.count, 0);
      const chips = l.syllables.slice(0, 16).map((x) => h("span", { className: "sd-shape sd-syllable", title: `e.g. «${x.example}»` }, h("strong", { textContent: x.shape }), ` ${share(x.count, total)}`));
      return [
        h("h4", { className: "sd-h", textContent: "Syllables" }),
        h("p", { className: "note sd-syllable-note", textContent: `${syllableNote(l.syllables)} Words are cut so the next syllable starts with the longest run of consonants that starts a word somewhere.` }),
        h("div", { className: "sd-list" }, ...chips),
      ];
    };

    const whereSection = (l: Loaded): HTMLElement[] => {
      if (l.where.length === 0) return [];
      const cell = (n: number, total: number) => h("td", { className: `num${n === 0 ? " sd-gap" : ""}`, textContent: n === 0 ? "–" : share(n, total), title: n.toLocaleString() });
      const rows = l.where.map((r) => {
        const total = r.initial + r.medial + r.final + r.alone;
        return h("tr", {}, h("td", {}, soundButton(l.sounds.find((x) => x.sound === r.sound) ?? { sound: r.sound, count: total, words: 0, example: "" })), cell(r.initial, total), cell(r.medial, total), cell(r.final, total), cell(r.alone, total));
      });
      const gaps = gapsNote(l.where);
      const running = l.shapes.reduce((n, x) => n + x.count, 0);
      const shapeChips = l.shapes.slice(0, 16).map((x) => h("span", { className: "sd-shape", title: `e.g. «${x.example}», ${x.words} ${x.words === 1 ? "word" : "words"}` }, h("strong", { textContent: x.shape }), ` ${share(x.count, running)}`));
      return [
        h("h4", { className: "sd-h", textContent: "Word shapes" }),
        h("p", { className: "note", textContent: "Each word as consonants (C) and vowels (V), by share of running words. ? is a letter the chart does not place." }),
        h("div", { className: "sd-list" }, ...shapeChips),
        ...syllableSection(l),
        ...clusterSection(l),
        h("h4", { className: "sd-h", textContent: "Where sounds occur" }),
        ...(gaps === "" ? [] : [h("p", { className: "note", textContent: gaps })]),
        h(
          "div",
          { className: "sd-scroll" },
          h("table", { className: "fq-table sd-where" }, h("thead", {}, h("tr", {}, ...["Sound", "Word-initial", "Inside", "Word-final", "Alone"].map((t) => h("th", { textContent: t })))), h("tbody", {}, ...rows)),
        ),
      ];
    };

    const askPhoible = async () => {
      phoibleBusy = true;
      paint();
      try {
        const lang = await projectLanguage();
        if (lang.glottocode.trim() === "") {
          phoible = null;
          phoibleNote = "Give the project's Glottocode first, in the Language row of the Texts panel's Import and export.";
        } else {
          phoible = await phoibleCompare({ glottocode: lang.glottocode.trim() });
          phoibleInv = 0;
          phoibleNote = "";
        }
      } catch (e) {
        phoible = null;
        phoibleNote = errorText(e);
      }
      phoibleBusy = false;
      paint();
    };

    const phoibleSection = (l: Loaded): HTMLElement[] => {
      const ask = h("button", { type: "button", disabled: phoibleBusy, textContent: phoibleBusy ? "Comparing…" : phoible === null ? "Compare with PHOIBLE" : "Compare again", title: "Set these sounds beside PHOIBLE's inventories of the project's language and its relatives" });
      ask.onclick = () => void askPhoible();
      const head = [h("h4", { className: "sd-h", textContent: "Compared with PHOIBLE" }), h("div", { className: "sd-bar" }, ask)];
      if (phoible === null) return [...head, h("p", { className: "note sd-phoible-note", textContent: phoibleNote || "See which phonemes PHOIBLE lists for the language that these transcriptions have not shown yet, and which sounds here are rare among its relatives." })];
      const c = phoible;
      const ours = l.sounds;
      const chips = (sounds: readonly string[], title: (x: string) => string) => h("div", { className: "sd-list" }, ...sounds.map((x) => h("span", { className: "sd-shape ipa", title: title(x), textContent: x })));
      const out: HTMLElement[] = [...head, h("p", { className: "note sd-phoible-note", textContent: phoibleLine(c) })];
      if (c.own.length > 0) {
        const inv = c.own[Math.min(phoibleInv, c.own.length - 1)]!;
        if (c.own.length > 1) {
          const which = h("select");
          which.setAttribute("aria-label", "Inventory");
          c.own.forEach((x, i) => which.append(new Option(`${x.source.toUpperCase()}: ${x.name}, ${x.phonemes.length} phonemes`, String(i), false, i === phoibleInv)));
          which.onchange = () => {
            phoibleInv = Number(which.value);
            paint();
          };
          out.push(h("div", { className: "sd-bar" }, which));
        }
        const check = checkInventory(ours, inv.phonemes);
        out.push(
          h("p", { className: "note", textContent: `${inv.source.toUpperCase()} lists ${inv.phonemes.length} phonemes; ${check.inBoth.length} of them are in these transcriptions.` }),
          ...(check.notFound.length === 0 ? [] : [h("p", { className: "note", textContent: "Listed there but not in these transcriptions yet (not elicited, written another way, or not in this variety):" }), chips(check.notFound, () => "In PHOIBLE's inventory, not found here")]),
          ...(check.notListed.length === 0 ? [] : [h("p", { className: "note", textContent: "Here but not listed there (allophones, loans, another analysis, or slips):" }), chips(check.notListed, (x) => `«${x}» is in these transcriptions but not in PHOIBLE's inventory`)]),
        );
      }
      if (c.relatives > 0) {
        const r = againstRelatives(ours, c);
        out.push(
          h("p", { className: "note", textContent: r.missing.length === 0 ? `Every sound most ${c.comparedWith} have is here.` : `Most ${c.comparedWith} have these, but these transcriptions do not:` }),
          chips(
            r.missing.map((x) => x.phoneme),
            (x) => `${shareOf(r.missing.find((m) => m.phoneme === x)!.languages, c.relatives)} languages`,
          ),
          ...(r.rare.length === 0
            ? []
            : [
                h("p", { className: "note", textContent: `Here, but rare among ${c.comparedWith} (under 5%): worth a second listen, or a feature of this language.` }),
                chips(
                  r.rare.map((x) => x.sound),
                  (x) => `${shareOf(r.rare.find((m) => m.sound === x)!.languages, c.relatives)} languages`,
                ),
              ]),
        );
      }
      out.push(h("p", { className: "note sd-credit", textContent: `From ${c.edition}, phoible.org. Moran, Steven & McCloy, Daniel (eds.) 2019. PHOIBLE 2.0. Jena: Max Planck Institute for the Science of Human History.` }));
      return out;
    };

    const envSection = (l: Loaded): HTMLElement[] => {
      if (l.sounds.length < 2) return [];
      const known = new Set(l.sounds.map((x) => x.sound));
      const a = envA !== null && known.has(envA) ? envA : l.sounds[0]!.sound;
      const b = envB !== null && known.has(envB) ? envB : l.sounds.find((x) => x.sound !== a)!.sound;
      const pick = (value: string, label: string, set: (v: string) => void) => {
        const sel = h("select", { className: "sd-env-pick" });
        sel.setAttribute("aria-label", label);
        for (const x of l.sounds) sel.append(new Option(x.sound, x.sound, false, x.sound === value));
        sel.onchange = () => {
          set(sel.value);
          paint();
        };
        return sel;
      };
      const e = environments(l.words, a, b);
      const num = (n: number) => h("td", { className: `num${n === 0 ? " sd-gap" : ""}`, textContent: n === 0 ? "–" : n.toLocaleString() });
      const side = (title: string, rows: NeighbourRow[]) =>
        h(
          "table",
          { className: "fq-table sd-env-side" },
          h("thead", {}, h("tr", {}, h("th", { textContent: title }), h("th", { className: "ipa", textContent: a }), h("th", { className: "ipa", textContent: b }))),
          h("tbody", {}, ...rows.slice(0, 30).map((r) => h("tr", { className: r.a > 0 && r.b > 0 ? "sd-env-both" : "" }, h("td", { className: "ipa", textContent: r.neighbour }), num(r.a), num(r.b)))),
        );
      const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy every environment of the two sounds with counts and examples" });
      copy.onclick = () =>
        writeText(environmentsTsv(e)).then(
          () => showToast({ tone: "info", lines: [`${e.whole.length} environments of «${a}» and «${b}» copied.`] }),
          (err) => showToast({ tone: "error", lines: [errorText(err)] }),
        );
      const rows = e.whole.slice(0, 40).map((r) =>
        h(
          "tr",
          { className: r.a > 0 && r.b > 0 ? "sd-env-both" : "" },
          h("td", { className: "ipa", textContent: envLabel(r) }),
          num(r.a),
          num(r.b),
          h("td", {}, r.exampleA === "" ? "" : wordButton(r.exampleA, r.a)),
          h("td", {}, r.exampleB === "" ? "" : wordButton(r.exampleB, r.b)),
        ),
      );
      return [
        h("h4", { className: "sd-h", textContent: "Environments" }),
        h("div", { className: "sd-bar" }, pick(a, "First sound", (v) => (envA = v)), h("span", { className: "sd-tilde", textContent: "and" }), pick(b, "Second sound", (v) => (envB = v)), copy),
        h("p", { className: "note sd-env-note", textContent: distributionNote(e) }),
        h("div", { className: "sd-env-sides" }, side("Before", e.before), side("After", e.after)),
        h(
          "div",
          { className: "sd-scroll" },
          h("table", { className: "fq-table sd-env" }, h("thead", {}, h("tr", {}, ...["Environment", a, b, `e.g. ${a}`, `e.g. ${b}`].map((t) => h("th", { textContent: t })))), h("tbody", {}, ...rows)),
        ),
      ];
    };

    const pairsSection = (l: Loaded): HTMLElement[] => {
      const keyOf = (c: Contrast) => `${c.a}\u001F${c.b}`;
      const shown = contrastsOf(l.pairs, pairSound);
      const which = h("select");
      which.setAttribute("aria-label", "Contrasts of");
      which.append(new Option("Every sound", "", false, pairSound === null));
      const inPairs = new Set(l.pairs.flatMap((c) => [c.a, c.b]));
      for (const s of l.sounds) if (inPairs.has(s.sound)) which.append(new Option(`«${s.sound}»`, s.sound, false, s.sound === pairSound));
      which.onchange = () => {
        pairSound = which.value === "" ? null : which.value;
        openContrast = null;
        paint();
      };
      const chips = shown.slice(0, 60).map((c) => {
        const open = openContrast === keyOf(c);
        const chip = h("button", { type: "button", className: `sd-contrast${open ? " is-open" : ""}`, textContent: contrastLabel(c) });
        chip.setAttribute("aria-pressed", String(open));
        chip.onclick = () => {
          openContrast = open ? null : keyOf(c);
          paint();
        };
        return chip;
      });
      const picked = shown.find((c) => keyOf(c) === openContrast) ?? null;
      const detail: HTMLElement[] = [];
      if (picked !== null) {
        const copy = h("button", { type: "button", textContent: "Copy pairs" });
        copy.onclick = () =>
          writeText(pairsTable(picked)).then(
            () => showToast({ tone: "info", lines: [`${picked.pairs.length} pairs for ${picked.a} ~ ${picked.b} copied.`] }),
            (e) => showToast({ tone: "error", lines: [errorText(e)] }),
          );
        const more = picked.total > picked.pairs.length ? [h("p", { className: "note", textContent: `The ${picked.pairs.length} with the commonest words of ${picked.total}.` })] : [];
        const env = h("button", { type: "button", textContent: "Environments", title: `Compare where «${picked.a}» and «${picked.b}» occur, under Environments` });
        env.onclick = () => {
          envA = picked.a;
          envB = picked.b;
          paint();
          el.querySelector(".sd-env-note")?.scrollIntoView({ block: "center" });
        };
        detail.push(
          h("div", { className: "sd-bar" }, h("strong", { className: "sd-glyph", textContent: `${picked.a} ~ ${picked.b}` }), copy, env),
          ...more,
          h("ul", { className: "sd-pairs" }, ...picked.pairs.map((p) => h("li", {}, wordButton(p.wordA, p.countA), h("span", { className: "sd-tilde", textContent: "~" }), wordButton(p.wordB, p.countB)))),
        );
      }
      return [
        h("h4", { className: "sd-h", textContent: "Minimal pairs" }),
        h("div", { className: "sd-bar" }, h("p", { className: "note", textContent: pairsNote(l.pairs, Math.min(l.types, WORDS_READ), l.types) }), l.pairs.length > 0 ? which : null),
        h("div", { className: "sd-list" }, ...chips),
        ...detail,
      ];
    };

    const copyTable = () => {
      if (loaded === null) return;
      const where = (s: Sound): string => {
        const p = placeOf(s.sound);
        switch (p.kind) {
          case "consonant":
            return `${p.voiced ? "voiced" : "voiceless"} ${PLACE_NAMES[p.place].toLowerCase()} ${MANNER_NAMES[p.manner].toLowerCase()}`;
          case "vowel":
            return `${HEIGHT_NAMES[p.at.height].toLowerCase()} ${p.at.back < 0.67 ? "front" : p.at.back > 1.33 ? "back" : "central"} ${p.at.rounded ? "rounded" : "unrounded"} vowel`;
          case "affricate":
            return "affricate or double articulation";
          case "other":
            return p.group.toLowerCase();
          case "unplaced":
            return `not on the chart: ${p.why}`;
        }
      };
      const lines = ["Sound\tCount\tWords\tExample\tOn the chart", ...loaded.sounds.map((s) => [s.sound, s.count, s.words, s.example, where(s)].join("\t"))];
      writeText(`${lines.join("\n")}\n`).then(
        () => showToast({ tone: "info", lines: [`${loaded?.sounds.length ?? 0} sounds copied as a table.`] }),
        (e) => showToast({ tone: "error", lines: [errorText(e)] }),
      );
    };

    const paint = () => {
      const s = getState();
      if (s.kind !== "open") return;
      if (problem !== null) {
        el.replaceChildren(emptyState("Sounds", problem));
        return;
      }
      if (loaded === null) {
        el.replaceChildren(emptyState("Sounds", "Reading the transcriptions…"));
        return;
      }
      const pick = h("select");
      pick.setAttribute("aria-label", "Track");
      pick.append(new Option("Every transcription track", "", false, track === ""));
      for (const t of loaded.tracks) pick.append(new Option(`${t.name || "(unnamed)"} (${t.words.toLocaleString()} words)`, t.id, false, t.id === track));
      pick.onchange = () => {
        track = pick.value;
        void load();
      };
      const scope = h("select");
      scope.setAttribute("aria-label", "Texts");
      scope.append(new Option("Whole project", "all", false, !onlyThisText));
      const text = shownText();
      scope.append(new Option(text === null ? "The text on screen (none open)" : `«${text.title || "(untitled)"}» only`, "text", false, onlyThisText));
      scope.onchange = () => {
        onlyThisText = scope.value === "text";
        void load();
      };
      const wholeBox = h("input", { type: "checkbox", checked: whole });
      wholeBox.onchange = () => {
        whole = wholeBox.checked;
        localStorage.setItem(WHOLE_KEY, whole ? "on" : "off");
        paint();
      };
      const copy = h("button", { type: "button", textContent: "Copy table", disabled: loaded.sounds.length === 0 });
      copy.onclick = copyTable;
      const c = loaded.chart;
      const copyChart = h("button", { type: "button", textContent: "Copy chart", disabled: loaded.sounds.length === 0, title: "Copy the consonant chart and the vowels for a paper, in the format chosen for Copy example in the Gloss panel" });
      copyChart.onclick = () => {
        let format = parseFormat(null);
        try {
          format = parseFormat(localStorage.getItem(FORMAT_KEY));
        } catch {
          /* the default */
        }
        const out = chartClipboard(c, format);
        const label = EXAMPLE_FORMATS.find((f) => f.id === format)?.label ?? "";
        void (out.html === null ? writeText(out.text) : writeHtml(out.html, out.text)).then(
          () => showToast({ tone: "info", lines: [`Copied the chart as ${label}.`] }),
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      };
      const consonants = consonantTable(c);
      const vowels = vowelChart(c);
      const omitted = loaded.omitted > 0 ? [h("p", { className: "note", textContent: `The ${loaded.omitted.toLocaleString()} rarest word types were left out.` })] : [];
      el.replaceChildren(
        h("div", { className: "sd-bar" }, pick, scope, h("label", {}, wholeBox, " Whole IPA chart"), copy, copyChart),
        h("p", { className: "note", textContent: soundsNote(loaded.sounds, c, loaded.running) }),
        ...omitted,
        ...(consonants === null ? [] : [h("h4", { className: "sd-h", textContent: "Consonants (pulmonic)" }), consonants]),
        ...(vowels === null ? [] : [h("h4", { className: "sd-h", textContent: "Vowels" }), vowels]),
        ...listed("Affricates and double articulations", c.affricates),
        ...[...c.others].flatMap(([group, sounds]) => listed(group, sounds)),
        ...pairsSection(loaded),
        ...envSection(loaded),
        ...phoibleSection(loaded),
        ...whereSection(loaded),
        ...listed("Not on the IPA chart", c.unplaced.map((u) => u.sound), [...new Set(c.unplaced.map((u) => u.why))].join("; ").replace(/^./u, (x) => x.toUpperCase()) + ". Often a spelling slip, or a spelling rather than IPA."),
      );
    };

    const draw = () => {
      const s = getState();
      if (s.kind !== "open") {
        key = "";
        loaded = null;
        el.replaceChildren(emptyState("No project open", "Open a project to see its sounds."));
        return;
      }
      if (s.info.projectId !== project) {
        project = s.info.projectId;
        track = null;
        onlyThisText = false;
        loaded = null;
        phoible = null;
        phoibleNote = "";
      }
      const wanted = `${s.info.projectId}|${dataGeneration()}|${onlyThisText ? (s.text?.id ?? "") : ""}`;
      if (wanted === key) return;
      key = wanted;
      paint();
      void load();
    };

    const unprovide = provide("sounds", () => [{ id: "sounds:again", title: "Count the sounds again", group: "Sounds", words: ["phoneme", "phone", "inventory", "ipa", "chart", "consonant", "vowel", "phonology"], run: () => void load() }]);
    return { element: el, render: draw, dispose: unprovide };
  },
};
