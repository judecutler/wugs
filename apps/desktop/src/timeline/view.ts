// The canvas timeline: waveform on top, one lane per tier below, all
// drawn in one devicePixelRatio-aware canvas every animation frame from
// whatever peak strips are cached. Nothing in the draw loop awaits or
// allocates beyond scratch state created up front.
import { AnalysisLayer } from "./analysis-layer";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  addSegment,
  autoSegment,
  deleteSegment,
  deleteSegments,
  editSegmentContent,
  errorText,
  mergeSegments,
  restoreSegments,
  setSegmentTimes,
  speechSpans,
  splitSegment,
  type PeaksInfo,
  type SegmentRow,
  type TextDetail,
} from "../api";
import { h } from "../dom";
import * as playback from "../playback";
import { getSelection, getState, refreshConflicts, refreshText, reloadTexts, selectSegment, setMediaSource, showToast, subscribeSelection } from "../store";
import { tint } from "../assist/logic";
import { crossedDragThreshold, dragEdge, grabOffset, hitTest, DRAG_PX, type Edge, type Hit } from "./gestures";
import { laneBounds, snapEdge, snapRange, type Span } from "./snap";
import { UndoHistory, type UndoEntry, type UndoStack } from "./undo";
import { StripCache } from "./strips";
import {
  clamp,
  fitPxPerSec,
  msToFrame,
  pan,
  timeToX,
  visibleSpanMs,
  xToTime,
  zoomAround,
  zoomBucket,
  type MinMax,
  type Viewport,
} from "./viewport";

export const RULER_H = 22;
export const WAVE_H = 110;
export const TIER_H = 48;
/** The shortest segment a create-drag makes, ms. */
const MIN_SEGMENT_MS = 20;
const MAX_DRAWN_CHANNELS = 2;
/** One keyboard nudge of a segment edge, ms. */
const NUDGE_MS = 10;

/** Timeline edits of the open project, one stack per text. Outlives the
 * view (it is rebuilt when no text is open); another project starts empty. */
export const timelineHistory = new UndoHistory();

const warnFindings = (lines: string[]) => {
  if (lines.length > 0) showToast({ tone: "warn", lines });
};

export type Lane = { tierId: string | null; name: string; segments: SegmentRow[] };

type Drag =
  | { kind: "maybe-pan"; startX: number; downMs: number }
  | { kind: "pan"; lastX: number }
  | { kind: "maybe-create"; lane: number; startX: number; anchorMs: number }
  | { kind: "create"; lane: number; anchorMs: number; currentMs: number }
  | { kind: "resize"; id: string; edge: Edge; downX: number; offsetMs: number; active: boolean };

/** A dragged segment's times. The store's rows are never mutated, so a failed
 * save leaves nothing to undo. */
type ResizeDraft = { id: string; startMs: number; endMs: number };

type RulerStep = { ms: number; decimals: number };
const RULER_STEPS: RulerStep[] = [
  { ms: 10, decimals: 2 },
  { ms: 20, decimals: 2 },
  { ms: 50, decimals: 2 },
  { ms: 100, decimals: 1 },
  { ms: 200, decimals: 1 },
  { ms: 500, decimals: 1 },
  { ms: 1000, decimals: 0 },
  { ms: 2000, decimals: 0 },
  { ms: 5000, decimals: 0 },
  { ms: 10_000, decimals: 0 },
  { ms: 30_000, decimals: 0 },
  { ms: 60_000, decimals: 0 },
  { ms: 300_000, decimals: 0 },
  { ms: 600_000, decimals: 0 },
];

function formatTime(ms: number, decimals: number): string {
  const minutes = Math.floor(ms / 60_000);
  const seconds = (ms % 60_000) / 1000;
  return `${minutes}:${seconds.toFixed(decimals).padStart(decimals > 0 ? 3 + decimals : 2, "0")}`;
}

/** Rolling frame statistics for the on-screen perf readout. */
class FrameStats {
  private drawMs = new Float64Array(240);
  private gapMs = new Float64Array(240);
  private scratch = new Float64Array(240);
  private n = 0;
  private lastFrameAt = 0;

  record(drawMs: number, now: number): void {
    const i = this.n % this.drawMs.length;
    this.drawMs[i] = drawMs;
    this.gapMs[i] = this.lastFrameAt === 0 ? 16.7 : now - this.lastFrameAt;
    this.lastFrameAt = now;
    this.n++;
  }

  private filled(): number {
    return Math.min(this.n, this.drawMs.length);
  }

  avgDraw(): number {
    const len = this.filled();
    if (len === 0) return 0;
    let sum = 0;
    for (let i = 0; i < len; i++) sum += this.drawMs[i] ?? 0;
    return sum / len;
  }

  p95Draw(): number {
    const len = this.filled();
    if (len === 0) return 0;
    this.scratch.set(this.drawMs.subarray(0, len));
    this.scratch.subarray(0, len).sort();
    return this.scratch[Math.min(len - 1, Math.floor(len * 0.95))] ?? 0;
  }

  fps(): number {
    const len = this.filled();
    if (len === 0) return 0;
    let sum = 0;
    for (let i = 0; i < len; i++) sum += this.gapMs[i] ?? 0;
    return sum > 0 ? (1000 * len) / sum : 0;
  }
}

export class TimelineView {
  readonly element: HTMLElement;
  /** Praat's pitch, intensity and formants over the waveform. */
  readonly analysis = new AnalysisLayer();
  private canvas = h("canvas", { className: "tl-canvas" });
  private editBox = h("input", { className: "tl-edit", hidden: true });
  private confirmBar = h("div", { className: "tl-confirm", hidden: true });
  private ctx: CanvasRenderingContext2D;

  private text: TextDetail | null = null;
  private lanes: Lane[] = [];
  private info: PeaksInfo | null = null;
  private strips: StripCache | null = null;
  private vpState: Viewport = { t0Ms: 0, pxPerSec: 100, durationMs: 0, widthPx: 800 };
  private selfSelecting = false;
  private unsubscribeSelection: () => void;
  private drag: Drag | null = null;
  private resizeDraft: ResizeDraft | null = null;
  private hoverCursor = "";
  private editing: SegmentRow | null = null;
  /** Speech spans of the loaded recording, for snapping; empty until ready. */
  private spans: Span[] = [];
  private spansFor: string | null = null;
  /** Alt held during the current gesture: snapping off. */
  private altHeld = false;
  private nudging = false;

  private stats = new FrameStats();
  showPerf = false;
  private interacted = false;
  private lastLog = 0;
  private sampleOut: MinMax = { min: 0, max: 0 };
  private raf = 0;
  private observer: ResizeObserver;
  private windowMoves: (ev: MouseEvent) => void = () => {};
  private windowUps: (ev: MouseEvent) => void = () => {};
  private colors = { line: "#dcd8cf", ink: "#1f1d1a", muted: "#7a756c", accent: "#2e5e4e", wave: "#4a7a68", box: "#ffffff", warn: "#8a5a00" };

  constructor() {
    this.element = h("div", { className: "tl-wrap", tabIndex: 0 }, this.canvas, this.editBox, this.confirmBar);
    const ctx = this.canvas.getContext("2d");
    if (ctx === null) throw new Error("no 2d canvas context");
    this.ctx = ctx;
    const style = getComputedStyle(document.documentElement);
    const read = (name: string, fallback: string) => style.getPropertyValue(name).trim() || fallback;
    this.colors = {
      line: read("--line", this.colors.line),
      ink: read("--ink", this.colors.ink),
      muted: read("--muted", this.colors.muted),
      accent: read("--accent", this.colors.accent),
      wave: this.colors.wave,
      box: "#ffffff",
      warn: read("--warn", this.colors.warn),
    };

    this.observer = new ResizeObserver(() => this.resize());
    this.observer.observe(this.element);
    this.unsubscribeSelection = subscribeSelection(() => this.revealSelected());
    this.wireMouse();
    this.wireKeys();
    const loop = (now: number) => {
      const before = performance.now();
      this.draw();
      this.stats.record(performance.now() - before, now);
      this.logMaybe(now);
      this.raf = requestAnimationFrame(loop);
    };
    this.raf = requestAnimationFrame(loop);
  }

  dispose(): void {
    cancelAnimationFrame(this.raf);
    this.unsubscribeSelection();
    this.observer.disconnect();
    window.removeEventListener("mousemove", this.windowMoves);
    window.removeEventListener("mouseup", this.windowUps);
    playback.setSource(null);
    setMediaSource(null);
  }

  private vp(): Viewport {
    return this.vpState;
  }

  /** The stretch of recording on screen, and how long the recording is. */
  visibleRange(): { startMs: number; endMs: number; durationMs: number } {
    const v = this.vpState;
    return { startMs: v.t0Ms, endMs: v.t0Ms + visibleSpanMs(v), durationMs: v.durationMs };
  }

  private setVp(next: Viewport): void {
    this.vpState = clamp(next);
  }

  // ------------------------------------------------------------ data in

  setText(text: TextDetail | null): void {
    const sameText = this.text !== null && text !== null && this.text.id === text.id;
    this.text = text;
    if (!sameText) {
      this.closeEditor(false);
      this.confirmBar.hidden = true;
    }
    this.lanes = [];
    if (text !== null) {
      const byTier = new Map<string, SegmentRow[]>();
      for (const seg of text.segments) {
        if (seg.startMs === null || seg.endMs === null) continue;
        const list = byTier.get(seg.tier);
        if (list === undefined) byTier.set(seg.tier, [seg]);
        else list.push(seg);
      }
      if (text.tiers.length === 0) {
        // No tier yet: one virtual lane; the first segment drawn on it
        // creates the default utterance tier (tierId null does that).
        this.lanes.push({ tierId: null, name: "utterance", segments: [] });
      }
      for (const tier of text.tiers) {
        this.lanes.push({ tierId: tier.id, name: tier.name, segments: byTier.get(tier.id) ?? [] });
      }
    }
    this.fitDuration();
  }

  setMedia(info: PeaksInfo | null, sourceId: string | null): void {
    this.analysis.setSource(info !== null ? sourceId : null);
    this.info = info;
    this.strips = info !== null && sourceId !== null ? new StripCache(sourceId, info.sampleRate, info.durationMs) : null;
    playback.setSource(info !== null ? convertFileSrc(info.mediaPath) : null);
    setMediaSource(info !== null ? sourceId : null);
    // Spans for snapping come in the background, computed once per
    // recording; until they arrive, edges simply do not snap.
    this.spans = [];
    this.spansFor = info !== null ? sourceId : null;
    if (info !== null && sourceId !== null) {
      speechSpans({ source: sourceId })
        .then((spans) => {
          if (this.spansFor === sourceId) this.spans = spans;
        })
        .catch(() => {});
    }
    this.fitDuration();
    const v = this.vp();
    this.setVp({ ...v, t0Ms: 0, pxPerSec: fitPxPerSec(v) });
  }

  laneNames(): string[] {
    return this.lanes.map((l) => l.name);
  }

  /** Duration: the recording's, or past the last timed segment when only
   * the annotations exist (an ELAN import without its media still lays out). */
  private fitDuration(): void {
    let duration = this.info?.durationMs ?? 0;
    for (const lane of this.lanes) {
      for (const seg of lane.segments) duration = Math.max(duration, (seg.endMs ?? 0) + 2000);
    }
    this.setVp({ ...this.vp(), durationMs: duration });
  }

  private resize(): void {
    const rect = this.element.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.floor(rect.width));
    const hgt = Math.max(1, Math.floor(rect.height));
    this.canvas.width = Math.floor(w * dpr);
    this.canvas.height = Math.floor(hgt * dpr);
    this.canvas.style.width = `${w}px`;
    this.canvas.style.height = `${hgt}px`;
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.setVp({ ...this.vp(), widthPx: w });
  }

  // ------------------------------------------------------------- drawing

  private draw(): void {
    const v = this.vp();
    const ctx = this.ctx;
    const w = v.widthPx;
    const hgt = this.canvas.height / (window.devicePixelRatio || 1);
    ctx.clearRect(0, 0, w, hgt);
    if (this.text === null) return;

    if (this.strips !== null && this.info !== null) {
      const channels = Math.min(this.info.channels, MAX_DRAWN_CHANNELS);
      for (let c = 0; c < channels; c++) this.strips.prefetch(v, c);
      this.drawWave(v, channels);
      this.drawSpeech(v);
      this.analysis.draw(this.ctx, v, RULER_H, WAVE_H);
    } else {
      ctx.fillStyle = this.colors.muted;
      ctx.font = "12px system-ui";
      ctx.fillText("No recording loaded. Pick a source above or import a WAV.", 12, RULER_H + WAVE_H / 2);
    }
    this.drawRuler(v);
    this.drawLanes(v);
    this.drawCursor(v);
    if (this.showPerf) this.drawPerf();
  }

  private drawWave(v: Viewport, channels: number): void {
    const ctx = this.ctx;
    const strips = this.strips;
    const info = this.info;
    if (strips === null || info === null) return;
    const bucket = zoomBucket(v.pxPerSec);
    const laneH = WAVE_H / channels;
    const msPerPx = 1000 / v.pxPerSec;
    ctx.strokeStyle = this.colors.wave;
    ctx.lineWidth = 1;
    ctx.globalAlpha = this.analysis.showing() ? 0.3 : 1;
    for (let c = 0; c < channels; c++) {
      const mid = RULER_H + laneH * c + laneH / 2;
      const amp = laneH / 2 - 2;
      ctx.beginPath();
      for (let x = 0; x < v.widthPx; x++) {
        const t = v.t0Ms + x * msPerPx;
        if (t >= v.durationMs) break;
        const f0 = msToFrame(t, info.sampleRate);
        const f1 = msToFrame(t + msPerPx, info.sampleRate);
        if (!strips.sample(bucket, c, t, f0, f1, this.sampleOut)) continue;
        const lo = mid - (this.sampleOut.min / 32768) * amp;
        const hi = mid - (this.sampleOut.max / 32768) * amp;
        ctx.moveTo(x + 0.5, Math.min(lo, hi));
        ctx.lineTo(x + 0.5, Math.max(lo, hi) + 1);
      }
      ctx.stroke();
    }
    ctx.globalAlpha = 1;
    ctx.strokeStyle = this.colors.line;
    ctx.beginPath();
    ctx.moveTo(0, RULER_H + WAVE_H + 0.5);
    ctx.lineTo(v.widthPx, RULER_H + WAVE_H + 0.5);
    ctx.stroke();
  }

  /** A thin band along the waveform's foot where speech was found: the
   * edges a drawn or dragged segment snaps to. */
  private drawSpeech(v: Viewport): void {
    const ctx = this.ctx;
    const end = v.t0Ms + visibleSpanMs(v);
    ctx.fillStyle = "rgba(46, 94, 78, 0.35)";
    for (const [s, e] of this.spans) {
      if (e < v.t0Ms || s > end) continue;
      const x0 = timeToX(v, s);
      ctx.fillRect(x0, RULER_H + WAVE_H - 4, Math.max(1, timeToX(v, e) - x0), 3);
    }
  }

  private drawRuler(v: Viewport): void {
    const ctx = this.ctx;
    const step = RULER_STEPS.find((s) => (s.ms / 1000) * v.pxPerSec >= 70) ?? { ms: 600_000, decimals: 0 };
    ctx.fillStyle = this.colors.muted;
    ctx.strokeStyle = this.colors.line;
    ctx.font = "10px system-ui";
    ctx.beginPath();
    const first = Math.floor(v.t0Ms / step.ms) * step.ms;
    const end = v.t0Ms + visibleSpanMs(v);
    for (let t = first; t <= end; t += step.ms) {
      const x = Math.round(timeToX(v, t)) + 0.5;
      ctx.moveTo(x, RULER_H - 6);
      ctx.lineTo(x, RULER_H);
      ctx.fillText(formatTime(t, step.decimals), x + 3, RULER_H - 8);
    }
    ctx.moveTo(0, RULER_H + 0.5);
    ctx.lineTo(v.widthPx, RULER_H + 0.5);
    ctx.stroke();
  }

  private drawLanes(v: Viewport): void {
    const ctx = this.ctx;
    for (let i = 0; i < this.lanes.length; i++) {
      const lane = this.lanes[i];
      if (lane === undefined) continue;
      const top = RULER_H + WAVE_H + i * TIER_H;
      ctx.strokeStyle = this.colors.line;
      ctx.beginPath();
      ctx.moveTo(0, top + TIER_H + 0.5);
      ctx.lineTo(v.widthPx, top + TIER_H + 0.5);
      ctx.stroke();
      for (const seg of lane.segments) {
        const draft = this.resizeDraft;
        this.drawSegment(v, draft !== null && draft.id === seg.id ? { ...seg, ...draft } : seg, top);
      }
      if (this.drag?.kind === "create" && this.drag.lane === i) {
        const r = this.createRange(i, this.drag.anchorMs, this.drag.currentMs);
        const x0 = timeToX(v, r.startMs);
        const x1 = timeToX(v, r.endMs);
        ctx.fillStyle = "rgba(46, 94, 78, 0.18)";
        ctx.fillRect(x0, top + 4, x1 - x0, TIER_H - 8);
      }
    }
  }

  private drawSegment(v: Viewport, seg: SegmentRow, top: number): void {
    if (seg.startMs === null || seg.endMs === null) return;
    const ctx = this.ctx;
    const x0 = timeToX(v, seg.startMs);
    const x1 = timeToX(v, seg.endMs);
    if (x1 < 0 || x0 > v.widthPx) return;
    const y = top + 4;
    const hgt = TIER_H - 8;
    const selected = seg.id === this.selected;
    // Machine output stays tinted until a person edits it.
    const machine = seg.machine === null ? null : tint(seg.machine);
    ctx.fillStyle = machine === "very-unsure" ? "rgba(196, 64, 48, 0.22)" : machine === "unsure" ? "rgba(214, 170, 40, 0.28)" : selected ? "rgba(46, 94, 78, 0.14)" : this.colors.box;
    ctx.strokeStyle = selected ? this.colors.accent : this.colors.line;
    ctx.lineWidth = selected ? 2 : 1;
    ctx.fillRect(x0, y, x1 - x0, hgt);
    ctx.strokeRect(x0 + 0.5, y + 0.5, x1 - x0 - 1, hgt - 1);
    ctx.lineWidth = 1;
    if (x1 - x0 > 24 && seg.content !== "") {
      ctx.save();
      ctx.beginPath();
      ctx.rect(x0 + 3, y, x1 - x0 - 6, hgt);
      ctx.clip();
      ctx.fillStyle = this.colors.ink;
      ctx.font = "12px system-ui";
      ctx.fillText(seg.content, x0 + 6, y + hgt / 2 + 4);
      ctx.restore();
    }
  }

  private drawCursor(v: Viewport): void {
    if (this.info === null) return;
    const tMs = playback.currentMs();
    if (playback.isPlaying()) this.followCursor(v, tMs);
    const x = Math.round(timeToX(this.vp(), tMs)) + 0.5;
    if (x < 0 || x > this.vp().widthPx) return;
    const ctx = this.ctx;
    ctx.strokeStyle = this.colors.warn;
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, RULER_H + WAVE_H + this.lanes.length * TIER_H);
    ctx.stroke();
  }

  private followCursor(v: Viewport, tMs: number): void {
    const x = timeToX(v, tMs);
    if (x > v.widthPx * 0.9 || x < 0) {
      this.setVp({ ...v, t0Ms: tMs - visibleSpanMs(v) * 0.1 });
    }
  }

  private drawPerf(): void {
    const ctx = this.ctx;
    const strips = this.strips;
    const hits = strips?.hits ?? 0;
    const misses = strips?.misses ?? 0;
    const rate = hits + misses > 0 ? (100 * hits) / (hits + misses) : 100;
    const line1 = `draw ${this.stats.avgDraw().toFixed(2)} ms avg · ${this.stats.p95Draw().toFixed(2)} ms p95`;
    const line2 = `${this.stats.fps().toFixed(0)} fps · strip hits ${rate.toFixed(1)}%`;
    const w = 240;
    ctx.fillStyle = "rgba(31, 29, 26, 0.82)";
    ctx.fillRect(this.vp().widthPx - w - 8, 8, w, 40);
    ctx.fillStyle = "#ffffff";
    ctx.font = "11px ui-monospace, monospace";
    ctx.fillText(line1, this.vp().widthPx - w, 24);
    ctx.fillText(line2, this.vp().widthPx - w, 40);
  }

  private logMaybe(now: number): void {
    if (!this.interacted || now - this.lastLog < 5000) return;
    this.lastLog = now;
    this.interacted = false;
    const strips = this.strips;
    const hits = strips?.hits ?? 0;
    const misses = strips?.misses ?? 0;
    const rate = hits + misses > 0 ? (100 * hits) / (hits + misses) : 100;
    console.log(
      `[timeline] draw ${this.stats.avgDraw().toFixed(2)}ms avg, ${this.stats.p95Draw().toFixed(2)}ms p95, ` +
        `${this.stats.fps().toFixed(0)} fps, strip hits ${rate.toFixed(1)}%`,
    );
  }

  // --------------------------------------------------------- interaction

  private laneAt(y: number): number {
    const i = Math.floor((y - RULER_H - WAVE_H) / TIER_H);
    return i >= 0 && i < this.lanes.length ? i : -1;
  }

  private hitAt(lane: Lane, x: number): { hit: Hit; seg: SegmentRow } | null {
    const hit = hitTest(this.vp(), lane.segments, x);
    const seg = hit === null ? undefined : lane.segments.find((c) => c.id === hit.id);
    return hit === null || seg === undefined ? null : { hit, seg };
  }

  private wireMouse(): void {
    const pos = (ev: MouseEvent) => {
      const rect = this.canvas.getBoundingClientRect();
      return { x: ev.clientX - rect.left, y: ev.clientY - rect.top };
    };

    this.canvas.addEventListener("wheel", (ev) => {
      ev.preventDefault();
      this.interacted = true;
      const { x } = pos(ev);
      if (ev.shiftKey) {
        this.setVp(pan(this.vp(), ev.deltaY !== 0 ? ev.deltaY : ev.deltaX));
      } else {
        this.setVp(zoomAround(this.vp(), x, Math.exp(-ev.deltaY * 0.0022)));
      }
    }, { passive: false });

    this.canvas.addEventListener("mousedown", (ev) => {
      this.altHeld = ev.altKey;
      if (this.editing !== null) this.closeEditor(true);
      this.element.focus();
      const { x, y } = pos(ev);
      const tMs = xToTime(this.vp(), x);
      if (ev.button === 1) {
        ev.preventDefault();
        this.drag = { kind: "pan", lastX: x };
        return;
      }
      if (ev.button !== 0) return;
      const laneIdx = this.laneAt(y);
      const lane = laneIdx === -1 ? undefined : this.lanes[laneIdx];
      if (lane === undefined) {
        this.drag = { kind: "maybe-pan", startX: x, downMs: tMs };
        return;
      }
      const found = this.hitAt(lane, x);
      if (found !== null) {
        this.select(found.seg.id);
        if (found.hit.kind === "edge") {
          const edgeMs = (found.hit.edge === "start" ? found.seg.startMs : found.seg.endMs) ?? tMs;
          this.drag = {
            kind: "resize",
            id: found.seg.id,
            edge: found.hit.edge,
            downX: x,
            offsetMs: grabOffset(edgeMs, tMs),
            active: false,
          };
        }
        return;
      }
      this.select(null);
      this.drag = { kind: "maybe-create", lane: laneIdx, startX: x, anchorMs: tMs };
    });

    this.windowMoves = (ev) => {
      const { x, y } = pos(ev);
      const drag = this.drag;
      if (drag === null) {
        this.updateHoverCursor(x, y);
        return;
      }
      this.interacted = true;
      this.altHeld = ev.altKey;
      const tMs = Math.max(0, xToTime(this.vp(), x));
      switch (drag.kind) {
        case "maybe-pan":
          if (Math.abs(x - drag.startX) > DRAG_PX) this.drag = { kind: "pan", lastX: x };
          break;
        case "pan":
          this.setVp(pan(this.vp(), drag.lastX - x));
          drag.lastX = x;
          break;
        case "maybe-create":
          if (Math.abs(x - drag.startX) > DRAG_PX) {
            this.drag = { kind: "create", lane: drag.lane, anchorMs: drag.anchorMs, currentMs: tMs };
          }
          break;
        case "create":
          drag.currentMs = tMs;
          break;
        case "resize": {
          if (!drag.active && !crossedDragThreshold(drag.downX, x)) break;
          drag.active = true;
          const seg = this.findSegment(drag.id);
          if (seg === null || seg.startMs === null || seg.endMs === null) break;
          // Optimistic: the box follows the mouse on a copy; the intent
          // commits on release and the store refresh squares everything up.
          let { startMs, endMs } = dragEdge(drag.edge, seg.startMs, seg.endMs, tMs, drag.offsetMs, MIN_SEGMENT_MS);
          if (!this.altHeld) {
            // Pull the moved edge out to where speech starts or ends, never
            // onto a neighbour; Alt drags freely.
            const b = laneBounds(this.laneOf(seg)?.segments ?? [], seg.startMs, seg.endMs, seg.id);
            if (drag.edge === "start") startMs = Math.min(snapEdge("start", startMs, this.spans, undefined, b.lo), endMs - MIN_SEGMENT_MS);
            else endMs = Math.max(snapEdge("end", endMs, this.spans, undefined, b.hi), startMs + MIN_SEGMENT_MS);
          }
          this.resizeDraft = { id: drag.id, startMs, endMs };
          break;
        }
      }
    };
    window.addEventListener("mousemove", this.windowMoves);

    this.windowUps = (ev: MouseEvent) => {
      this.altHeld = ev.altKey;
      const drag = this.drag;
      this.drag = null;
      if (drag === null) return;
      switch (drag.kind) {
        case "maybe-pan":
          this.seek(drag.downMs);
          break;
        case "create":
          void this.commitCreate(drag.lane, drag.anchorMs, drag.currentMs);
          break;
        case "resize":
          if (drag.active) void this.commitResize(drag.edge);
          break;
        case "pan":
        case "maybe-create":
          break;
      }
    };
    window.addEventListener("mouseup", this.windowUps);

    this.canvas.addEventListener("dblclick", (ev) => {
      const { x, y } = pos(ev);
      const laneIdx = this.laneAt(y);
      const lane = laneIdx === -1 ? undefined : this.lanes[laneIdx];
      if (lane === undefined) return;
      const seg = this.hitAt(lane, x)?.seg;
      if (seg !== undefined) this.openEditor(seg, laneIdx);
    });
  }

  private updateHoverCursor(x: number, y: number): void {
    const laneIdx = this.laneAt(y);
    const lane = laneIdx === -1 ? undefined : this.lanes[laneIdx];
    let cursor = "";
    if (lane !== undefined && this.hitAt(lane, x)?.hit.kind === "edge") cursor = "ew-resize";
    else if (y < RULER_H + WAVE_H) cursor = "pointer";
    if (cursor !== this.hoverCursor) {
      this.hoverCursor = cursor;
      this.canvas.style.cursor = cursor;
    }
  }

  private wireKeys(): void {
    this.element.addEventListener("keydown", (ev) => {
      if (this.editing !== null) return;
      const mod = ev.ctrlKey || ev.metaKey;
      if (mod && !ev.altKey && (ev.key === "z" || ev.key === "Z")) {
        ev.preventDefault();
        void (ev.shiftKey ? this.redo() : this.undo());
        return;
      }
      if (mod && !ev.altKey && (ev.key === "y" || ev.key === "Y")) {
        ev.preventDefault();
        void this.redo();
        return;
      }
      if (mod || ev.altKey) return;
      if (ev.code === "BracketLeft" || ev.code === "BracketRight") {
        // [ ] move the start, Shift+[ ] the end, by NUDGE_MS.
        ev.preventDefault();
        void this.nudge(ev.shiftKey ? "end" : "start", ev.code === "BracketLeft" ? -NUDGE_MS : NUDGE_MS);
      } else if (ev.key === "s" || ev.key === "S") {
        ev.preventDefault();
        void this.splitAtPlayhead();
      } else if (ev.key === "m" || ev.key === "M") {
        ev.preventDefault();
        void this.mergeWithNext();
      } else if (ev.key === " ") {
        ev.preventDefault();
        this.togglePlay();
      } else if ((ev.key === "Delete" || ev.key === "Backspace") && this.selected !== null) {
        ev.preventDefault();
        this.askDelete(this.selected);
      } else if (ev.key === "Escape") {
        this.select(null);
        this.confirmBar.hidden = true;
      }
    });
  }

  /** The selected segment of this text, from the shared selection. */
  private get selected(): string | null {
    const sel = getSelection();
    return sel !== null && sel.textId === this.text?.id ? sel.segmentId : null;
  }

  private select(id: string | null): void {
    const seg = id === null ? null : this.findSegment(id);
    const textId = this.text?.id;
    this.selfSelecting = true;
    selectSegment(seg === null || textId === undefined ? null : { textId, tierId: seg.tier, segmentId: seg.id });
    this.selfSelecting = false;
    this.confirmBar.hidden = true;
  }

  /** A selection made elsewhere scrolls into view; one made here never moves the view under a drag. */
  private revealSelected(): void {
    if (this.selfSelecting || this.selected === null) return;
    const seg = this.findSegment(this.selected);
    if (seg === null || seg.startMs === null || seg.endMs === null) return;
    const v = this.vp();
    const span = visibleSpanMs(v);
    if (seg.startMs >= v.t0Ms && seg.endMs <= v.t0Ms + span) return;
    this.setVp({ ...v, t0Ms: seg.startMs - span * 0.1 });
  }

  private seek(tMs: number): void {
    if (this.info === null) return;
    playback.seek(Math.max(0, Math.min(this.info.durationMs, tMs)));
  }

  private togglePlay(): void {
    if (this.info === null) return;
    playback.toggle();
  }

  // ------------------------------------------------------------- intents

  private async commitCreate(laneIdx: number, aMs: number, bMs: number): Promise<void> {
    const text = this.text;
    const lane = this.lanes[laneIdx];
    if (text === null || lane === undefined) return;
    if (Math.abs(bMs - aMs) < MIN_SEGMENT_MS) return;
    const range = this.createRange(laneIdx, aMs, bMs);
    const startMs = Math.round(range.startMs);
    const endMs = Math.round(range.endMs);
    try {
      const added = await addSegment({ textId: text.id, tierId: lane.tierId, startMs, endMs, content: "" });
      if (added.findings.length > 0) showToast({ tone: "warn", lines: added.findings });
      const ids = [added.id];
      this.remember({
        label: "draw segment",
        undo: async () => void (await deleteSegments({ textId: text.id, segmentIds: ids })),
        redo: async () => void (await restoreSegments({ textId: text.id, segmentIds: ids })),
      });
      await reloadTexts();
      await refreshText();
      this.select(added.id);
      await refreshConflicts();
    } catch (e) {
      showToast({ tone: "error", lines: [errorText(e)] });
      await refreshText();
    }
  }

  private async commitResize(edge: Edge): Promise<void> {
    const draft = this.resizeDraft;
    const text = this.text;
    if (text === null || draft === null) return;
    const before = this.findSegment(draft.id);
    const was = edge === "start" ? before?.startMs : before?.endMs;
    const now = Math.round(edge === "start" ? draft.startMs : draft.endMs);
    try {
      const findings = await setSegmentTimes({ textId: text.id, segmentId: draft.id, ...(edge === "start" ? { startMs: now } : { endMs: now }) });
      if (findings.length > 0) showToast({ tone: "warn", lines: findings });
      if (was !== null && was !== undefined) this.rememberRetime(text.id, draft.id, edge, was, now);
      await refreshText();
      await refreshConflicts();
    } catch (e) {
      showToast({ tone: "error", lines: [errorText(e)] });
      await refreshText();
    } finally {
      // Kept through the refresh so the box does not jump back and forth;
      // dropped either way, so a failed save falls back to the stored times.
      if (this.resizeDraft === draft) this.resizeDraft = null;
    }
  }

  /** One-click confirm, and Undo brings it back (a restore). */
  private askDelete(segId: string): void {
    const label = this.findSegment(segId)?.content ?? "";
    const doDelete = async () => {
      this.confirmBar.hidden = true;
      const text = this.text;
      if (text === null) return;
      try {
        const findings = await deleteSegment({ textId: text.id, segmentId: segId });
        if (findings.length > 0) showToast({ tone: "warn", lines: findings });
        const ids = [segId];
        this.remember({
          label: "delete segment",
          undo: async () => void (await restoreSegments({ textId: text.id, segmentIds: ids })),
          redo: async () => void (await deleteSegments({ textId: text.id, segmentIds: ids })),
        });
        this.select(null);
        await reloadTexts();
        await refreshText();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
    };
    this.confirmBar.replaceChildren(
      h("span", { textContent: label === "" ? "Delete this segment?" : `Delete “${label.slice(0, 40)}”?` }),
      h("button", { className: "danger", textContent: "Delete", onclick: () => void doDelete() }),
      h("button", { textContent: "Keep", onclick: () => (this.confirmBar.hidden = true) }),
    );
    this.confirmBar.hidden = false;
  }

  private laneOf(seg: SegmentRow): Lane | null {
    return this.lanes.find((l) => l.segments.some((s) => s.id === seg.id)) ?? null;
  }

  /** A drawn range, snapped outward to speech edges unless Alt is held. */
  private createRange(laneIdx: number, aMs: number, bMs: number): { startMs: number; endMs: number } {
    const lo = Math.min(aMs, bMs);
    const hi = Math.max(aMs, bMs);
    if (this.altHeld) return { startMs: lo, endMs: hi };
    return snapRange(lo, hi, this.spans, laneBounds(this.lanes[laneIdx]?.segments ?? [], lo, hi));
  }

  // ------------------------------------------------------- undo and redo

  /** This text's undo stack, or null when no text is open. */
  history(): UndoStack | null {
    const s = getState();
    if (s.kind !== "open" || this.text === null) return null;
    return timelineHistory.stack(s.info.projectId, this.text.id);
  }

  private remember(entry: UndoEntry, key: string | null = null): void {
    this.history()?.push(entry, key);
  }

  private rememberRetime(textId: string, segId: string, edge: Edge, was: number, now: number, key: string | null = null): void {
    const set = (ms: number) => setSegmentTimes({ textId, segmentId: segId, ...(edge === "start" ? { startMs: ms } : { endMs: ms }) });
    this.remember({ label: "move edge", undo: async () => void (await set(was)), redo: async () => void (await set(now)) }, key);
  }

  private async afterEdit(): Promise<void> {
    await reloadTexts();
    await refreshText();
    await refreshConflicts();
  }

  async undo(): Promise<void> {
    await this.step("undo");
  }

  async redo(): Promise<void> {
    await this.step("redo");
  }

  private async step(which: "undo" | "redo"): Promise<void> {
    const stack = this.history();
    if (stack === null) return;
    try {
      const label = which === "undo" ? await stack.undo() : await stack.redo();
      if (label === null) return;
      await this.afterEdit();
    } catch (e) {
      showToast({ tone: "error", lines: [`Could not ${which} that edit: ${errorText(e)}`] });
      await refreshText();
    }
  }

  private selectedSegment(): SegmentRow | null {
    const id = this.selected;
    const seg = id === null ? null : this.findSegment(id);
    if (seg === null || seg.startMs === null || seg.endMs === null) {
      showToast({ tone: "info", lines: ["Select a segment on the timeline first."] });
      return null;
    }
    return seg;
  }

  /** Cut the selected segment at the playhead; its text stays on the first part. */
  async splitAtPlayhead(): Promise<void> {
    const text = this.text;
    const seg = this.selectedSegment();
    if (text === null || seg === null || seg.startMs === null || seg.endMs === null) return;
    const at = Math.round(playback.currentMs());
    if (!(at > seg.startMs + MIN_SEGMENT_MS / 2 && at < seg.endMs - MIN_SEGMENT_MS / 2)) {
      showToast({ tone: "info", lines: ["Put the playhead inside the selected segment (click the waveform), then split."] });
      return;
    }
    const first = seg.id;
    try {
      const added = await splitSegment({ textId: text.id, segmentId: first, atMs: at });
      warnFindings(added.findings);
      const second = added.id;
      // What the first part says just before an undo merges the parts back,
      // so a redo can put the text back where it was.
      let firstText: string | null = null;
      this.remember({
        label: "split",
        undo: async () => {
          firstText = this.findSegment(first)?.content ?? null;
          await mergeSegments({ textId: text.id, firstId: first, secondId: second });
        },
        redo: async () => {
          await restoreSegments({ textId: text.id, segmentIds: [second] });
          await setSegmentTimes({ textId: text.id, segmentId: first, endMs: at });
          if (firstText !== null && firstText !== this.findSegment(first)?.content) {
            await editSegmentContent({ textId: text.id, segmentId: first, content: firstText });
          }
        },
      });
      await this.afterEdit();
    } catch (e) {
      showToast({ tone: "error", lines: [errorText(e)] });
      await refreshText();
    }
  }

  /** Join the selected segment with the next one on its track. */
  async mergeWithNext(): Promise<void> {
    const text = this.text;
    const seg = this.selectedSegment();
    if (text === null || seg === null || seg.endMs === null) return;
    const lane = this.laneOf(seg);
    const ordered = [...(lane?.segments ?? [])].sort((a, b) => (a.startMs ?? 0) - (b.startMs ?? 0) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    const next = ordered[ordered.findIndex((s) => s.id === seg.id) + 1];
    if (next === undefined) {
      showToast({ tone: "info", lines: ["There is no next segment on this track to merge with."] });
      return;
    }
    const [first, second, wasEnd, wasText] = [seg.id, next.id, seg.endMs, seg.content];
    try {
      warnFindings(await mergeSegments({ textId: text.id, firstId: first, secondId: second }));
      this.remember({
        label: "merge",
        undo: async () => {
          await restoreSegments({ textId: text.id, segmentIds: [second] });
          await setSegmentTimes({ textId: text.id, segmentId: first, endMs: wasEnd });
          if (this.findSegment(first)?.content !== wasText) await editSegmentContent({ textId: text.id, segmentId: first, content: wasText });
        },
        redo: async () => void (await mergeSegments({ textId: text.id, firstId: first, secondId: second })),
      });
      await this.afterEdit();
    } catch (e) {
      showToast({ tone: "error", lines: [errorText(e)] });
      await refreshText();
    }
  }

  /** Move one edge of the selected segment by `deltaMs` (keyboard). Quick
   * repeats on one edge undo as one step. */
  private async nudge(edge: Edge, deltaMs: number): Promise<void> {
    const text = this.text;
    const id = this.selected;
    const seg = id === null ? null : this.findSegment(id);
    if (this.nudging || text === null || seg === null || seg.startMs === null || seg.endMs === null) return;
    const was = edge === "start" ? seg.startMs : seg.endMs;
    const now =
      edge === "start" ? Math.max(0, Math.min(seg.startMs + deltaMs, seg.endMs - MIN_SEGMENT_MS)) : Math.max(seg.startMs + MIN_SEGMENT_MS, seg.endMs + deltaMs);
    if (now === was) return;
    this.nudging = true;
    try {
      warnFindings(await setSegmentTimes({ textId: text.id, segmentId: seg.id, ...(edge === "start" ? { startMs: now } : { endMs: now }) }));
      this.rememberRetime(text.id, seg.id, edge, was, now, `${seg.id}:${edge}`);
      await refreshText();
    } catch (e) {
      showToast({ tone: "error", lines: [errorText(e)] });
    } finally {
      this.nudging = false;
    }
  }

  /**
   * One segment per stretch of speech in `source`'s recording, on the
   * selected segment's track (else the utterance track), leaving stretches
   * the person already covered alone. Undo takes them all back.
   */
  async autoSegment(source: string): Promise<void> {
    const text = this.text;
    if (text === null) return;
    const sel = this.selected === null ? null : this.findSegment(this.selected);
    const tierId = sel?.tier ?? this.lanes.find((l) => l.tierId !== null && l.name === "utterance")?.tierId ?? null;
    try {
      const out = await autoSegment({ textId: text.id, source, tierId });
      if (out.ids.length === 0) {
        showToast({ tone: "info", lines: [out.skipped > 0 ? "Every stretch of speech already has a segment." : "No speech was found in this recording."] });
        return;
      }
      const ids = out.ids;
      const entry: UndoEntry = {
        label: "auto-segment",
        undo: async () => void (await deleteSegments({ textId: text.id, segmentIds: ids })),
        redo: async () => void (await restoreSegments({ textId: text.id, segmentIds: ids })),
      };
      this.remember(entry);
      await this.afterEdit();
      const stack = this.history();
      showToast({
        tone: "info",
        lines: [
          `Added ${ids.length} segment${ids.length === 1 ? "" : "s"}.` +
            (out.skipped > 0 ? ` ${out.skipped} stretch${out.skipped === 1 ? " was" : "es were"} already covered and left alone.` : ""),
          ...out.findings,
        ],
        action: {
          label: "Undo",
          run: () => {
            if (stack !== null && stack.isNewest(entry)) void this.undo();
            else showToast({ tone: "info", lines: ["Later edits came after it; use Undo on the timeline to step back to it."] });
          },
        },
      });
    } catch (e) {
      showToast({ tone: "error", lines: [errorText(e)] });
      await refreshText();
    }
  }

  private findSegment(id: string): SegmentRow | null {
    for (const lane of this.lanes) {
      for (const seg of lane.segments) if (seg.id === id) return seg;
    }
    return null;
  }

  // -------------------------------------------------------- content edit

  private openEditor(seg: SegmentRow, laneIdx: number): void {
    if (seg.startMs === null || seg.endMs === null) return;
    const v = this.vp();
    const x0 = Math.max(0, timeToX(v, seg.startMs));
    const x1 = Math.min(v.widthPx, timeToX(v, seg.endMs));
    this.editing = seg;
    const box = this.editBox;
    box.value = seg.content;
    box.style.left = `${x0}px`;
    box.style.top = `${RULER_H + WAVE_H + laneIdx * TIER_H + 8}px`;
    box.style.width = `${Math.max(120, x1 - x0)}px`;
    box.hidden = false;
    box.focus();
    box.select();
    box.onkeydown = (ev) => {
      if (ev.key === "Enter") {
        ev.preventDefault();
        this.closeEditor(true);
      } else if (ev.key === "Escape") {
        this.closeEditor(false);
      }
    };
    box.onblur = () => this.closeEditor(true);
  }

  private closeEditor(save: boolean): void {
    const seg = this.editing;
    if (seg === null) return;
    this.editing = null;
    const value = this.editBox.value;
    this.editBox.onblur = null;
    this.editBox.hidden = true;
    const text = this.text;
    if (!save || text === null || value === seg.content) return;
    const was = seg.content;
    seg.content = value;
    void (async () => {
      try {
        const result = await editSegmentContent({ textId: text.id, segmentId: seg.id, content: value });
        this.remember({
          label: "text edit",
          undo: async () => void (await editSegmentContent({ textId: text.id, segmentId: seg.id, content: was })),
          redo: async () => void (await editSegmentContent({ textId: text.id, segmentId: seg.id, content: value })),
        });
        // The merged value can differ when another device's edit folded in.
        seg.content = result.content;
        if (result.findings.length > 0) showToast({ tone: "warn", lines: result.findings });
        await refreshConflicts();
        await refreshText();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
        await refreshText();
      }
    })();
  }
}
