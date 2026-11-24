// Words to gloss: every word form with uses not yet broken into morphs,
// commonest first (src-tauri/src/gloss_types.rs). An analysis typed or
// accepted once goes to every unanalysed use of the form, as the Gloss
// panel's apply-to-others does for one word. Words in ../wordtypes/logic.ts.
import { errorText, glossApplyType, glossBreakAll, glossWordTypes, type MorphSpec, type WordTypeRow, type WordTypes } from "../api";
import { emptyState, h } from "../dom";
import { glossRuns } from "../gloss/example";
import { formatBreakdown, joinedBreakdown, type MorphType } from "../gloss/morphs";
import { provide } from "../palette/registry";
import { dataGeneration, getState, refreshAll, showToast } from "../store";
import { typeWithShortcuts } from "../typing";
import { appliedLine, meaningTitle, typedAnalysis, typesLine, withMeaning } from "../wordtypes/logic";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

/** Rows drawn; the rest wait until the first ones are analysed. */
const DRAWN = 200;

const small = (g: string) => h("span", {}, ...glossRuns(g).map((r) => (r.caps ? h("span", { className: "sc", textContent: r.text }) : document.createTextNode(r.text))));
const parsed = (m: MorphSpec) => ({ surface: m.surface, type: m.type as MorphType });

export const wordTypesPanel: PanelDef = {
  title: "Words to gloss",
  create(): PanelView {
    const el = h("div", { className: "panel panel-wordtypes" });
    let data: WordTypes | null = null;
    let seq = 0;
    let shown = "";
    let busy = false;
    /** What is typed per form, kept through redraws. */
    const drafts = new Map<string, { breakdown: string; glosses: string }>();
    const summary = h("p", { className: "note" });
    const list = h("div", { className: "wt-list", role: "list" });
    const reread = h("button", { type: "button", textContent: "Read again" });
    const breakAll = h("button", { type: "button" });

    async function apply(row: WordTypeRow, morphs: MorphSpec[]) {
      busy = true;
      draw();
      try {
        const r = await glossApplyType({ form: row.form, morphs });
        drafts.delete(row.form);
        showToast({ tone: "info", lines: [appliedLine(row.form, r.applied, r.texts, r.linked)] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      await refreshAll();
      void load();
    }

    const rowOf = (row: WordTypeRow) => {
      const d = drafts.get(row.form) ?? { breakdown: "", glosses: "" };
      const s = row.suggestion;
      const breakdown = typeWithShortcuts(h("input", { className: "wt-breakdown", value: d.breakdown, placeholder: s ? formatBreakdown(s.morphs.map(parsed)) : "morphs", spellcheck: false, ariaLabel: `Morphs of ${row.form}` }));
      const glosses = h("input", { className: "wt-glosses", value: d.glosses, placeholder: s ? s.morphs.map((m) => m.gloss ?? "?").join(" ") : "glosses", spellcheck: false, ariaLabel: `Glosses of ${row.form}` });
      breakdown.dataset.key = `wt-b:${row.form}`;
      glosses.dataset.key = `wt-g:${row.form}`;
      const keep = () => drafts.set(row.form, { breakdown: breakdown.value, glosses: glosses.value });
      breakdown.oninput = keep;
      glosses.oninput = keep;
      const go = () => {
        const typed = typedAnalysis(breakdown.value, glosses.value);
        if (!typed.ok) {
          showToast({ tone: "error", lines: [typed.error] });
          return;
        }
        void apply(row, typed.morphs);
      };
      const enter = (e: KeyboardEvent) => {
        if (e.key === "Enter" && !e.isComposing) {
          e.preventDefault();
          // Enter on an empty breakdown takes the suggestion, as the Gloss panel's ghost does.
          if (breakdown.value.trim() === "" && s) void apply(row, s.morphs);
          else go();
        }
      };
      breakdown.onkeydown = enter;
      glosses.onkeydown = enter;
      const take = s
        ? h(
            "button",
            {
              type: "button",
              className: "wt-take",
              disabled: busy,
              title: s.guessed ? "A guess from the morphs the project knows: use it for every unanalysed use" : `Analysed so ${s.count === 1 ? "once" : `${s.count} times`} before: use it for every unanalysed use`,
              onclick: () => void apply(row, s.morphs),
            },
            s.guessed ? h("em", { textContent: "guess " }) : null,
            joinedBreakdown(s.morphs.map(parsed)),
            " ",
            small(s.morphs.map((m) => m.gloss ?? "?").join("-")),
          )
        : h("span");
      return h(
        "div",
        { className: "wt-row", role: "listitem" },
        h("button", { type: "button", className: "link-button wt-form", textContent: row.form, title: `Every «${row.form}» in the Concordance`, onclick: () => askConcordance(row.form, "word", null, true) }),
        h("span", { className: "note wt-n", textContent: row.unanalysed === row.uses ? `×${row.uses}` : `${row.unanalysed} of ${row.uses}` }),
        breakdown,
        glosses,
        h("button", { type: "button", textContent: "Apply", disabled: busy, title: "Give every unanalysed use of this word this analysis", onclick: go }),
        take,
        ...(row.meanings.length === 0
          ? []
          : [
              h(
                "div",
                { className: "wt-meanings note" },
                "Maybe ",
                ...row.meanings.flatMap((m, i) => {
                  const b = h("button", {
                    type: "button",
                    className: "linkish",
                    textContent: `‘${m.word}’`,
                    title: meaningTitle(row.form, m),
                    onclick: () => {
                      const next = withMeaning(row.form, breakdown.value, glosses.value, m.word);
                      breakdown.value = next.breakdown;
                      glosses.value = next.glosses;
                      keep();
                      glosses.focus();
                    },
                  });
                  return [...(i === 0 ? [] : [", "]), b, ` ${m.together}/${m.of}`];
                }),
              ),
            ]),
      );
    };

    const draw = () => {
      if (data === null) {
        list.replaceChildren();
        breakAll.hidden = true;
        return;
      }
      const focused = document.activeElement instanceof HTMLElement && el.contains(document.activeElement) ? document.activeElement.dataset.key : undefined;
      summary.textContent = typesLine(data.rows.length, data.unbrokenSentences, data.unbrokenWords);
      breakAll.hidden = data.unbrokenSentences === 0;
      breakAll.disabled = busy;
      breakAll.textContent = `Break ${data.unbrokenSentences === 1 ? "1 sentence" : `${data.unbrokenSentences} sentences`} into words`;
      list.replaceChildren(...data.rows.slice(0, DRAWN).map(rowOf));
      if (focused !== undefined) list.querySelector<HTMLElement>(`[data-key="${CSS.escape(focused)}"]`)?.focus();
    };

    async function load() {
      const mine = ++seq;
      try {
        const r = await glossWordTypes({ textId: null });
        if (mine !== seq) return;
        data = r;
      } catch (e) {
        if (mine !== seq) return;
        data = null;
        summary.textContent = `Could not read the words: ${errorText(e)}`;
      }
      draw();
    }

    reread.onclick = () => void load();
    breakAll.onclick = async () => {
      busy = true;
      draw();
      try {
        const n = await glossBreakAll({ textId: null });
        showToast({ tone: "info", lines: [`Broke ${n === 1 ? "1 sentence" : `${n} sentences`} into words, as Break into words does in the Gloss panel.`] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      await refreshAll();
      void load();
    };

    const unprovide = provide("wordtypes", () => [
      { id: "wordtypes:first", title: "Gloss the most frequent unanalysed word", group: "Words to gloss", words: ["word analyses", "gloss by type", "bulk gloss", "unanalysed"], run: () => list.querySelector<HTMLInputElement>(".wt-breakdown")?.focus() },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}` : "closed";
      if (key === shown) return;
      shown = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        drafts.clear();
        el.replaceChildren(emptyState("Words to gloss", "Open a project to gloss its words one form at a time.", "The wug is sorting the words."));
        return;
      }
      el.replaceChildren(
        h("div", { className: "cc-bar" }, reread, breakAll),
        summary,
        h("p", { className: "note", textContent: "Type a word's morphs and their glosses once (ni- kani, 1SG count) and press Enter or Apply: every use not yet analysed gets it. Enter in an empty field takes the suggestion." }),
        list,
      );
      draw();
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
