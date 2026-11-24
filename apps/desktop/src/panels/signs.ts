// Signs (the Glyphs kit's first sign analysis): a script's inventory, where
// each sign stands in the word, which signs follow which, and the entropies
// that tell writing from other sequences. Reads a track's words through
// `sound_words`, as the Sounds panel does. Rules in ../glyphs/signs.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, soundWords } from "../api";
import { emptyState, h } from "../dom";
import { entropy, placeShare, scriptNote, signPairs, signsTsv, signTable, type SignMode, type SignRow } from "../glyphs/signs";
import { provide } from "../palette/registry";
import { dataGeneration, getState, showToast } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

const MODE_KEY = "wugs.signs.mode";

export const signsPanel: PanelDef = {
  title: "Signs",
  create(): PanelView {
    const el = h("div", { className: "panel panel-signs" });
    let mode: SignMode = localStorage.getItem(MODE_KEY) === "pieces" ? "pieces" : "letters";
    let track = "";
    let loaded: { tracks: { id: string; name: string; words: number }[]; words: { word: string; count: number }[] } | null = null;
    let problem: string | null = null;
    let key = "";
    let seq = 0;

    const load = async () => {
      const mine = ++seq;
      try {
        const r = await soundWords({ tierId: track === "" ? null : track, textId: null });
        if (mine !== seq) return;
        loaded = { tracks: r.tracks, words: r.words };
        problem = null;
      } catch (e) {
        if (mine !== seq) return;
        problem = errorText(e);
      }
      paint();
    };

    const paint = () => {
      if (problem !== null) {
        el.replaceChildren(emptyState("Signs", problem));
        return;
      }
      if (loaded === null) {
        el.replaceChildren(emptyState("Signs", "Reading the texts…"));
        return;
      }
      const l = loaded;
      const pick = h("select", { className: "sg-track", title: "Whose words to read: one track, or every transcription track" }, h("option", { value: "", textContent: "Every track" }), ...l.tracks.map((t) => h("option", { value: t.id, textContent: `${t.name} (${t.words.toLocaleString()} words)`, selected: t.id === track })));
      pick.setAttribute("aria-label", "Track");
      pick.onchange = () => {
        track = pick.value;
        void load();
      };
      const modePick = h(
        "select",
        { className: "sg-mode", title: "How a word is cut into signs" },
        h("option", { value: "letters", textContent: "Each letter is a sign", selected: mode === "letters" }),
        h("option", { value: "pieces", textContent: "Signs split by - or . (ko-no-so)", selected: mode === "pieces" }),
      );
      modePick.setAttribute("aria-label", "Signs");
      modePick.onchange = () => {
        mode = modePick.value === "pieces" ? "pieces" : "letters";
        try {
          localStorage.setItem(MODE_KEY, mode);
        } catch {
          // Kept for this run only.
        }
        paint();
      };
      const rows = signTable(l.words, mode);
      const e = entropy(l.words, mode);
      const copy = h("button", { type: "button", textContent: "Copy table", disabled: rows.length === 0, title: "Every sign with its count, word types and places in the word" });
      copy.onclick = () =>
        void writeText(signsTsv(rows)).then(
          () => showToast({ tone: "info", lines: [`${rows.length} signs copied.`] }),
          (err) => showToast({ tone: "error", lines: [errorText(err)] }),
        );
      const bar = h("div", { className: "cc-bar" }, pick, modePick, copy);
      if (rows.length === 0) {
        el.replaceChildren(bar, emptyState("Signs", "No words on this track yet. Transcribe or import a text, written as it stands on the page."));
        return;
      }
      const signCell = (r: SignRow) => {
        const b = h("button", { type: "button", className: "sd-sound sg-sign", textContent: r.sign, title: `e.g. «${r.example}». Every line with it in the Concordance.` });
        b.onclick = () => askConcordance(`*${r.sign}*`, "word", null, false);
        return b;
      };
      const table = h(
        "table",
        { className: "fq-table sg-table" },
        h("thead", {}, h("tr", {}, ...["Sign", "Count", "Words", "First", "Inside", "Last", "Alone", "Example"].map((t) => h("th", { textContent: t })))),
        h(
          "tbody",
          {},
          ...rows.slice(0, 400).map((r) => h("tr", {}, h("td", {}, signCell(r)), h("td", { className: "num", textContent: r.count.toLocaleString() }), h("td", { className: "num", textContent: r.words.toLocaleString() }), h("td", { className: "num", textContent: placeShare(r.initial, r) }), h("td", { className: "num", textContent: placeShare(r.medial, r) }), h("td", { className: "num", textContent: placeShare(r.final, r) }), h("td", { className: "num", textContent: placeShare(r.alone, r) }), h("td", { textContent: r.example }))),
        ),
      );
      const pairs = signPairs(l.words, mode, 40);
      el.replaceChildren(
        bar,
        h("p", { className: "note sg-note", textContent: scriptNote(e) }),
        h("div", { className: "sd-scroll" }, table),
        h("h4", { className: "sd-h", textContent: "Signs in a row" }),
        h("p", { className: "note", textContent: "The commonest two signs written together inside a word: in a syllabary or a script with signs for sounds, these are the language's sound sequences; a pair that always comes together may be one unit." }),
        h("div", { className: "sd-list sg-pairs" }, ...pairs.map((p) => h("span", { className: "sd-shape", title: `${p.count.toLocaleString()} times` }, h("strong", { textContent: mode === "pieces" ? `${p.first}-${p.second}` : `${p.first}${p.second}` }), ` ${p.count.toLocaleString()}`))),
      );
    };

    const render = () => {
      const s = getState();
      if (s.kind !== "open") {
        key = "";
        loaded = null;
        el.replaceChildren(emptyState("No project open", "Open a project to count its signs."));
        return;
      }
      const now = `${s.info.projectId}|${dataGeneration()}`;
      if (now === key) return;
      key = now;
      void load();
    };

    const unprovide = provide("signs", () => [{ id: "signs:copy", title: "Copy the sign inventory", group: "Signs", words: ["script", "decipherment", "syllabary", "glyphs", "entropy", "writing system"], run: () => el.querySelector<HTMLButtonElement>(".cc-bar button")?.click() }]);
    return { element: el, render, dispose: unprovide };
  },
};
