// Hear it everywhere (roadmap 72): play one segment of any text from any
// panel. The text's linked recording is loaded into the window's one audio
// element when it is not loaded already; the Timeline follows.
import { convertFileSrc } from "@tauri-apps/api/core";
import { errorText, peaksInfo } from "../api";
import * as playback from "../playback";
import { getState, mediaSource, selectSegment, selectText, setMediaSource, showToast } from "../store";
import { h } from "../dom";
import { ICONS } from "../timeline/icons";
import { planHearing } from "./logic";

export type Heard = { textId: string; tierId: string; segmentId: string; startMs: number | null; endMs: number | null };

/** Open the segment (selecting its text if another is shown) and play it.
 * `select: false` plays without selecting (a panel that already did). */
export async function hear(seg: Heard, select = true): Promise<void> {
  const s = getState();
  if (s.kind !== "open") return;
  const plan = planHearing(
    s.texts.find((t) => t.id === seg.textId),
    mediaSource(),
    s.text?.id ?? null,
  );
  if (select) {
    if (s.text?.id !== seg.textId) await selectText(seg.textId);
    const now = getState();
    if (now.kind === "open" && now.text?.id === seg.textId) selectSegment({ textId: seg.textId, tierId: seg.tierId, segmentId: seg.segmentId });
  }
  if (plan.kind === "none") {
    showToast({ tone: "info", lines: [plan.reason] });
    return;
  }
  if (seg.startMs === null || seg.endMs === null) {
    showToast({ tone: "info", lines: ["This segment has no times, so there is nothing to play."] });
    return;
  }
  if (plan.load !== null) {
    try {
      const info = await peaksInfo({ sourceId: plan.load });
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
