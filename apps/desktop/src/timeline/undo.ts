// Undo and redo of timeline edits, pure and unit-tested. The op log is
// never rewritten: each entry's undo() and redo() call ordinary commands
// that write the inverse edit as a new write (create -> delete, delete ->
// restore, retime -> the old times, split -> merge, merge -> unmerge).
// Nothing a person did since may be lost to an undo, wherever they did it
// (the Transcribe or Gloss panel, another device): each entry keeps what
// its undo or redo would delete or overwrite (a SegmentCheck), and the
// command refuses, writing nothing, when that no longer holds. If an undo
// or redo fails or is refused, the entry is dropped and the error is passed
// on for the view to show.

export type UndoEntry = {
  /** Shown in the buttons' titles: "Undo split". */
  label: string;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  /** When this edit folds into `older` (same key, in quick succession), the
   * entry kept in its place. Without it, older's undo and this redo. */
  fold?: (older: UndoEntry) => UndoEntry;
};

/** A segment as an edit left it (session.rs `SegmentStateView`). */
export type SegmentState = {
  id: string;
  deleted: boolean;
  startMs: number | null;
  endMs: number | null;
  content: string;
  /** Words (tokens) made from its transcription. */
  words: number;
};

/** The parts of a SegmentState an undo depends on; absent parts are not checked. */
export type SegmentCheck = {
  id: string;
  deleted?: boolean;
  startMs?: number;
  endMs?: number;
  content?: string;
  words?: number;
};

type Part = "deleted" | "startMs" | "endMs" | "content" | "words";

/** A check of `parts` of `s` (all of them by default: for an undo that deletes it). */
export function checkOf(s: SegmentState, parts: readonly Part[] = ["deleted", "startMs", "endMs", "content", "words"]): SegmentCheck {
  const c: SegmentCheck = { id: s.id };
  for (const p of parts) {
    if (p === "deleted") c.deleted = s.deleted;
    else if (p === "content") c.content = s.content;
    else if (p === "words") c.words = s.words;
    else if (s[p] !== null) c[p] = s[p];
  }
  return c;
}

type EdgeEntry = UndoEntry & { fromMs: number };

/**
 * A segment edge moved from `fromMs` to `toMs`. `write(ms, expectMs)` writes
 * the edge, refused unless it is still at `expectMs`. Quick repeats (keyboard
 * nudges) fold into one entry from the first `fromMs` to the last `toMs`.
 */
export function edgeEntry(label: string, fromMs: number, toMs: number, write: (ms: number, expectMs: number) => Promise<void>): EdgeEntry {
  return {
    label,
    fromMs,
    undo: () => write(fromMs, toMs),
    redo: () => write(toMs, fromMs),
    fold: (older) => edgeEntry(label, (older as Partial<EdgeEntry>).fromMs ?? fromMs, toMs, write),
  };
}

/** How long a run of same-key edits (keyboard nudges) stays one entry, ms. */
export const COALESCE_MS = 1500;
const LIMIT = 200;

type Kept = UndoEntry & { key: string | null; at: number };

export class UndoStack {
  private done: Kept[] = [];
  private undone: Kept[] = [];
  private busy = false;

  private notify: () => void;

  /** `notify` runs after every change, so buttons can follow. */
  constructor(notify: () => void = () => {}) {
    this.notify = notify;
  }

  /**
   * Record an edit that has just been made. A new edit clears redo. With a
   * `key`, an edit right after another with the same key (within
   * COALESCE_MS of `now`) folds into it: one undo goes back past both.
   */
  push(entry: UndoEntry, key: string | null = null, now = Date.now()): void {
    this.undone = [];
    const top = this.done[this.done.length - 1];
    if (key !== null && top !== undefined && top.key === key && now - top.at <= COALESCE_MS) {
      const kept = entry.fold?.(top) ?? { ...entry, undo: top.undo };
      this.done[this.done.length - 1] = { ...kept, key, at: now };
    } else {
      this.done.push({ ...entry, key, at: now });
      if (this.done.length > LIMIT) this.done.shift();
    }
    this.notify();
  }

  /** Whether `entry` (as pushed) is still the next one Undo would take, so a
   * toast's Undo never takes back a later edit instead. */
  isNewest(entry: UndoEntry): boolean {
    const top = this.done[this.done.length - 1];
    return top !== undefined && top.undo === entry.undo;
  }

  canUndo(): boolean {
    return !this.busy && this.done.length > 0;
  }

  canRedo(): boolean {
    return !this.busy && this.undone.length > 0;
  }

  undoLabel(): string | null {
    return this.done[this.done.length - 1]?.label ?? null;
  }

  redoLabel(): string | null {
    return this.undone[this.undone.length - 1]?.label ?? null;
  }

  /** Undo the newest edit. Resolves to its label, or null when there was
   * nothing to undo or another undo/redo is still running. */
  async undo(): Promise<string | null> {
    return this.step(this.done, this.undone, (e) => e.undo());
  }

  async redo(): Promise<string | null> {
    return this.step(this.undone, this.done, (e) => e.redo());
  }

  private async step(from: Kept[], to: Kept[], run: (e: Kept) => Promise<void>): Promise<string | null> {
    if (this.busy) return null;
    const entry = from.pop();
    if (entry === undefined) return null;
    this.busy = true;
    this.notify();
    try {
      await run(entry);
      // A redone edit is a fresh one: it never folds into the next nudge.
      to.push({ ...entry, key: null });
      return entry.label;
    } finally {
      this.busy = false;
      this.notify();
    }
  }

  clear(): void {
    this.done = [];
    this.undone = [];
    this.notify();
  }
}

/** One stack per text of the open project; another project starts empty. */
export class UndoHistory {
  private project: string | null = null;
  private stacks = new Map<string, UndoStack>();
  /** Called after every push, undo, redo or clear, for the buttons. */
  onChange: () => void = () => {};

  stack(projectId: string, textId: string): UndoStack {
    if (projectId !== this.project) {
      this.project = projectId;
      this.stacks.clear();
    }
    let s = this.stacks.get(textId);
    if (s === undefined) {
      s = new UndoStack(() => this.onChange());
      this.stacks.set(textId, s);
    }
    return s;
  }
}
