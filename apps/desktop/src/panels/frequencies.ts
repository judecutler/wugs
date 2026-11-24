// Frequencies (Scrolls, SCROLLS.md §5 step 3): how often each word, morph,
// gloss or translation word occurs, in the whole project or one text. A row
// opens the Concordance on that item, so every count can be checked line by line.
// A word list also says which words the lexicon lacks, so a dictionary can be
// grown from the texts, most used word first.
import { fillScopes, scopeOf } from "../metadata/scope";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, frequencies, frequencyTrends, lexiconAdd, lexiconAddWords } from "../api";
import { fromTo, sparkPoints, steadiness, trendsLine, trendsTsv, type Trend, type Trends } from "../frequencies/trends";
import { FREQ_KINDS, type FreqKind } from "../concordance/logic";
import { emptyState, h } from "../dom";
import { spreadReading } from "../concordance/plot";
import { decades, zipfFit, zipfLine, zipfPoints } from "../frequencies/zipf";
import { ADD_AT_ONCE, addedLine, addLabel, asTsv, dpCell, hasSpread, spreadNote, cqpFor, unlabelledNote, filterRows, inTexts, rangeNote, TEXT_RANGES, lexiconCell, lexiconNote, notInLexicon, FREQ_SORTS, perThousand, ranked, richness, sortRows, summary, type FreqSort, type Frequencies, type RankedRow } from "../frequencies/logic";
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
    const scopePicker = h("select", { className: "fq-scope", title: "Count in the whole project, one text, the texts with one detail (add details in the Texts panel), or one track (a speaker's, as CHAT files have)" });
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
    const range = h("select", { className: "fq-range", title: "List only what is met in at least this many texts: words the texts share rather than one text's names; with 3 or 4 words in a row, lexical bundles" }, ...TEXT_RANGES.map((r) => h("option", { value: String(r.min), textContent: r.label })));
    range.setAttribute("aria-label", "In how many texts");
    const missingOnly = h("input", { type: "checkbox" });
    const missingBox = h("label", { className: "fq-missing", title: "List only the words no lexicon entry or form is spelled as" }, missingOnly, " Only words not in the lexicon");
    // Every shown word not in the lexicon as an entry, after a second click.
    const addAll = h("button", { type: "button", className: "fq-add-all", title: "Make each word listed (after the filter) that no entry or form is spelled as an entry with an empty sense, to gloss in the Lexicon panel" });
    let addArmed = false;
    const table = h("table", { className: "fq-table" });
    // Zipf's law: frequency against rank on log scales, drawn when opened.
    const zipf = h("details", { className: "fq-zipf" }, h("summary", { textContent: "Zipf plot" }));
    const zipfBody = h("div", {});
    zipf.append(zipfBody);
    const drawZipf = () => {
      if (!zipf.open || data === null) {
        zipfBody.replaceChildren();
        return;
      }
      const points = zipfPoints(data.rows.map((r) => r.count));
      const fit = zipfFit(points);
      const W = 420;
      const H = 260;
      const box = { left: 46, top: 10, right: 12, bottom: 36 };
      const maxRank = Math.max(1, ...points.map((p) => p.rank));
      const maxCount = Math.max(1, ...points.map((p) => p.count));
      const xd = decades(maxRank);
      const yd = decades(maxCount);
      const lx = (r: number) => box.left + (Math.log10(r) / Math.log10(xd.at(-1)!)) * (W - box.left - box.right || 1);
      const ly = (c: number) => H - box.bottom - (Math.log10(c) / Math.log10(Math.max(10, yd.at(-1)!))) * (H - box.top - box.bottom);
      const NS = "http://www.w3.org/2000/svg";
      const svg = (tag: string, attrs: Record<string, string | number>, text?: string) => {
        const e = document.createElementNS(NS, tag);
        for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
        if (text !== undefined) e.textContent = text;
        return e;
      };
      const root = svg("svg", { viewBox: `0 0 ${W} ${H}`, class: "vw-plot fq-zipf-plot", role: "img", "aria-label": "Frequency against rank on log scales" });
      for (const d of xd) root.append(svg("line", { x1: lx(d), x2: lx(d), y1: box.top, y2: H - box.bottom, class: "vw-grid" }), svg("text", { x: lx(d), y: H - box.bottom + 14, class: "vw-tick", "text-anchor": "middle" }, d.toLocaleString("en")));
      for (const d of yd) if (ly(d) >= box.top - 1) root.append(svg("line", { x1: box.left, x2: W - box.right, y1: ly(d), y2: ly(d), class: "vw-grid" }), svg("text", { x: box.left - 5, y: ly(d) + 4, class: "vw-tick", "text-anchor": "end" }, d.toLocaleString("en")));
      root.append(svg("text", { x: (box.left + W - box.right) / 2, y: H - 4, class: "vw-axis", "text-anchor": "middle" }, "Rank"), svg("text", { x: 12, y: (box.top + H - box.bottom) / 2, class: "vw-axis", "text-anchor": "middle", transform: `rotate(-90 12 ${(box.top + H - box.bottom) / 2})` }, "Count"));
      if (fit !== null) {
        const at = (r: number) => 10 ** (fit.intercept - fit.alpha * Math.log10(r));
        root.append(svg("line", { x1: lx(1), y1: ly(Math.max(1, at(1))), x2: lx(maxRank), y2: ly(Math.max(1, at(maxRank))), class: "fq-zipf-fit" }));
      }
      const byRank = rankedRows;
      for (const p of points) {
        const dot = svg("circle", { cx: lx(p.rank), cy: ly(p.count), r: 2.4, class: "fq-zipf-dot" });
        dot.append(svg("title", {}, `#${p.rank} ${byRank[p.rank - 1]?.item ?? ""}: ${p.count}`));
        root.append(dot);
      }
      zipfBody.replaceChildren(root, h("p", { className: "note", textContent: zipfLine(fit, data.rows.length, data.rows.length < data.types) }));
    };
    zipf.addEventListener("toggle", drawZipf);

    // Words on the rise and falling over the years a detail gives, read when opened.
    const over = h("details", { className: "fq-trends" }, h("summary", { textContent: "Over time" }));
    const overBody = h("div", {});
    over.append(overBody);
    let trendKey: string | null = null;
    let trendMin = 10;
    let trends: Trends | null = null;
    let trendSeq = 0;
    const loadTrends = () => {
      if (!over.open) return;
      if (kind !== "word" && kind !== "translation") {
        overBody.replaceChildren(h("p", { className: "note", textContent: "Trends over time are counted for words and translation words." }));
        return;
      }
      const mine = ++trendSeq;
      overBody.replaceChildren(h("p", { className: "note", textContent: "Counting…" }));
      const sc = scopeOf(scope);
      void frequencyTrends({ kind, key: trendKey, minCount: trendMin, textId: sc.textId, within: sc.within }).then(
        (t) => {
          if (mine !== trendSeq) return;
          trends = t;
          trendKey = t.key === "" ? null : t.key;
          drawTrends();
        },
        (e) => {
          if (mine === trendSeq) overBody.replaceChildren(h("p", { className: "note", textContent: `Could not count: ${errorText(e)}` }));
        },
      );
    };
    const drawTrends = () => {
      const t = trends;
      if (t === null) return;
      const keyPicker = h("select", { title: "The detail whose year places each text" }, ...t.keys.map((k) => h("option", { value: k, textContent: `By ${k.replace(/_/g, " ")}`, selected: k === t.key })));
      keyPicker.setAttribute("aria-label", "Years from");
      keyPicker.onchange = () => {
        trendKey = keyPicker.value;
        loadTrends();
      };
      const minPicker = h("select", { title: "Leave out words used fewer times than this" }, ...[5, 10, 20, 50].map((n) => h("option", { value: String(n), textContent: `${n}+ uses`, selected: n === trendMin })));
      minPicker.setAttribute("aria-label", "Uses at least");
      minPicker.onchange = () => {
        trendMin = Number(minPicker.value);
        loadTrends();
      };
      const copyTrends = h("button", { type: "button", textContent: "Copy table", title: "Copy both lists with each word's rate per 1000 words in every period" });
      copyTrends.onclick = () =>
        void writeText(trendsTsv(t)).then(
          () => showToast({ tone: "info", lines: ["Copied the trends. Paste them into a spreadsheet to chart them."] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      const NS = "http://www.w3.org/2000/svg";
      const spark = (x: Trend) => {
        const e = document.createElementNS(NS, "svg");
        e.setAttribute("viewBox", "-2 -2 64 24");
        e.setAttribute("class", "fq-spark");
        e.setAttribute("aria-hidden", "true");
        const pts = sparkPoints(x.rates, 60, 20);
        if (pts !== null) {
          const line = document.createElementNS(NS, "polyline");
          line.setAttribute("points", pts);
          e.append(line);
        }
        return e;
      };
      const list = (title: string, rows: Trend[]) =>
        h(
          "div",
          { className: "fq-trend-list" },
          h("h4", { textContent: title }),
          rows.length === 0
            ? h("p", { className: "note", textContent: "None." })
            : h(
                "table",
                { className: "fq-trend-table" },
                h("tbody", {}, ...rows.slice(0, 50).map((x) => {
                  const tr = h(
                    "tr",
                    { tabIndex: 0, title: `${x.item}: ${x.total} uses, its rate changing ${steadiness(x.tau)} (Kendall's tau ${x.tau.toFixed(2)}). Show every «${x.item}» in the Concordance` },
                    h("td", { className: "fq-item", textContent: x.item }),
                    h("td", {}, spark(x)),
                    h("td", { className: "fq-num", textContent: fromTo(x) }),
                    h("td", { className: "fq-num", textContent: String(x.total) }),
                  );
                  const sc = scopeOf(scope);
                  const asked = kind === "translation" ? "translation" : "word";
                  const open = () => askConcordance(x.item, asked, sc.textId === null ? null : { id: sc.textId, title: scopePicker.selectedOptions[0]?.textContent ?? "" }, true, sc.within);
                  tr.onclick = open;
                  tr.onkeydown = (e) => {
                    if (e.key === "Enter") open();
                  };
                  return tr;
                })),
              ),
        );
      const live = t.periodWords.filter((w) => w > 0).length;
      overBody.replaceChildren(
        ...(t.keys.length === 0 ? [] : [h("div", { className: "cc-bar" }, keyPicker, minPicker, copyTrends)]),
        h("p", { className: "note", textContent: trendsLine(t, trendMin) }),
        ...(t.key === "" || live < 3 ? [] : [h("div", { className: "fq-trends-lists" }, list("Rising", t.rising), list("Falling", t.falling)), h("p", { className: "note", textContent: "Each line runs from the first period to the last, per 1000 words; the numbers are its first and last rates. Click a word to see its lines." })]),
      );
    };
    over.addEventListener("toggle", loadTrends);

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
      const rows = inTexts(filterRows(rankedRows, filter.value), Number(range.value));
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
      drawZipf();
      count.textContent = summary(data, noun()) + rich + perWord + spreadNote(rankedRows) + lexiconNote(rankedRows) + (filter.value.trim() !== "" ? ` ${rows.length} match the filter.` : "") + rangeNote(rows.length, Number(range.value), ngrams() ? size : 1) + cut + ` Counted at ${countedAt}.`;
      const withLexicon = marked();
      missingBox.hidden = !withLexicon;
      // Words never broken into morphs either: a glossed word's root is what belongs in the lexicon (Checks adds those).
      const missing = withLexicon ? rows.filter((r) => r.lexicon === "none") : [];
      addAll.hidden = missing.length === 0;
      addAll.textContent = addLabel(missing.length, addArmed);
      addAll.onclick = () => {
        if (!addArmed) {
          addArmed = true;
          drawRows();
          return;
        }
        addArmed = false;
        addAll.disabled = true;
        const words = missing.slice(0, ADD_AT_ONCE);
        void lexiconAddWords({ words: words.map((r) => r.item) }).then(
          (added) => {
            for (const r of words) r.lexicon = "entry";
            touchLexicon();
            addAll.disabled = false;
            drawRows();
            showToast({ tone: "info", lines: [addedLine(added, words.length)] });
          },
          (e) => {
            addAll.disabled = false;
            drawRows();
            showToast({ tone: "error", lines: [`Could not add the words: ${errorText(e)}`] });
          },
        );
      };
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
      addArmed = false;
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
        loadTrends();
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
    range.onchange = drawRows;
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
        range.value = "1";
        data = null;
        trendKey = null;
        trends = null;
      }
      drawChips();
      drawScopes();
      drawSizer();
      el.replaceChildren(h("div", { className: "cc-bar" }, scopePicker, sizer, sorter, recount, copy), chips, h("div", { className: "cc-bar" }, filter, range, missingBox, addAll), count, zipf, over, h("div", { className: "fq-rows" }, table));
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
