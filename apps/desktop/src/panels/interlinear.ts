// Interlinear: the whole selected text as interlinear glossed text, every
// sentence with its words, morphs, glosses and free translation, the way a
// FLEx or Toolbox user reads a text. Read-only: a sentence opens in the Gloss
// panel when clicked, ▶ plays it.
import { convertFileSrc } from "@tauri-apps/api/core";
import { abbreviationMeanings, errorText, facsimileView, glossGet, setAbbreviationMeaning, type TextDetail } from "../api";
import { cropStyle, type FacsimileView } from "../facsimile/logic";
import { abbreviationList, abbreviationsOf, unexplainedNote } from "../gloss/abbreviations";
import { emptyState, h } from "../dom";
import { glossRuns, wordLines, type ExampleWord } from "../gloss/example";
import { numbering } from "../gloss/document";
import { translationLines } from "../gloss/translation";
import { hear, playButton } from "../hear/hear";
import * as playback from "../playback";
import { lineAt, readAlongSpan, analysed, nextWhere, progress, sentencesOf, sentenceTracks, SHOWS, showOf, topicsIn, type Line, type Show } from "../interlinear/logic";
import { provide } from "../palette/registry";
import { dataGeneration, getSelection, getState, selectSegment, showToast, subscribeSelection } from "../store";
import type { PanelDef, PanelView } from "./panel";

/** Show only the sentences still to gloss (kept while the app runs). */
let show: Show = "all";

/** Show the list of abbreviations the text's glosses use (kept while the app runs). */
let showAbbreviations = false;

/** Show each sentence's area of its page image under it (kept on this computer). */
const CROPS_KEY = "wugs.interlinear.pageLines";
let showCrops = localStorage.getItem(CROPS_KEY) !== "off";

/** The natural size of page images already loaded, by URL. */
const sizes = new Map<string, { width: number; height: number }>();

/** How many sentences are read from the engine at once. */
const BATCH = 8;

async function linesOf(text: TextDetail): Promise<Line[]> {
  // Numbered on their own track, as Copy example numbers them.
  const segs = text.segments.map((s) => ({ ...s, parent: s.parent ?? null }));
  const tiers = text.tiers.map((t) => ({ id: t.id, utterance: t.utterance === true, role: t.role, name: t.name }));
  // A transcript with a track per speaker names the speaker on each line.
  const speakers = sentenceTracks(segs, tiers).length > 1;
  const numbers = numbering(sentencesOf(segs, tiers, new Map()));
  const sentences = sentencesOf(segs, tiers, numbers);
  const lines: Line[] = [];
  for (let i = 0; i < sentences.length; i += BATCH) {
    const views = await Promise.all(sentences.slice(i, i + BATCH).map((seg) => glossGet({ textId: text.id, segmentId: seg.id })));
    views.forEach((v, j) => {
      const seg = sentences[i + j]!;
      const full = text.segments.find((s) => s.id === seg.id)!;
      const translation = translationLines(full, text.segments, text.tiers).find((l) => l.segment !== null)?.segment?.content ?? null;
      const words: ExampleWord[] = v.words.map((w) => ({ form: w.form, morphs: w.morphs.map((m) => ({ surface: m.surface, type: m.type, gloss: m.gloss })) }));
      lines.push({ id: seg.id, number: numbers.get(seg.id) ?? i + j + 1, content: seg.content, words, translation, startMs: seg.startMs, endMs: full.endMs ?? null, notes: (v.segmentNotes ?? []).map((n) => n.body), topics: v.topics ?? [], ...(speakers ? { speaker: tiers.find((t) => t.id === seg.tier)?.name ?? "" } : {}) });
    });
  }
  return lines;
}

function glossLine(gloss: string): HTMLElement {
  return h("span", { className: "il-gloss" }, ...glossRuns(gloss).map((r) => (r.caps ? h("span", { className: "sc", textContent: r.text }) : document.createTextNode(r.text))));
}

export const interlinearPanel: PanelDef = {
  title: "Interlinear",
  create(): PanelView {
    const el = h("div", { className: "panel panel-interlinear" });
    let key = "";
    let seq = 0;
    let lines: Line[] = [];
    /** The text's page images and marked areas, when it has any. */
    let facs: FacsimileView | null = null;
    /** The project's own meanings of the abbreviations in `lines`. */
    let own = new Map<string, string>();
    /** What is typed in a meaning field and not saved yet, by abbreviation. */
    const drafts = new Map<string, string>();

    /** Read along: the whole text plays and the sentence being heard is marked and kept in view. */
    let reading = false;
    let readFrame = 0;
    let marked: string | null = null;
    const follow = () => {
      if (!reading) return;
      if (!playback.isPlaying()) {
        stopReading();
        return;
      }
      const now = lineAt(lines, playback.currentMs());
      const id = now?.id ?? null;
      if (id !== marked) {
        el.querySelector(".il-row.il-heard")?.classList.remove("il-heard");
        marked = id;
        if (id !== null) {
          const row = el.querySelector<HTMLElement>(`.il-row[data-id="${CSS.escape(id)}"]`);
          row?.classList.add("il-heard");
          row?.scrollIntoView({ block: "nearest" });
        }
      }
      readFrame = window.requestAnimationFrame(follow);
    };
    const stopReading = () => {
      reading = false;
      window.cancelAnimationFrame(readFrame);
      el.querySelector(".il-row.il-heard")?.classList.remove("il-heard");
      marked = null;
      const b = el.querySelector<HTMLButtonElement>(".il-read");
      if (b !== null) b.textContent = "Read along";
    };
    const readAlong = async (text: TextDetail) => {
      if (reading) {
        playback.pause();
        stopReading();
        return;
      }
      const span = readAlongSpan(lines);
      if (span === null) {
        showToast({ tone: "info", lines: ["No sentence of this text is timed, so there is nothing to play."] });
        return;
      }
      const seg = text.segments.find((x) => x.id === span.first.id);
      if (seg === undefined) return;
      await hear({ textId: text.id, tierId: seg.tier, segmentId: seg.id, startMs: span.startMs, endMs: span.endMs }, false);
      // Give the audio a moment to start before following it.
      window.setTimeout(() => {
        if (!playback.isPlaying()) return;
        reading = true;
        const b = el.querySelector<HTMLButtonElement>(".il-read");
        if (b !== null) b.textContent = "Stop";
        readFrame = window.requestAnimationFrame(follow);
      }, 150);
    };

    /** Select the next sentence still to gloss, for the Gloss panel, and bring its row into view. */
    const goNext = () => {
      const s = getState();
      if (s.kind !== "open" || s.text === null) return;
      const text = s.text;
      // Next to translate while those are listed, else next to gloss.
      const want = show === "translate" ? showOf("translate") : showOf("gloss");
      const next = nextWhere(lines, getSelection()?.segmentId ?? null, want.keep);
      const seg = next === null ? undefined : text.segments.find((x) => x.id === next.id);
      if (seg === undefined) {
        showToast({ tone: "info", lines: [lines.length === 0 ? "This text has no sentences yet." : want.done] });
        return;
      }
      selectSegment({ textId: text.id, tierId: seg.tier, segmentId: seg.id });
      window.requestAnimationFrame(() => el.querySelector<HTMLElement>(`.il-row[data-id="${CSS.escape(seg.id)}"]`)?.scrollIntoView({ block: "nearest" }));
    };

    /** Every abbreviation the text's glosses use with its meaning: the
     * project's own where someone wrote one (an editable field), else the
     * Leipzig list's, shown as the field's placeholder. */
    const abbreviationsSection = (text: TextDetail): HTMLElement => {
      const list = abbreviationList(abbreviationsOf(lines), own);
      if (list.length === 0) return h("section", { className: "il-abbr" }, h("p", { className: "note", textContent: "No grammatical glosses in this text yet. Abbreviations written in capitals (SG, PST, 3) show up here." }));
      const rows = list.map((a) => {
        const field = h("input", {
          type: "text",
          value: drafts.get(a.abbr) ?? own.get(a.abbr) ?? "",
          placeholder: a.source === "leipzig" ? a.meaning ?? "" : "What does it mean?",
          title: a.source === "none" ? "Not in the Leipzig list" : "Leave empty to use the Leipzig meaning",
        });
        field.dataset.abbr = a.abbr;
        field.setAttribute("aria-label", `Meaning of ${a.abbr}`);
        field.oninput = () => drafts.set(a.abbr, field.value);
        const save = () => {
          const typed = field.value.trim();
          if (!drafts.has(a.abbr)) return;
          drafts.delete(a.abbr);
          if (typed === (own.get(a.abbr) ?? "")) return;
          void setAbbreviationMeaning(a.abbr, typed).then(
            () => {
              if (typed === "") own.delete(a.abbr);
              else own.set(a.abbr, typed);
              const now = getState();
              if (now.kind === "open" && now.text?.id === text.id) paint(now.text);
            },
            (e) => showToast({ tone: "error", lines: [errorText(e)] }),
          );
        };
        field.onblur = save;
        field.onkeydown = (e) => {
          if (e.key === "Enter" && !e.isComposing) save();
          else if (e.key === "Escape") {
            drafts.delete(a.abbr);
            field.value = own.get(a.abbr) ?? "";
          }
        };
        return h("li", { className: `il-abbr-row${a.source === "none" ? " il-abbr-unknown" : ""}` }, h("span", { className: "sc il-abbr-code", textContent: a.abbr }), field);
      });
      const note = unexplainedNote(list);
      return h(
        "section",
        { className: "il-abbr" },
        h("p", { className: "note", textContent: "What each abbreviation in this text means, for the list a paper prints with its examples. Empty fields use the Leipzig Glossing Rules' meaning; what you write is kept for the whole project." }),
        ...(note === null ? [] : [h("p", { className: "note il-abbr-warn", textContent: note })]),
        h("ul", { className: "il-abbr-list" }, ...rows),
      );
    };

    const paint = (text: TextDetail) => {
      const sel = getSelection();
      const shown = lines.filter(showOf(show).keep);
      const rows = shown.map((l) => {
        const seg = text.segments.find((s) => s.id === l.id);
        const play = playButton("Play this sentence", () => {
          if (seg !== undefined) void hear({ textId: text.id, tierId: seg.tier, segmentId: seg.id, startMs: seg.startMs, endMs: seg.endMs });
        });
        play.disabled = seg?.startMs === null || seg?.startMs === undefined;
        const body = analysed(l)
          ? h(
              "div",
              { className: "il-words" },
              ...l.words.map((w) => {
                const { morphs, glosses } = wordLines(w);
                return h("span", { className: "il-word" }, h("span", { className: "il-form", textContent: w.form }), h("span", { className: "il-morphs", textContent: w.morphs.length > 0 ? morphs : "" }), glossLine(glosses));
              }),
            )
          : h("p", { className: "il-plain", textContent: l.content.trim() === "" ? "(not transcribed)" : l.content });
        const area = showCrops ? facs?.marked.find((m) => m.segment === l.id) : undefined;
        const page = area === undefined ? undefined : facs?.pages.find((p) => p.source === area.area.source);
        let crop: HTMLElement | null = null;
        if (area !== undefined && page?.path) {
          const url = convertFileSrc(page.path);
          const natural = sizes.get(url);
          if (natural === undefined) {
            const img = new Image();
            img.onload = () => {
              sizes.set(url, { width: img.naturalWidth || 1000, height: img.naturalHeight || 1000 });
              const now = getState();
              if (now.kind === "open" && now.text?.id === text.id) paint(now.text);
            };
            img.src = url;
          } else {
            const st = cropStyle(area.area, natural);
            crop = h("div", { className: "il-crop", title: `${page.label}: where this line stands on the page` });
            crop.style.backgroundImage = `url("${url}")`;
            crop.style.backgroundSize = st.backgroundSize;
            crop.style.backgroundPosition = st.backgroundPosition;
            crop.style.aspectRatio = st.aspectRatio;
            crop.setAttribute("role", "img");
            crop.setAttribute("aria-label", `The line on ${page.label}`);
          }
        }
        const row = h(
          "li",
          { className: `il-row${l.speaker === undefined ? "" : " il-row-speaker"}${sel?.segmentId === l.id ? " il-selected" : ""}${analysed(l) ? "" : " il-unglossed"}`, tabIndex: 0, title: "Open this sentence in the Gloss panel" },
          h("span", { className: "il-num", textContent: `(${l.number})` }),
          play,
          l.speaker === undefined ? null : h("span", { className: "il-speaker", textContent: l.speaker }),
          h("div", { className: "il-body" }, crop, body, l.translation === null ? null : h("p", { className: "il-tr", textContent: `‘${l.translation}’` }), ...(l.notes ?? []).map((n) => h("p", { className: "il-note", textContent: `Note: ${n}` }))),
        );
        row.dataset.id = l.id;
        const pick = () => seg !== undefined && selectSegment({ textId: text.id, tierId: seg.tier, segmentId: seg.id });
        row.onclick = (e) => {
          if ((e.target as HTMLElement).closest("button") === null) pick();
        };
        row.onkeydown = (e) => {
          if (e.key === "Enter") pick();
          else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            const next = e.key === "ArrowDown" ? row.nextElementSibling : row.previousElementSibling;
            if (next instanceof HTMLElement) next.focus();
          }
        };
        return row;
      });
      const focused = el.contains(document.activeElement) ? [...el.querySelectorAll(".il-row")].indexOf(document.activeElement as Element) : -1;
      const focusedAbbr = document.activeElement instanceof HTMLInputElement && el.contains(document.activeElement) ? (document.activeElement.dataset.abbr ?? null) : null;
      const topicChoices = topicsIn(lines).map((t) => showOf(`topic:${t}`));
      const only = h("select", { className: "il-show", title: "Which sentences to list: all, those still to gloss or translate, or those filed under a grammar topic" }, ...[...SHOWS, ...topicChoices].map((x) => h("option", { value: x.id, textContent: x.label, selected: x.id === show })));
      only.setAttribute("aria-label", "Which sentences to list");
      only.onchange = () => {
        show = only.value as Show;
        paint(text);
      };
      const nextLabel = showOf(show).next;
      const next = h("button", { type: "button", textContent: nextLabel, title: show === "translate" ? "Open the next sentence without a translation in the Gloss panel, where its translation line is" : "Open the next sentence still to gloss in the Gloss panel" });
      next.onclick = goNext;
      const abbrButton = h("button", { type: "button", textContent: "Abbreviations", title: "The abbreviations this text's glosses use, and what each means" });
      abbrButton.setAttribute("aria-expanded", String(showAbbreviations));
      abbrButton.onclick = () => {
        showAbbreviations = !showAbbreviations;
        paint(text);
      };
      const read = h("button", { type: "button", className: "il-read", textContent: reading ? "Stop" : "Read along", title: "Play the whole text from its recording, marking each sentence as it is heard" });
      read.onclick = () => void readAlong(text);
      const bar = h("div", { className: "il-bar" }, only, next, abbrButton, read);
      if ((facs?.marked.length ?? 0) > 0) {
        const crops = h("button", { type: "button", textContent: "Page lines", title: "Show each sentence's line from its page image above it" });
        crops.setAttribute("aria-pressed", String(showCrops));
        crops.onclick = () => {
          showCrops = !showCrops;
          localStorage.setItem(CROPS_KEY, showCrops ? "on" : "off");
          paint(text);
        };
        bar.append(crops);
      }
      const abbrSection = showAbbreviations ? abbreviationsSection(text) : null;
      const empty = show !== "all" && shown.length === 0 && lines.length > 0 ? h("p", { className: "note", textContent: showOf(show).done }) : null;
      el.replaceChildren(h("h3", { className: "il-title", textContent: text.title || "(untitled)" }), h("p", { className: "note", textContent: progress(lines) }), bar, ...(abbrSection === null ? [] : [abbrSection]), ...(empty === null ? [] : [empty]), h("ol", { className: "il-list" }, ...rows));
      if (marked !== null) el.querySelector(`.il-row[data-id="${CSS.escape(marked)}"]`)?.classList.add("il-heard");
      if (focused >= 0) el.querySelectorAll<HTMLElement>(".il-row")[focused]?.focus();
      if (focusedAbbr !== null) el.querySelector<HTMLInputElement>(`input[data-abbr="${CSS.escape(focusedAbbr)}"]`)?.focus();
    };

    const draw = () => {
      const s = getState();
      if (s.kind !== "open") {
        key = "";
        el.replaceChildren(emptyState("No project open", "Open a project to read its texts interlinear."));
        return;
      }
      const text = s.text;
      if (text === null) {
        key = "";
        el.replaceChildren(emptyState("Interlinear", "Select a text in the Texts panel to read it here as interlinear glossed text, sentence by sentence."));
        return;
      }
      const wanted = `${text.id}|${dataGeneration()}|${text.segments.length}|${text.segments.map((x) => x.content).join("\u0001").length}`;
      if (wanted === key) {
        paint(text);
        return;
      }
      key = wanted;
      const mine = ++seq;
      if (lines.length === 0) el.replaceChildren(emptyState("Interlinear", "Reading the text…"));
      void Promise.all([linesOf(text), facsimileView({ text: text.id }).catch(() => null)]).then(
        async ([ls, f]) => {
          if (mine !== seq) return;
          facs = f;
          const used = abbreviationsOf(ls);
          const meanings = used.length === 0 ? [] : await abbreviationMeanings(used).catch(() => used.map(() => null));
          if (mine !== seq) return;
          lines = ls;
          own = new Map(used.flatMap((a, i) => (meanings[i] ? [[a, meanings[i]!] as [string, string]] : [])));
          const now = getState();
          if (now.kind === "open" && now.text?.id === text.id) paint(now.text);
        },
        (e) => {
          if (mine === seq) el.replaceChildren(emptyState("Interlinear", errorText(e)));
        },
      );
    };

    const unsubscribe = subscribeSelection(draw);
    const unprovide = provide("interlinear", () => [
      { id: "interlinear:next", title: "Go to the next sentence to gloss", group: "Interlinear", words: ["unglossed", "next", "skip", "todo"], run: goNext },
      { id: "interlinear:read", title: "Read the text along with its recording", group: "Interlinear", words: ["play all", "listen", "karaoke", "proofread", "follow"], run: () => el.querySelector<HTMLButtonElement>(".il-read")?.click() },
    ]);
    return {
      element: el,
      render: draw,
      dispose: () => {
        stopReading();
        unsubscribe();
        unprovide();
      },
    };
  },
};
