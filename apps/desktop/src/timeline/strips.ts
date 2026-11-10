// Peak strips fetched over IPC, keyed by (zoom bucket, strip index,
// channel). The draw loop never waits on this cache: it draws from
// whatever strips exist, scaling a neighbouring zoom bucket's strip while
// the right one loads, and prefetch() tops the cache up between frames.
import { peaksQuery } from "../api";
import {
  mayRetryStrip,
  stripIndex,
  stripKey,
  stripMinMaxInto,
  stripQuery,
  stripRange,
  stripSpanMs,
  type MinMax,
  type PeakStrip,
  type Viewport,
} from "./viewport";

const MAX_STRIPS = 24;
/** How far off the exact zoom bucket the fallback looks for a stale strip. */
const BUCKET_SLACK = 8;

export class StripCache {
  private strips = new Map<string, PeakStrip>();
  private inFlight = new Set<string>();
  private failedAt = new Map<string, number>();
  private warned = false;
  /** Draw-loop counters for the perf readout; reset by the overlay. */
  hits = 0;
  misses = 0;

  constructor(
    private sourceId: string,
    private sampleRate: number,
    private durationMs: number,
  ) {}

  /** Request the strips `v` needs (visible ±1) that are not here yet. */
  prefetch(v: Viewport, channel: number): void {
    const { bucket, first, last } = stripRange(v);
    const span = stripSpanMs(bucket);
    for (let i = first; i <= last; i++) {
      if (i * span >= this.durationMs) break;
      const key = stripKey(bucket, i, channel);
      if (this.strips.has(key) || this.inFlight.has(key)) continue;
      if (!mayRetryStrip(this.failedAt.get(key), performance.now())) continue;
      this.inFlight.add(key);
      const q = stripQuery(bucket, i, this.sampleRate);
      peaksQuery({ sourceId: this.sourceId, channel, ...q })
        .then((slice) => {
          this.failedAt.delete(key);
          this.remember(key, slice);
        })
        .catch((e) => {
          // A failed fetch (project closed, file pruned) leaves a blank
          // strip and is retried after a backoff, not every frame.
          this.failedAt.set(key, performance.now());
          if (!this.warned) {
            this.warned = true;
            console.warn("[timeline] peak strip fetch failed; retrying every 2s, further failures not logged", e);
          }
        })
        .finally(() => {
          this.inFlight.delete(key);
        });
    }
  }

  private remember(key: string, strip: PeakStrip): void {
    if (this.strips.size >= MAX_STRIPS) {
      // Maps iterate in insertion order; re-inserting on hit would cost a
      // delete per pixel, so evict oldest-fetched instead of LRU. At 24
      // strips (~8 viewports) the difference never shows.
      const oldest = this.strips.keys().next().value;
      if (oldest !== undefined) this.strips.delete(oldest);
    }
    this.strips.set(key, strip);
  }

  /** Min/max of channel over [f0, f1) frames around time `tMs`, preferring
   * `bucket` and falling back to nearby zoom buckets. Allocation-free. */
  sample(bucket: number, channel: number, tMs: number, f0: number, f1: number, out: MinMax): boolean {
    const exact = this.strips.get(stripKey(bucket, stripIndex(bucket, tMs), channel));
    if (exact !== undefined && stripMinMaxInto(exact, f0, f1, out)) {
      this.hits++;
      return true;
    }
    this.misses++;
    for (let d = 1; d <= BUCKET_SLACK; d++) {
      for (let s = -1; s <= 1; s += 2) {
        const b = bucket + d * s;
        const strip = this.strips.get(stripKey(b, stripIndex(b, tMs), channel));
        if (strip !== undefined && stripMinMaxInto(strip, f0, f1, out)) return true;
      }
    }
    return false;
  }
}
