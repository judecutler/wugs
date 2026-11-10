// The Praat controls in the Timeline toolbar (roadmap item 46): Open in
// Praat, then Bring back, which shows what Praat changed before anything is
// written. Display rules are in ./logic.ts.
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import {
  errorText,
  praatApply,
  praatBringBack,
  praatDiscard,
  praatOpen,
  praatPending,
  praatSetPath,
  praatStatus,
  type PraatReturn,
  type PraatStatus,
} from "../api";
import { h } from "../dom";
import { refreshText, showToast } from "../store";
import { changeSummary, notBroughtBack, praatRange, type Range } from "./logic";

export type PraatContext = {
  textId: string;
  sourceId: string | null;
  selected: { startMs: number | null; endMs: number | null } | null;
  visible: Range & { durationMs: number };
};

/** `element` goes in the toolbar; `review` is a row of its own under it. */
export function praatControls(context: () => PraatContext | null): { element: HTMLElement; review: HTMLElement; refresh: () => void } {
  const element = h("span", { className: "praat-bar" });
  const reviewRow = h("div", { className: "praat-review", hidden: true });
  let status: PraatStatus | null = null;
  let pendingFor: string | null = null;
  let review: { text: string; view: PraatReturn } | null = null;
  let armedDiscard = false;
  let armedReopen = false;
  let busy = false;
  let checkedText = "";

  const fail = (e: unknown) => showToast({ tone: "error", lines: [errorText(e)] });

  async function run(action: () => Promise<void>) {
    if (busy) return;
    busy = true;
    draw();
    try {
      await action();
    } catch (e) {
      fail(e);
    } finally {
      busy = false;
      draw();
    }
  }

  async function choosePraat() {
    const path = await pickFile({ title: "Where is Praat?" });
    if (typeof path !== "string") return;
    status = await praatSetPath({ path });
  }

  function openInPraat(ctx: PraatContext) {
    void run(async () => {
      if (ctx.sourceId === null) return;
      const r = praatRange(ctx.selected, ctx.visible, ctx.visible.durationMs);
      if (pendingFor === ctx.textId && !armedReopen) {
        // Opening again replaces what is waiting; say so once.
        armedReopen = true;
        showToast({ tone: "warn", lines: ["This text is already open in Praat. Bring back its changes first, or click Open in Praat again to start over without them."] });
        return;
      }
      armedReopen = false;
      const opened = await praatOpen({ text: ctx.textId, source: ctx.sourceId, fromMs: r.startMs, toMs: r.endMs });
      pendingFor = ctx.textId;
      review = null;
      const lines = ["Opening in Praat. Edit there, then come back and click Bring back from Praat."];
      if (opened.notShown > 0) lines.push(`${opened.notShown} segment(s) overlap others on their tier or have no times; Praat cannot show them, and they stay as they are.`);
      showToast({ tone: "info", lines });
    });
  }

  function bringBack(ctx: PraatContext) {
    void run(async () => {
      review = { text: ctx.textId, view: await praatBringBack({ text: ctx.textId }) };
    });
  }

  function apply(text: string) {
    void run(async () => {
      const res = await praatApply({ text });
      review = null;
      await refreshText();
      showToast({ tone: res.findings.length > 0 ? "warn" : "info", lines: [res.changes === 0 ? "Nothing to bring back." : `Brought back ${res.changes} change(s) from Praat.`, ...res.findings] });
    });
  }

  function discard(text: string) {
    if (!armedDiscard) {
      armedDiscard = true;
      draw();
      return;
    }
    void run(async () => {
      await praatDiscard({ text });
      armedDiscard = false;
      pendingFor = null;
      review = null;
    });
  }

  function draw() {
    const ctx = context();
    reviewRow.hidden = true;
    if (ctx === null) {
      element.replaceChildren();
      return;
    }
    if (status === null || (status.path === null || status.problem !== null)) {
      const why = status?.problem ?? "Looking for Praat…";
      element.replaceChildren(
        h("button", {
          textContent: "Praat…",
          title: `${why} Click to choose where Praat is.`,
          disabled: busy || status === null,
          onclick: () => void run(choosePraat),
        }),
      );
      return;
    }
    const open = h("button", {
      textContent: "Open in Praat",
      title: ctx.sourceId === null ? "Pick a recording first" : `Open this text and its recording in ${status.version ?? "Praat"}, at the selected segment or what is on screen`,
      disabled: busy || ctx.sourceId === null,
      onclick: () => openInPraat(ctx),
    });
    const parts: Node[] = [open];
    if (pendingFor === ctx.textId) {
      parts.push(
        h("button", {
          textContent: busy ? "Waiting for Praat…" : "Bring back from Praat",
          title: "Ask Praat to save this text's TextGrid, then show what changed before writing anything",
          disabled: busy,
          onclick: () => bringBack(ctx),
        }),
        h("button", {
          className: armedDiscard ? "danger" : "",
          textContent: armedDiscard ? "Discard: sure?" : "Discard",
          title: "Stop the handoff. Changes already brought back stay; the rest are not brought back.",
          disabled: busy,
          onclick: () => discard(ctx.textId),
        }),
      );
    }
    if (review !== null && review.text === ctx.textId) {
      const text = review.text;
      const summary = changeSummary(review.view);
      const notes = notBroughtBack(review.view.notBroughtBack);
      reviewRow.replaceChildren(
        h("span", { className: "praat-summary", textContent: summary === null ? "Praat changed nothing wugs takes back." : `From Praat: ${summary}.` }),
        ...(summary === null ? [] : [h("button", { className: "primary", textContent: "Apply", disabled: busy, onclick: () => apply(text) })]),
        h("button", { textContent: summary === null ? "OK" : "Not now", disabled: busy, onclick: () => ((review = null), draw()) }),
        ...(notes.length > 0 ? [h("span", { className: "praat-note", textContent: `Not brought back: ${notes.join("; ")}.` })] : []),
      );
      reviewRow.hidden = false;
    }
    element.replaceChildren(...parts);
  }

  function refresh() {
    const ctx = context();
    if (status === null) {
      praatStatus().then((st) => {
        status = st;
        draw();
      }, fail);
    }
    if (ctx !== null && ctx.textId !== checkedText) {
      checkedText = ctx.textId;
      armedDiscard = false;
      armedReopen = false;
      const asked = ctx.textId;
      praatPending({ text: asked }).then((p) => {
        if (context()?.textId === asked) pendingFor = p ? asked : null;
        draw();
      }, fail);
    }
    draw();
  }

  return { element, review: reviewRow, refresh };
}
