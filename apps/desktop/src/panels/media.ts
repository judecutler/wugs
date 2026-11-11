import { token } from "../theme/tokens";
import { captureRecoveredDiscard, captureRecoveredImport, captureRecoveredList, errorText, listSources, type Recovered, type SourceRow } from "../api";
import { canRecord } from "../capture/recorder.ts";
import { defaultLabel, formatElapsed, meterFill } from "../capture/pcm.ts";
import {
  armDiscard,
  decayLevel,
  disarmDiscard,
  discardTake,
  getTake,
  keepTake,
  renameTake,
  saveTake,
  startTake,
  stopTake,
  subscribeTake,
  takeElapsedMs,
  takeLevel,
  takeStartError,
} from "../capture/session.ts";
import { isLive } from "../capture/take.ts";
import { emptyState, h } from "../dom";
import { attempt, dataGeneration, getState, refreshAll, showToast } from "../store";
import { sliced, type PanelDef } from "./panel";

const MIC_KEY = "wugs.mic";

// The media bin: record straight into the project, and list its sources.
// The take itself lives in capture/session.ts, so this panel can be closed
// and reopened mid-recording; it only draws what that module holds.
// Speaker and recording date join the row once the Person and Session
// workflows land; dragging onto the timeline comes with step 2.
export const mediaPanel: PanelDef = {
  title: "Media",
  create() {
    const el = h("div", { className: "panel panel-media" });
    const bar = h("div", { className: "rec-bar" });
    const recoveredBox = h("div", { className: "rec-recovered" });
    const list = h("div");
    el.append(bar, recoveredBox, list);
    let sources: SourceRow[] = [];
    let recovered: Recovered[] = [];
    let recoveredError: string | null = null;
    const recoveredLabels = new Map<string, string>();
    let discardArmedId: string | null = null;
    let mics: MediaDeviceInfo[] = [];
    let raf = 0;

    const meter = h("canvas", { className: "rec-meter", width: 240, height: 28 });
    const time = h("span", { className: "rec-time" });
    const meterColors = { ok: token("--meter-ok", "#4d7c5a"), clip: token("--meter-clip", "#b3402f"), bg: token("--meter-bg", "#222") };
    const drawMeter = () => {
      const g = meter.getContext("2d");
      if (g === null) return;
      const { width, height } = meter;
      const level = takeLevel();
      g.clearRect(0, 0, width, height);
      g.fillStyle = level.peak > 0.99 ? meterColors.clip : meterColors.ok;
      g.fillRect(0, 0, meterFill(level.rms) * width, height);
      g.fillStyle = meterColors.bg;
      g.fillRect(Math.min(width - 2, meterFill(level.peak) * width), 0, 2, height);
      decayLevel();
      if (getTake().kind === "recording") {
        time.textContent = formatElapsed(takeElapsedMs());
        raf = requestAnimationFrame(drawMeter);
      }
    };

    const refreshMics = async () => {
      if (!canRecord()) return;
      mics = (await navigator.mediaDevices.enumerateDevices()).filter((d) => d.kind === "audioinput");
      if (getTake().kind === "idle") drawBar();
    };
    const onDeviceChange = () => void refreshMics();
    navigator.mediaDevices?.addEventListener?.("devicechange", onDeviceChange);
    void refreshMics();

    const start = async () => {
      if (getState().kind !== "open") return;
      const device = localStorage.getItem(MIC_KEY);
      await startTake(device === "" ? null : device);
      // Labels of the inputs appear only once permission is granted.
      void refreshMics();
    };

    /** The message and armed state the naming row was last drawn with. */
    let pendingSig: string | null = null;

    function drawBar() {
      const take = getTake();
      cancelAnimationFrame(raf);
      if (take.kind !== "pending") pendingSig = null;
      if (getState().kind === "closed" && !isLive(take)) {
        bar.replaceChildren();
        return;
      }
      const note = (text: string | null) => (text === null ? "" : h("p", { className: "note error-text rec-error", textContent: text }));
      switch (take.kind) {
        case "idle":
        case "saved":
        case "discarded":
        case "starting": {
          if (!canRecord()) {
            bar.replaceChildren(h("p", { className: "note", textContent: "This system's web view cannot record audio. Import a recording (WAV, M4A, MP3, FLAC) from the timeline instead." }));
            return;
          }
          const starting = take.kind === "starting";
          const record = h("button", { className: "rec-start", textContent: starting ? "Starting…" : "Record", disabled: starting });
          record.onclick = () => void start();
          const picker = h(
            "select",
            { title: "Microphone", disabled: starting },
            h("option", { value: "", textContent: "Default input" }),
            ...mics.filter((m) => m.deviceId !== "default" && m.deviceId !== "").map((m) => h("option", { value: m.deviceId, textContent: m.label || "Microphone" })),
          );
          picker.value = localStorage.getItem(MIC_KEY) ?? "";
          picker.onchange = () => localStorage.setItem(MIC_KEY, picker.value);
          bar.replaceChildren(record, ...(mics.length > 1 ? [picker] : []), note(takeStartError()));
          return;
        }
        case "recording": {
          const stopButton = h("button", { textContent: "Stop" });
          stopButton.onclick = () => void stopTake();
          time.textContent = formatElapsed(takeElapsedMs());
          bar.replaceChildren(h("span", { className: "rec-dot", textContent: "●" }), time, meter, stopButton);
          raf = requestAnimationFrame(drawMeter);
          return;
        }
        case "stopping":
          bar.replaceChildren(h("span", { className: "note", textContent: "Finishing the recording…" }));
          return;
        case "saving":
          bar.replaceChildren(h("span", { className: "note", textContent: `Saving "${take.name}"…` }));
          return;
        case "pending": {
          // A rename changes only the name, which the field already shows:
          // redrawing would replace the field under the typist and select
          // its text, so the next key would overwrite the whole name.
          const sig = `${take.message ?? ""}\u0000${take.armed}`;
          if (pendingSig === sig && bar.querySelector("input.rec-label") !== null) return;
          pendingSig = sig;
          const label = h("input", { className: "rec-label", type: "text", value: take.name, ariaLabel: "Recording name" });
          const saveButton = h("button", { className: "primary", textContent: "Save" });
          label.oninput = () => renameTake(label.value);
          saveButton.onclick = () => void saveTake();
          label.onkeydown = (e) => {
            if (e.key === "Enter") void saveTake();
          };
          const keep = h("button", { textContent: "Keep for later", title: "Leave the audio on disk; it comes back under Recovered recordings when this project is open" });
          keep.onclick = () => void keepTake();
          const drop = h("button", { className: "danger", textContent: take.armed ? "Really discard?" : "Discard" });
          drop.onclick = () => (take.armed ? void discardTake() : armDiscard());
          // Disarm a discard the user walked away from.
          if (take.armed) drop.onblur = () => disarmDiscard();
          bar.replaceChildren(h("span", { className: "rec-time", textContent: formatElapsed(takeElapsedMs()) }), label, saveButton, keep, drop, note(take.message));
          if (take.message === null && !take.armed) {
            label.focus();
            label.select();
          }
        }
      }
    }

    const loadRecovered = async () => {
      if (getState().kind !== "open") {
        recovered = [];
        drawRecovered();
        return;
      }
      try {
        recovered = await captureRecoveredList();
        recoveredError = null;
      } catch (e) {
        recoveredError = errorText(e);
      }
      drawRecovered();
    };

    const importRecovered = async (r: Recovered) => {
      const label = recoveredLabels.get(r.id) ?? defaultLabel(new Date(r.modifiedMs));
      try {
        await captureRecoveredImport({ id: r.id, label });
        showToast({ tone: "info", lines: ["Recovered recording saved to the media bin."] });
        recoveredError = null;
        await refreshAll();
      } catch (e) {
        recoveredError = errorText(e);
      }
      await loadRecovered();
    };

    const discardRecovered = async (r: Recovered) => {
      if (discardArmedId !== r.id) {
        discardArmedId = r.id;
        drawRecovered();
        return;
      }
      discardArmedId = null;
      try {
        await captureRecoveredDiscard({ id: r.id });
        recoveredError = null;
      } catch (e) {
        recoveredError = errorText(e);
      }
      await loadRecovered();
    };

    function drawRecovered() {
      if (recovered.length === 0 && recoveredError === null) {
        recoveredBox.replaceChildren();
        return;
      }
      recoveredBox.replaceChildren(
        h("h3", { textContent: "Recovered recordings" }),
        h("p", { className: "note", textContent: "These were still on disk when the app last closed. Nothing is imported or deleted until you choose." }),
        ...recovered.map((r) => {
          const label = h("input", { className: "rec-label", type: "text", value: recoveredLabels.get(r.id) ?? defaultLabel(new Date(r.modifiedMs)), ariaLabel: "Recording name" });
          label.oninput = () => recoveredLabels.set(r.id, label.value);
          const armed = discardArmedId === r.id;
          const importButton = h("button", { className: "primary", textContent: "Import" });
          importButton.onclick = () => void importRecovered(r);
          const drop = h("button", { className: "danger", textContent: armed ? "Really discard?" : "Discard" });
          drop.onclick = () => void discardRecovered(r);
          if (armed) drop.onblur = () => ((discardArmedId = null), drawRecovered());
          return h("div", { className: "rec-bar" }, h("span", { className: "rec-time", textContent: formatElapsed(r.durationMs) }), label, importButton, drop);
        }),
        recoveredError === null ? "" : h("p", { className: "note error-text rec-error", textContent: recoveredError }),
      );
    }

    const drawList = () => {
      if (getState().kind === "closed") {
        list.replaceChildren(emptyState("Media bin", "Recordings of the open project appear here."));
        return;
      }
      if (sources.length === 0) {
        list.replaceChildren(
          emptyState("No recordings yet", "Record above, or import sources from ELAN or FLEx.", "Nothing for the wug to listen to yet."),
        );
        return;
      }
      list.replaceChildren(
        h(
          "table",
          { className: "dense" },
          h("thead", {}, h("tr", {}, h("th", { textContent: "Source" }), h("th", { textContent: "Kind" }), h("th", { textContent: "Access" }))),
          h(
            "tbody",
            {},
            ...sources.map((src) =>
              h(
                "tr",
                { title: src.hasMedia ? src.id : `${src.id} (no media file attached)` },
                h("td", { textContent: src.label || "(unlabelled)" }),
                h("td", { textContent: src.kind ?? "" }),
                h("td", {}, h("span", { className: `access access-${src.access ?? "open"}`, textContent: src.access ?? "open" })),
              ),
            ),
          ),
        ),
      );
    };
    const slice = () => {
      const s = getState();
      return s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}` : "closed";
    };
    const view = sliced(el, slice, () => {
      const s = getState();
      drawList();
      drawBar();
      if (s.kind === "open") {
        void attempt(async () => {
          sources = await listSources();
          drawList();
        });
        void loadRecovered();
      } else {
        sources = [];
        void loadRecovered();
      }
    });
    // A take that just ended (saved, kept, discarded) changes what is on disk.
    let wasLive = false;
    const unsubscribeTake = subscribeTake(() => {
      drawBar();
      const live = isLive(getTake());
      if (wasLive && !live) void loadRecovered();
      wasLive = live;
    });
    wasLive = isLive(getTake());
    drawBar();
    // Closing this panel must not touch a recording: only the listener goes.
    const dispose = () => {
      view.dispose?.();
      unsubscribeTake();
      navigator.mediaDevices?.removeEventListener?.("devicechange", onDeviceChange);
      cancelAnimationFrame(raf);
    };
    return { ...view, dispose };
  },
};
