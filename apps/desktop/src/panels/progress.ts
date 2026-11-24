// Progress: how far the documentation has come, text by text, for a grant
// report, a deposit form or a supervisor (src-tauri/src/progress.rs). Words
// and sums in ../progress/logic.ts. Read-only; a row opens its text.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { brownMorphemes, projectLanguage, saveDocument, consentGaps, lexiconList, errorText, projectProgress, speakerLengths, vocabularyGrowth, workLog, worldLand } from "../api";
import { placesNote, textPlaces } from "../progress/places";
import { corpusDescription } from "../progress/description";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { nextSteps, stepsLine, type LexiconGaps } from "../progress/next";
import { shows } from "../lexicon/logic";
import { SHOW_PANEL } from "./panel";
import { mapView } from "../typology/map";
import { dayLabel, workColumns, worklogLine, worklogTsv, type Day } from "../progress/worklog";
import { localDay } from "../dates";
import { hasPeople, isChild, speakerCells, speakerColumns, speakersNote, speakersTsv, type SpeakerRow } from "../progress/speakers";
import { brownNote, brownRows, brownTsv, type BrownTrack } from "../progress/brown";
import { ageLabel, agePoints, ageNote, ageTicks, ageTsv, inMorphemes, plotted } from "../progress/mluage";
import { standalone } from "./vowels";
import { forecastNote, growthNote, growthPath, ticks, type Box, type Growth } from "../progress/growth";
import { composition, compositionNote, compositionTsv, detailKeys, keyLabel, wordShare } from "../progress/composition";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { COLUMNS, progressTsv, rowCells, summaryLines, totals, type Progress } from "../progress/logic";
import { dataGeneration, getState, openLexiconList, selectText, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

export const progressPanel: PanelDef = {
  title: "Progress",
  create(): PanelView {
    const el = h("div", { className: "panel panel-progress" });
    let data: Progress | null = null;
    let seq = 0;
    let shown = "";
    const summary = h("ul", { className: "pg-summary" });
    const table = h("table", { className: "pg-table" });
    const note = h("p", { className: "note" });
    let growth: Growth | null = null;
    let detail = "";
    const makeUp = h("div", { className: "pg-makeup" });
    const growthBox = h("div", { className: "pg-growth" });
    const workBox = h("div", { className: "pg-work" });
    let work: Day[] = [];
    const speakersBox = h("div", { className: "pg-speakers" });
    const placesBox = h("div", { className: "pg-places" });
    // What to do next, read after the counts with the lexicon's lists and the consent records.
    const nextBox = h("div", { className: "pg-next" });
    let lexiconGaps: LexiconGaps = {};
    let uncovered: number | null = null;
    const drawNext = () => {
      if (data === null) {
        nextBox.replaceChildren();
        return;
      }
      const steps = nextSteps(data, lexiconGaps, uncovered);
      nextBox.replaceChildren(
        h("h3", { className: "pg-head", textContent: "What to do next" }),
        h("p", { className: "note", textContent: stepsLine(steps) }),
        ...(steps.length === 0
          ? []
          : [
              h(
                "ul",
                { className: "pg-steps" },
                ...steps.map((st) =>
                  h(
                    "li",
                    {},
                    h("span", { textContent: st.text }),
                    h("button", {
                      type: "button",
                      className: "link-button",
                      textContent: `Open ${st.button}`,
                      onclick: () => {
                        if (st.list !== undefined) openLexiconList(st.list);
                        window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: st.panel }));
                      },
                    }),
                  ),
                ),
              ),
            ]),
      );
    };
    let land: string | null = null;
    let speakers: SpeakerRow[] = [];
    /** The track whose MLU is charted over age. */
    let ageTrack: string | null = null;
    /** Brown's morphemes per track and session, and the track shown. */
    let brown: BrownTrack[] = [];
    let brownTrack: string | null = null;
    const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy every text's figures and the totals, to paste into a spreadsheet or a report" });
    const recount = h("button", { type: "button", textContent: "Count again" });
    const describe = h("button", { type: "button", textContent: "Describe the corpus…", title: "Save a description of the collection for a data paper, an archive deposit or a report: its contents, annotation, speakers and access, with places left to write" });
    describe.onclick = async () => {
      if (data === null) return;
      try {
        const folder = await pickFolder({ directory: true, title: "Where should the description go?" });
        if (typeof folder !== "string") return;
        const language = await projectLanguage();
        const gaps = await consentGaps().catch(() => null);
        const html = corpusDescription({
          progress: data,
          speakers,
          language: { name: language.name, glottocode: language.glottocode, iso: language.iso },
          restricted: data.texts.filter((t) => (t.access ?? "open") !== "open").length,
          uncovered: gaps === null || gaps.records === 0 ? null : gaps.texts.filter((t) => t.records === 0).length,
          date: localDay(new Date()),
          generator: "wugs",
        });
        const r = await saveDocument({ folder, title: `${language.name.trim() || "Corpus"} description`, extension: "html", contents: html });
        showToast({ tone: "info", lines: [`Saved the description in ${r.path}. Open it in Word or a browser and fill in the places marked in yellow.`, ...r.notes] });
      } catch (e) {
        showToast({ tone: "error", lines: [`Could not save the description: ${errorText(e)}`] });
      }
    };

    const draw = () => {
      if (data === null) {
        summary.replaceChildren();
        table.replaceChildren();
        return;
      }
      summary.replaceChildren(...summaryLines(data).map((line) => h("li", { textContent: line })));
      drawNext();
      const row = (cells: string[], id: string | null) => {
        const tr = h("tr", {}, ...cells.map((c, i) => h(i === 0 ? "th" : "td", i === 0 ? { scope: "row", textContent: c } : { textContent: c })));
        if (id !== null) {
          tr.tabIndex = 0;
          tr.title = "Open this text";
          tr.onclick = () => void selectText(id);
          tr.onkeydown = (e) => {
            if (e.key === "Enter") void selectText(id);
          };
        }
        return tr;
      };
      const parts: Node[] = [h("thead", {}, h("tr", {}, ...COLUMNS.map((c) => h("th", { scope: "col", textContent: c })))), h("tbody", {}, ...data.texts.map((t) => row(rowCells(t), t.id)))];
      if (data.texts.length > 1) parts.push(h("tfoot", {}, row(rowCells(totals(data)), null)));
      table.replaceChildren(...parts);
      drawMakeUp();
      drawPlaces();
      note.textContent =
        "A sentence is a segment on a transcription track; a word is glossed once it is broken into morphs; speech transcribed adds up the timed sentences that have a transcription.";
    };

    const NS = "http://www.w3.org/2000/svg";
    const svg = (tag: string, attrs: Record<string, string | number>, text?: string): SVGElement => {
      const e = document.createElementNS(NS, tag);
      for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
      if (text !== undefined) e.textContent = text;
      return e;
    };

    const drawGrowth = () => {
      if (growth === null) {
        growthBox.replaceChildren();
        return;
      }
      const g = growth;
      const parts: Node[] = [h("h3", { className: "pg-head", textContent: "Vocabulary growth" })];
      if (g.tokens > 0) {
        const box: Box = { width: 420, height: 204, left: 46, right: 10, top: 10, bottom: 34 };
        const w = box.width - box.left - box.right;
        const ht = box.height - box.top - box.bottom;
        const chart = svg("svg", { viewBox: `0 0 ${box.width} ${box.height}`, class: "pg-chart", role: "img" });
        chart.setAttribute("aria-label", "Different words met against running words, texts in the order they were made");
        for (const t of ticks(g.tokens)) {
          const x = box.left + (t * w) / g.tokens;
          chart.append(svg("line", { x1: x, x2: x, y1: box.top, y2: box.top + ht, class: "grid" }), svg("text", { x, y: box.height - 19, class: "tick", "text-anchor": "middle" }, t.toLocaleString("en")));
        }
        for (const t of ticks(g.types)) {
          const y = box.top + ht - (g.types === 0 ? 0 : (t * ht) / g.types);
          chart.append(svg("line", { x1: box.left, x2: box.left + w, y1: y, y2: y, class: "grid" }), svg("text", { x: box.left - 6, y: y + 4, class: "tick", "text-anchor": "end" }, t.toLocaleString("en")));
        }
        for (const t of g.texts.slice(1)) {
          const x = box.left + (t.at * w) / g.tokens;
          const mark = svg("line", { x1: x, x2: x, y1: box.top, y2: box.top + ht, class: "text-start" });
          mark.append(svg("title", {}, `«${t.title}» starts here`));
          chart.append(mark);
        }
        chart.append(svg("path", { d: growthPath(g, box), class: "curve" }));
        chart.append(svg("text", { x: box.left + w / 2, y: box.height - 3, class: "axis", "text-anchor": "middle" }, "running words"));
        chart.append(svg("text", { x: 0, y: 0, class: "axis", "text-anchor": "middle", transform: `translate(10 ${box.top + ht / 2}) rotate(-90)` }, "different words"));
        parts.push(chart);
      }
      parts.push(h("p", { className: "note", textContent: `${growthNote(g)}${g.texts.length > 1 ? " Dashed lines mark where each text starts, in the order the texts were made." : ""}` }));
      const forecast = forecastNote(g);
      if (forecast !== "") parts.push(h("p", { className: "note pg-forecast", textContent: forecast }));
      growthBox.replaceChildren(...parts);
    };

    const drawMakeUp = () => {
      if (data === null) {
        makeUp.replaceChildren();
        return;
      }
      const keys = detailKeys(data.texts);
      if (keys.length === 0) {
        makeUp.replaceChildren(
          h("h3", { className: "pg-head", textContent: "The corpus by detail" }),
          h("p", { className: "note", textContent: "Give the texts details (genre, speaker, date…) in the Texts panel's Details or with Import details…, and their share of the words shows here." }),
        );
        return;
      }
      if (!keys.some((k) => k.key === detail)) detail = keys[0]!.key;
      const picker = h("select", { className: "pg-detail", title: "Which detail to break the corpus down by" }, ...keys.map((k) => h("option", { value: k.key, textContent: `${keyLabel(k.key)} (${k.texts} ${k.texts === 1 ? "text" : "texts"})`, selected: k.key === detail })));
      picker.setAttribute("aria-label", "Detail");
      picker.onchange = () => {
        detail = picker.value;
        drawMakeUp();
      };
      const rows = composition(data.texts, detail);
      const copyMakeUp = h("button", { type: "button", textContent: "Copy table", title: "Copy the corpus's make-up by this detail" });
      copyMakeUp.onclick = () =>
        void writeText(compositionTsv(rows, detail)).then(
          () => showToast({ tone: "info", lines: [`Copied the corpus by ${keyLabel(detail)}.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      const n = (x: number) => x.toLocaleString("en");
      const head = h("tr", {}, ...[keyLabel(detail), "Texts", "Sentences", "Words", "Share", "Glossed"].map((c) => h("th", { scope: "col", textContent: c[0]!.toUpperCase() + c.slice(1) })));
      const body = rows.map((r) => h("tr", {}, h("th", { scope: "row", textContent: r.value }), h("td", { textContent: n(r.texts) }), h("td", { textContent: n(r.sentences) }), h("td", { textContent: n(r.words) }), h("td", { textContent: wordShare(r, rows) }), h("td", { textContent: n(r.glossed) })));
      makeUp.replaceChildren(
        h("h3", { className: "pg-head", textContent: "The corpus by detail" }),
        h("div", { className: "cc-bar" }, picker, copyMakeUp),
        h("div", { className: "pg-rows" }, h("table", { className: "pg-table" }, h("thead", {}, head), h("tbody", {}, ...body))),
        h("p", { className: "note", textContent: compositionNote(rows, detail) }),
      );
    };

    /** Where the texts were recorded, from their coordinates details, on Natural Earth's land. */
    const drawPlaces = () => {
      if (data === null) {
        placesBox.replaceChildren();
        return;
      }
      const found = textPlaces(data.texts.map((t) => ({ title: t.title, meta: t.meta ?? [] })));
      if (found.places.length === 0) {
        placesBox.replaceChildren(...(found.wordsOnly > 0 ? [h("h3", { className: "pg-head", textContent: "Where the texts were recorded" }), h("p", { className: "note", textContent: placesNote(found, data.texts.length) })] : []));
        return;
      }
      if (land === null) {
        void worldLand().then((l) => {
          land = l;
          drawPlaces();
        }, () => undefined);
        return;
      }
      // At least 8° across: Natural Earth 1:110m is too coarse to show a smaller area.
      const view = mapView(found.places, { x: 8, y: 5 });
      const { x, y, w, h: hh } = view.box;
      const r = Math.max(w, hh) / 70;
      const NS = "http://www.w3.org/2000/svg";
      const node = <K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, text?: string) => {
        const e = document.createElementNS(NS, tag);
        for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
        if (text !== undefined) e.textContent = text;
        return e;
      };
      const map = node("svg", { viewBox: `${x} ${y} ${w} ${hh}`, class: "tm-map pg-map", role: "img", "aria-label": "Where the texts were recorded" });
      map.append(node("rect", { x, y, width: w, height: hh, class: "tm-sea" }));
      for (const dx of [0, 360]) map.append(node("path", { d: land, transform: `translate(${dx} 0)`, class: "tm-land", "stroke-width": r / 4 }));
      // One dot per site, titled with the texts recorded there.
      const sites = new Map<string, { x: number; y: number; titles: string[] }>();
      for (const p of view.placed) {
        const k = `${p.x.toFixed(4)},${p.y.toFixed(4)}`;
        const s = sites.get(k) ?? { x: p.x, y: p.y, titles: [] };
        s.titles.push(p.title || "(untitled)");
        sites.set(k, s);
      }
      for (const s of sites.values()) {
        const dot = node("circle", { cx: s.x, cy: s.y, r: r * Math.min(2, 1 + Math.log10(s.titles.length)), class: "tm-dot pg-site", "stroke-width": r / 4 });
        dot.append(node("title", {}, s.titles.join(", ")));
        map.append(dot);
      }
      placesBox.replaceChildren(h("h3", { className: "pg-head", textContent: "Where the texts were recorded" }), map, h("p", { className: "note", textContent: placesNote(found, data.texts.length) }));
    };

    /** Each speaker's utterance lengths: MLU in words and morphemes. */
    const drawSpeakers = () => {
      if (speakers.length === 0) {
        speakersBox.replaceChildren();
        return;
      }
      const people = hasPeople(speakers);
      const textCol = people ? 4 : 1;
      const copySpeakers = h("button", { type: "button", textContent: "Copy table", title: "Copy each speaker's figures, to paste into a spreadsheet" });
      copySpeakers.onclick = () =>
        void writeText(speakersTsv(speakers)).then(
          () => showToast({ tone: "info", lines: [`Copied ${speakers.length === 1 ? "one speaker" : `${speakers.length} speakers`}.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      const t = h(
        "table",
        { className: "pg-table pg-speaker-table" },
        h("thead", {}, h("tr", {}, ...speakerColumns(speakers).map((c, i) => h("th", { textContent: c, className: i < textCol ? "txt" : "" })))),
        h("tbody", {}, ...speakers.map((r) => h("tr", {}, ...speakerCells(r, people).map((c, i) => h("td", { textContent: c, className: i < textCol ? "" : "num" }))))),
      );
      speakersBox.replaceChildren(h("div", { className: "pg-speakers-head" }, h("h3", { className: "pg-head", textContent: "Speakers" }), copySpeakers), h("div", { className: "pg-rows" }, t), h("p", { className: "note", textContent: speakersNote(speakers) }), ...ageChart(), ...brownTable());
    };

    /** Brown's fourteen morphemes session by session, for tracks with CHAT's %mor. */
    const brownTable = (): Node[] => {
      if (brown.length === 0) return [];
      const t = brown.find((b) => b.tier === brownTrack) ?? brown.find((b) => isChild(b.name)) ?? brown[0]!;
      brownTrack = t.tier;
      const picker = h("select", { className: "pg-brown-track", title: "Whose morphemes to list" }, ...brown.map((b) => h("option", { value: b.tier, textContent: b.name || "(no name)", selected: b.tier === t.tier })));
      picker.setAttribute("aria-label", "Track");
      picker.onchange = () => {
        brownTrack = picker.value;
        drawSpeakers();
      };
      const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy each morpheme's uses session by session" });
      copy.onclick = () =>
        void writeText(brownTsv(t)).then(
          () => showToast({ tone: "info", lines: [`Copied Brown's morphemes for ${t.name || "the track"}.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      const table = h(
        "table",
        { className: "pg-table pg-brown" },
        h("thead", {}, h("tr", {}, h("th", { textContent: "Morpheme" }), ...t.sessions.map((s) => h("th", { textContent: s.title, title: `${s.utterances} utterances with %mor` })), h("th", { textContent: "All" }))),
        h("tbody", {}, ...brownRows(t).map((r) => h("tr", { className: r.total === 0 ? "pg-brown-none" : "" }, h("th", { scope: "row", textContent: r.morpheme }), ...r.counts.map((c) => h("td", { textContent: c === 0 ? "·" : String(c) })), h("td", { textContent: String(r.total) })))),
      );
      return [h("div", { className: "pg-speakers-head" }, h("h3", { className: "pg-head", textContent: "Brown's morphemes" }), picker, copy), h("div", { className: "pg-rows" }, table), h("p", { className: "note", textContent: brownNote(t) })];
    };

    /** MLU session by session against the speaker's age, for tracks with an age in two texts or more. */
    const ageChart = (): Node[] => {
      const charted = speakers.map((r) => ({ row: r, points: agePoints(r) })).filter((c) => c.points.length >= 2);
      if (charted.length === 0) return [];
      const pick = charted.find((c) => c.row.tier === ageTrack) ?? charted.find((c) => isChild(c.row.name)) ?? charted[0]!;
      ageTrack = pick.row.tier;
      const { row, points } = pick;
      const picker = h("select", { className: "pg-age-track", title: "Whose utterance length to chart" }, ...charted.map((c) => h("option", { value: c.row.tier, textContent: c.row.name || "(no name)", selected: c.row.tier === ageTrack })));
      picker.setAttribute("aria-label", "Track");
      picker.onchange = () => {
        ageTrack = picker.value;
        drawSpeakers();
      };
      const morphemes = inMorphemes(points);
      const box: Box = { width: 420, height: 220, left: 40, right: 12, top: 10, bottom: 34 };
      const w = box.width - box.left - box.right;
      const ht = box.height - box.top - box.bottom;
      const from = Math.floor(points[0]!.months - 0.3);
      const to = Math.max(from + 1, Math.ceil(points[points.length - 1]!.months + 0.3));
      const top = Math.max(2, Math.ceil(Math.max(...points.map((p) => plotted(p, morphemes))) + 0.5));
      const x = (m: number) => box.left + ((m - from) / (to - from)) * w;
      const y = (v: number) => box.top + ht - (v / top) * ht;
      const chart = svg("svg", { viewBox: `0 0 ${box.width} ${box.height}`, class: "pg-chart pg-age-chart", role: "img" });
      chart.setAttribute("aria-label", `${row.name}'s mean length of utterance by age`);
      for (const m of ageTicks(from, to)) chart.append(svg("line", { x1: x(m), x2: x(m), y1: box.top, y2: box.top + ht, class: "grid" }), svg("text", { x: x(m), y: box.height - 19, class: "tick", "text-anchor": "middle" }, ageLabel(m)));
      for (let v = 0; v <= top; v++) chart.append(svg("line", { x1: box.left, x2: box.left + w, y1: y(v), y2: y(v), class: "grid" }), svg("text", { x: box.left - 6, y: y(v) + 4, class: "tick", "text-anchor": "end" }, String(v)));
      if (morphemes && isChild(row.name))
        for (const [at, name] of [[2, "II"], [2.5, "III"], [3, "IV"], [3.75, "V"], [4.5, "past V"]] as const)
          if (at < top) chart.append(svg("line", { x1: box.left, x2: box.left + w, y1: y(at), y2: y(at), class: "text-start" }), svg("text", { x: box.left + w - 2, y: y(at) - 3, class: "tick", "text-anchor": "end" }, `stage ${name}`));
      chart.append(svg("path", { d: points.map((p, i) => `${i === 0 ? "M" : "L"}${x(p.months).toFixed(1)} ${y(plotted(p, morphemes)).toFixed(1)}`).join(" "), class: "curve" }));
      for (const p of points) {
        const dot = svg("circle", { cx: x(p.months), cy: y(plotted(p, morphemes)), r: 3.5, class: "pg-age-dot" });
        dot.append(svg("title", {}, `«${p.title}» at ${ageLabel(p.months)}: ${plotted(p, morphemes).toFixed(2)} ${morphemes ? "morphemes" : "words"} over ${p.utterances} utterances`));
        chart.append(dot);
      }
      chart.append(svg("text", { x: box.left + w / 2, y: box.height - 3, class: "axis", "text-anchor": "middle" }, "age (years;months)"));
      chart.append(svg("text", { x: 0, y: 0, class: "axis", "text-anchor": "middle", transform: `translate(10 ${box.top + ht / 2}) rotate(-90)` }, morphemes ? "MLU in morphemes" : "MLU in words"));
      const copy = h("button", { type: "button", textContent: "Copy table", title: "Copy each session's age and MLU" });
      copy.onclick = () =>
        void writeText(ageTsv(points)).then(
          () => showToast({ tone: "info", lines: [`Copied ${points.length} sessions.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      const save = h("button", { type: "button", textContent: "Save picture…", title: "Save the chart as an SVG file" });
      save.onclick = () => {
        const contents = standalone(chart as SVGSVGElement);
        void (async () => {
          const folder = await pickFolder({ directory: true, title: "Where should the picture go?" });
          if (typeof folder !== "string") return;
          try {
            const r = await saveDocument({ folder, title: `${row.name} MLU by age`, extension: "svg", contents });
            showToast({ tone: "info", lines: [`Saved the picture as ${r.path}.`] });
          } catch (e) {
            showToast({ tone: "error", lines: [errorText(e)] });
          }
        })();
      };
      const left = row.byText.length - points.length;
      const note = `${ageNote(row.name || "This speaker", points, isChild(row.name))}${left > 0 ? ` ${left === 1 ? "One text has" : `${left} texts have`} no age for this speaker and ${left === 1 ? "is" : "are"} left out.` : ""} Ages come from each text's CHAT age for the track, else its date against the speaker's birth date in the People panel.`;
      return [h("div", { className: "pg-speakers-head" }, h("h3", { className: "pg-head", textContent: "Length over age" }), picker, copy, save), chart, h("p", { className: "note", textContent: note })];
    };

    /** The work log: the latest days with any work, newest first. */
    const drawWork = () => {
      const cols = workColumns(work);
      const copyWork = h("button", { type: "button", textContent: "Copy table", title: "Copy every day's figures, to paste into a field diary, a spreadsheet or a report" });
      copyWork.onclick = () =>
        void writeText(worklogTsv(work)).then(
          () => showToast({ tone: "info", lines: [`Copied ${work.length === 1 ? "one day" : `${work.length} days`}.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      copyWork.disabled = work.length === 0;
      const t = h(
        "table",
        { className: "pg-table pg-work-table" },
        h("thead", {}, h("tr", {}, h("th", { textContent: "Day" }), ...cols.map((c) => h("th", { textContent: c.label, title: c.title })), h("th", { textContent: "Devices", title: "How many devices wrote that day" }))),
        h("tbody", {}, ...work.map((d) => h("tr", {}, h("td", { textContent: dayLabel(d.date) }), ...cols.map((c) => h("td", { className: "num", textContent: d[c.key] === 0 ? "" : d[c.key].toLocaleString("en") })), h("td", { className: "num", textContent: String(d.devices) })))),
      );
      workBox.replaceChildren(h("div", { className: "pg-speakers-head" }, h("h3", { className: "pg-head", textContent: "Day by day" }), copyWork), h("p", { className: "note", textContent: worklogLine(work, localDay(new Date())) }), ...(work.length === 0 ? [] : [h("div", { className: "pg-rows" }, t)]));
    };

    async function load() {
      const mine = ++seq;
      workLog({ offsetMinutes: -new Date().getTimezoneOffset(), days: 60 }).then(
        (r) => {
          if (mine !== seq) return;
          work = r;
          drawWork();
        },
        () => {
          if (mine !== seq) return;
          work = [];
          workBox.replaceChildren();
        },
      );
      brownMorphemes().then(
        (r) => {
          if (mine !== seq) return;
          brown = r;
          drawSpeakers();
        },
        () => {
          if (mine !== seq) return;
          brown = [];
        },
      );
      speakerLengths().then(
        (r) => {
          if (mine !== seq) return;
          speakers = r;
          drawSpeakers();
        },
        () => {
          if (mine !== seq) return;
          speakers = [];
          drawSpeakers();
        },
      );
      vocabularyGrowth().then(
        (g) => {
          if (mine !== seq) return;
          growth = g;
          drawGrowth();
        },
        () => {
          if (mine !== seq) return;
          growth = null;
          drawGrowth();
        },
      );
      Promise.all([lexiconList({ query: "" }), consentGaps()]).then(
        ([rows, gaps]) => {
          if (mine !== seq) return;
          lexiconGaps = { gloss: rows.filter((r) => shows(r, "gloss")).length, category: rows.filter((r) => shows(r, "category")).length, example: rows.filter((r) => shows(r, "example")).length, sound: rows.filter((r) => shows(r, "sound")).length };
          uncovered = gaps.records === 0 && gaps.total === 0 ? null : gaps.texts.filter((t) => t.records === 0).length;
          drawNext();
        },
        () => undefined,
      );
      try {
        const r = await projectProgress();
        if (mine !== seq) return;
        data = r;
      } catch (e) {
        if (mine !== seq) return;
        data = null;
        note.textContent = `Could not count: ${errorText(e)}`;
      }
      draw();
    }

    copy.onclick = () => {
      if (data === null) return;
      void writeText(progressTsv(data)).then(
        () => showToast({ tone: "info", lines: ["Copied the progress table. Paste it into a spreadsheet or a report."] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };
    recount.onclick = () => void load();

    const unprovide = provide("progress", () => [
      { id: "progress:copy", title: "Copy the progress table", group: "Progress", words: ["report", "grant", "statistics", "hours", "how much"], run: () => copy.click() },
      { id: "progress:days", title: "Show what was done day by day", group: "Progress", words: ["work log", "diary", "journal", "today", "this week", "report"], run: () => workBox.scrollIntoView({ block: "start" }) },
      { id: "progress:speakers", title: "Show each speaker's mean length of utterance", group: "Progress", words: ["MLU", "child", "acquisition", "Brown", "utterance length", "CHILDES"], run: () => speakersBox.scrollIntoView({ block: "start" }) },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}|${s.texts.map((t) => `${t.id}:${t.segments}`).join(",")}` : "closed";
      if (key === shown) return;
      shown = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        el.replaceChildren(emptyState("Progress", "Open a project to see how far it has come.", "The wug is keeping score."));
        return;
      }
      el.replaceChildren(h("div", { className: "cc-bar" }, recount, copy, describe), summary, nextBox, h("div", { className: "pg-rows" }, table), note, makeUp, placesBox, speakersBox, growthBox, workBox);
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
