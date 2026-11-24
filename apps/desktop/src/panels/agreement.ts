// Agreement: two tracks of the text on screen compared segment by segment, for
// inter-annotator reliability (two people's transcriptions of one recording),
// or a machine or converted track against the one a person corrected. Rules in
// ../agreement/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText } from "../api";
import {
  agreement,
  agreementLines,
  agreementTsv,
  comparable,
  defaultTracks,
  differs,
  startOf,
  type Row,
} from "../agreement/logic";
import { emptyState, h } from "../dom";
import { hear, playButton } from "../hear/hear";
import { provide } from "../palette/registry";
import { getSelection, getState, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

const DIFF_KEY = "wugs.agreement.differences";
const MAX_ROWS = 500;

const secs = (ms: number) => (ms / 1000).toFixed(2);
const signed = (ms: number) =>
  ms === 0 ? "0" : `${ms > 0 ? "+" : "−"}${Math.abs(Math.round(ms))}`;

export const agreementPanel: PanelDef = {
  title: "Agreement",
  create(): PanelView {
    const el = h("div", { className: "panel panel-agreement" });
    let pick: [string, string] | null = null;
    let pickFor = "";
    let onlyDiff = localStorage.getItem(DIFF_KEY) === "on";

    const paint = () => {
      const s = getState();
      if (s.kind !== "open") {
        el.replaceChildren(
          emptyState(
            "No project open",
            "Open a project to compare two tracks of a text.",
          ),
        );
        return;
      }
      if (s.text === null) {
        el.replaceChildren(
          emptyState(
            "Agreement",
            "Select a text with two transcribed tracks of one recording (two annotators, or a machine track and a corrected one) to see how far they agree.",
          ),
        );
        return;
      }
      const text = s.text;
      const fallback = defaultTracks(text.tiers, text.segments);
      if (fallback === null) {
        el.replaceChildren(
          emptyState(
            "Agreement",
            "This text needs two transcription tracks with timed, labelled segments. Add a second track on the Timeline, or import an ELAN file with both annotators' tiers.",
          ),
        );
        return;
      }
      if (
        pickFor !== text.id ||
        pick === null ||
        !comparable(text.tiers, text.segments).some((t) => t.id === pick![0]) ||
        !comparable(text.tiers, text.segments).some((t) => t.id === pick![1])
      ) {
        pick = fallback;
        pickFor = text.id;
      }
      const [tierA, tierB] = pick;
      const nameOf = (id: string) =>
        text.tiers.find((t) => t.id === id)?.name ?? "";
      const nameA = nameOf(tierA);
      const nameB = nameOf(tierB);
      const tracks = comparable(text.tiers, text.segments);
      const options = () =>
        tracks.map((t) => h("option", { value: t.id, textContent: t.name }));
      const aPick = h(
        "select",
        {
          title:
            "The reference track: the error rate counts against its sounds",
        },
        ...options(),
      );
      const bPick = h(
        "select",
        { title: "The track to compare with it" },
        ...options(),
      );
      aPick.value = tierA;
      bPick.value = tierB;
      const change = () => {
        pick = [aPick.value, bPick.value];
        paint();
      };
      aPick.onchange = change;
      bPick.onchange = change;
      const swap = h("button", {
        textContent: "Swap",
        title: "Make the other track the reference",
      });
      swap.onclick = () => {
        pick = [tierB, tierA];
        paint();
      };
      const same = tierA === tierB;
      const r = same ? null : agreement(text.segments, tierA, tierB);
      const diffBox = h("input", { type: "checkbox", checked: onlyDiff });
      diffBox.onchange = () => {
        onlyDiff = diffBox.checked;
        localStorage.setItem(DIFF_KEY, onlyDiff ? "on" : "off");
        paint();
      };
      const copy = h("button", {
        textContent: "Copy table",
        disabled: r === null || r.rows.length === 0,
        title:
          "Every pair and unmatched segment with times, labels and how far apart, for a spreadsheet or R",
      });
      copy.onclick = () => {
        if (r === null) return;
        void writeText(agreementTsv(r, nameA, nameB)).then(
          () =>
            showToast({
              tone: "info",
              lines: [`Copied ${r.rows.length} rows.`],
            }),
          (e) => showToast({ tone: "error", lines: [errorText(e)] }),
        );
      };
      const bar = h(
        "div",
        { className: "vw-bar" },
        h("label", {}, "Reference ", aPick),
        h("label", {}, "Compared ", bPick),
        swap,
        h(
          "label",
          {
            title:
              "List only pairs whose labels differ and segments with no partner",
          },
          diffBox,
          " Only differences",
        ),
        copy,
      );
      if (r === null) {
        el.replaceChildren(
          bar,
          h("p", {
            className: "note",
            textContent: "Pick two different tracks.",
          }),
        );
        return;
      }
      const summary = h(
        "div",
        { className: "ag-summary" },
        ...agreementLines(r, nameA, nameB).map((line) =>
          h("p", { textContent: line }),
        ),
      );
      const rows = onlyDiff ? r.rows.filter(differs) : r.rows;
      const selected = getSelection()?.segmentId ?? null;
      const play = (tierId: string, id: string, start: number, end: number) =>
        playButton(
          "Hear this segment",
          () =>
            void hear({
              textId: text.id,
              tierId,
              segmentId: id,
              startMs: start,
              endMs: end,
            }),
        );
      const line = (row: Row) => {
        if (row.kind === "pair") {
          const { a, b } = row.pair;
          const tr = h(
            "tr",
            { className: row.pair.same ? "" : "ag-differs" },
            h("td", { className: "fq-num", textContent: secs(a.start) }),
            h(
              "td",
              { className: "ag-play" },
              play(tierA, a.id, a.start, a.end),
            ),
            h("td", { className: "ag-label", textContent: a.label }),
            h("td", { className: "ag-label", textContent: b.label }),
            h("td", {
              className: "fq-num",
              textContent: row.pair.same ? "" : String(row.pair.distance),
              title: "Sounds inserted, left out or changed",
            }),
            h("td", {
              className: "fq-num",
              textContent: `${signed(b.start - a.start)} / ${signed(b.end - a.end)}`,
              title:
                "How much later (+) or earlier (−) the compared track's start and end are, in ms",
            }),
          );
          if (selected === a.id || selected === b.id)
            tr.classList.add("is-selected");
          return tr;
        }
        const tierId = row.kind === "a" ? tierA : tierB;
        const tr = h(
          "tr",
          { className: "ag-alone" },
          h("td", { className: "fq-num", textContent: secs(row.seg.start) }),
          h(
            "td",
            { className: "ag-play" },
            play(tierId, row.seg.id, row.seg.start, row.seg.end),
          ),
          h("td", {
            className: row.kind === "a" ? "ag-label" : "ag-none",
            textContent: row.kind === "a" ? row.seg.label : "—",
          }),
          h("td", {
            className: row.kind === "b" ? "ag-label" : "ag-none",
            textContent: row.kind === "b" ? row.seg.label : "—",
          }),
          h("td", {}),
          h("td", {
            className: "note",
            textContent:
              row.kind === "a" ? `only on ${nameA}` : `only on ${nameB}`,
          }),
        );
        if (selected === row.seg.id) tr.classList.add("is-selected");
        return tr;
      };
      const body: Node[] = [summary];
      if (rows.length === 0)
        body.push(
          h("p", {
            className: "note",
            textContent: onlyDiff
              ? "No differences: every pair has the same label and every segment a partner."
              : "Nothing to list.",
          }),
        );
      else {
        body.push(
          h(
            "div",
            { className: "ag-scroll" },
            h(
              "table",
              { className: "pg-table ag-table" },
              h(
                "thead",
                {},
                h(
                  "tr",
                  {},
                  ...[
                    "Time (s)",
                    "",
                    nameA,
                    nameB,
                    "Sounds apart",
                    "Start / end (ms)",
                  ].map((t) => h("th", { scope: "col", textContent: t })),
                ),
              ),
              h(
                "tbody",
                {},
                ...rows
                  .slice(0, MAX_ROWS)
                  .sort((x, y) => startOf(x) - startOf(y))
                  .map(line),
              ),
            ),
          ),
        );
        if (rows.length > MAX_ROWS)
          body.push(
            h("p", {
              className: "note",
              textContent: `The first ${MAX_ROWS} of ${rows.length} rows; Copy table has them all.`,
            }),
          );
      }
      el.replaceChildren(bar, ...body);
    };

    const unprovide = provide("agreement", () => [
      {
        id: "agreement:copy",
        title: "Copy the agreement table",
        group: "Agreement",
        words: [
          "inter-annotator",
          "reliability",
          "kappa",
          "compare tracks",
          "error rate",
          "two transcribers",
        ],
        run: () =>
          el
            .querySelector<HTMLButtonElement>(".vw-bar button:last-of-type")
            ?.click(),
      },
    ]);
    return { element: el, render: paint, dispose: unprovide };
  },
};
