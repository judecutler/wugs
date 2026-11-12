// Concordance (roadmap 71): every occurrence of a word, morph, gloss or
// translation word, one row per hit with the keyword in the middle, sortable
// by the words either side; a row opens its segment, ▶ plays it.
import { concordance, errorText } from "../api";
import { asTsv, clip, countLine, isRtl, keyword, KINDS, left, right, SORTS, sortHits, type ConcordanceKind, type Hit, type SortBy } from "../concordance/logic";
import { emptyState, h } from "../dom";
import { hear, playButton } from "../hear/hear";
import { provide } from "../palette/registry";
import { getState, selectSegment, selectText, showToast } from "../store";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { SHOW_PANEL, type PanelDef, type PanelView } from "./panel";

/** A query another panel asked for (Frequencies), taken by the panel once it is open. */
type Asked = { query: string; kind: ConcordanceKind; text: { id: string; title: string } | null };
let pending: (Asked & { project: string }) | null = null;
const askers = new Set<() => void>();

/** Show the Concordance for exactly this item (no wildcards), in one text
 * or the whole project, opening the panel if it is closed. */
export function askConcordance(query: string, kind: ConcordanceKind, text: { id: string; title: string } | null = null): void {
  const s = getState();
  if (s.kind !== "open") return;
  pending = { query, kind, text, project: s.info.projectId };
  window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "concordance" }));
  for (const take of askers) take();
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
    let kind: ConcordanceKind = "word";
    let sortBy: SortBy = "text";
    let hits: Hit[] = [];
    let total = 0;
    let asked = "";
    let seq = 0;
    let timer = 0;
    /** The item Frequencies asked for: met exactly and in its text while the query is unchanged. */
    let exactFor: Asked | null = null;
    const input = h("input", { type: "search", className: "cc-query", placeholder: KINDS[0]!.hint, spellcheck: false });
    input.setAttribute("aria-label", "Find");
    const chips = h("div", { className: "scopes", role: "radiogroup", ariaLabel: "Look in" });
    const sorter = h("select", { className: "cc-sort", title: "Order of the rows" }, ...SORTS.map((x) => h("option", { value: x.id, textContent: x.label })));
    sorter.setAttribute("aria-label", "Sort");
    const copy = h("button", { type: "button", textContent: "Copy rows", title: "Copy every row as a table to paste into a spreadsheet" });
    const count = h("p", { className: "note cc-count" });
    const rows = h("div", { className: "cc-rows", role: "list" });

    const drawChips = () =>
      chips.replaceChildren(
        ...KINDS.map((k) => {
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

    const drawRows = () => {
      if (asked === "") {
        count.textContent = "Type a word, morph or gloss. * stands for any letters: ni* finds every word starting ni.";
        rows.replaceChildren();
        return;
      }
      const texts = new Set(hits.map((x) => x.textId)).size;
      const scoped = exactFor !== null && exactFor.query === asked && exactFor.kind === kind ? exactFor.text : null;
      count.textContent = countLine(hits.length, total, texts) + (scoped !== null ? ` Only in «${scoped.title}».` : "");
      rows.replaceChildren(
        ...sortHits(hits, sortBy).map((hit) => {
          const play = playButton("Open this segment and play it", () => void open(hit, true));
          play.classList.add("cc-play");
          play.disabled = hit.startMs === null;
          const row = h(
            "div",
            { className: "cc-row", role: "listitem", tabIndex: 0, title: `${hit.textTitle}: open this segment`, dir: isRtl(keyword(hit)) ? "rtl" : "ltr" },
            play,
            h("span", { className: "cc-left", textContent: clip(left(hit), CONTEXT, "left") }),
            h("span", { className: "cc-key", textContent: keyword(hit) }),
            h("span", { className: "cc-right", textContent: clip(right(hit), CONTEXT, "right") }),
            h(
              "span",
              { className: "cc-meta" },
              h("span", { textContent: hit.breakdown === null ? "" : `${hit.breakdown} ‘${hit.glosses ?? ""}’` }),
              h("span", { textContent: hit.textTitle }),
            ),
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
        concordance({ query, kind, exact: exactAsk !== null, textId: exactAsk?.text?.id ?? null }).then(
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
            count.textContent = `Could not search: ${errorText(e)}`;
          },
        );
      }, 200);
    }

    input.oninput = run;
    sorter.onchange = () => {
      sortBy = sorter.value as SortBy;
      drawRows();
    };
    copy.onclick = () => {
      if (hits.length === 0) return;
      void writeText(asTsv(sortHits(hits, sortBy))).then(
        () =>
          showToast({
            tone: "info",
            lines: [total > hits.length ? `Copied the first ${hits.length} of ${total} rows. Narrow the search to copy the rest.` : `Copied ${hits.length} rows. Paste them into a spreadsheet.`],
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
      asked = "";
      exactFor = null;
      input.value = "";
      if (now === "closed") {
        el.replaceChildren(emptyState("Concordance", "Open a project to see every place a word, morph or gloss is used.", "The wug is counting words."));
        return;
      }
      drawChips();
      drawRows();
      el.replaceChildren(h("div", { className: "cc-bar" }, input, sorter, copy), chips, count, rows);
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
      exactFor = { query: pending.query, kind: pending.kind, text: pending.text };
      input.value = pending.query;
      input.placeholder = KINDS.find((k) => k.id === kind)?.hint ?? "";
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
