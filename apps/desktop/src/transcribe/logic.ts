// The transcription loop's decisions, kept apart from the DOM so they are
// testable without one.
export type Seg = { id: string; tier: string; startMs: number | null; endMs: number | null };
export type TierLike = { id: string; name: string; role?: string | null };

const ms = (v: number | null): number => v ?? Number.POSITIVE_INFINITY;

/** One tier's segments in time order. Ties break by id; untimed ones sort last. */
export function tierSegments<T extends Seg>(segments: readonly T[], tierId: string): T[] {
  return segments
    .filter((s) => s.tier === tierId)
    .sort((a, b) => ms(a.startMs) - ms(b.startMs) || ms(a.endMs) - ms(b.endMs) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
}

/** The segment after (+1) or before (-1) `id`, or null at either end or when `id` is not listed. */
export function neighbor<T extends { id: string }>(ordered: readonly T[], id: string, step: 1 | -1): T | null {
  const i = ordered.findIndex((s) => s.id === id);
  return i === -1 ? null : (ordered[i + step] ?? null);
}

/** The tier to open on: the first with role transcription, else (tiers made before roles existed) one named
 * "transcription" or the implicit "utterance", else the first. */
export function defaultTier<T extends TierLike>(tiers: readonly T[]): T | null {
  const named = (n: string) => tiers.find((t) => t.name.trim().toLowerCase() === n);
  return tiers.find((t) => t.role === "transcription") ?? named("transcription") ?? named("utterance") ?? tiers[0] ?? null;
}

/** Only a field that differs from what is stored writes an op. Text equal under NFC is the same text. */
export const needsCommit = (stored: string, typed: string): boolean => stored.normalize("NFC") !== typed.normalize("NFC");

/** The index range [from, to) of at most `size` rows around `centre`. */
export function renderWindow(total: number, centre: number, size: number): [number, number] {
  if (total <= size) return [0, total];
  const from = Math.max(0, Math.min(total - size, centre - Math.floor(size / 2)));
  return [from, from + size];
}

const isHigh = (c: number) => c >= 0xd800 && c <= 0xdbff;
const isLow = (c: number) => c >= 0xdc00 && c <= 0xdfff;

/** Replace the selection [start, end) of `value` with `insert`. Offsets are UTF-16 code units, as the
 * DOM reports them; an offset inside a surrogate pair moves out of it. A combining mark inserted at a
 * caret that follows its base lands after the base. NFC is the core's job, not done here. */
export function insertAtCaret(value: string, start: number, end: number, insert: string): { value: string; caret: number } {
  const clamp = (i: number) => Math.max(0, Math.min(value.length, i));
  const inPair = (i: number) => i > 0 && i < value.length && isLow(value.charCodeAt(i)) && isHigh(value.charCodeAt(i - 1));
  let a = clamp(Math.min(start, end));
  let b = clamp(Math.max(start, end));
  const collapsed = a === b;
  if (inPair(a)) a -= 1;
  if (collapsed) b = a;
  else if (inPair(b)) b += 1;
  return { value: value.slice(0, a) + insert + value.slice(b), caret: a + insert.length };
}

/** "1:02.350 – 1:04.100", or a dash for an untimed segment. */
export function formatRange(startMs: number | null, endMs: number | null): string {
  if (startMs === null || endMs === null) return "—";
  const one = (t: number) => {
    const total = Math.max(0, Math.round(t));
    const sec = Math.floor(total / 1000);
    return `${Math.floor(sec / 60)}:${String(sec % 60).padStart(2, "0")}.${String(total % 1000).padStart(3, "0")}`;
  };
  return `${one(startMs)} – ${one(endMs)}`;
}

/** True while an input method owns the key: composing, or the Enter that confirmed a candidate (keyCode 229). */
export const imeBusy = (ev: Pick<KeyLike, "isComposing" | "keyCode">, composing: boolean): boolean => composing || ev.isComposing || ev.keyCode === 229;

export type KeyLike = { key: string; shiftKey: boolean; ctrlKey: boolean; metaKey: boolean; altKey: boolean; isComposing: boolean; keyCode: number };
export type KeyAction = "commit-next" | "commit-prev" | "revert" | "replay" | null;

/** What a keydown in the field does. Nothing while an input method is composing: WebKit sends the
 * Enter that confirms a candidate after compositionend, with isComposing false and keyCode 229.
 * Replay is Tab (as in ELAN) and Ctrl/Cmd+Enter; Ctrl+Space belongs to the OS input-source switch.
 * Shift+Tab is left to the browser so keyboard users can still leave the field. */
export function keyAction(ev: KeyLike, composing: boolean): KeyAction {
  if (imeBusy(ev, composing)) return null;
  if (ev.altKey) return null;
  const chord = ev.ctrlKey || ev.metaKey;
  if (ev.key === "Enter") return chord ? "replay" : ev.shiftKey ? "commit-prev" : "commit-next";
  if (chord) return null;
  if (ev.key === "Escape") return "revert";
  if (ev.key === "Tab" && !ev.shiftKey) return "replay";
  return null;
}

/** Whether an input method is composing. The flag outlives compositionend by one task, because the
 * key event that confirmed the candidate arrives after it. */
export function createComposition(timers: { set(cb: () => void): number; clear(id: number): void }) {
  let active = false;
  let pending = 0;
  return {
    get active() {
      return active;
    },
    start(): void {
      timers.clear(pending);
      active = true;
    },
    end(): void {
      timers.clear(pending);
      pending = timers.set(() => {
        active = false;
      });
    },
  };
}
