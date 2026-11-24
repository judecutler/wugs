import { token } from "../theme/tokens";
import { THEME_EVENT } from "../theme/theme";
import { captureRecoveredDiscard, captureRecoveredImport, captureRecoveredList, errorText, listSources, mediaOverview, recordingLevels, sourceSetDetail, type Recovered, type SourceRow } from "../api";
import { CARD_TYPE, cardDrag, cards, detailsLine, duration, normalized, recordingDetailKeys, recordingDetailLabel, wavePath, type Card } from "../media/logic";
import { canRecord } from "../capture/recorder.ts";
import { parsePattern, readName, writesFor } from "../media/names";
import { dbLabel, levelNotes, levelsTsv, type Levels } from "../media/levels";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
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
import { activeKit, attempt, dataGeneration, getState, refreshAll, showToast, useRecording } from "../store";
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
    // Details read from the recordings' file names by a pattern.
    const NAME_KEY = "wugs.media.namePattern";
    const namePattern = h("input", { type: "text", className: "bin-pattern", ariaLabel: "File name pattern", placeholder: "{date}_{participants}_{genre}" });
    try {
      namePattern.value = localStorage.getItem(NAME_KEY) ?? "{date}_{participants}_{genre}";
    } catch {
      namePattern.value = "{date}_{participants}_{genre}";
    }
    const replaceBox = h("input", { type: "checkbox" });
    const nameNote = h("p", { className: "note" });
    const namePreview = h("table", { className: "pg-table bin-names" });
    const fillNames = h("button", { type: "button", textContent: "Fill details" });
    const namesBox = h(
      "details",
      { className: "bin-from-names" },
      h("summary", { textContent: "Details from file names" }),
      h("p", { className: "note", textContent: "Write how your files are named, with the details in braces: {date}, {place}, {participants}, {genre}, {recorded_by}, any other name, or {skip} for a part to leave out. 20260312_Ana_story.wav under {date}_{participants}_{genre} gives the date 2026-03-12, Ana and story." }),
      h("div", { className: "cc-bar" }, namePattern, h("label", { title: "Write over details already filled in" }, replaceBox, " Replace filled details"), fillNames),
      nameNote,
      h("div", { className: "pg-rows" }, namePreview),
    );
    /** Each recording with audio, what the pattern reads from its name, and what would be written. */
    const nameReads = () => {
      const p = parsePattern(namePattern.value);
      if ("error" in p) return { error: p.error, rows: [] as { src: SourceRow; read: [string, string][] | null; writes: [string, string][] }[] };
      return {
        error: null,
        keys: p.keys,
        rows: sources.filter((x) => x.kind === "audio" && x.hasMedia).map((src) => {
          const read = readName(p, src.label);
          return { src, read, writes: read === null ? [] : writesFor(read, src.details, replaceBox.checked) };
        }),
      };
    };
    function drawNames() {
      const r = nameReads();
      if (r.error !== null) {
        nameNote.textContent = r.error;
        namePreview.replaceChildren();
        fillNames.disabled = true;
        return;
      }
      const keys = r.keys ?? [];
      const fits = r.rows.filter((x) => x.read !== null).length;
      const writes = r.rows.reduce((n, x) => n + x.writes.length, 0);
      nameNote.textContent = r.rows.length === 0 ? "No recordings yet." : `${fits} of ${r.rows.length} file names fit the pattern; ${writes} ${writes === 1 ? "detail" : "details"} would be filled.`;
      fillNames.disabled = writes === 0;
      namePreview.replaceChildren(
        h("thead", {}, h("tr", {}, h("th", { scope: "col", textContent: "Recording" }), ...keys.map((k) => h("th", { scope: "col", textContent: recordingDetailLabel(k) })))),
        h(
          "tbody",
          {},
          ...r.rows.slice(0, 50).map((x) =>
            h(
              "tr",
              { className: x.read === null ? "bin-nofit" : "" },
              h("th", { scope: "row", textContent: x.src.label || "(unnamed)" }),
              ...(x.read === null
                ? [h("td", { colSpan: Math.max(1, keys.length), className: "note", textContent: "does not fit" })]
                : keys.map((k) => {
                    const v = x.read!.find(([d]) => d === k)?.[1] ?? "";
                    const writing = x.writes.some(([d]) => d === k);
                    return h("td", { className: writing ? "bin-new" : "", textContent: v, title: writing ? "Will be filled" : "Already so, or kept" });
                  })),
            ),
          ),
        ),
      );
    }
    namePattern.oninput = () => {
      try {
        localStorage.setItem(NAME_KEY, namePattern.value);
      } catch {
        /* kept for this session */
      }
      drawNames();
    };
    replaceBox.onchange = drawNames;
    fillNames.onclick = async () => {
      const r = nameReads();
      if (r.error !== null) return;
      fillNames.disabled = true;
      let written = 0;
      try {
        for (const x of r.rows)
          for (const [key, value] of x.writes) {
            sources = await sourceSetDetail({ source: x.src.id, key, value });
            written += 1;
          }
        showToast({ tone: "info", lines: [`Filled ${written} ${written === 1 ? "detail" : "details"} from the file names.`] });
      } catch (e) {
        showToast({ tone: "error", lines: [`Filled ${written} before a problem: ${errorText(e)}`] });
      }
      drawList();
    };
    // Recording levels: peak, loudness, the room's noise, clipping.
    const levelsNote = h("p", { className: "note" });
    const levelsTable = h("table", { className: "pg-table bin-levels" });
    const checkLevels = h("button", { type: "button", textContent: "Check levels", title: "Read every recording here once: its peak, how loud the speech is against the room's noise, and where it clips" });
    const copyLevels = h("button", { type: "button", textContent: "Copy table", disabled: true });
    let levelRows: { name: string; levels: Levels }[] = [];
    const levelsBox = h(
      "details",
      { className: "bin-levels-box" },
      h("summary", { textContent: "Recording levels" }),
      h("p", { className: "note", textContent: "How well each recording was made: the loudest moment, the speech level and the room's noise under it (in dB below full scale), and where it clips. Worth a look before analysing pitch or formants, and before depositing." }),
      h("div", { className: "cc-bar" }, checkLevels, copyLevels),
      levelsNote,
      h("div", { className: "pg-rows" }, levelsTable),
    );
    const drawLevels = () => {
      const flagged = levelRows.filter((r) => levelNotes(r.levels).length > 0).length;
      levelsNote.textContent = levelRows.length === 0 ? "" : flagged === 0 ? `All ${levelRows.length} recordings read look fine.` : `${flagged} of ${levelRows.length} recordings have something to look at.`;
      copyLevels.disabled = levelRows.length === 0;
      levelsTable.replaceChildren(
        ...(levelRows.length === 0
          ? []
          : [
              h("thead", {}, h("tr", {}, ...["Recording", "Length", "Peak", "Speech", "Room", "Clips", "Notes"].map((t) => h("th", { scope: "col", textContent: t })))),
              h(
                "tbody",
                {},
                ...levelRows.map(({ name, levels: l }) => {
                  const notes = levelNotes(l);
                  return h(
                    "tr",
                    { className: notes.length > 0 ? "bin-flag" : "" },
                    h("th", { scope: "row", textContent: name }),
                    h("td", { textContent: duration(l.durationMs) }),
                    h("td", { className: "num", textContent: dbLabel(l.peakDb) }),
                    h("td", { className: "num", textContent: dbLabel(l.speechDb) }),
                    h("td", { className: "num", textContent: dbLabel(l.floorDb) }),
                    h("td", { className: "num", textContent: String(l.clips) }),
                    h("td", { textContent: notes.length === 0 ? "Fine" : notes.join(" ") }),
                  );
                }),
              ),
            ]),
      );
    };
    checkLevels.onclick = async () => {
      checkLevels.disabled = true;
      levelRows = [];
      const audio = sources.filter((x) => x.kind === "audio" && x.hasMedia);
      let missing = 0;
      try {
        for (const [i, src] of audio.entries()) {
          levelsNote.textContent = `Reading ${i + 1} of ${audio.length}…`;
          const l = await recordingLevels({ sourceId: src.id });
          if (l === null) missing += 1;
          else levelRows.push({ name: src.label || "(unnamed)", levels: l });
          drawLevels();
        }
      } catch (e) {
        showToast({ tone: "error", lines: [`Could not read the levels: ${errorText(e)}`] });
      } finally {
        checkLevels.disabled = false;
      }
      drawLevels();
      if (missing > 0) levelsNote.textContent += ` ${missing} ${missing === 1 ? "recording is" : "recordings are"} not on this computer.`;
    };
    copyLevels.onclick = () =>
      void writeText(levelsTsv(levelRows)).then(
        () => showToast({ tone: "info", lines: ["Copied the recording levels as a table."] }),
        (e) => showToast({ tone: "error", lines: [errorText(e)] }),
      );
    el.append(bar, recoveredBox, list, namesBox, levelsBox);
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

    // A card replaced mid-drag never gets its dragend, so redraws wait for the drag to end.
    let redrawAfterDrag = false;
    let redrawTimer = 0;
    /** The recording whose details are open under the cards. */
    let detailsFor: string | null = null;
    let detailsPending = false;
    list.addEventListener("focusout", () =>
      window.setTimeout(() => {
        if (detailsPending && !list.contains(document.activeElement)) {
          detailsPending = false;
          drawList();
        }
      }),
    );
    const detailsSection = (): HTMLElement | null => {
      const src = sources.find((x) => x.id === detailsFor);
      if (src === undefined) return null;
      const close = h("button", { type: "button", textContent: "Close", onclick: () => ((detailsFor = null), drawList()) });
      return h(
        "section",
        { className: "bin-details", ariaLabel: `Details of ${src.label}` },
        h("div", { className: "bin-details-head" }, h("h3", { textContent: `Details of «${src.label || "(unnamed recording)"}»` }), close),
        h("p", { className: "note", textContent: "What an archive asks about a recording. The catalogue sheet uses its date and place for a text that has none of its own." }),
        ...recordingDetailKeys(src.details).map((key) => {
          const had = src.details.find(([k]) => k === key)?.[1] ?? "";
          const field = h("input", { value: had, className: "bin-detail", ariaLabel: recordingDetailLabel(key), placeholder: key === "date" ? "YYYY-MM-DD" : "" });
          field.dataset.key = key;
          let sent = had;
          const save = () => {
            const next = field.value.trim();
            if (next === sent) return;
            sent = next;
            void attempt(async () => {
              sources = await sourceSetDetail({ source: src.id, key, value: next });
              // Saving on leaving a field must not redraw under the next one.
              if (list.contains(document.activeElement)) detailsPending = true;
              else drawList();
            });
          };
          field.onkeydown = (e) => {
            if (e.key === "Enter") save();
          };
          field.onblur = save;
          return h("label", { className: "bin-detail-row" }, h("span", { textContent: recordingDetailLabel(key) }), field);
        }),
      );
    };

    const drawList = () => {
      drawNames();
      if (cardDrag.active()) {
        redrawAfterDrag = true;
        // A dragend the webview lost would leave the bin stale: look again once the drag has lapsed.
        window.clearTimeout(redrawTimer);
        redrawTimer = window.setTimeout(drawList, cardDrag.remaining() + 50);
        return;
      }
      window.clearTimeout(redrawTimer);
      redrawAfterDrag = false;
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
        detailsSection() ?? "",
        others.length === 0
          ? ""
          : h("p", { className: "note bin-others", textContent: `Also in the project, without audio here: ${others.map((x) => `${x.label || x.id} (${x.access ?? "open"})`).join(", ")}.` }),
      );
    };

    /** Waveforms by recording, for this project, asked for one at a time
     * (each is read, or first computed, off the session thread). */
    let fetchedFor = "";
    const waves = new Map<string, { path: string; durationMs: number } | "waiting" | "missing" | "failed">();
    const queue: string[] = [];
    let fetching = false;
    let wavesFor = "";
    async function fetchWaves() {
      if (fetching) return;
      fetching = true;
      const project = wavesFor;
      try {
        for (let id = queue.shift(); id !== undefined && project === wavesFor; id = queue.shift()) {
          try {
            const o = await mediaOverview({ sourceId: id });
            if (project !== wavesFor) break;
            if (o.state === "missing") waves.set(id, "missing");
            else {
              const n = normalized(o.mins, o.maxs);
              waves.set(id, { path: wavePath(n.mins, n.maxs, WAVE_W, WAVE_H), durationMs: o.durationMs });
            }
          } catch {
            if (project !== wavesFor) break;
            waves.set(id, "failed");
          }
          drawList();
        }
      } finally {
        fetching = false;
      }
      if (project !== wavesFor && queue.length > 0) void fetchWaves();
    }
    function wave(id: string) {
      if (!waves.has(id)) {
        waves.set(id, "waiting");
        queue.push(id);
        void fetchWaves();
      }
      return waves.get(id)!;
    }

    function card(c: Card, shown: string | null): HTMLElement {
      const w = wave(c.id);
      const here = w !== "missing" && w !== "failed";
      const art = h("div", { className: "bin-wave" });
      if (typeof w === "object") art.innerHTML = `<svg viewBox="0 0 ${WAVE_W} ${WAVE_H}" preserveAspectRatio="none" aria-hidden="true"><path d="${w.path}"/></svg>`;
      else if (!here) art.append(h("span", { className: "bin-wave-note", textContent: w === "missing" ? "Audio not on this computer" : "Could not read the audio" }));
      const kit = activeKit();
      const timeline = kit === null || kit.tools.includes("timeline");
      const blocker = !here ? "Its audio is not on this computer" : !timeline ? `${kit?.name ?? "This kit"} has no Timeline` : shown === null ? "Select a text first" : null;
      const use = () => {
        if (blocker !== null || c.inThisText) return;
        useRecording(c.id);
        window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "timeline" }));
      };
      const where = c.inThisText ? "In this text" : c.usedBy.length > 0 ? `In ${c.usedBy.map((t) => `«${t}»`).join(", ")}` : "Not in a text yet";
      const button = h("button", {
        type: "button",
        textContent: c.inThisText ? "In use" : "Use for this text",
        disabled: c.inThisText || blocker !== null,
        title: blocker ?? "Load it on the Timeline for the text on screen; the text remembers it",
      });
      button.setAttribute("aria-label", c.inThisText ? `${c.label}: in use for this text` : `Use ${c.label} for this text`);
      button.onclick = use;
      // A double-click on the button is two clicks, not also the card's double-click.
      button.ondblclick = (e) => e.stopPropagation();
      const el = h(
        "div",
        {
          className: (c.inThisText ? "bin-card is-current" : "bin-card") + (here ? "" : " is-missing"),
          role: "listitem",
          draggable: blocker === null,
          title: blocker === null ? `${c.label}: drag onto the Timeline, or double-click, to use it for the text on screen` : `${c.label}: ${blocker}`,
        },
        art,
        h("div", { className: "bin-name", textContent: c.label }),
        h("div", { className: "bin-meta" }, h("span", { className: "bin-time", textContent: typeof w === "object" ? duration(w.durationMs) : "" }), h("span", { className: `access access-${c.access}`, textContent: c.access })),
        h("div", { className: "note bin-where", textContent: where }),
        h("button", {
          type: "button",
          className: "link-button bin-details-open",
          textContent: detailsLine(sources.find((x) => x.id === c.id)?.details ?? []),
          title: "Date, place, who recorded it and more, for the archive",
          ondblclick: (e: MouseEvent) => e.stopPropagation(),
          onclick: () => {
            detailsFor = detailsFor === c.id ? null : c.id;
            drawList();
            list.querySelector<HTMLInputElement>(".bin-detail")?.focus();
          },
        }),
        button,
      );
      el.ondragstart = (e) => {
        cardDrag.start();
        e.dataTransfer?.setData(CARD_TYPE, c.id);
        if (e.dataTransfer !== null) e.dataTransfer.effectAllowed = "link";
      };
      el.ondragend = () => {
        cardDrag.end();
        if (redrawAfterDrag) drawList();
      };
      el.ondblclick = use;
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
        queue.length = 0;
        // The last project's cards would be drawn and asked for against this one.
        sources = [];
      } else {
        // Any redraw (new data, another text) asks again for audio that was missing.
        for (const [id, w] of waves) if (w === "missing" || w === "failed") waves.delete(id);
      }
      drawList();
      drawBar();
      // A text switch only moves the "In this text" mark: the list is fetched
      // again only when the project or its data changed.
      const fetchKey = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}` : "closed";
      if (fetchKey === fetchedFor) return;
      fetchedFor = fetchKey;
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
