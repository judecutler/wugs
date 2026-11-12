// Hear it everywhere (roadmap 72): play one segment of any text from any
// panel. The text's linked recording is loaded into the window's one audio
// element when it is not loaded already; the Timeline follows.
import { convertFileSrc } from "@tauri-apps/api/core";
import { errorText, peaksInfo } from "../api";
import * as playback from "../playback";
import { getState, mediaSource, selectSegment, selectText, setMediaSource, showToast } from "../store";
import { h } from "../dom";
import { ICONS } from "../timeline/icons";
import { planHearing, recordingOf } from "./logic";

export type Heard = { textId: string; tierId: string; segmentId: string; startMs: number | null; endMs: number | null };

/** The recording to hear (or transcribe) the text on screen from, or null
 * (`recordingOf`): Suggest and Speech assist ask this, not the loaded one. */
export function recordingForShown(): string | null {
  const s = getState();
  if (s.kind !== "open" || s.text === null) return null;
  const id = s.text.id;
  return recordingOf(s.texts.find((t) => t.id === id), s.texts, mediaSource(), id);
}

/** Open the segment (selecting its text if another is shown) and play it.
 * `select: false` plays without selecting (a panel that already did);
 * `quiet` says nothing when there is no recording (a keystroke's replay). */
export async function hear(seg: Heard, select = true, quiet = false): Promise<void> {
  const s = getState();
  if (s.kind !== "open") return;
  if (select) {
    if (s.text?.id !== seg.textId) await selectText(seg.textId);
    const now = getState();
    // Overtaken by another selection, or the text failed to open: play nothing.
    if (now.kind !== "open" || now.text?.id !== seg.textId) return;
    selectSegment({ textId: seg.textId, tierId: seg.tierId, segmentId: seg.segmentId });
  }
  const now = getState();
  if (now.kind !== "open" || now.text?.id !== seg.textId) return;
  const plan = planHearing(now.texts.find((t) => t.id === seg.textId), now.texts, mediaSource(), now.text.id);
  if (plan.kind === "none") {
    if (!quiet) showToast({ tone: "info", lines: [plan.reason] });
    return;
  }
  if (seg.startMs === null || seg.endMs === null) {
    showToast({ tone: "info", lines: ["This segment has no times, so there is nothing to play."] });
    return;
  }
  if (plan.load !== null) {
    try {
      const info = await peaksInfo({ sourceId: plan.load });
      const after = getState();
      if (after.kind !== "open" || after.text?.id !== seg.textId) return;
      playback.setSource(convertFileSrc(info.mediaPath));
      setMediaSource(plan.load);
    } catch (e) {
      showToast({ tone: "error", lines: [`The recording did not load: ${errorText(e)}`] });
      return;
    }
  }
  playback.playRange(seg.startMs, seg.endMs);
}

/** The ▶ every panel shows beside a segment. It never takes focus from a
 * field (mousedown is held back), and it does not pass its click on. */
export function playButton(title: string, onPlay: () => void): HTMLButtonElement {
  const b = h("button", { type: "button", className: "play-seg", title });
  b.innerHTML = ICONS.play;
  b.setAttribute("aria-label", title);
  b.onmousedown = (ev) => {
    ev.preventDefault();
    ev.stopPropagation();
  };
  b.onclick = (ev) => {
    ev.stopPropagation();
    onPlay();
  };
  return b;
}
