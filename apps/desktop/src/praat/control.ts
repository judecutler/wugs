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

  // A confirmation lasts five seconds and ends with any other action.
  let disarmTimer: ReturnType<typeof setTimeout> | undefined;
  function arm(which: "reopen" | "discard") {
    armedReopen = which === "reopen";
    armedDiscard = which === "discard";
    clearTimeout(disarmTimer);
    disarmTimer = setTimeout(() => {
      disarm();
      draw();
    }, 5000);
  }
  function disarm() {
    armedReopen = false;
    armedDiscard = false;
    clearTimeout(disarmTimer);
  }

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

  function openInPraat() {
    const wasArmed = armedReopen;
    disarm();
    void run(async () => {
      // Read now, not when the toolbar was drawn: the selection may have moved.
      const ctx = context();
      if (ctx === null || ctx.sourceId === null) return;
      const r = praatRange(ctx.selected, ctx.visible, ctx.visible.durationMs);
      // Asked fresh, so a slow or failed earlier check cannot hide a waiting handoff.
      const waiting = await praatPending({ text: ctx.textId });
      if (waiting && !wasArmed) {
        // Opening again replaces what is waiting.
        pendingFor = ctx.textId;
        arm("reopen");
        showToast({ tone: "warn", lines: ["This text is already open in Praat. Bring back its changes first, or click Open again: start over to drop them."] });
        return;
      }
      const opened = await praatOpen({ text: ctx.textId, source: ctx.sourceId, fromMs: r.startMs, toMs: r.endMs });
      pendingFor = ctx.textId;
      review = null;
      const lines = ["Opening in Praat. Edit there, then come back and click Bring back from Praat."];
      if (opened.notShown > 0) lines.push(`${opened.notShown} segment(s) overlap others on their tier or have no times; Praat cannot show them, and they stay as they are.`);
      showToast({ tone: "info", lines });
    });
  }

  function bringBack(text: string) {
    disarm();
    void run(async () => {
      review = { text, view: await praatBringBack({ text }) };
    });
  }

  function apply(text: string) {
    disarm();
    void run(async () => {
      const res = await praatApply({ text });
      review = null;
      await refreshText();
      showToast({ tone: res.findings.length > 0 ? "warn" : "info", lines: [res.changes === 0 ? "Nothing to bring back." : `Brought back ${res.changes} change(s) from Praat.`, ...res.findings] });
    });
  }

  function discard(text: string) {
    if (!armedDiscard) {
      arm("discard");
      draw();
      return;
    }
    disarm();
    void run(async () => {
      await praatDiscard({ text });
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
      className: armedReopen ? "danger" : "",
      textContent: armedReopen ? "Open again: start over" : "Open in Praat",
      title: ctx.sourceId === null ? "Pick a recording first" : `Open this text and its recording in ${status.version ?? "Praat"}, at the selected segment or what is on screen`,
      disabled: busy || ctx.sourceId === null,
      onclick: () => openInPraat(),
    });
    const parts: Node[] = [open];
    if (pendingFor === ctx.textId) {
      parts.push(
        h("button", {
          textContent: busy ? "Waiting for Praat…" : "Bring back from Praat",
          title: "Ask Praat to save this text's TextGrid, then show what changed before writing anything",
          disabled: busy,
          onclick: () => bringBack(ctx.textId),
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
        h("button", { textContent: summary === null ? "OK" : "Not now", disabled: busy, onclick: () => ((review = null), disarm(), draw()) }),
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
      disarm();
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
