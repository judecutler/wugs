// Keywords (Scrolls, SCROLLS.md §5 step 6): the words one text uses more,
// or less, than the project's other texts (or than another text, a detail's
// texts or one speaker's track), scored by log-likelihood and log
// ratio as AntConc's Keyword List and Sketch Engine's keywords do. A row
// opens the Concordance on that word in the text.
import { fillScopes, scopeOf } from "../metadata/scope";
import { whereWords } from "../metadata/logic";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, keywords, textsAlike } from "../api";
import { alikeLine, alikeTsv, barShare, nearest, SHORT, type Alike } from "../keywords/alike";
import { emptyState, h } from "../dom";
import { asTsv, KEYWORD_SORTS, keywordRows, perThousand, significant, summary, type KeywordSide, type KeywordSort, type Keywords } from "../keywords/logic";
import { provide } from "../palette/registry";
import { dataGeneration, getState, showToast } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

type Kind = "word" | "translation";
const MINIMUMS = [1, 2, 3, 5, 10];
const DRAWN = 500;

export const keywordsPanel: PanelDef = {
  title: "Keywords",
  create(): PanelView {
    const el = h("div", { className: "panel panel-keywords" });
    let kind: Kind = "word";
    let side: KeywordSide = "more";
    let sortBy: KeywordSort = "ll";
    let textId = "";
    let againstId = "";
    let data: Keywords | null = null;
    let seq = 0;
    const textPicker = h("select", { title: "The text, or the texts with one detail (add details in the Texts panel), to compare with the project's other texts" });
    textPicker.setAttribute("aria-label", "Text");
    const againstPicker = h("select", { title: "What to compare it with: the rest of the project, or one other text, the texts with a detail, or one speaker's track" });
    againstPicker.setAttribute("aria-label", "Against");
    const sizer = h(
      "select",
      { className: "kw-size", title: "Compare single words, or runs of words in a row inside a segment (key clusters)" },
      ...[1, 2, 3, 4, 5].map((n) => h("option", { value: String(n), textContent: n === 1 ? "Single words" : `${n} words in a row` })),
    );
    sizer.setAttribute("aria-label", "Words in a row");
    const min = h("select", { title: "Leave out rare words: one used more here needs this many uses here, one used less needs this many in the other texts" }, ...MINIMUMS.map((n) => h("option", { value: String(n), textContent: n === 1 ? "Every word" : `Seen ${n}+ times` })));
    min.setAttribute("aria-label", "Minimum");
    min.value = "2";
    const sorter = h("select", { title: "Order of the rows" }, ...KEYWORD_SORTS.map((x) => h("option", { value: x.id, textContent: x.label })));
    sorter.setAttribute("aria-label", "Sort");
    const kinds = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Compare" });
    const sides = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Show" });
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy these rows as a table to paste into a spreadsheet" });
    const note = h("p", { className: "note fq-count" });
    const table = h("table", { className: "fq-table kw-table" });
    let alike: Alike | null = null;
    const alikeNote = h("p", { className: "note" });
    const alikeList = h("div", { className: "kw-alike", role: "list" });
    const copyAlike = h("button", { type: "button", textContent: "Copy table", title: "Every text against every other, to paste into a spreadsheet or R" });
    const alikeHead = h("h3", { textContent: "Texts most like this one" });

    const drawAlike = () => {
      const sc = scopeOf(textId);
      // Only for one text; a detail's texts have no single row.
      const hidden = alike === null || sc.textId === null;
      alikeHead.hidden = alikeNote.hidden = alikeList.hidden = copyAlike.hidden = hidden;
      if (hidden || alike === null || sc.textId === null) return;
      alikeNote.textContent = alikeLine(alike, sc.textId);
      copyAlike.disabled = alike.similarity.length === 0;
      alikeList.replaceChildren(
        ...nearest(alike, sc.textId).map((n) => {
          const bar = h("span", { className: `kw-bar ${n.similarity < 0 ? "kw-bar-neg" : ""}` });
          bar.style.width = `${(barShare(n.similarity) * 50).toFixed(1)}%`;
          const row = h(
            "div",
            { className: "kw-alike-row", role: "listitem", title: `${n.words.toLocaleString("en")} words${n.words < SHORT ? " (short: the number is unsteady)" : ""}. Compare with it: pick it above` },
            h("span", { className: "kw-alike-title", textContent: n.title || "(untitled)" }),
            h("span", { className: "kw-alike-track" }, bar),
            h("span", { className: "fq-num", textContent: n.similarity.toFixed(2) }),
          );
          return row;
        }),
      );
    };

    const chips = <T extends string>(box: HTMLElement, options: { id: T; label: string; title: string }[], current: () => T, pick: (v: T) => void) =>
      box.replaceChildren(
        ...options.map((o) => {
          const b = h("button", { type: "button", textContent: o.label, title: o.title });
          b.setAttribute("role", "radio");
          b.setAttribute("aria-checked", String(o.id === current()));
          b.onclick = () => pick(o.id);
          return b;
        }),
      );
    const drawChips = () => {
      chips(kinds, [{ id: "word" as Kind, label: "Words", title: "Compare transcribed words" }, { id: "translation" as Kind, label: "Translations", title: "Compare the words of free translations" }], () => kind, (v) => {
        kind = v;
        drawChips();
        void load();
      });
      chips(sides, [{ id: "more" as KeywordSide, label: "Used more", title: "Words this text uses more than the other texts" }, { id: "less" as KeywordSide, label: "Used less", title: "Words this text uses less than the other texts" }], () => side, (v) => {
        side = v;
        drawChips();
        drawRows();
      });
    };

    const drawTexts = () => {
      const s = getState();
      const texts = s.kind === "open" ? s.texts : [];
      const fallback = (s.kind === "open" && s.text !== null ? s.text.id : texts[0]?.id) ?? "";
      // One text, or the texts with one detail; never the whole project, which has nothing to be compared with.
      textId = fillScopes(textPicker, textId === "" ? fallback : textId, drawTexts);
      textPicker.querySelector('option[value=""]')?.remove();
      if (textId === "") textId = fallback;
      textPicker.value = textId;
      againstId = fillScopes(againstPicker, againstId, drawTexts, "The rest of the project");
    };
    // The reference written out for the summary, or null for the rest of the project.
    const againstLabel = (): string | null => {
      const sc = scopeOf(againstId);
      if (sc.within !== null) return whereWords(sc.within);
      return sc.textId === null ? null : `«${againstPicker.selectedOptions[0]?.textContent ?? ""}»`;
    };
    const within = () => scopeOf(textId).within;
    const title = () => {
      const w = within();
      if (w === null) return textPicker.selectedOptions[0]?.textContent ?? "";
      const words = whereWords(w);
      return words.charAt(0).toUpperCase() + words.slice(1);
    };
    const shown = () => (data === null ? [] : keywordRows(data.rows, side, sortBy));

    const drawRows = () => {
      if (data === null) {
        table.replaceChildren();
        return;
      }
      const rows = shown();
      note.textContent = summary(title(), data, within() !== null, againstLabel()) + (rows.length === 0 && data.referenceTokens > 0 ? ` No word is used ${side === "more" ? "more" : "less"} here.` : "") + (rows.length > DRAWN ? ` The first ${DRAWN} are drawn.` : "");
      const head = h(
        "tr",
        {},
        ...[
          ["Word", ""],
          ["Here", "Times in this text (per 1000 words on hover)"],
          againstId === "" ? ["Rest", "Times in the project's other texts (per 1000 words on hover)"] : ["There", `Times in ${againstLabel() ?? "the other side"} (per 1000 words on hover)`],
          ["LL", "Log-likelihood (4-term, as AntConc): how sure the difference is; below zero when the text uses the word less"],
          ["Ratio", "Log ratio: 1 means twice as often here, −1 half as often (an absent word counts as 0.5)"],
        ].map(([t, tip]) => h("th", { textContent: t, scope: "col", title: tip })),
      );
      const sc = scopeOf(textId);
      const text = sc.textId === null ? null : { id: sc.textId, title: title() };
      const there = scopeOf(againstId);
      const thereText = there.textId === null ? null : { id: there.textId, title: againstPicker.selectedOptions[0]?.textContent ?? "" };
      table.replaceChildren(
        h("thead", {}, head),
        h(
          "tbody",
          {},
          ...rows.slice(0, DRAWN).map((r) => {
            const tr = h(
              "tr",
              { tabIndex: 0, title: `${perThousand(r.focus, data!.focusTokens)} per 1000 here, ${perThousand(r.reference, data!.referenceTokens)} ${againstId === "" ? "in the rest" : "there"}; %DIFF ${r.percentDiff.toFixed(0)}; simple maths ${r.simpleMaths.toFixed(2)}. Show every «${r.item}» ${r.focus > 0 ? (sc.within !== null ? "in these texts" : "in this text") : againstId === "" ? "in the whole project" : "there"} in the Concordance` },
              h("td", { className: "fq-item", textContent: (significant(r, data!.critical) ? "• " : "") + r.item }),
              h("td", { className: "fq-num", textContent: r.focus.toLocaleString("en") }),
              h("td", { className: "fq-num", textContent: r.reference.toLocaleString("en") }),
              h("td", { className: "fq-num", textContent: r.logLikelihood.toFixed(2) }),
              h("td", { className: "fq-num", textContent: r.logRatio.toFixed(2) }),
            );
            const open = () => (r.focus > 0 ? askConcordance(r.item, kind, text, true, sc.within) : againstId === "" ? askConcordance(r.item, kind, null) : askConcordance(r.item, kind, thereText, true, there.within));
            tr.onclick = open;
            tr.onkeydown = (e) => {
              if (e.key === "Enter") open();
              else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                e.preventDefault();
                const next = e.key === "ArrowDown" ? tr.nextElementSibling : tr.previousElementSibling;
                if (next instanceof HTMLElement) next.focus();
              }
            };
            return tr;
          }),
        ),
      );
    };

    async function loadAlike(): Promise<void> {
      const mine = seq;
      try {
        const r = await textsAlike(kind);
        if (mine !== seq) return;
        alike = r;
      } catch (e) {
        if (mine !== seq) return;
        alike = null;
        alikeNote.hidden = false;
        alikeNote.textContent = `Could not compare the texts: ${errorText(e)}`;
        return;
      }
      drawAlike();
    }

    async function load(): Promise<void> {
      const mine = ++seq;
      alike = null;
      drawAlike();
      void loadAlike();
      data = null;
      table.replaceChildren();
      if (textId === "") {
        note.textContent = "Add or import texts to compare them.";
        return;
      }
      note.textContent = "Counting…";
      try {
        const sc = scopeOf(textId);
        const ag = scopeOf(againstId);
        const r = await keywords({ kind, textId: sc.textId, within: sc.within, againstText: ag.textId, against: ag.within, minCount: Number(min.value), size: Number(sizer.value) });
        if (mine !== seq) return;
        data = r;
        drawRows();
      } catch (e) {
        if (mine !== seq) return;
        note.textContent = `Could not count: ${errorText(e)}`;
      }
    }

    textPicker.onchange = () => {
      textId = textPicker.value;
      void load();
    };
    againstPicker.onchange = () => {
      againstId = againstPicker.value;
      void load();
    };
    min.onchange = () => void load();
    sizer.onchange = () => void load();
    sorter.onchange = () => {
      sortBy = sorter.value as KeywordSort;
      drawRows();
    };
    copyAlike.onclick = () => {
      if (alike === null) return;
      void writeText(alikeTsv(alike)).then(
        () => showToast({ tone: "info", lines: [`Copied ${alike?.texts.length ?? 0} texts against each other.`] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };
    copy.onclick = () => {
      const rows = shown();
      if (rows.length === 0) return;
      void writeText(asTsv(rows)).then(
        () => showToast({ tone: "info", lines: [`Copied ${rows.length} rows. Paste them into a spreadsheet.`] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    const unprovide = provide("keywords", () => [
      { id: "keywords:text", title: "Find the keywords of a text", group: "Keywords", words: ["keyness", "key words", "log ratio", "antconc", "compare texts"], run: () => textPicker.focus() },
    ]);

    let drawnFor = "";
    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}|${s.texts.map((t) => `${t.id}:${t.title}:${t.segments}`).join(",")}` : "closed";
      if (key === drawnFor) return;
      drawnFor = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        el.replaceChildren(emptyState("Keywords", "Open a project to see which words set each text apart.", "The wug is weighing every word."));
        return;
      }
      drawChips();
      drawTexts();
      el.replaceChildren(h("div", { className: "cc-bar" }, textPicker, h("span", { className: "kw-against", textContent: "against" }), againstPicker), kinds, sides, h("div", { className: "cc-bar" }, sizer, min, sorter, copy), note, h("div", { className: "fq-rows" }, table), alikeHead, h("div", { className: "cc-bar" }, copyAlike), alikeNote, alikeList);
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
