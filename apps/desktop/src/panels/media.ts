import { token } from "../theme/tokens";
import { THEME_EVENT } from "../theme/theme";
import { captureRecoveredDiscard, captureRecoveredImport, captureRecoveredList, errorText, listSources, peaksInfo, peaksQuery, type Recovered, type SourceRow } from "../api";
import { CARD_TYPE, cards, duration, normalized, wavePath, type Card } from "../media/logic";
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
import { attempt, dataGeneration, getState, refreshAll, showToast, useRecording } from "../store";
import { SHOW_PANEL, sliced, type PanelDef } from "./panel";

/** A card's waveform box, in SVG units. */
const WAVE_W = 120;
const WAVE_H = 32;

const MIC_KEY = "wugs.mic";

// The media bin: record straight into the project, and list its sources.
// The take itself lives in capture/session.ts, so this panel can be closed
// and reopened mid-recording; it only draws what that module holds.
// Recordings show as cards (docs/design/EDITOR.md): a waveform, the name,
// the length, the sharing level and which texts use it; drag one onto the
// Timeline, double-click it, or click Use, to use it for the text on screen.
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
    const readMeter = () => ({ ok: token("--meter-ok", "#4d7c5a"), clip: token("--meter-clip", "#b3402f"), bg: token("--meter-bg", "#222") });
    let meterColors = readMeter();
    const onTheme = () => (meterColors = readMeter());
    window.addEventListener(THEME_EVENT, onTheme);
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
      const st = getState();
      const texts = st.kind === "open" ? st.texts : [];
      const shown = st.kind === "open" ? (st.text?.id ?? null) : null;
      const bin = cards(sources, texts, shown);
      const others = sources.filter((x) => !(x.kind === "audio" && x.hasMedia));
      list.replaceChildren(
        bin.length === 0 ? emptyState("No recordings yet", "Record above, import one on the Timeline, or drop a recording here.", "Nothing for the wug to listen to yet.") : h("div", { className: "bin-grid", role: "list" }, ...bin.map((c) => card(c, shown))),
        others.length === 0
          ? ""
          : h("p", { className: "note bin-others", textContent: `Also in the project, without audio here: ${others.map((x) => x.label || x.id).join(", ")}.` }),
      );
    };

    /** Waveforms by recording, for this project: drawn once, from the coarsest peaks. */
    const waves = new Map<string, { path: string; durationMs: number } | "loading" | "failed">();
    let wavesFor = "";
    function wave(id: string, into: HTMLElement, time: HTMLElement) {
      const done = waves.get(id);
      const paint = (w: { path: string; durationMs: number }) => {
        into.innerHTML = `<svg viewBox="0 0 ${WAVE_W} ${WAVE_H}" preserveAspectRatio="none" aria-hidden="true"><path d="${w.path}"/></svg>`;
        time.textContent = duration(w.durationMs);
      };
      if (typeof done === "object") return paint(done);
      if (done !== undefined) return;
      waves.set(id, "loading");
      const project = wavesFor;
      void (async () => {
        try {
          const info = await peaksInfo({ sourceId: id });
          const slice = await peaksQuery({ sourceId: id, channel: 0, startFrame: 0, endFrame: info.frames, pixels: WAVE_W });
          if (project !== wavesFor) return;
          const n = normalized(slice.mins, slice.maxs);
          waves.set(id, { path: wavePath(n.mins, n.maxs, WAVE_W, WAVE_H), durationMs: info.durationMs });
        } catch {
          if (project !== wavesFor) return;
          waves.set(id, "failed");
        }
        drawList();
      })();
    }

    function card(c: Card, shown: string | null): HTMLElement {
      const art = h("div", { className: "bin-wave" });
      const time = h("span", { className: "bin-time" });
      wave(c.id, art, time);
      const use = () => {
        useRecording(c.id);
        window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "timeline" }));
      };
      const where = c.inThisText ? "In this text" : c.usedBy.length > 0 ? `In ${c.usedBy.map((t) => `«${t}»`).join(", ")}` : "Not in a text yet";
      const button = h("button", {
        type: "button",
        textContent: c.inThisText ? "In use" : "Use for this text",
        disabled: c.inThisText || shown === null,
        title: shown === null ? "Select a text first" : "Load it on the Timeline for the text on screen; the text remembers it",
      });
      button.onclick = use;
      const el = h(
        "div",
        { className: c.inThisText ? "bin-card is-current" : "bin-card", role: "listitem", draggable: true, title: `${c.label}: drag onto the Timeline, or double-click, to use it for the text on screen` },
        art,
        h("div", { className: "bin-name", textContent: c.label }),
        h("div", { className: "bin-meta" }, time, h("span", { className: `access access-${c.access}`, textContent: c.access })),
        h("div", { className: "note bin-where", textContent: where }),
        button,
      );
      el.ondragstart = (e) => {
        e.dataTransfer?.setData(CARD_TYPE, c.id);
        if (e.dataTransfer !== null) e.dataTransfer.effectAllowed = "link";
      };
      el.ondblclick = () => {
        if (shown !== null && !c.inThisText) use();
      };
      return el;
    }

    const slice = () => {
      const s = getState();
      return s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}|${s.text?.id ?? ""}|${s.texts.map((t) => `${t.id}:${t.title}:${t.recording ?? ""}`).join(",")}` : "closed";
    };
    const view = sliced(el, slice, () => {
      const s = getState();
      const project = s.kind === "open" ? s.info.projectId : "";
      if (project !== wavesFor) {
        wavesFor = project;
        waves.clear();
      }
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
      window.removeEventListener(THEME_EVENT, onTheme);
    };
    return { ...view, dispose };
  },
};
