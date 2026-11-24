// Concordance (roadmap 71): every occurrence of a word, morph, gloss or
// translation word, one row per hit with the keyword in the middle, sortable
// by the words either side; a row opens its segment, ▶ plays it.
import { whereWords, type Where } from "../metadata/logic";
import { deleteSavedSearch, saveSearch, savedSearches, fileUnderTopic, topicsOverview, concordance, concordanceQuery, corpusTracks, dispersion, worldLand, errorText, getText, glossGet, hitCategories, saveHitClips, setHitCategories, setHitCategory, textMetadata, type TextDetail } from "../api";
import { categoriesLine, crossTab, crossTabTsv, independence, independenceLine, lineKey, parseCategories, type Sorted } from "../concordance/categories";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { hitClipsLines } from "../measure/logic";
import { clipboardForAll, EXAMPLE_FORMATS, FORMAT_KEY, parseFormat, type Example } from "../gloss/example";
import { translationLines } from "../gloss/translation";
import { CLUSTER_SIDES, CLUSTER_SIZES, clusterLine, clusters, clustersTsv, type Cluster, type ClusterSide } from "../concordance/clusters";
import { byDetail, byPeriod, detailTsv, marks, NOT_GIVEN, per1000, periodTsv, plotDetails, PLOT_SORTS, plotTsv, sortPlots, spreadWords, timeKeys, type Dispersion, type PlotSort } from "../concordance/plot";
import { bySite, dotShare, hasSites, siteTitle, sitesLine, sitesTsv } from "../concordance/sites";
import { mapView } from "../typology/map";
import { nearHits, nearLine, NEAR_SIDES, NEAR_WINDOWS, type NearSide } from "../concordance/near";
import { savedLine, type SavedSearch, asTsv, clip, countLine, kwicHtml, hasTags, isRtl, keyword, left, markWord, sortedWord, speakerOf, withTags, PANEL_KINDS, QUERY_HELP, right, SORTS, sortHits, thin, thinLine, THIN_SIZES, type ConcordanceKind, type Hit, type PanelKind, type SortBy } from "../concordance/logic";
import { linesUnder } from "../concordance/under";
import { around } from "../concordance/around";
import { positions, positionsTsv } from "../concordance/positions";
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
/** Natural Earth's land, read once for the plot's map. */
let land: string | null = null;

/** The sentences behind these lines (each once, in the order shown) as
 * numbered glossed examples, in the format chosen for Copy example; a
 * translation line brings the sentence it translates. */
export async function copyAsExamples(lines: readonly Pick<Hit, "textId" | "segmentId">[]): Promise<void> {
  const seen = new Set<string>();
  const picked: Pick<Hit, "textId" | "segmentId">[] = [];
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
    // File the lines' sentences under a grammar topic (the Topics panel gathers them).
    const fileTopic = h("button", { type: "button", textContent: "File under a topic…", title: "File the sentences these lines come from under a grammar topic, to copy them later from the Topics panel as examples for a chapter" });
    const topicName = h("input", { className: "cc-topic-name", placeholder: "Topic, such as negation", spellcheck: false });
    topicName.setAttribute("aria-label", "Topic");
    const topicNames = h("datalist", { id: "cc-topic-names" });
    topicName.setAttribute("list", topicNames.id);
    const topicGo = h("button", { type: "button", className: "primary", textContent: "File them" });
    const topicCancel = h("button", { type: "button", textContent: "Cancel" });
    const topicBar = h("div", { className: "cc-bar cc-topic-bar" }, topicName, topicNames, topicGo, topicCancel);
    topicBar.hidden = true;
    // Searches kept with the project under a name (CQPweb's saved queries).
    let saved: SavedSearch[] = [];
    const savedPick = h("select", { className: "cc-saved", title: "Searches kept with this project: pick one to run it again" });
    savedPick.setAttribute("aria-label", "Saved searches");
    const keepSearch = h("button", { type: "button", textContent: "Keep search…", title: "Keep this search with the project under a name, to run it again later or on another device" });
    const keepName = h("input", { className: "cc-keep-name", placeholder: "Name, such as negated verbs", spellcheck: false });
    keepName.setAttribute("aria-label", "The search's name");
    const keepGo = h("button", { type: "button", className: "primary", textContent: "Keep it" });
    const keepCancel = h("button", { type: "button", textContent: "Cancel" });
    const keepBar = h("div", { className: "cc-bar cc-keep-bar" }, keepName, keepGo, keepCancel);
    keepBar.hidden = true;
    const drawSaved = () => {
      savedPick.hidden = saved.length === 0;
      savedPick.replaceChildren(
        h("option", { value: "", textContent: `Saved searches (${saved.length})` }),
        h("optgroup", { label: "Run" }, ...saved.map((x) => h("option", { value: `r:${x.name}`, textContent: x.name, title: savedLine(x) }))),
        h("optgroup", { label: "Forget" }, ...saved.map((x) => h("option", { value: `x:${x.name}`, textContent: `Forget «${x.name}»` }))),
      );
      savedPick.value = "";
    };
    const loadSaved = () =>
      void savedSearches().then(
        (r) => {
          saved = r;
          drawSaved();
        },
        () => undefined,
      );
    savedPick.onchange = () => {
      const v = savedPick.value;
      savedPick.value = "";
      const x = saved.find((s) => s.name === v.slice(2));
      if (x === undefined) return;
      if (v.startsWith("r:")) {
        askConcordance(x.query, x.kind as PanelKind, x.textId ? { id: x.textId, title: x.textTitle ?? "" } : null, x.exact, x.within);
        return;
      }
      void deleteSavedSearch(x.name).then(
        (r) => {
          saved = r;
          drawSaved();
          showToast({ tone: "info", lines: [`Forgot the search «${x.name}».`] });
        },
        (e) => showToast({ tone: "error", lines: [errorText(e)] }),
      );
    };
    keepSearch.onclick = () => {
      keepBar.hidden = !keepBar.hidden;
      if (keepBar.hidden) return;
      keepName.value = keepName.value || input.value.trim().slice(0, 80);
      keepName.focus();
      keepName.select();
    };
    keepCancel.onclick = () => {
      keepBar.hidden = true;
    };
    const keepIt = async () => {
      const query = input.value.trim();
      if (query === "" || keepName.value.trim() === "") return;
      const scoped = exactFor !== null && exactFor.query === query && exactFor.kind === kind ? exactFor : null;
      try {
        saved = await saveSearch({ name: keepName.value, kind, query, textId: scoped?.text?.id ?? null, textTitle: scoped?.text?.title ?? null, exact: scoped?.exact ?? false, within: scoped?.within ?? null });
        drawSaved();
        keepBar.hidden = true;
        showToast({ tone: "info", lines: [`Kept the search as «${keepName.value.trim()}». Pick it under Saved searches to run it again.`] });
        keepName.value = "";
      } catch (e) {
        showToast({ tone: "error", lines: [`Could not keep it: ${errorText(e)}`] });
      }
    };
    keepGo.onclick = () => void keepIt();
    keepName.onkeydown = (e) => {
      if (e.key === "Enter") void keepIt();
      else if (e.key === "Escape") keepBar.hidden = true;
    };
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
    /** Each track's name, for the speaker of a conversation's lines. */
    let trackNames: { generation: number; names: Map<string, string>; asking: boolean } = { generation: -1, names: new Map(), asking: false };
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
    /** Lines opened to their wider context (`textId|segmentId`), and the texts read for them in this data generation. */
    const opened = new Set<string>();
    const aroundTexts = new Map<string, TextDetail | null>();
    let aroundGeneration = -1;
    const aroundOf = (hit: Hit): HTMLElement | null => {
      if (!opened.has(`${hit.textId}|${hit.segmentId}`)) return null;
      if (aroundGeneration !== dataGeneration()) {
        aroundTexts.clear();
        aroundGeneration = dataGeneration();
      }
      if (!aroundTexts.has(hit.textId)) {
        aroundTexts.set(hit.textId, null);
        const gen = aroundGeneration;
        void getText({ textId: hit.textId }).then(
          (t) => {
            if (gen !== aroundGeneration) return;
            aroundTexts.set(hit.textId, t);
            drawRows();
          },
          (e) => showToast({ tone: "error", lines: [`Could not read «${hit.textTitle}»: ${errorText(e)}`] }),
        );
      }
      const text = aroundTexts.get(hit.textId);
      if (text === null || text === undefined) return h("div", { className: "cc-around note", textContent: "Reading the text…" });
      const near = around(text.segments, hit.segmentId);
      if (near === null) return h("div", { className: "cc-around note", textContent: "This sentence is no longer in the text." });
      const line = (content: string, me: boolean) => h("p", { className: me ? "cc-around-me" : "", textContent: content });
      const mine = text.segments.find((x) => x.id === hit.segmentId)?.content ?? "";
      return h(
        "div",
        { className: "cc-around", dir: isRtl(keyword(hit)) ? "rtl" : "ltr" },
        ...near.before.map((x) => line(x.content, false)),
        line(mine, true),
        ...near.after.map((x) => line(x.content, false)),
        near.before.length + near.after.length === 0 ? h("p", { className: "note", textContent: "Nothing else on this track." }) : null,
      );
    };
    /** Thinned to a random sample of this many lines (0: every line). */
    let thinTo = 0;
    const thinner = h("select", { className: "cc-sort cc-thin", title: "Show a random sample of the lines, the same each time for the same query (CQPweb's thin)" }, h("option", { value: "0", textContent: "Every line" }), ...THIN_SIZES.map((n) => h("option", { value: String(n), textContent: `${n} at random` })));
    thinner.setAttribute("aria-label", "Lines shown");
    // Near: only the lines with another word close to the match (AntConc's context words).
    const nearInput = h("input", { type: "search", className: "cc-near-word", placeholder: "near a word", title: "Keep only the lines with this word close to the match. * stands for any letters" });
    nearInput.setAttribute("aria-label", "Near a word");
    const nearWindow = h("select", { title: "How close" }, ...NEAR_WINDOWS.map((n) => h("option", { value: String(n), textContent: n === 1 ? "next to it" : `within ${n} words` })));
    nearWindow.setAttribute("aria-label", "How close");
    nearWindow.value = "3";
    const nearSide = h("select", { title: "On which side of the match" }, ...NEAR_SIDES.map((s) => h("option", { value: s.id, textContent: s.label })));
    nearSide.setAttribute("aria-label", "Which side");
    const nearBar = h("div", { className: "cc-bar cc-near" }, nearInput, nearWindow, nearSide);
    // The commonest words at each position around the match, drawn while open.
    const posBox = h("details", { className: "cc-positions" }, h("summary", { textContent: "Words around it" }));
    const posBody = h("div", {});
    posBox.append(posBody);
    posBox.hidden = true;
    const drawPositions = (lines: Hit[]) => {
      posBox.hidden = lines.length === 0 || plotOn || clustersOn;
      if (!posBox.open || posBox.hidden) {
        posBody.replaceChildren();
        return;
      }
      const cols = positions(lines);
      const copyPos = h("button", { type: "button", textContent: "Copy table", title: "The commonest words at each position, to paste into a spreadsheet" });
      copyPos.onclick = () =>
        void writeText(positionsTsv(cols)).then(
          () => showToast({ tone: "info", lines: ["Copied the words around it."] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      const column = (c: (typeof cols)[number]) =>
        h(
          "div",
          { className: "cc-pos-col" },
          h("div", { className: "cc-pos-head", textContent: c.label }),
          ...c.rows.map((r) => {
            const b = h("button", { type: "button", className: "cc-pos-word", title: `Keep only the lines with «${r.word}» ${c.label}` }, h("span", { textContent: r.word }), h("span", { className: "cc-pos-n", textContent: String(r.count) }));
            b.onclick = () => {
              nearInput.value = r.word;
              const d = Math.abs(c.offset);
              nearWindow.value = String(NEAR_WINDOWS.find((x) => x >= d) ?? 10);
              nearSide.value = c.offset < 0 ? "before" : "after";
              drawRows();
            };
            return b;
          }),
        );
      posBody.replaceChildren(
        h("div", { className: "cc-pos-grid" }, ...cols.filter((c) => c.offset < 0).map(column), h("div", { className: "cc-pos-col cc-pos-match" }, h("div", { className: "cc-pos-head", textContent: "match" }), h("strong", { textContent: asked })), ...cols.filter((c) => c.offset > 0).map(column)),
        h("p", { className: "note", textContent: "Each word counted once per line, capitals and punctuation aside. Click a word to keep only the lines with it near the match." }),
        h("div", { className: "cc-bar" }, copyPos),
      );
    };
    posBox.addEventListener("toggle", () => drawPositions(chosen()));
    const nearOn = () => nearInput.value.trim() !== "";
    const kept = (): Hit[] => (nearOn() ? nearHits(hits, nearInput.value, Number(nearWindow.value), nearSide.value as NearSide) : hits);
    // Sorting the lines into categories (CQPweb's categorise): kept in the project per search.
    let sortOn = false;
    let sorted: Sorted | null = null;
    let sortedFor = "";
    const sortToggle = h("button", { type: "button", className: "cc-sort-toggle", textContent: "Sort into categories", title: "Put each line in a category of your own (literal, figurative, unclear…) and count them, as CQPweb's Categorise does. The categories are kept with the project for this search" });
    const catInput = h("input", { className: "cc-cats-input", placeholder: "literal, figurative, unclear", spellcheck: false, title: "The categories for this search, separated by commas; Enter keeps them" });
    catInput.setAttribute("aria-label", "Categories");
    const catShow = h("select", { className: "cc-cats-show", title: "Which lines to list" });
    catShow.setAttribute("aria-label", "Lines to list");
    const catNote = h("p", { className: "note cc-cats-note" });
    // The sorted lines compared across texts, speakers or a detail (a variationist's cross-table).
    const crossPick = h("select", { className: "cc-cross-by", title: "Compare the categories across texts, speakers or a detail of the texts, with a chi-square test" });
    crossPick.setAttribute("aria-label", "Compare by");
    const crossBox = h("div", { className: "cc-cross" });
    /** Each hit text's details, read once per data generation for the cross-table. */
    const crossMeta = new Map<string, Map<string, string>>();
    let crossGeneration = -1;
    let crossReading = false;
    const catBar = h("div", { className: "cc-bar cc-cats" }, catInput, catShow, crossPick);
    const catKey = () => `${kind}|${asked}|${dataGeneration()}`;
    const categoryOf = (hit: Hit): string => (sorted === null ? "" : (new Map(sorted.lines).get(lineKey(hit)) ?? ""));
    /** The lines after the category filter, when sorting is on. */
    const chosen = (): Hit[] => {
      const k = kept();
      if (!sortOn || sorted === null || catShow.value === "" || catShow.value === "all") return k;
      const of = new Map(sorted.lines);
      return catShow.value === "none" ? k.filter((h) => !sorted!.categories.includes(of.get(lineKey(h)) ?? "")) : k.filter((h) => of.get(lineKey(h)) === catShow.value);
    };
    const drawCats = () => {
      catBar.hidden = !sortOn || plotOn || clustersOn || asked === "";
      catNote.hidden = catBar.hidden;
      crossBox.hidden = catBar.hidden;
      if (catBar.hidden) return;
      const want = catKey();
      if (want !== sortedFor) {
        sortedFor = want;
        sorted = null;
        catNote.textContent = "Reading this search's categories…";
        const forKey = want;
        void hitCategories({ kind, query: asked }).then(
          (r) => {
            if (sortedFor !== forKey) return;
            sorted = r;
            if (document.activeElement !== catInput) catInput.value = r.categories.join(", ");
            drawRows();
          },
          (e) => {
            if (sortedFor === forKey) catNote.textContent = `Could not read the categories: ${errorText(e)}`;
          },
        );
        return;
      }
      if (sorted === null) return;
      const was = catShow.value || "all";
      catShow.replaceChildren(h("option", { value: "all", textContent: "Every line" }), h("option", { value: "none", textContent: "Not sorted yet" }), ...sorted.categories.map((c) => h("option", { value: c, textContent: `Only «${c}»` })));
      catShow.value = [...catShow.options].some((o) => o.value === was) ? was : "all";
      catNote.textContent = categoriesLine(kept(), sorted);
      drawCross();
    };
    const drawCross = () => {
      const lines = kept();
      if (sorted === null || sorted.categories.length < 2) {
        crossPick.hidden = true;
        crossBox.replaceChildren();
        return;
      }
      crossPick.hidden = false;
      if (crossGeneration !== dataGeneration()) {
        crossMeta.clear();
        crossGeneration = dataGeneration();
      }
      const missing = [...new Set(lines.map((x) => x.textId))].filter((id) => !crossMeta.has(id));
      if (missing.length > 0 && !crossReading) {
        crossReading = true;
        const gen = crossGeneration;
        void Promise.all(missing.map((id) => textMetadata({ textId: id }).then((m) => [id, new Map(m.fields.map((f) => [f.key, f.value]))] as const, () => [id, new Map<string, string>()] as const))).then((pairs) => {
          crossReading = false;
          if (gen !== crossGeneration) return;
          for (const [id, m] of pairs) crossMeta.set(id, m);
          drawCross();
        });
      }
      const keys = [...new Set([...crossMeta.values()].flatMap((m) => [...m.keys()]))].sort();
      const who = speakerOf(lines, trackNames.names);
      const was = crossPick.value;
      crossPick.replaceChildren(
        h("option", { value: "", textContent: "Compare by…" }),
        h("option", { value: "text", textContent: "By text" }),
        ...(who === null ? [] : [h("option", { value: "speaker", textContent: "By speaker" })]),
        ...keys.map((k) => h("option", { value: `meta:${k}`, textContent: `By ${k.replace(/_/g, " ")}` })),
      );
      crossPick.value = [...crossPick.options].some((o) => o.value === was) ? was : "";
      const by = crossPick.value;
      if (by === "") {
        crossBox.replaceChildren();
        return;
      }
      const name = by === "text" ? "text" : by === "speaker" ? "speaker" : by.slice(5).replace(/_/g, " ");
      const groupOf = (x: Hit): string => (by === "text" ? x.textTitle || "(untitled)" : by === "speaker" ? (who?.(x) ?? "") || "(no name)" : crossMeta.get(x.textId)?.get(by.slice(5)) || "(not given)");
      const t = crossTab(lines, sorted, groupOf);
      const test = independence(t.cells);
      const copyCross = h("button", { type: "button", textContent: "Copy table", title: "The counts per group and category, to paste into a spreadsheet or R" });
      copyCross.onclick = () =>
        void writeText(crossTabTsv(t, name)).then(
          () => showToast({ tone: "info", lines: ["Copied the table. Paste it into a spreadsheet or R."] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      const pct = (n: number, of: number) => (of === 0 ? "" : ` (${Math.round((100 * n) / of)}%)`);
      crossBox.replaceChildren(
        h(
          "table",
          { className: "cc-cross-table" },
          h("thead", {}, h("tr", {}, h("th", { textContent: name[0]!.toUpperCase() + name.slice(1) }), ...t.categories.map((c) => h("th", { textContent: c })), h("th", { textContent: "Lines" }))),
          h(
            "tbody",
            {},
            ...t.groups.map((g, i) => {
              const row = t.cells[i]!;
              const total = row.reduce((a, b) => a + b, 0);
              return h("tr", {}, h("th", { scope: "row", textContent: g }), ...row.map((n) => h("td", { textContent: `${n}${pct(n, total)}` })), h("td", { textContent: String(total) }));
            }),
          ),
        ),
        h("p", { className: "note", textContent: independenceLine(test, name) }),
        h("div", { className: "cc-bar" }, copyCross),
      );
    };
    crossPick.onchange = () => drawCross();
    const saveCats = () => {
      if (sorted === null) return;
      const next = parseCategories(catInput.value);
      if (next.join("\n") === sorted.categories.join("\n")) return;
      const forKey = sortedFor;
      void setHitCategories({ kind, query: asked, categories: next }).then(
        (r) => {
          if (sortedFor !== forKey) return;
          sorted = r;
          drawRows();
        },
        (e) => showToast({ tone: "error", lines: [errorText(e)] }),
      );
    };
    catInput.onkeydown = (e) => {
      if (e.key === "Enter" && !e.isComposing) {
        e.preventDefault();
        saveCats();
      }
    };
    catInput.onblur = saveCats;
    catShow.onchange = () => drawRows();
    /** A row's category picker; a choice is kept at once. */
    const categoryPicker = (hit: Hit): HTMLElement => {
      const pick = h("select", { className: "cc-cat", title: "This line's category" }, h("option", { value: "", textContent: "–" }), ...(sorted?.categories ?? []).map((c) => h("option", { value: c, textContent: c })));
      pick.setAttribute("aria-label", "Category");
      pick.value = categoryOf(hit);
      pick.onclick = (e) => e.stopPropagation();
      pick.onkeydown = (e) => e.stopPropagation();
      pick.onchange = () => {
        const forKey = sortedFor;
        void setHitCategory({ kind, query: asked, segmentId: hit.segmentId, at: hit.at, category: pick.value }).then(
          (r) => {
            if (sortedFor !== forKey) return;
            sorted = r;
            if (sorted !== null) catNote.textContent = categoriesLine(kept(), sorted);
            drawCross();
          },
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      };
      return pick;
    };
    sortToggle.onclick = () => {
      sortOn = !sortOn;
      sortToggle.setAttribute("aria-pressed", String(sortOn));
      sortToggle.classList.toggle("active", sortOn);
      drawRows();
    };
    const sampled = (): Hit[] => {
      const k = chosen();
      return thinTo > 0 ? thin(k, thinTo, `${kind}|${asked}|${nearInput.value}`) : k;
    };
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

    /** The plot on a map: a dot per recording site sized by its rate, as a dialect atlas shows a word. */
    const drawSites = (d: Dispersion) => {
      const found = bySite(d.texts);
      count.textContent = `${spreadWords(d)} ${sitesLine(found)}`;
      if (land === null) {
        rows.replaceChildren(h("p", { className: "note", textContent: "Drawing the map…" }));
        void worldLand().then((l) => {
          land = l;
          if (plotOn && plotBy === "map") drawPlot();
        }, (e) => showToast({ tone: "error", lines: [`Could not draw the map: ${errorText(e)}`] }));
        return;
      }
      const view = mapView(found.sites, { x: 8, y: 5 });
      const { x, y, w, h: hh } = view.box;
      const r = Math.max(w, hh) / 70;
      const NS = "http://www.w3.org/2000/svg";
      const node = <K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string) => {
        const e = document.createElementNS(NS, tag);
        for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
        if (text !== undefined) e.textContent = text;
        return e;
      };
      const map = node("svg", { viewBox: `${x} ${y} ${w} ${hh}`, class: "tm-map cc-map", role: "img", "aria-label": `Where «${asked}» was found` });
      map.append(node("rect", { x, y, width: w, height: hh, class: "tm-sea" }));
      for (const dx of [0, 360]) map.append(node("path", { d: land, transform: `translate(${dx} 0)`, class: "tm-land", "stroke-width": r / 4 }));
      // Smaller dots drawn last, so a big one never hides a small one.
      for (const p of [...view.placed].sort((a, b) => dotShare(b, found.sites) - dotShare(a, found.sites))) {
        const share = dotShare(p, found.sites);
        const dot = node("circle", { cx: p.x, cy: p.y, r: share === 0 ? r * 0.8 : r * (0.8 + 2.2 * share), class: share === 0 ? "tm-dot cc-site-none" : "tm-dot cc-site", "stroke-width": r / 4 });
        dot.append(node("title", {}, siteTitle(p)));
        map.append(dot);
      }
      rows.replaceChildren(map);
    };

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
      const times = timeKeys(d.texts);
      const mappable = hasSites(d.texts);
      if (!keys.includes(plotBy) && !times.some((k) => `time:${k}` === plotBy) && !(plotBy === "map" && mappable)) plotBy = "";
      plotByPicker.replaceChildren(
        h("option", { value: "", textContent: "Each text", selected: plotBy === "" }),
        ...keys.map((k) => h("option", { value: k, textContent: `By ${k.replace(/_/g, " ")}`, selected: k === plotBy })),
        ...times.map((k) => h("option", { value: `time:${k}`, textContent: `Over time, by ${k.replace(/_/g, " ")}`, selected: `time:${k}` === plotBy })),
        ...(mappable ? [h("option", { value: "map", textContent: "On a map, by where recorded", selected: plotBy === "map" })] : []),
      );
      plotByPicker.disabled = keys.length === 0 && !mappable;
      plotSorter.hidden = plotBy !== "";
      count.textContent = spreadWords(d);
      if (plotBy === "map") {
        drawSites(d);
        return;
      }
      if (plotBy.startsWith("time:")) {
        const key = plotBy.slice(5);
        const p = byPeriod(d.texts, key);
        const top = Math.max(0, ...p.rows.map((g) => (g.words === 0 ? 0 : g.hits / g.words)));
        const unit = p.width === 1 ? "year" : p.width === 10 ? "decade" : "century";
        count.textContent = `${spreadWords(d)} Over time, by the year in each text's ${key.replace(/_/g, " ")}: matches per 1000 words in each ${unit}, oldest first.${p.undated > 0 ? ` ${p.undated === 1 ? "1 text gives" : `${p.undated} texts give`} no year and ${p.undated === 1 ? "is" : "are"} left out.` : ""}`;
        rows.replaceChildren(
          ...p.rows.map((g) => {
            const fill = h("span", { className: "cc-detail-fill" });
            fill.style.width = `${top === 0 || g.words === 0 ? 0 : Math.round((100 * (g.hits / g.words)) / top)}%`;
            return h(
              "div",
              { className: `cc-plot-row${g.hits === 0 ? " cc-plot-none" : ""}`, role: "listitem" },
              h("span", { className: "cc-plot-title", textContent: g.label }),
              h("span", { className: "cc-plot-count", textContent: g.texts === 0 ? "no texts" : `${g.hits} · ${per1000(g.hits, g.words)} per 1000 · ${g.texts === 1 ? "1 text" : `${g.texts} texts`}` }),
              h("div", { className: "cc-plot-bar" }, fill),
            );
          }),
        );
        return;
      }
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
      nearBar.hidden = on || plotOn;
      sortToggle.hidden = on || plotOn;
      copyExamples.hidden = on || plotOn;
      fileTopic.hidden = on || plotOn;
      saveClipsButton.hidden = on || plotOn;
    };

    const setPlot = (on: boolean) => {
      if (on && clustersOn) setClusters(false);
      plotOn = on;
      plotToggle.setAttribute("aria-pressed", String(on));
      plotToggle.classList.toggle("active", on);
      sorter.hidden = on;
      thinner.hidden = on;
      nearBar.hidden = on;
      sortToggle.hidden = on;
      copyExamples.hidden = on;
      fileTopic.hidden = on;
      saveClipsButton.hidden = on;
      plotSorter.hidden = !on;
      plotByPicker.hidden = !on;
    };

    const drawRows = () => {
      // Drawn again below for plain lines; the plot, clusters and an empty search have none.
      posBox.hidden = true;
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
      const near = nearOn() ? kept().length : hits.length;
      const thinned = thinTo > 0 && near > thinTo;
      count.textContent =
        countLine(hits.length, total, texts) +
        (only.length > 0 ? ` Only in ${only.join(", among ")}.` : "") +
        (nearOn() && hits.length > 0 ? nearLine(near, hits.length, nearInput.value, Number(nearWindow.value), nearSide.value as NearSide) : "") +
        (thinned ? thinLine(thinTo, near, nearOn() ? near : total) : "");
      tagsToggle.hidden = !hasTags(hits);
      drawCats();
      drawPositions(chosen());
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
      // A conversation's lines name their speaker (their track), read once per data generation.
      if (trackNames.generation !== dataGeneration() && !trackNames.asking) {
        trackNames.asking = true;
        const gen = dataGeneration();
        void corpusTracks().then(
          (ts) => {
            trackNames = { generation: gen, names: new Map(ts.map((t) => [t.id, t.name])), asking: false };
            drawRows();
          },
          () => {
            trackNames = { generation: gen, names: new Map(), asking: false };
          },
        );
      }
      const who = speakerOf(lines, trackNames.names);
      rows.replaceChildren(
        ...lines.map((hit) => {
          const play = playButton("Open this segment and play it", () => void open(hit, true));
          play.classList.add("cc-play");
          play.disabled = hit.startMs === null;
          const openKey = `${hit.textId}|${hit.segmentId}`;
          const more = h("button", { type: "button", className: "cc-more", textContent: "⋯", title: "Show the sentences around this one" });
          more.setAttribute("aria-expanded", String(opened.has(openKey)));
          more.setAttribute("aria-label", "Sentences around it");
          more.onclick = (e) => {
            e.stopPropagation();
            if (opened.has(openKey)) opened.delete(openKey);
            else opened.add(openKey);
            drawRows();
          };
          const row = h(
            "div",
            { className: sortOn && sorted !== null ? "cc-row cc-row-cat" : "cc-row", role: "listitem", tabIndex: 0, title: `${hit.textTitle}: open this segment`, dir: isRtl(keyword(hit)) ? "rtl" : "ltr" },
            ...(sortOn && sorted !== null && sorted.categories.length > 0 ? [categoryPicker(hit)] : []),
            play,
            context(clip(left(hit), CONTEXT, "left"), "left"),
            h("span", { className: "cc-key", textContent: keyword(hit) }),
            context(clip(right(hit), CONTEXT, "right"), "right"),
            h(
              "span",
              { className: "cc-meta" },
              h("span", { textContent: hit.breakdown === null ? "" : `${hit.breakdown} ‘${hit.glosses ?? ""}’` }),
              h("span", { textContent: who === null ? hit.textTitle : `${hit.textTitle} · ${who(hit)}` }),
            ),
            more,
            ...(underOn ? [h("span", { className: "cc-under", textContent: under.get(hit.textId)?.get(hit.segmentId) ?? "" })] : []),
            aroundOf(hit),
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
    nearInput.oninput = () => drawRows();
    nearWindow.onchange = () => drawRows();
    nearSide.onchange = () => drawRows();
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
    fileTopic.onclick = () => {
      if (hits.length === 0) return;
      topicBar.hidden = !topicBar.hidden;
      if (topicBar.hidden) return;
      topicGo.textContent = `File ${sampled().length === 1 ? "its sentence" : "their sentences"}`;
      topicName.focus();
      void topicsOverview()
        .then((t) => topicNames.replaceChildren(...t.topics.map((x) => h("option", { value: x.name }))))
        .catch(() => undefined);
    };
    topicCancel.onclick = () => {
      topicBar.hidden = true;
    };
    const fileThem = async () => {
      const name = topicName.value.trim();
      if (name === "" || hits.length === 0) return;
      topicGo.disabled = true;
      try {
        const r = await fileUnderTopic(sampled().map((x) => ({ textId: x.textId, segmentId: x.segmentId })), name);
        topicBar.hidden = true;
        showToast({ tone: "info", lines: [`Filed ${r.filed} ${r.filed === 1 ? "sentence" : "sentences"} under «${name}»${r.already > 0 ? `; ${r.already} ${r.already === 1 ? "was" : "were"} there already` : ""}. The Topics panel lists them.`] });
      } catch (e) {
        showToast({ tone: "error", lines: [`Could not file them: ${errorText(e)}`] });
      } finally {
        topicGo.disabled = false;
      }
    };
    topicGo.onclick = () => void fileThem();
    topicName.onkeydown = (e) => {
      if (e.key === "Enter") void fileThem();
      else if (e.key === "Escape") topicBar.hidden = true;
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
      if (plotOn && plot !== null && plotBy === "map") {
        void writeText(sitesTsv(bySite(plot.texts).sites)).then(
          () => showToast({ tone: "info", lines: ["Copied the matches by site. Paste them into a spreadsheet or a GIS."] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
        return;
      }
      if (plotOn && plot !== null && plotBy.startsWith("time:")) {
        void writeText(periodTsv(byPeriod(plot.texts, plotBy.slice(5)), plotBy.slice(5))).then(
          () => showToast({ tone: "info", lines: ["Copied the matches over time. Paste them into a spreadsheet."] }),
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
          return writeHtml(kwicHtml(lines, 40, shown), asTsv(lines, new Map(pairs), shown, speakerOf(lines, trackNames.names), sortOn && sorted !== null && sorted.categories.length > 0 ? categoryOf : null));
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
      { id: "concordance:clips", title: "Save the concordance lines as sound clips", group: "Concordance", words: ["wav", "tokens", "praat", "phonetics", "extract"], run: () => saveClipsButton.click() },
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
      nearInput.value = "";
      sorted = null;
      sortedFor = "";
      plot = null;
      asked = "";
      exactFor = null;
      under.clear();
      underEpoch += 1;
      underGeneration = -1;
      opened.clear();
      aroundTexts.clear();
      input.value = "";
      saved = [];
      keepBar.hidden = true;
      if (now !== "closed") loadSaved();
      if (now === "closed") {
        el.replaceChildren(emptyState("Concordance", "Open a project to see every place a word, morph or gloss is used.", "The wug is counting words."));
        return;
      }
      drawChips();
      drawRows();
      setPlot(plotOn);
      setClusters(clustersOn);
      el.replaceChildren(h("div", { className: "cc-bar" }, input, savedPick, sorter, thinner, plotSorter, plotByPicker, sizePicker, sidePicker, plotToggle, clusterToggle, tagsToggle, underToggle, sortToggle, copy, copyExamples, fileTopic, keepSearch, saveClipsButton), topicBar, keepBar, nearBar, catBar, catNote, crossBox, chips, count, posBox, rows);
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
