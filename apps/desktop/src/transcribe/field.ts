// The transcribe field's state, apart from the DOM: what the field holds,
// what it last matched in the project, and how a commit, a move or a revert
// changes both. The panel is wiring over this. Typed text is never dropped:
// a failed commit keeps it as a draft, and a written text outranks the
// store's copy until the store catches up.
import { needsCommit } from "./logic.ts";

export type EditFn = (segmentId: string, content: string) => Promise<{ content: string }>;

export type FieldOptions = {
  edit: EditFn;
  /** A rejected edit. The typed text is kept as a draft. */
  onFailure?: (error: unknown, segmentId: string) => void;
  /** After every commit settles, whether it saved or failed. The panel refreshes the store here. */
  onSettled?: () => void | Promise<void>;
};

export type FieldController = ReturnType<typeof createFieldController>;

export function createFieldController(opts: FieldOptions) {
  let segmentId: string | null = null;
  let base = "";
  let value = "";
  /** Text whose save failed, by segment. Shown again, marked, until it saves or is reverted. */
  const drafts = new Map<string, string>();
  /** Text sent or saved by this device, by segment, until the store echoes it. */
  const written = new Map<string, string>();
  /** The newest commit per segment; an older one settling late must not undo it. */
  const newest = new Map<string, number>();
  let seq = 0;

  /** The segment's text as this device knows it: what it wrote if the store has not caught up. */
  function known(id: string, stored: string | undefined): string {
    const w = written.get(id);
    if (w === undefined) return stored ?? "";
    if (stored === w) written.delete(id);
    return w;
  }

  async function commit(): Promise<void> {
    if (segmentId === null || !needsCommit(base, value)) return;
    const id = segmentId;
    const content = value;
    const previousBase = base;
    const previousWritten = written.get(id);
    const mine = ++seq;
    newest.set(id, mine);
    base = content;
    written.set(id, content);
    try {
      const result = await opts.edit(id, content);
      if (newest.get(id) === mine) {
        written.set(id, result.content);
        drafts.delete(id);
        if (segmentId === id && base === content) {
          if (value === content) value = result.content;
          base = result.content;
        }
      }
    } catch (e) {
      if (newest.get(id) === mine) {
        drafts.set(id, content);
        if (previousWritten === undefined) written.delete(id);
        else written.set(id, previousWritten);
        if (segmentId === id && base === content) base = previousBase;
      }
      opts.onFailure?.(e, id);
    }
    await opts.onSettled?.();
  }

  return {
    get segmentId() {
      return segmentId;
    },
    get value() {
      return value;
    },
    get base() {
      return base;
    },
    type(next: string): void {
      value = next;
      if (segmentId !== null && !needsCommit(base, next)) drafts.delete(segmentId);
    },
    /** Switch the field to another segment. The previous one is committed first, without waiting. */
    show(id: string, stored: string | undefined): void {
      void commit();
      segmentId = id;
      base = known(id, stored);
      const draft = drafts.get(id);
      if (draft !== undefined && needsCommit(base, draft)) value = draft;
      else {
        drafts.delete(id);
        value = base;
      }
    },
    /** The store changed under the current segment: an untouched field follows it, a draft stays. */
    sync(stored: string | undefined): void {
      if (segmentId === null) return;
      const text = known(segmentId, stored);
      if (value !== base) return;
      base = text;
      value = text;
    },
    /** Esc: back to the segment's saved text, dropping any draft. */
    revert(stored: string | undefined): void {
      if (segmentId === null) return;
      drafts.delete(segmentId);
      base = known(segmentId, stored);
      value = base;
    },
    /** Commit, then hold no segment. */
    leave(): Promise<void> {
      const done = commit();
      segmentId = null;
      return done;
    },
    commit,
    /** What a list row for `id` shows: its draft if one failed to save, else its known text. */
    label(id: string, stored: string | undefined): { text: string; unsaved: boolean } {
      const draft = drafts.get(id);
      return draft !== undefined ? { text: draft, unsaved: true } : { text: known(id, stored), unsaved: false };
    },
    unsavedIds: (): string[] => [...drafts.keys()],
  };
}
