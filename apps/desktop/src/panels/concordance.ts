// Concordance (roadmap 71): every occurrence of a word, morph, gloss or
// translation word, one row per hit with the keyword in the middle, sortable
// by the words either side; a row opens its segment, ▶ plays it.
import { whereWords, type Where } from "../metadata/logic";
import { concordance, concordanceQuery, dispersion, errorText, getText, glossGet, saveHitClips, textMetadata, type TextDetail } from "../api";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { hitClipsLines } from "../measure/logic";
import { clipboardForAll, EXAMPLE_FORMATS, FORMAT_KEY, parseFormat, type Example } from "../gloss/example";
import { translationLines } from "../gloss/translation";
import { CLUSTER_SIDES, CLUSTER_SIZES, clusterLine, clusters, clustersTsv, type Cluster, type ClusterSide } from "../concordance/clusters";
import { byDetail, detailTsv, marks, NOT_GIVEN, per1000, plotDetails, PLOT_SORTS, plotTsv, sortPlots, spreadWords, type Dispersion, type PlotSort } from "../concordance/plot";
import { asTsv, clip, countLine, kwicHtml, hasTags, isRtl, keyword, left, markWord, sortedWord, withTags, PANEL_KINDS, QUERY_HELP, right, SORTS, sortHits, thin, thinLine, THIN_SIZES, type ConcordanceKind, type Hit, type PanelKind, type SortBy } from "../concordance/logic";
import { linesUnder } from "../concordance/under";
import { emptyState, h } from "../dom";
import { hear, playButton } from "../hear/hear";
import { provide } from "../palette/registry";
import { dataGeneration, getState, selectSegment, selectText, showToast } from "../store";
import { writeHtml, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { SHOW_PANEL, type PanelDef, type PanelView } from "./panel";

/** A query another panel asked for (Frequencies), taken by the panel once it is open. */
type Asked = { query: string; kind: PanelKind; text: { id: string; title: string } | null; exact: boolean; within: Where | null };
let pending: (Asked & { project: string }) | null = null;
const askers = new Set<() => void>();

/** Show the Concordance for this item, in one text or the whole project,
 * opening the panel if it is closed: met exactly (a Frequencies row, whose
 * spelling may hold a `?` glottal stop) or as a pattern (`walk*` from Collocates). */
export function askConcordance(query: string, kind: PanelKind, text: { id: string; title: string } | null = null, exact = true, within: Where | null = null): void {
  const s = getState();
  if (s.kind !== "open") return;
  pending = { query, kind, text, exact, within, project: s.info.projectId };
  window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "concordance" }));
  for (const take of askers) take();
}

/** At most this many sentences are copied as examples at once. */
const EXAMPLES_MAX = 50;

/** The sentences behind these lines (each once, in the order shown) as
 * numbered glossed examples, in the format chosen for Copy example; a
 * translation line brings the sentence it translates. */
async function copyAsExamples(lines: readonly Hit[]): Promise<void> {
  const seen = new Set<string>();
  const picked: Hit[] = [];
  let more = 0;
  const texts = new Map<string, TextDetail>();
  const examples: Example[] = [];
  let unglossed = 0;
  for (const hit of lines) {
    let t = texts.get(hit.textId);
    if (t === undefined) {
      t = await getText({ textId: hit.textId });
      texts.set(hit.textId, t);
    }
    const seg = t.segments.find((s) => s.id === hit.segmentId);
    const sentence = seg?.parent ? t.segments.find((s) => s.id === seg.parent) : seg;
    if (sentence === undefined || seen.has(sentence.id)) continue;
    seen.add(sentence.id);
    if (picked.length === EXAMPLES_MAX) {
      more += 1;
      continue;
    }
    picked.push(hit);
    const v = await glossGet({ textId: hit.textId, segmentId: sentence.id });
    const translation = translationLines(sentence, t.segments, t.tiers).find((l) => l.segment !== null)?.segment?.content ?? null;
    const words = v.words.map((w) => ({ form: w.form, morphs: w.morphs.map((m) => ({ surface: m.surface, type: m.type, gloss: m.gloss })) }));
    if (!words.some((w) => w.morphs.length > 0)) unglossed += 1;
    examples.push({ number: examples.length + 1, translation, words: words.length > 0 ? words : [{ form: sentence.content, morphs: [] }] });
  }
  if (examples.length === 0) return;
  let stored: string | null = null;
  try {
    stored = localStorage.getItem(FORMAT_KEY);
  } catch {
    stored = null;
  }
  const format = parseFormat(stored);
  const c = clipboardForAll(examples, format);
  if (c.html !== null) await writeHtml(c.html, c.text);
  else await writeText(c.text);
  const label = EXAMPLE_FORMATS.find((f) => f.id === format)?.label ?? "";
  showToast({
    tone: "info",
    lines: [
      `Copied ${examples.length} ${examples.length === 1 ? "sentence" : "sentences"} as numbered examples for ${label} (the format chosen for Copy example in the Gloss panel).`,
      ...(more > 0 ? [`${more} more ${more === 1 ? "sentence was" : "sentences were"} left out; narrow the search to copy them.`] : []),
      ...(unglossed > 0 ? [`${unglossed} ${unglossed === 1 ? "is" : "are"} not glossed yet and ${unglossed === 1 ? "shows" : "show"} its words only.`] : []),
    ],
  });
}

/** Characters of context shown on each side of the keyword. */
const CONTEXT = 48;

/** A row opens its segment; ▶ also plays it, from the text's own recording (roadmap 72). */
async function open(hit: Hit, play: boolean): Promise<void> {
  if (play) {
    await hear(hit);
    return;
  }
  await selectText(hit.textId);
  const s = getState();
  if (s.kind === "open" && s.text?.id === hit.textId) selectSegment({ textId: hit.textId, tierId: hit.tierId, segmentId: hit.segmentId });
}

export const concordancePanel: PanelDef = {
  title: "Concordance",
  create(): PanelView {
    const el = h("div", { className: "panel panel-concordance" });
    let kind: PanelKind = "word";
    let sortBy: SortBy = "text";
    let hits: Hit[] = [];
    let total = 0;
    let asked = "";
    let seq = 0;
    let timer = 0;
    /** The item Frequencies asked for: met exactly and in its text while the query is unchanged. */
    let exactFor: Asked | null = null;
    /** The Plot view (AntConc's Plot): where in each text the item occurs. */
    let plotOn = false;
    let plot: Dispersion | null = null;
    let plotSort: PlotSort = "text";
    /** The Clusters view (AntConc's Clusters): word sequences that hold the item. */
    let clustersOn = false;
    let clusterSize = 2;
    let clusterSide: ClusterSide = "right";
    let shownClusters: Cluster[] = [];
    const input = h("input", { type: "search", className: "cc-query", placeholder: PANEL_KINDS[0]!.hint, spellcheck: false });
    input.setAttribute("aria-label", "Find");
    const chips = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Look in" });
    const sorter = h("select", { className: "cc-sort", title: "Order of the rows" }, ...SORTS.map((x) => h("option", { value: x.id, textContent: x.label })));
    sorter.setAttribute("aria-label", "Sort");
    const copy = h("button", { type: "button", textContent: "Copy rows", title: "Copy every row as a table to paste into a spreadsheet, or into a document as lined-up concordance lines" });
    const copyExamples = h("button", { type: "button", textContent: "Copy as examples", title: "Copy the sentences these lines come from as numbered glossed examples for a paper (at most 50)" });
    const saveClipsButton = h("button", { type: "button", textContent: "Save clips…", title: "Save the sentences these lines come from as sound files, each cut from its text's recording, with a clips.csv index naming the text and what was searched for: every token of a word, ready for Praat" });
    const plotToggle = h("button", { type: "button", textContent: "Plot", title: "Show where in each text it occurs, and how evenly it is spread (AntConc's Plot)" });
    plotToggle.setAttribute("aria-pressed", "false");
    const plotSorter = h("select", { className: "cc-sort", title: "Order of the texts" }, ...PLOT_SORTS.map((x) => h("option", { value: x.id, textContent: x.label })));
    plotSorter.setAttribute("aria-label", "Order of the texts");
    /** "" plots each text; a detail's key sums the texts by its values. */
    let plotBy = "";
    const plotByPicker = h("select", { className: "cc-sort cc-plot-by", title: "Plot each text, or sum the texts by one of their details (genre, speaker…) to see how the item varies" });
    plotByPicker.setAttribute("aria-label", "Plot by");
    const clusterToggle = h("button", { type: "button", textContent: "Clusters", title: "Count the word sequences that hold it: the item and the words after or before it (AntConc's Clusters)" });
    clusterToggle.setAttribute("aria-pressed", "false");
    const sizePicker = h("select", { className: "cc-sort", title: "Words in a cluster" }, ...CLUSTER_SIZES.map((n) => h("option", { value: String(n), textContent: `${n} words` })));
    sizePicker.setAttribute("aria-label", "Words in a cluster");
    const sidePicker = h("select", { className: "cc-sort", title: "Where the item stands in the cluster" }, ...CLUSTER_SIDES.map((x) => h("option", { value: x.id, textContent: x.label })));
    sidePicker.setAttribute("aria-label", "Where the item stands");
    const TAGS_KEY = "wugs.concordance.tags";
    let tagsOn = (() => {
      try {
        return localStorage.getItem(TAGS_KEY) === "on";
      } catch {
        return false;
      }
    })();
    const tagsToggle = h("button", { type: "button", textContent: "Tags", title: "Show each word's part of speech beside it (word_TAG), as AntConc shows a tagged corpus" });
    tagsToggle.setAttribute("aria-pressed", String(tagsOn));
    tagsToggle.classList.toggle("active", tagsOn);
    tagsToggle.hidden = true;
    const UNDER_KEY = "wugs.concordance.translations";
    let underOn = (() => {
      try {
        return localStorage.getItem(UNDER_KEY) === "on";
      } catch {
        return false;
      }
    })();
    const underToggle = h("button", { type: "button", className: "cc-under-toggle", textContent: "With translations", title: "Show each line's translation under it; under a line found in a translation, the sentence it translates" });
    underToggle.setAttribute("aria-pressed", String(underOn));
    underToggle.classList.toggle("active", underOn);
    /** Each text read so far: the line under each of its segments. Read again when the data changes. */
    const under = new Map<string, Map<string, string>>();
    let underGeneration = -1;
    let underLoading = false;
    /** Bumped whenever `under` is emptied, so a read begun before is dropped. */
    let underEpoch = 0;
    /** Read the texts behind the lines on screen that are not read yet, then draw again. */
    const readUnder = (ids: string[]) => {
      underLoading = true;
      const epoch = underEpoch;
      void (async () => {
        for (const id of ids) {
          try {
            const t = await getText({ textId: id });
            if (epoch !== underEpoch) return;
            under.set(id, linesUnder(t.segments, t.tiers));
          } catch {
            // A text that cannot be read shows no lines under it.
            under.set(id, new Map());
          }
        }
      })().finally(() => {
        underLoading = false;
        // A read dropped by a newer epoch leaves its texts unread: drawing again reads them.
        if (underOn) drawRows();
      });
    };
    /** Thinned to a random sample of this many lines (0: every line). */
    let thinTo = 0;
    const thinner = h("select", { className: "cc-sort cc-thin", title: "Show a random sample of the lines, the same each time for the same query (CQPweb's thin)" }, h("option", { value: "0", textContent: "Every line" }), ...THIN_SIZES.map((n) => h("option", { value: String(n), textContent: `${n} at random` })));
    thinner.setAttribute("aria-label", "Lines shown");
    const sampled = (): Hit[] => (thinTo > 0 ? thin(hits, thinTo, `${kind}|${asked}`) : hits);
    /** The lines as shown: thinned when asked, with tags when they have them and Tags is on. */
    const shownHits = (): Hit[] => (tagsOn ? sampled().map(withTags) : sampled());
    const count = h("p", { className: "note cc-count" });
    const rows = h("div", { className: "cc-rows", role: "list" });

    const drawChips = () =>
      chips.replaceChildren(
        ...PANEL_KINDS.map((k) => {
          const b = h("button", { type: "button", textContent: k.label, title: `Find ${k.hint}` });
          b.setAttribute("role", "radio");
          b.setAttribute("aria-checked", String(k.id === kind));
          b.onclick = () => {
            kind = k.id;
            input.placeholder = k.hint;
            drawChips();
            run();
          };
          return b;
        }),
      );

    /** Columns of a plot bar; matches closer than one column share a mark. */
    const COLUMNS = 240;
    const drawPlot = () => {
      if (kind === "query") {
        count.textContent = "The plot shows words, morphs, glosses and translation words. Choose one of those to plot.";
        rows.replaceChildren();
        return;
      }
      if (plot === null) {
        count.textContent = "Counting…";
        rows.replaceChildren();
        return;
      }
      const d = plot;
      const keys = plotDetails(d.texts);
      if (!keys.includes(plotBy)) plotBy = "";
      plotByPicker.replaceChildren(h("option", { value: "", textContent: "Each text", selected: plotBy === "" }), ...keys.map((k) => h("option", { value: k, textContent: `By ${k.replace(/_/g, " ")}`, selected: k === plotBy })));
      plotByPicker.disabled = keys.length === 0;
      plotSorter.hidden = plotBy !== "";
      count.textContent = spreadWords(d);
      if (plotBy !== "") {
        const groups = byDetail(d.texts, plotBy);
        const top = Math.max(0, ...groups.map((g) => (g.words === 0 ? 0 : g.hits / g.words)));
        count.textContent = `${spreadWords(d)} Summed by ${plotBy.replace(/_/g, " ")}: matches per 1000 words in the texts with each value.`;
        rows.replaceChildren(
          ...groups.map((g) => {
            const fill = h("span", { className: "cc-detail-fill" });
            fill.style.width = `${top === 0 || g.words === 0 ? 0 : Math.round((100 * (g.hits / g.words)) / top)}%`;
            return h(
              "div",
              { className: `cc-plot-row${g.hits === 0 ? " cc-plot-none" : ""}`, role: "listitem" },
              h("span", { className: `cc-plot-title${g.value === NOT_GIVEN ? " note" : ""}`, textContent: g.value }),
              h("span", { className: "cc-plot-count", textContent: `${g.hits} · ${per1000(g.hits, g.words)} per 1000 · ${g.texts === 1 ? "1 text" : `${g.texts} texts`}` }),
              h("div", { className: "cc-plot-bar" }, fill),
            );
          }),
        );
        return;
      }
      rows.replaceChildren(
        ...sortPlots(d.texts, plotSort).map((t) => {
          const bar = h("div", { className: "cc-plot-bar", role: "img" });
          bar.setAttribute("aria-label", `${t.hits} in ${t.words} words`);
          for (const col of marks(t.positions, t.words, COLUMNS)) {
            const m = h("span", { className: "cc-plot-mark" });
            m.style.left = `${(col / COLUMNS) * 100}%`;
            bar.append(m);
          }
          const row = h(
            "div",
            { className: `cc-plot-row${t.hits === 0 ? " cc-plot-none" : ""}`, role: "listitem", tabIndex: 0, title: t.hits === 0 ? `Not in «${t.title}»` : `Show the ${t.hits} ${t.hits === 1 ? "line" : "lines"} in «${t.title}»` },
            h("span", { className: "cc-plot-title", textContent: t.title || "(untitled)" }),
            h("span", { className: "cc-plot-count", textContent: `${t.hits} · ${per1000(t.hits, t.words)} per 1000` }),
            bar,
          );
          const show = () => {
            if (t.hits === 0) return;
            // The lines in this text, met as the plot met them.
            const prior = exactFor !== null && exactFor.query === asked && exactFor.kind === kind ? exactFor : null;
            exactFor = { query: asked, kind: kind as ConcordanceKind, text: { id: t.textId, title: t.title }, exact: prior?.exact ?? false, within: prior?.within ?? null };
            setPlot(false);
            run();
          };
          row.onclick = show;
          row.onkeydown = (e) => {
            if (e.key === "Enter") show();
            else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault();
              const next = e.key === "ArrowDown" ? row.nextElementSibling : row.previousElementSibling;
              if (next instanceof HTMLElement) next.focus();
            }
          };
          return row;
        }),
      );
    };

    const drawClusters = () => {
      shownClusters = clusters(hits, clusterSize, clusterSide);
      count.textContent = clusterLine(shownClusters, hits.length, total, clusterSize);
      rows.replaceChildren(
        ...shownClusters.slice(0, 500).map((c) => {
          const phrase = c.words.join(" ");
          const row = h(
            "div",
            { className: "cc-cluster", role: "listitem", tabIndex: 0, title: `Show the lines with «${phrase}»` },
            h("span", { className: "cc-cluster-words" }, ...c.words.flatMap((w, i) => [i > 0 ? " " : "", i >= c.at && i < c.at + c.len ? h("strong", { textContent: w }) : w])),
            h("span", { className: "cc-plot-count", textContent: `${c.count === 1 ? "1 line" : `${c.count} lines`} · ${c.texts === 1 ? "1 text" : `${c.texts} texts`}` }),
          );
          const show = () => {
            // The cluster as words in a row, met exactly in the same text and texts as before.
            const prior = exactFor !== null && exactFor.query === asked && exactFor.kind === kind ? exactFor : null;
            if (kind !== "translation") kind = "word";
            exactFor = { query: phrase, kind: kind as ConcordanceKind, text: prior?.text ?? null, exact: true, within: prior?.within ?? null };
            input.value = phrase;
            input.placeholder = PANEL_KINDS.find((k) => k.id === kind)!.hint;
            setClusters(false);
            drawChips();
            run();
          };
          row.onclick = show;
          row.onkeydown = (e) => {
            if (e.key === "Enter") show();
            else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault();
              const next = e.key === "ArrowDown" ? row.nextElementSibling : row.previousElementSibling;
              if (next instanceof HTMLElement) next.focus();
            }
          };
          return row;
        }),
      );
    };

    const setClusters = (on: boolean) => {
      clustersOn = on;
      if (on && plotOn) setPlot(false);
      clusterToggle.setAttribute("aria-pressed", String(on));
      clusterToggle.classList.toggle("active", on);
      sizePicker.hidden = !on;
      sidePicker.hidden = !on;
      sorter.hidden = on || plotOn;
      thinner.hidden = on || plotOn;
      copyExamples.hidden = on || plotOn;
      saveClipsButton.hidden = on || plotOn;
    };

    const setPlot = (on: boolean) => {
      if (on && clustersOn) setClusters(false);
      plotOn = on;
      plotToggle.setAttribute("aria-pressed", String(on));
      plotToggle.classList.toggle("active", on);
      sorter.hidden = on;
      thinner.hidden = on;
      copyExamples.hidden = on;
      saveClipsButton.hidden = on;
      plotSorter.hidden = !on;
      plotByPicker.hidden = !on;
    };

    const drawRows = () => {
      if (plotOn && asked !== "") {
        drawPlot();
        return;
      }
      if (clustersOn && asked !== "") {
        drawClusters();
        return;
      }
      if (asked === "") {
        count.textContent = kind === "query" ? QUERY_HELP : "Type a word, morph or gloss. * stands for any letters: ni* finds every word starting ni.";
        rows.replaceChildren();
        return;
      }
      const texts = new Set(hits.map((x) => x.textId)).size;
      const scoped = exactFor !== null && exactFor.query === asked && exactFor.kind === kind ? exactFor : null;
      const only = [scoped?.text ? `«${scoped.text.title}»` : null, scoped?.within ? whereWords(scoped.within) : null].filter((x) => x !== null);
      const thinned = thinTo > 0 && hits.length > thinTo;
      count.textContent = countLine(hits.length, total, texts) + (only.length > 0 ? ` Only in ${only.join(", among ")}.` : "") + (thinned ? thinLine(thinTo, hits.length, total) : "");
      tagsToggle.hidden = !hasTags(hits);
      // The word the lines are sorted by is marked, as concordancers colour it.
      const mark = sortedWord(sortBy);
      const context = (text: string, side: "left" | "right"): HTMLElement => {
        const span = h("span", { className: side === "left" ? "cc-left" : "cc-right" });
        const [before, word, after] = mark !== null && (mark < 0) === (side === "left") ? markWord(text, side, mark) : [text, "", ""];
        span.append(before, ...(word === "" ? [] : [h("mark", { className: "cc-sorted", textContent: word })]), after);
        return span;
      };
      const lines = sortHits(shownHits(), sortBy);
      if (underOn) {
        if (dataGeneration() !== underGeneration) {
          under.clear();
          underEpoch += 1;
          underGeneration = dataGeneration();
        }
        const missing = [...new Set(lines.map((x) => x.textId))].filter((id) => !under.has(id));
        if (missing.length > 0 && !underLoading) readUnder(missing);
      }
      rows.replaceChildren(
        ...lines.map((hit) => {
          const play = playButton("Open this segment and play it", () => void open(hit, true));
          play.classList.add("cc-play");
          play.disabled = hit.startMs === null;
          const row = h(
            "div",
            { className: "cc-row", role: "listitem", tabIndex: 0, title: `${hit.textTitle}: open this segment`, dir: isRtl(keyword(hit)) ? "rtl" : "ltr" },
            play,
            context(clip(left(hit), CONTEXT, "left"), "left"),
            h("span", { className: "cc-key", textContent: keyword(hit) }),
            context(clip(right(hit), CONTEXT, "right"), "right"),
            h(
              "span",
              { className: "cc-meta" },
              h("span", { textContent: hit.breakdown === null ? "" : `${hit.breakdown} ‘${hit.glosses ?? ""}’` }),
              h("span", { textContent: hit.textTitle }),
            ),
            ...(underOn ? [h("span", { className: "cc-under", textContent: under.get(hit.textId)?.get(hit.segmentId) ?? "" })] : []),
          );
          row.onclick = () => void open(hit, false);
          row.onkeydown = (e) => {
            if (e.key === "Enter") void open(hit, false);
            else if (e.key === " ") {
              e.preventDefault();
              void open(hit, true);
            } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault();
              const next = e.key === "ArrowDown" ? row.nextElementSibling : row.previousElementSibling;
              if (next instanceof HTMLElement) next.focus();
            }
          };
          return row;
        }),
      );
    };

    function run(): void {
      window.clearTimeout(timer);
      const query = input.value.trim();
      const mine = ++seq;
      if (query === "") {
        asked = "";
        hits = [];
        total = 0;
        drawRows();
        return;
      }
      timer = window.setTimeout(() => {
        // An item asked for from Frequencies is met exactly, in its text, until the query is edited.
        const exactAsk = exactFor !== null && exactFor.query === query && exactFor.kind === kind ? exactFor : null;
        plot = null;
        if (plotOn && kind !== "query") {
          const k = kind;
          dispersion({ query, kind: k, exact: exactAsk?.exact ?? false, textId: exactAsk?.text?.id ?? null, within: exactAsk?.within ?? null }).then(
            (d) => {
              if (mine !== seq) return;
              plot = d;
              asked = query;
              drawRows();
            },
            (e) => {
              if (mine === seq) count.textContent = `Could not plot: ${errorText(e)}`;
            },
          );
        }
        const asking =
          kind === "query"
            ? concordanceQuery({ query, textId: exactAsk?.text?.id ?? null })
            : concordance({ query, kind, exact: exactAsk?.exact ?? false, textId: exactAsk?.text?.id ?? null, within: exactAsk?.within ?? null });
        asking.then(
          (r) => {
            if (mine !== seq) return;
            asked = query;
            hits = r.hits;
            total = r.total;
            drawRows();
          },
          (e) => {
            if (mine !== seq) return;
            hits = [];
            total = 0;
            rows.replaceChildren();
            count.textContent = kind === "query" ? `${errorText(e)}. ${QUERY_HELP}` : `Could not search: ${errorText(e)}`;
          },
        );
      }, 200);
    }

    input.oninput = run;
    sorter.onchange = () => {
      sortBy = sorter.value as SortBy;
      drawRows();
    };
    thinner.onchange = () => {
      thinTo = Number(thinner.value);
      drawRows();
    };
    copyExamples.onclick = () => {
      if (hits.length === 0) return;
      copyExamples.disabled = true;
      void copyAsExamples(sortHits(sampled(), sortBy))
        .catch((e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }))
        .finally(() => {
          copyExamples.disabled = false;
        });
    };
    saveClipsButton.onclick = async () => {
      if (hits.length === 0) return;
      const folder = await pickFile({ directory: true, title: "Where should the clips go?" });
      if (typeof folder !== "string") return;
      const lines = sortHits(sampled(), sortBy);
      saveClipsButton.disabled = true;
      try {
        const r = await saveHitClips({ query: asked, hits: lines.map((h) => ({ textId: h.textId, segmentId: h.segmentId, keyword: keyword(h) })), folder });
        showToast({ tone: "info", lines: hitClipsLines(r) });
      } catch (e) {
        showToast({ tone: "error", lines: [`Could not save the clips: ${errorText(e)}`] });
      } finally {
        saveClipsButton.disabled = false;
      }
    };
    plotToggle.onclick = () => {
      setPlot(!plotOn);
      if (plotOn && asked !== "") run();
      else drawRows();
    };
    clusterToggle.onclick = () => {
      setClusters(!clustersOn);
      drawRows();
    };
    sizePicker.onchange = () => {
      clusterSize = Number(sizePicker.value);
      drawRows();
    };
    sidePicker.onchange = () => {
      clusterSide = sidePicker.value as ClusterSide;
      drawRows();
    };
    plotByPicker.onchange = () => {
      plotBy = plotByPicker.value;
      drawPlot();
    };
    plotSorter.onchange = () => {
      plotSort = plotSorter.value as PlotSort;
      drawRows();
    };
    underToggle.onclick = () => {
      underOn = !underOn;
      underToggle.setAttribute("aria-pressed", String(underOn));
      underToggle.classList.toggle("active", underOn);
      try {
        localStorage.setItem(UNDER_KEY, underOn ? "on" : "off");
      } catch {
        // storage unavailable: the choice lasts this session
      }
      drawRows();
    };
    tagsToggle.onclick = () => {
      tagsOn = !tagsOn;
      tagsToggle.setAttribute("aria-pressed", String(tagsOn));
      tagsToggle.classList.toggle("active", tagsOn);
      try {
        localStorage.setItem(TAGS_KEY, tagsOn ? "on" : "off");
      } catch {
        // storage unavailable: the choice lasts this session
      }
      drawRows();
    };

    copy.onclick = () => {
      if (clustersOn && asked !== "") {
        void writeText(clustersTsv(shownClusters)).then(
          () => showToast({ tone: "info", lines: [`Copied ${shownClusters.length} clusters. Paste them into a spreadsheet.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
        return;
      }
      if (plotOn && plot !== null && plotBy !== "") {
        void writeText(detailTsv(byDetail(plot.texts, plotBy), plotBy)).then(
          () => showToast({ tone: "info", lines: [`Copied the matches by ${plotBy.replace(/_/g, " ")}. Paste them into a spreadsheet.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
        return;
      }
      if (plotOn && plot !== null) {
        void writeText(plotTsv(plot)).then(
          () => showToast({ tone: "info", lines: [`Copied the plot of ${plot?.texts.length ?? 0} texts. Paste it into a spreadsheet.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
        return;
      }
      if (hits.length === 0) return;
      // As a KWIC table for a word processor, and tab-separated text beside it for anything else.
      const lines = sortHits(shownHits(), sortBy);
      // The tab-separated copy carries each line's times, analysis and its
      // text's details, so it goes straight into R or a spreadsheet; a text
      // whose details cannot be read just has none.
      const ids = [...new Set(lines.map((l) => l.textId))];
      void Promise.all(ids.map((id) => textMetadata({ textId: id }).then((m): [string, Map<string, string>] => [id, new Map(m.fields.map((f) => [f.key, f.value]))], (): [string, Map<string, string>] => [id, new Map()])))
        .then((pairs) => {
          // With translations on, each line's translation goes along as it shows.
          const shown = underOn ? (h: Hit) => under.get(h.textId)?.get(h.segmentId) ?? null : undefined;
          return writeHtml(kwicHtml(lines, 40, shown), asTsv(lines, new Map(pairs), shown));
        })
        .then(
        () =>
          showToast({
            tone: "info",
            lines: [total > hits.length ? `Copied the first ${hits.length} of ${total} rows. Narrow the search to copy the rest.` : `Copied ${hits.length} rows. Paste them into a spreadsheet, or into Word as a lined-up table.`],
          }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    const unprovide = provide("concordance", () => [
      {
        id: "concordance:find",
        title: "Find every occurrence (concordance)",
        group: "Concordance",
        words: ["kwic", "keyword in context", "occurrences", "corpus", "where is"],
        run: () => input.focus(),
      },
    ]);

    let shown: string | null = null;
    const render = () => {
      const s = getState();
      const now = s.kind === "open" ? s.info.projectId : "closed";
      if (now === shown) return;
      shown = now;
      // Another project: its occurrences, not the last one's.
      seq += 1;
      hits = [];
      total = 0;
      plot = null;
      asked = "";
      exactFor = null;
      under.clear();
      underEpoch += 1;
      underGeneration = -1;
      input.value = "";
      if (now === "closed") {
        el.replaceChildren(emptyState("Concordance", "Open a project to see every place a word, morph or gloss is used.", "The wug is counting words."));
        return;
      }
      drawChips();
      drawRows();
      setPlot(plotOn);
      setClusters(clustersOn);
      el.replaceChildren(h("div", { className: "cc-bar" }, input, sorter, thinner, plotSorter, plotByPicker, sizePicker, sidePicker, plotToggle, clusterToggle, tagsToggle, underToggle, copy, copyExamples, saveClipsButton), chips, count, rows);
      take();
    };
    /** A query asked for from another panel replaces the one typed here. */
    const take = () => {
      if (pending === null || shown === null || shown === "closed") return;
      // Asked in another project (switched since): drop it.
      if (pending.project !== shown) {
        pending = null;
        return;
      }
      ({ kind } = pending);
      exactFor = { query: pending.query, kind: pending.kind, text: pending.text, exact: pending.exact, within: pending.within };
      input.value = pending.query;
      input.placeholder = PANEL_KINDS.find((k) => k.id === kind)?.hint ?? "";
      pending = null;
      drawChips();
      run();
    };
    askers.add(take);
    return {
      element: el,
      render,
      dispose: () => {
        window.clearTimeout(timer);
        askers.delete(take);
        unprovide();
      },
    };
  },
};
