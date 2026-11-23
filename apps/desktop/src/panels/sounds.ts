// Sounds: the project's sound inventory on the IPA chart. A transcription
// track's words (src-tauri/src/sounds.rs) are cut into sounds and placed as
// the chart places them (../sounds/ipa.ts): pulmonic consonants by place and
// manner, voiceless left of voiced; vowels on the trapezoid, unrounded left
// of rounded; affricates, clicks, implosives and other consonants under it;
// and what the IPA does not use (capitals, other letters) listed apart, since
// those are usually spelling slips. Each sound opens the Concordance on the
// words that hold it.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, soundWords } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
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

type Loaded = { tracks: { id: string; name: string; words: number }[]; sounds: Sound[]; chart: Chart; running: number; omitted: number; types: number; pairs: Contrast[] };

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
        loaded = { tracks: r.tracks, sounds, chart: chart(sounds), running: r.running, omitted: r.omitted, types: r.words.length, pairs: minimalPairs(r.words) };
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
        detail.push(
          h("div", { className: "sd-bar" }, h("strong", { className: "sd-glyph", textContent: `${picked.a} ~ ${picked.b}` }), copy),
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
      const consonants = consonantTable(c);
      const vowels = vowelChart(c);
      const omitted = loaded.omitted > 0 ? [h("p", { className: "note", textContent: `The ${loaded.omitted.toLocaleString()} rarest word types were left out.` })] : [];
      el.replaceChildren(
        h("div", { className: "sd-bar" }, pick, scope, h("label", {}, wholeBox, " Whole IPA chart"), copy),
        h("p", { className: "note", textContent: soundsNote(loaded.sounds, c, loaded.running) }),
        ...omitted,
        ...(consonants === null ? [] : [h("h4", { className: "sd-h", textContent: "Consonants (pulmonic)" }), consonants]),
        ...(vowels === null ? [] : [h("h4", { className: "sd-h", textContent: "Vowels" }), vowels]),
        ...listed("Affricates and double articulations", c.affricates),
        ...[...c.others].flatMap(([group, sounds]) => listed(group, sounds)),
        ...pairsSection(loaded),
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
