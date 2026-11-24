// Frequencies (Scrolls, SCROLLS.md §5 step 3): how often each word, morph,
// gloss or translation word occurs, in the whole project or one text. A row
// opens the Concordance on that item, so every count can be checked line by line.
// A word list also says which words the lexicon lacks, so a dictionary can be
// grown from the texts, most used word first.
import { fillScopes, scopeOf } from "../metadata/scope";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, frequencies, lexiconAdd } from "../api";
import { FREQ_KINDS, type FreqKind } from "../concordance/logic";
import { emptyState, h } from "../dom";
import { spreadReading } from "../concordance/plot";
import { asTsv, dpCell, hasSpread, spreadNote, cqpFor, unlabelledNote, filterRows, lexiconCell, lexiconNote, notInLexicon, FREQ_SORTS, perThousand, ranked, richness, sortRows, summary, type FreqSort, type Frequencies, type RankedRow } from "../frequencies/logic";
import { provide } from "../palette/registry";
import { dataGeneration, getState, showToast, touchLexicon } from "../store";
import { askConcordance } from "./concordance";
import type { PanelDef, PanelView } from "./panel";

const NOUNS: Record<FreqKind, { one: string; many: string }> = {
  word: { one: "word", many: "words" },
  morph: { one: "morph", many: "morphs" },
  gloss: { one: "gloss", many: "glosses" },
  translation: { one: "translation word", many: "translation words" },
  lemma: { one: "lemma", many: "lemmas" },
  tag: { one: "tag", many: "tags" },
};

/** Rows drawn at once; the filter finds the rest. */
const DRAWN = 500;

export const frequenciesPanel: PanelDef = {
  title: "Frequencies",
  create(): PanelView {
    const el = h("div", { className: "panel panel-frequencies" });
    let kind: FreqKind = "word";
    let sortBy: FreqSort = "count";
    let scope = "";
    let data: Frequencies | null = null;
    let rankedRows: RankedRow[] = [];
    let countedAt = "";
    let seq = 0;
    const chips = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Count" });
    const scopePicker = h("select", { className: "fq-scope", title: "Count in the whole project, one text, or the texts with one detail (add details in the Texts panel)" });
    scopePicker.setAttribute("aria-label", "Where to count");
    let size = 1;
    const sizer = h(
      "select",
      { className: "fq-size", title: "Count single items, or runs of words in a row inside a segment (n-grams, as AntConc's N-Grams tool)" },
      ...[1, 2, 3, 4, 5].map((n) => h("option", { value: String(n), textContent: n === 1 ? "Single items" : `${n} words in a row` })),
    );
    sizer.setAttribute("aria-label", "Words in a row");
    const drawSizer = () => {
      sizer.disabled = !(kind === "word" || kind === "translation");
      sizer.value = String(ngrams() ? size : 1);
      sizer.title = sizer.disabled ? "Words in a row count words or translation words" : "Count single items, or runs of words in a row inside a segment (n-grams, as AntConc's N-Grams tool)";
    };
    const ngrams = () => size > 1 && (kind === "word" || kind === "translation");
    const noun = () => (ngrams() ? { one: `${size}-word sequence`, many: `${size}-word sequences` } : NOUNS[kind]);
    const sorter = h("select", { className: "fq-sort", title: "Order of the rows" }, ...FREQ_SORTS.map((x) => h("option", { value: x.id, textContent: x.label })));
    sorter.setAttribute("aria-label", "Sort");
    const filter = h("input", { type: "search", className: "fq-filter", placeholder: "Filter the list", spellcheck: false });
    filter.setAttribute("aria-label", "Filter");
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy the list as a table to paste into a spreadsheet" });
    const recount = h("button", { type: "button", textContent: "Recount", title: "Count again (the list counts when the panel opens, the project syncs or texts change)" });
    const count = h("p", { className: "note fq-count" });
    const missingOnly = h("input", { type: "checkbox" });
    const missingBox = h("label", { className: "fq-missing", title: "List only the words no lexicon entry or form is spelled as" }, missingOnly, " Only words not in the lexicon");
    const table = h("table", { className: "fq-table" });

    const drawChips = () =>
      chips.replaceChildren(
        ...FREQ_KINDS.map((k) => {
          const b = h("button", { type: "button", textContent: k.label, title: `Count ${NOUNS[k.id].many}` });
          b.setAttribute("role", "radio");
          b.setAttribute("aria-checked", String(k.id === kind));
          b.onclick = () => {
            kind = k.id;
            drawChips();
            drawSizer();
            void load();
          };
          return b;
        }),
      );

    const drawScopes = () => {
      scope = fillScopes(scopePicker, scope, drawScopes);
    };

    /** Words carry the lexicon column (single transcription words only). */
    const marked = () => rankedRows.some((r) => r.lexicon !== undefined);
    const shownRows = () => {
      const rows = filterRows(rankedRows, filter.value);
      return sortRows(marked() && missingOnly.checked ? notInLexicon(rows) : rows, sortBy);
    };

    /** Make a word an entry (its gloss left for the Lexicon panel); the row says so at once. */
    async function addToLexicon(r: RankedRow) {
      try {
        await lexiconAdd({ lemma: r.item, gloss: "" });
      } catch (e) {
        showToast({ tone: "error", lines: [`Could not add «${r.item}»: ${errorText(e)}`] });
        return;
      }
      r.lexicon = "entry";
      touchLexicon();
      drawRows();
      showToast({ tone: "info", lines: [`Added «${r.item}» to the lexicon. Give it a gloss in the Lexicon panel.`] });
    }

    const drawRows = () => {
      if (data === null) {
        table.replaceChildren();
        return;
      }
      const rows = shownRows();
      // Rates per 1000 running words, so n-gram rates compare with the word list's.
      const tokens = data.words ?? data.tokens;
      const perWord =
        kind === "morph" || kind === "gloss"
          ? ` Each counts once per word, as the Concordance shows one line per word.`
          : kind === "lemma" || kind === "tag"
            ? unlabelledNote(data, kind)
            : "";
      const cut = rows.length > DRAWN ? ` The first ${DRAWN} are drawn; filter to find the others (Copy table has them all).` : "";
      // Richness is a property of a whole word list, not of n-grams, morphs or tags.
      const rich = !ngrams() && (kind === "word" || kind === "translation") ? richness(data) : "";
      count.textContent = summary(data, noun()) + rich + perWord + spreadNote(rankedRows) + lexiconNote(rankedRows) + (filter.value.trim() !== "" ? ` ${rows.length} match the filter.` : "") + cut + ` Counted at ${countedAt}.`;
      const withLexicon = marked();
      missingBox.hidden = !withLexicon;
      const spread = hasSpread(rankedRows);
      const columns = ["#", noun().one[0]!.toUpperCase() + noun().one.slice(1), "Count", "Per 1000 words", "Texts", ...(spread ? ["DP"] : []), ...(withLexicon ? ["In lexicon"] : [])];
      const head = h("tr", {}, ...columns.map((t) => h("th", { textContent: t, scope: "col" })));
      // A reverse list lines its words up at their ends, so shared endings read down the column.
      table.classList.toggle("fq-endings", sortBy === "ending");
      table.replaceChildren(
        h("thead", {}, head),
        h(
          "tbody",
          {},
          ...rows.slice(0, DRAWN).map((r) => {
            const tr = h(
              "tr",
              { tabIndex: 0, title: `Show every «${r.item}» in the Concordance` },
              h("td", { className: "fq-rank", textContent: String(r.rank) }),
              h("td", { className: "fq-item", textContent: r.item }),
              h("td", { className: "fq-num", textContent: r.count.toLocaleString("en") }),
              h("td", { className: "fq-num", textContent: perThousand(r.count, tokens) }),
              h("td", { className: "fq-num", textContent: String(r.texts) }),
              spread ? h("td", { className: "fq-num", textContent: dpCell(r.dp), title: typeof r.dp === "number" ? spreadReading(r.dp) : "" }) : null,
              withLexicon ? lexiconTd(r) : null,
            );
            const sc = scopeOf(scope);
            const text = sc.textId === null ? null : { id: sc.textId, title: scopePicker.selectedOptions[0]?.textContent ?? "" };
            // A lemma or tag opens the CQP query that finds exactly its lines.
            const open = () => (kind === "lemma" || kind === "tag" ? askConcordance(cqpFor(kind, r.item, sc.within), "query", text, true, null) : askConcordance(r.item, kind, text, true, sc.within));
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

    function lexiconTd(r: RankedRow): HTMLElement {
      const td = h("td", { className: `fq-lexicon fq-lexicon-${r.lexicon ?? "none"}`, textContent: lexiconCell(r.lexicon) });
      if (r.lexicon === "entry") return td;
      // A glossed word is usually inflected: its root belongs in the lexicon,
      // which the Checks panel's Not in the lexicon list adds.
      if (r.lexicon === "analysed") {
        td.title = "Broken into morphs in a text, but no entry or form is spelled this way; the Checks panel adds glossed roots";
        return td;
      }
      const add = h("button", { type: "button", className: "link-button", textContent: "Add", title: `Make «${r.item}» a lexicon entry` });
      add.tabIndex = -1;
      add.onclick = (e) => {
        e.stopPropagation();
        add.disabled = true;
        void addToLexicon(r);
      };
      td.append(" ", add);
      return td;
    }

    async function load(): Promise<void> {
      const mine = ++seq;
      count.textContent = "Counting…";
      // No row of an earlier count stays clickable while this one runs.
      data = null;
      rankedRows = [];
      table.replaceChildren();
      try {
        const sc = scopeOf(scope);
        const r = await frequencies({ kind, textId: sc.textId, within: sc.within, size: ngrams() ? size : 1 });
        if (mine !== seq) return;
        data = r;
        rankedRows = ranked(r.rows);
        countedAt = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
        drawRows();
      } catch (e) {
        if (mine !== seq) return;
        data = null;
        drawRows();
        count.textContent = `Could not count: ${errorText(e)}`;
      }
    }

    sizer.onchange = () => {
      size = Number(sizer.value);
      void load();
    };
    scopePicker.onchange = () => {
      scope = scopePicker.value;
      void load();
    };
    sorter.onchange = () => {
      sortBy = sorter.value as FreqSort;
      drawRows();
    };
    filter.oninput = drawRows;
    missingOnly.onchange = drawRows;
    recount.onclick = () => void load();
    copy.onclick = () => {
      if (data === null || rankedRows.length === 0) return;
      const rows = shownRows();
      void writeText(asTsv(rows, data.words ?? data.tokens, noun().one)).then(
        () => showToast({ tone: "info", lines: [`Copied ${rows.length} rows. Paste them into a spreadsheet.`] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    const unprovide = provide("frequencies", () => [
      { id: "frequencies:filter", title: "Filter the frequency list", group: "Frequencies", words: ["word list", "counts", "corpus", "types", "tokens"], run: () => filter.focus() },
    ]);

    let shown = "";
    const render = () => {
      const s = getState();
      const project = s.kind === "open" ? s.info.projectId : "closed";
      const key = s.kind === "open" ? `${project}|${dataGeneration()}|${s.texts.map((t) => `${t.id}:${t.title}:${t.segments}`).join(",")}` : "closed";
      if (key === shown) return;
      const sameProject = shown.split("|")[0] === project;
      shown = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        el.replaceChildren(emptyState("Frequencies", "Open a project to count its words, morphs and glosses.", "The wug is counting on its toes."));
        return;
      }
      if (!sameProject) {
        scope = "";
        filter.value = "";
        missingOnly.checked = false;
        data = null;
      }
      drawChips();
      drawScopes();
      drawSizer();
      el.replaceChildren(h("div", { className: "cc-bar" }, scopePicker, sizer, sorter, recount, copy), chips, h("div", { className: "cc-bar" }, filter, missingBox), count, h("div", { className: "fq-rows" }, table));
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
