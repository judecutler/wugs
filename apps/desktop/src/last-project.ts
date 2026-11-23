// Reopening the last project at launch. Storage and the opener are passed
// in so the policy is unit-tested without a webview (`npm test`).

const LAST_PROJECT_KEY = "wugs.lastProject";
const RECENT_KEY = "wugs.recentProjects";
/** How many project folders the menu and the welcome card offer. */
export const RECENT_MAX = 6;

type Store = Pick<Storage, "getItem" | "setItem">;

export const lastProject = (store: Store): string => store.getItem(LAST_PROJECT_KEY) ?? "";

/** Project folders opened on this computer, the latest first. */
export function recentProjects(store: Store): string[] {
  try {
    const v: unknown = JSON.parse(store.getItem(RECENT_KEY) ?? "[]");
    const list = Array.isArray(v) ? v.filter((x): x is string => typeof x === "string" && x !== "") : [];
    const last = lastProject(store);
    // Older versions remembered only the last one.
    return (last === "" || list.includes(last) ? list : [last, ...list]).slice(0, RECENT_MAX);
  } catch {
    return lastProject(store) === "" ? [] : [lastProject(store)];
  }
}

export function rememberProject(store: Store, path: string): void {
  store.setItem(LAST_PROJECT_KEY, path);
  const list = [path, ...recentProjects(store).filter((p) => p !== path)].slice(0, RECENT_MAX);
  store.setItem(RECENT_KEY, JSON.stringify(list));
}

/** Take a folder off the list (one that no longer opens). */
export function forgetProject(store: Store, path: string): void {
  store.setItem(RECENT_KEY, JSON.stringify(recentProjects(store).filter((p) => p !== path)));
  if (lastProject(store) === path) store.setItem(LAST_PROJECT_KEY, "");
}

export type AutoOpen = { kind: "skipped" } | { kind: "opened" } | { kind: "failed"; path: string; error: string };

/** Try the remembered project once, unless one is already open: a person who
 * clicked faster than the launch must not have their choice replaced. A failure keeps the remembered path: the
 * folder may sit on a drive that is not plugged in, or be open elsewhere
 * for the moment. It cannot loop, because it runs once per launch. */
export async function autoOpen(store: Store, researcher: string, projectOpen: () => boolean, open: (path: string, researcher: string) => Promise<void>, describe: (e: unknown) => string): Promise<AutoOpen> {
  const path = lastProject(store);
  if (path === "" || researcher.trim() === "" || projectOpen()) return { kind: "skipped" };
  try {
    await open(path, researcher);
    return { kind: "opened" };
  } catch (e) {
    return { kind: "failed", path, error: describe(e) };
  }
}
