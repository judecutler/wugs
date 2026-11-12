// Where commands come from. The shell registers its global ones; a panel
// registers its own while it is open and takes them back when it closes, so
// the palette lists what the person can do in the workspace as it is now.
import type { Command } from "./logic";

const sources = new Map<string, () => Command[]>();

/** Add a source of commands under a key; returns the way to take it back. */
export function provide(key: string, list: () => Command[]): () => void {
  sources.set(key, list);
  return () => {
    if (sources.get(key) === list) sources.delete(key);
  };
}

/** Every command now, in the order the sources were added; a repeated id keeps its first. */
export function allCommands(): Command[] {
  const seen = new Set<string>();
  const out: Command[] = [];
  for (const list of sources.values()) {
    for (const c of list()) {
      if (seen.has(c.id)) continue;
      seen.add(c.id);
      out.push(c);
    }
  }
  return out;
}
