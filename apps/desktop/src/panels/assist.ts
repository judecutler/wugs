// The Speech assist panel (roadmap item 27): download a phone model once,
// then fill empty segments with what it hears. Everything runs on this
// computer; the recording never leaves it. Machine transcriptions stay
// tinted until someone edits them (src/assist/logic.ts).
import { assistCancel, assistDelete, assistDownload, assistModels, assistSuggest, errorText, type AssistModel } from "../api";
import { batchTargets, megabytes, progressLabel, VERY_UNSURE_BELOW } from "../assist/logic";
import { emptyState, h } from "../dom";
import { assistModelList, getState, installedModel, mediaSource, refreshText, setAssistModels, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

/** A batch run in progress, shared by every instance of the panel. */
let run: { text: string; tier: string; done: number; total: number; filled: number; skipped: number; stop: boolean } | null = null;
let armed: string | null = null;

export async function reloadModels() {
  setAssistModels(await assistModels());
}

/** Fill one tier's empty segments one after another; each is its own write, so stopping keeps what was done. */
async function runBatch(text: string, tier: string, model: string, source: string, rerender: () => void) {
  const s = getState();
  if (s.kind !== "open" || s.text === null || run !== null) return;
  const targets = batchTargets(s.text.segments, tier);
  const project = s.info.path;
  run = { text, tier, done: 0, total: targets.length, filled: 0, skipped: 0, stop: false };
  let moved = false;
  rerender();
  const failures: string[] = [];
  for (const seg of targets) {
    if (run.stop) break;
    // The person opened another text or project: stop rather than keep writing out of sight.
    const now = getState();
    if (now.kind !== "open" || now.info.path !== project || now.text?.id !== text) {
      moved = true;
      break;
    }
    try {
      const r = await assistSuggest({ text, segment: seg.id, source, model });
      if (r.content === "") run.skipped += 1;
      else run.filled += 1;
    } catch (e) {
      run.skipped += 1;
      if (failures.length < 3) failures.push(errorText(e));
    }
    run.done += 1;
    await refreshText();
    rerender();
  }
  const r = run;
  run = null;
  showToast({
    tone: failures.length > 0 ? "warn" : "info",
    lines: [`Filled ${r.filled} of ${r.total} empty segments${r.stop ? " before stopping" : moved ? " before you switched texts" : ""}.`, ...(r.skipped > 0 ? [`${r.skipped} skipped (no speech heard, or an error).`] : []), ...failures],
  });
  rerender();
}

export const assistPanel: PanelDef = {
  title: "Speech assist",
  create(): PanelView {
    const el = h("div", { className: "panel panel-assist" });
    let tierChoice: string | null = null;

    function draw() {
      const models = assistModelList();
      const s = getState();
      const model = installedModel();
      el.replaceChildren(
        h("p", { className: "as-intro", textContent: "Suggests a phonetic transcription for empty segments, as IPA. It runs on this computer: your recordings are never sent anywhere. Suggestions stay tinted until you edit them." }),
        h(
          "div",
          { className: "as-legend" },
          h("span", { className: "as-swatch machine-unsure" }),
          " machine, unchecked",
          h("span", { className: "as-swatch machine-very-unsure" }),
          ` machine, under ${Math.round(VERY_UNSURE_BELOW * 100)}% sure`,
        ),
        h("h4", { textContent: "Models" }),
        ...(models.length === 0 ? [h("p", { className: "as-note", textContent: "Loading…" })] : models.map(modelCard)),
        h("h4", { textContent: "Fill empty segments" }),
        batchControls(s, model),
      );
    }

    function modelCard(m: AssistModel) {
      const dl = m.download;
      const actions: (HTMLElement | null)[] = [];
      if (dl !== null && dl.error === null) {
        actions.push(h("progress", { max: Math.max(1, dl.total), value: dl.done }), h("span", { className: "as-note", textContent: progressLabel(dl.done, dl.total) }), h("button", { type: "button", textContent: "Cancel", onclick: () => void act(() => assistCancel({ id: m.id })) }));
      } else if (m.installed === null) {
        actions.push(
          h("button", { type: "button", textContent: `Download (about ${m.approxMb} MB)`, onclick: () => void act(() => assistDownload({ id: m.id })) }),
          ...(dl?.error ? [h("span", { className: "as-error", textContent: dl.error }), h("button", { type: "button", textContent: "Dismiss", onclick: () => void act(() => assistCancel({ id: m.id })) })] : []),
        );
      } else {
        const bytes = m.installed.files.reduce((n, f) => n + f.bytes, 0);
        const key = `delete:${m.id}`;
        actions.push(
          h("span", { className: "badge", textContent: "downloaded" }),
          h("span", { className: "as-note", textContent: megabytes(bytes) }),
          h("button", {
            type: "button",
            className: "danger",
            textContent: armed === key ? "Delete? Click again" : "Delete",
            title: "Remove the model's files from this computer. Transcriptions it wrote stay.",
            onclick: () => {
              if (armed !== key) {
                armed = key;
                draw();
                window.setTimeout(() => {
                  if (armed === key) {
                    armed = null;
                    draw();
                  }
                }, 4000);
                return;
              }
              armed = null;
              void act(() => assistDelete({ id: m.id }));
            },
          }),
        );
      }
      const hashes = m.installed?.files.map((f) => `${f.name}: ${f.sha256}${f.published ? " (matches the published checksum)" : " (recorded when downloaded)"}`).join("\n");
      return h(
        "div",
        { className: "as-model" },
        h("strong", { textContent: m.name }),
        h("p", { className: "as-note", textContent: m.about }),
        h("p", { className: "as-note", textContent: `${m.license} · ${m.attribution}`, title: hashes ?? m.homepage }),
        h("div", { className: "as-actions" }, ...actions),
      );
    }

    function batchControls(s: ReturnType<typeof getState>, model: AssistModel | null) {
      if (s.kind !== "open" || s.text === null) return h("p", { className: "as-note", textContent: "Open a text to fill its empty segments." });
      if (model === null) return h("p", { className: "as-note", textContent: "Download a model first." });
      const source = mediaSource();
      if (source === null) return h("p", { className: "as-note", textContent: "Load the text's recording on the Timeline first." });
      const text = s.text;
      const tiers = text.tiers.filter((t) => t.kind === "time-aligned" || t.kind === null);
      if (tierChoice === null || !tiers.some((t) => t.id === tierChoice)) tierChoice = tiers.find((t) => t.role === "transcription")?.id ?? tiers[0]?.id ?? null;
      if (tierChoice === null) return h("p", { className: "as-note", textContent: "This text has no time-aligned track." });
      const count = batchTargets(text.segments, tierChoice).length;
      if (run !== null) {
        return h(
          "div",
          { className: "as-actions" },
          h("progress", { max: Math.max(1, run.total), value: run.done }),
          h("span", { className: "as-note", textContent: `${run.done} of ${run.total}` }),
          h("button", { type: "button", textContent: run.stop ? "Stopping…" : "Stop", disabled: run.stop, onclick: () => { if (run !== null) run.stop = true; draw(); } }),
        );
      }
      const picker = h("select", { onchange: () => { tierChoice = picker.value; draw(); } }, ...tiers.map((t) => h("option", { value: t.id, textContent: t.name, selected: t.id === tierChoice })));
      return h(
        "div",
        { className: "as-actions" },
        picker,
        h("button", {
          type: "button",
          textContent: count === 0 ? "No empty segments" : `Suggest for ${count} empty segment${count === 1 ? "" : "s"}`,
          disabled: count === 0,
          title: "Each segment takes a few seconds. You can stop at any time; what was filled stays.",
          onclick: () => void runBatch(text.id, tierChoice!, model.id, source, draw),
        }),
      );
    }

    async function act(f: () => Promise<void>) {
      try {
        await f();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      await reloadModels().catch(() => {});
    }

    function render() {
      if (getState().kind === "closed" && assistModelList().length === 0) {
        el.replaceChildren(emptyState("Speech assist", "Loading…"));
        return;
      }
      draw();
    }
    return { element: el, render };
  },
};
