// Checks: what to tidy before a grammar or a deposit. First check: morphs
// glossed more than one way across the project (src-tauri/src/consistency.rs),
// each with its glosses, how often each is used, where one occurs, and a
// button that makes one gloss the only one.
import { characterInventory, errorText, glossAddMissing, glossConsistency, glossMissing, glossUnify } from "../api";
import { addedLine, addLabel, cardNote, findable, glyph, inventoryNote, missingSummary, summary, unifiedLine, useLabel, type CharRow, type Inconsistent, type Inventory, type Missing } from "../checks/logic";
import { askConcordance } from "./concordance";
import { emptyState, h } from "../dom";
import { glossRuns } from "../gloss/example";
import { marked } from "../gloss/morphs";
import type { MorphType } from "../gloss/morphs";
import { provide } from "../palette/registry";
import { dataGeneration, getState, refreshAll, selectSegment, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

function glossNode(gloss: string): HTMLElement {
  return h("span", { className: "ck-gloss" }, ...glossRuns(gloss).map((r) => (r.caps ? h("span", { className: "sc", textContent: r.text }) : document.createTextNode(r.text))));
}

export const checksPanel: PanelDef = {
  title: "Checks",
  create(): PanelView {
    const el = h("div", { className: "panel panel-checks" });
    let rows: Inconsistent[] | null = null;
    let missing: Missing[] = [];
    let chars: Inventory | null = null;
    let showAllChars = false;
    let key = "";
    let seq = 0;
    let busy = false;
    /** The armed "use for all" button: morph key, kind and gloss. */
    let armed: string | null = null;

    const load = () => {
      const s = getState();
      if (s.kind !== "open") return;
      const mine = ++seq;
      Promise.all([glossConsistency(), glossMissing(), characterInventory()]).then(
        ([r, m, c]) => {
          if (mine !== seq) return;
          rows = r;
          missing = m;
          chars = c;
          paint();
        },
        (e) => {
          if (mine === seq) el.replaceChildren(emptyState("Checks", errorText(e)));
        },
      );
    };

    const unify = async (r: Inconsistent, to: string) => {
      busy = true;
      paint();
      let changed = 0;
      try {
        for (const v of r.variants) if (v.gloss !== to) changed += await glossUnify({ key: r.key, kind: r.kind, from: v.gloss, to });
        showToast({ tone: "info", lines: [unifiedLine(changed, to)] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      armed = null;
      await refreshAll();
      load();
    };

    /** Make lexicon entries for these roots (all of them when `keys` is empty). */
    const add = async (keys: string[]) => {
      busy = true;
      paint();
      try {
        const r = await glossAddMissing({ keys });
        showToast({ tone: "info", lines: [addedLine(r)] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      armed = null;
      await refreshAll();
      load();
    };

    const missingCard = (r: Missing): HTMLElement => {
      const glosses = r.variants.map((v) => {
        const show = h("button", { type: "button", className: "ck-show", textContent: `in «${v.textTitle || "(untitled)"}»`, title: "Open a sentence where it is glossed this way" });
        show.onclick = () => selectSegment({ textId: v.textId, tierId: v.tierId, segmentId: v.segmentId });
        return h("li", { className: "ck-variant" }, glossNode(v.gloss), h("span", { className: "ck-count", textContent: `× ${v.count}` }), show);
      });
      const addOne = h("button", { type: "button", textContent: addLabel(r), disabled: busy, title: `A lexicon entry «${r.surface}» with a sense per gloss, every use of it linked` });
      addOne.onclick = () => void add([r.key]);
      return h(
        "li",
        { className: "ck-card" },
        h("div", { className: "ck-head" }, h("span", { className: "ck-morph", textContent: r.surface }), h("span", { className: "ck-kind", textContent: `root · ${r.total}×` }), addOne),
        h("ul", { className: "ck-variants" }, ...glosses),
      );
    };

    const card = (r: Inconsistent): HTMLElement => {
      const variants = r.variants.map((v) => {
        const id = `${r.key}\u0001${r.kind}\u0001${v.gloss}`;
        const use = h("button", { type: "button", className: armed === id ? "danger" : "", textContent: useLabel(v, r, armed === id), disabled: busy, title: `Gloss every «${marked({ surface: r.surface, type: r.kind as MorphType })}» that reads otherwise as «${v.gloss}»` });
        use.onclick = () => {
          if (armed !== id) {
            armed = id;
            paint();
            return;
          }
          void unify(r, v.gloss);
        };
        const show = h("button", { type: "button", className: "ck-show", textContent: `in «${v.textTitle || "(untitled)"}»`, title: "Open a sentence where it is glossed this way" });
        show.onclick = () => selectSegment({ textId: v.textId, tierId: v.tierId, segmentId: v.segmentId });
        return h("li", { className: "ck-variant" }, glossNode(v.gloss), h("span", { className: "ck-count", textContent: `× ${v.count}` }), show, use);
      });
      return h(
        "li",
        { className: `ck-card${r.slip ? " ck-slip" : ""}` },
        h("div", { className: "ck-head" }, h("span", { className: "ck-morph", textContent: marked({ surface: r.surface, type: r.kind as MorphType }) }), h("span", { className: "ck-kind", textContent: r.kind })),
        h("p", { className: "note", textContent: cardNote(r) }),
        h("ul", { className: "ck-variants" }, ...variants),
      );
    };

    /** Where a character occurs: a sentence to open, or the entry using it. */
    const where = (r: CharRow): HTMLElement | null => {
      if (r.place !== null) {
        const p = r.place;
        const show = h("button", { type: "button", className: "ck-show", textContent: `in «${p.textTitle || "(untitled)"}»`, title: p.context });
        show.onclick = () => selectSegment({ textId: p.textId, tierId: p.tierId, segmentId: p.segmentId });
        return show;
      }
      return r.entry === null ? null : h("span", { className: "note", textContent: `in the entry «${r.entry[1]}»` });
    };

    const charCard = (r: CharRow): HTMLElement => {
      const find = findable(r) ? h("button", { type: "button", className: "ck-show", textContent: "Every word with it", title: `Open the Concordance on *${r.ch}*` }) : null;
      if (find !== null) find.onclick = () => askConcordance(`*${r.ch}*`, "word", null, false);
      return h(
        "li",
        { className: "ck-card ck-slip" },
        h("div", { className: "ck-head" }, h("span", { className: "ck-char", textContent: glyph(r) }), h("span", { className: "ck-kind", textContent: `${r.code} · ${r.name}` })),
        ...r.flags.map((f) => h("p", { className: "note", textContent: f })),
        h("div", { className: "ck-variant" }, h("span", { className: "ck-count", textContent: `× ${r.inTexts} in texts${r.inLexicon > 0 ? `, ${r.inLexicon} in the lexicon` : ""}` }), where(r), find),
      );
    };

    const charTable = (all: CharRow[]): HTMLElement => {
      const head = h("tr", {}, ...["", "Code", "Name", "Script", "Texts", "Lexicon"].map((t) => h("th", { textContent: t })));
      const rows = all.map((r) =>
        h(
          "tr",
          { className: r.flags.length > 0 ? "ck-flagged" : "" },
          h("td", { className: "ck-char", textContent: glyph(r) }),
          h("td", { textContent: r.code }),
          h("td", { textContent: r.name }),
          h("td", { textContent: r.script }),
          h("td", { className: "num", textContent: String(r.inTexts) }),
          h("td", { className: "num", textContent: String(r.inLexicon) }),
        ),
      );
      return h("table", { className: "fq-table ck-chars" }, h("thead", {}, head), h("tbody", {}, ...rows));
    };

    const charSection = (): HTMLElement[] => {
      if (chars === null) return [];
      const flagged = chars.rows.filter((r) => r.flags.length > 0);
      const toggle = h("button", { type: "button", textContent: showAllChars ? "Hide the full list" : `Every character (${chars.rows.length})` });
      toggle.onclick = () => {
        showAllChars = !showAllChars;
        paint();
      };
      return [
        h("h3", { className: "ck-title", textContent: "Characters" }),
        h("div", { className: "ck-bar" }, h("p", { className: "note", textContent: inventoryNote(chars) }), chars.rows.length > 0 ? toggle : null),
        h("ul", { className: "ck-list" }, ...flagged.map(charCard)),
        ...(showAllChars ? [charTable(chars.rows)] : []),
      ];
    };

    const paint = () => {
      if (rows === null) {
        el.replaceChildren(emptyState("Checks", "Looking through every gloss…"));
        return;
      }
      const again = h("button", { type: "button", textContent: "Check again", disabled: busy });
      again.onclick = load;
      const addAll = h("button", { type: "button", className: armed === "add-all" ? "danger" : "", disabled: busy, textContent: armed === "add-all" ? `Add ${missing.length}: sure?` : `Add all ${missing.length}` });
      addAll.onclick = () => {
        if (armed !== "add-all") {
          armed = "add-all";
          paint();
          return;
        }
        void add([]);
      };
      el.replaceChildren(
        h("h3", { className: "ck-title", textContent: "Glosses" }),
        h("div", { className: "ck-bar" }, h("p", { className: "note", textContent: summary(rows) }), again),
        h("ul", { className: "ck-list" }, ...rows.map(card)),
        h("h3", { className: "ck-title", textContent: "Not in the lexicon" }),
        h("div", { className: "ck-bar" }, h("p", { className: "note", textContent: missingSummary(missing) }), missing.length > 1 ? addAll : null),
        h("ul", { className: "ck-list" }, ...missing.map(missingCard)),
        ...charSection(),
      );
    };

    const draw = () => {
      const s = getState();
      if (s.kind !== "open") {
        key = "";
        rows = null;
        el.replaceChildren(emptyState("No project open", "Open a project to check its glosses."));
        return;
      }
      const wanted = `${s.info.projectId}|${dataGeneration()}`;
      if (wanted === key) return;
      key = wanted;
      if (!busy) load();
    };

    const unprovide = provide("checks", () => [{ id: "checks:again", title: "Check glosses again", group: "Checks", words: ["consistency", "typo", "inconsistent", "lexicon", "missing", "word list", "characters", "alphabet", "orthography", "unicode", "spelling"], run: load }]);
    return { element: el, render: draw, dispose: unprovide };
  },
};
