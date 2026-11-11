// The Speech assist panel (roadmap items 27 and 53): download phone models,
// choose which one powers phone transcription (Automatic, a picked model, or
// the project's pin), then fill empty segments with what it hears. Everything runs on this
// computer; the recording never leaves it. Machine transcriptions stay
// tinted until someone edits them (src/assist/logic.ts).
import { open as pickDirectory } from "@tauri-apps/plugin-dialog";
import { assistCancel, assistDelete, assistDownload, assistModels, assistSuggest, capabilityChoose, capabilityPin, correctionsExport, correctionsSummary, errorText, MODEL_CHANGED, type AssistModel, type CapabilityView, type CorrectionsSummary } from "../api";
import { batchTargets, correctionsLine, exportable, megabytes, progressLabel, VERY_UNSURE_BELOW } from "../assist/logic";
import { choiceOptions, pinTarget, statusLine } from "../capability/logic";
import { emptyState, h } from "../dom";
import { assistModelList, capability, getState, mediaSource, refreshCapabilities, refreshText, setAssistModels, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

/** A batch run in progress, shared by every instance of the panel. */
let run: { text: string; tier: string; done: number; total: number; filled: number; skipped: number; stop: boolean } | null = null;
let armed: string | null = null;

const CAP = "phone-transcription";

/** Models and what powers each capability: a download changes both. */
export async function reloadModels() {
  setAssistModels(await assistModels());
  await refreshCapabilities();
}

/** Fill one tier's empty segments one after another; each is its own write, so stopping keeps what was done.
 * The model in effect at the start does the whole run: if it changes meanwhile, the run stops. */
async function runBatch(text: string, tier: string, source: string, expect: string, modelName: string, rerender: () => void) {
  const s = getState();
  if (s.kind !== "open" || s.text === null || run !== null) return;
  const targets = batchTargets(s.text.segments, tier);
  const project = s.info.path;
  run = { text, tier, done: 0, total: targets.length, filled: 0, skipped: 0, stop: false };
  let moved = false;
  rerender();
  const failures: string[] = [];
  let changed: string | null = null;
  for (const seg of targets) {
    if (run.stop) break;
    // The person opened another text or project: stop rather than keep writing out of sight.
    const now = getState();
    if (now.kind !== "open" || now.info.path !== project || now.text?.id !== text) {
      moved = true;
      break;
    }
    try {
      const r = await assistSuggest({ text, segment: seg.id, source, expect });
      if (r.content === "") run.skipped += 1;
      else run.filled += 1;
    } catch (e) {
      const msg = errorText(e);
      if (msg.startsWith(MODEL_CHANGED)) {
        changed = msg;
        break;
      }
      run.skipped += 1;
      if (failures.length < 3) failures.push(msg);
    }
    run.done += 1;
    await refreshText();
    rerender();
  }
  const r = run;
  run = null;
  showToast({
    tone: failures.length > 0 || changed !== null ? "warn" : "info",
    lines: [
      `Filled ${r.filled} of ${r.total} empty segments with ${modelName}${r.stop ? " before stopping" : moved ? " before you switched texts" : changed !== null ? " before the model changed" : ""}.`,
      ...(changed !== null ? [`${changed} Start the run again to continue with it.`] : []),
      ...(r.skipped > 0 ? [`${r.skipped} skipped (no speech heard, or an error).`] : []),
      ...failures,
    ],
  });
  rerender();
}

export const assistPanel: PanelDef = {
  title: "Speech assist",
  create(): PanelView {
    const el = h("div", { className: "panel panel-assist" });
    let tierChoice: string | null = null;
    let corrections: CorrectionsSummary | null = null;
    /** The person's word on consent, given for one project only. */
    let confirmed = false;
    let countedFor: string | null = null;
    let exporting = false;

    function draw() {
      const models = assistModelList().filter((m) => m.capability === CAP);
      const s = getState();
      const phones = capability(CAP);
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
        ...(phones === null ? [] : [h("h4", { textContent: "Which model" }), chooser(phones, s.kind === "open")]),
        h("h4", { textContent: "Fill empty segments" }),
        batchControls(s, phones),
        ...(s.kind === "open" ? [h("h4", { textContent: "Share corrections" }), correctionControls()] : []),
      );
    }

    function modelCard(m: AssistModel) {
      const dl = m.download;
      const actions: (HTMLElement | null)[] = [];
      if (dl !== null && dl.error === null) {
        actions.push(h("progress", { max: Math.max(1, dl.total), value: dl.done }), h("span", { className: "as-note", textContent: progressLabel(dl.done, dl.total) }), h("button", { type: "button", textContent: "Cancel", onclick: () => void act(() => assistCancel({ id: m.id })) }));
      } else if (m.installed === null) {
        actions.push(
          h("button", { type: "button", textContent: m.approxMb === null ? "Download" : `Download (about ${m.approxMb} MB)`, disabled: !m.runsHere, title: m.runsHere ? "" : "This model needs a newer version of wugs.", onclick: () => void act(() => assistDownload({ id: m.id })) }),
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

    /** This computer's choice, the project's pin, and what runs now. */
    function chooser(v: CapabilityView, projectOpen: boolean) {
      const picker = h(
        "select",
        {
          disabled: v.pin !== null || run !== null,
          title: v.pin !== null ? "This project is pinned; unpin it to choose for this computer." : "Automatic picks the best downloaded model.",
          onchange: () => void act(() => capabilityChoose({ capability: v.id, choice: picker.value })),
        },
        ...choiceOptions(v).map((o) => h("option", { value: o.value, textContent: o.label, selected: o.value === v.machine, disabled: o.disabled })),
      );
      const target = pinTarget(v);
      const pinButton = !projectOpen || run !== null
        ? null
        : v.pin !== null
          ? h("button", { type: "button", textContent: "Unpin", title: "Let each computer choose again.", onclick: () => void act(() => capabilityPin({ capability: v.id, provider: "" })) })
          : target !== null
            ? h("button", { type: "button", textContent: "Pin for this project", title: "Everyone on this project uses this model, so a long transcription campaign is not switched halfway.", onclick: () => void act(() => capabilityPin({ capability: v.id, provider: target })) })
            : null;
      return h(
        "div",
        { className: "as-choice" },
        h("div", { className: "as-actions" }, picker, pinButton),
        h("p", { className: v.problem === null ? "as-note" : "as-error", textContent: statusLine(v) }),
      );
    }

    function batchControls(s: ReturnType<typeof getState>, phones: CapabilityView | null) {
      if (s.kind !== "open" || s.text === null) return h("p", { className: "as-note", textContent: "Open a text to fill its empty segments." });
      if (phones?.effective == null) return h("p", { className: "as-note", textContent: phones?.problem ?? "Download a model first." });
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
          onclick: () => void runBatch(text.id, tierChoice!, source, phones.effective!, phones.options.find((o) => o.id === phones.effective)?.name ?? phones.effective!, draw),
        }),
      );
    }

    /** Export what people checked, so models can be tested on (and learn from) the languages documented here. */
    function correctionControls() {
      if (corrections === null) return h("p", { className: "as-note", textContent: "Counting…" });
      const c = corrections;
      const vouch = confirmed && c.recordsOnly === null;
      const count = exportable(c, vouch);
      const consent = c.unrecorded === 0
        ? null
        : c.recordsOnly !== null
          ? h("p", { className: "as-note", textContent: c.recordsOnly })
          : h(
            "label",
            { className: "as-note" },
            h("input", { type: "checkbox", checked: confirmed, onchange: (e: Event) => { confirmed = (e.target as HTMLInputElement).checked; draw(); } }),
            ` I have the speakers' consent to use ${c.unrecorded === 1 ? "this recording" : `these ${c.unrecorded} recordings`} for training and testing speech models (no consent record in this project says either way).`,
          );
      return h(
        "div",
        { className: "as-choice" },
        h("p", { className: "as-note", textContent: "Each suggestion you checked pairs what the model heard with what you wrote. Exported as a folder of clips, it measures how well models do on this language and can help train better ones. Nothing is sent anywhere: you choose where the folder goes." }),
        h("p", { className: "as-note", textContent: correctionsLine(c) }),
        consent,
        h("div", { className: "as-actions" }, h("button", {
          type: "button",
          textContent: exporting ? "Exporting…" : count === 0 ? "Export corrections" : `Export ${count} correction${count === 1 ? "" : "s"}…`,
          disabled: count === 0 || exporting,
          onclick: () => void exportCorrections(),
        })),
      );
    }

    async function exportCorrections() {
      const folder = await pickDirectory({ directory: true, title: "Choose where the corrections folder goes" });
      if (typeof folder !== "string") return;
      exporting = true;
      draw();
      try {
        const r = await correctionsExport({ folder, unrecorded: confirmed && corrections?.recordsOnly === null });
        showToast({
          tone: r.failed > 0 ? "warn" : "info",
          lines: [`Exported ${r.written} correction${r.written === 1 ? "" : "s"} to ${r.folder}.`, ...(r.failed > 0 ? [`${r.failed} could not be cut from the recording.`, ...r.problems] : [])],
        });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      exporting = false;
      draw();
    }

    /** Recount after a render, one count at a time; a render during a count asks for one more. */
    let counting = false;
    let again = false;
    async function refreshCorrections() {
      const st = getState();
      if (st.kind !== "open") return;
      // Another project: its consent is a new question, and the old count is not its own.
      if (st.info.path !== countedFor) {
        countedFor = st.info.path;
        confirmed = false;
        corrections = null;
      }
      if (counting) {
        again = true;
        return;
      }
      counting = true;
      try {
        corrections = await correctionsSummary();
      } catch {
        corrections = null;
      }
      counting = false;
      draw();
      if (again) {
        again = false;
        void refreshCorrections();
      }
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
      void refreshCorrections();
    }
    return { element: el, render };
  },
};
