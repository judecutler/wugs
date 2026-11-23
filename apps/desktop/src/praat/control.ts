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
  acousticVoice,
  setAcousticVoice,
  type VoiceView,
  type PraatReturn,
  type PraatStatus,
} from "../api";
import { h } from "../dom";
import { refreshText, showToast } from "../store";
import { KINDS, type Kind, type Voice } from "./analysis";
import { changeSummary, notBroughtBack, overlayTitle, praatRange, type Range } from "./logic";

export type PraatContext = {
  textId: string;
  sourceId: string | null;
  selected: { startMs: number | null; endMs: number | null } | null;
  visible: Range & { durationMs: number };
};

/** `element` goes in the toolbar; `review` is a row of its own under it. */
/** What the analysis toggles switch: the Timeline's overlay layer. */
export type Overlays = { enabled: (kind: Kind) => boolean; toggle: (kind: Kind) => void; setVoice: (voice: Voice) => void };

const KIND_LABEL: Record<Kind, string> = { pitch: "Pitch", intensity: "Intensity", formants: "Formants" };

export function praatControls(
  context: () => PraatContext | null,
  overlays: () => Overlays | null,
): { element: HTMLElement; review: HTMLElement; refresh: () => void } {
  const element = h("span", { className: "praat-bar" });
  const reviewRow = h("div", { className: "praat-review", hidden: true });
  let status: PraatStatus | null = null;
  let pendingFor: string | null = null;
  let review: { text: string; view: PraatReturn } | null = null;
  let armedDiscard = false;
  let armedReopen = false;
  let busy = false;
  let checkedText = "";
  /** The loaded recording's voice (pitch range, formant ceiling), asked once per recording. */
  let voice: { source: string; view: VoiceView } | null = null;
  let askedVoice = "";

  const pushVoice = () => {
    const v = voice?.view.voices.find((x) => x.id === voice?.view.voice);
    const layer = overlays();
    if (v !== undefined && layer !== null) layer.setVoice(v);
  };

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
    const praatFound = status !== null && status.path !== null && status.problem === null;
    // Without Praat, the curves come from wugs's own analysis (the
    // acoustic-analysis capability's built-in provider).
    const layer = overlays();
    const toggles =
      layer === null || ctx.sourceId === null || status === null
        ? []
        : KINDS.map((kind) =>
            h("button", {
              className: layer.enabled(kind) ? "active" : "",
              textContent: KIND_LABEL[kind],
              title: overlayTitle(kind, praatFound),
              onclick: () => {
                pushVoice();
                layer.toggle(kind);
                draw();
              },
            }),
          );
    if (!praatFound) {
      const why = status?.problem ?? "Looking for Praat…";
      element.replaceChildren(
        ...toggles,
        ...voicePicker(ctx),
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
      title: ctx.sourceId === null ? "Pick a recording first" : `Open this text and its recording in ${status?.version ?? "Praat"}, at the selected segment or what is on screen`,
      disabled: busy || ctx.sourceId === null,
      onclick: () => openInPraat(),
    });
    const parts: Node[] = [...toggles, ...voicePicker(ctx), open];
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

  /** The voice choice, shown once any overlay is on. */
  function voicePicker(ctx: PraatContext): Node[] {
    const layer = overlays();
    const shown = voice !== null && ctx.sourceId !== null && voice.source === ctx.sourceId ? voice : null;
    if (shown === null || layer === null || !KINDS.some((k) => layer.enabled(k))) return [];
    const select = h(
      "select",
      { className: "praat-voice", title: "The speaker's voice in this recording: the pitch range and formant ceiling the analysis uses. Saved with the recording, for everyone on the project." },
      ...shown.view.voices.map((v) => h("option", { value: v.id, textContent: v.name })),
    );
    select.value = shown.view.voice;
    select.onchange = () => {
      const source = shown.source;
      setAcousticVoice({ source, voice: select.value }).then((view) => {
        voice = { source, view };
        pushVoice();
        draw();
      }, fail);
    };
    return [select];
  }

  function refresh() {
    const ctx = context();
    const src = ctx?.sourceId ?? null;
    if (src !== null && src !== askedVoice) {
      askedVoice = src;
      acousticVoice({ source: src }).then((view) => {
        if (context()?.sourceId !== src) return;
        voice = { source: src, view };
        pushVoice();
        draw();
      }, fail);
    }
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
