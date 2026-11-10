// Reopening the last project at launch. Storage and the opener are passed
// in so the policy is unit-tested without a webview (`npm test`).

const LAST_PROJECT_KEY = "wugs.lastProject";

type Store = Pick<Storage, "getItem" | "setItem">;

export const lastProject = (store: Store): string => store.getItem(LAST_PROJECT_KEY) ?? "";
export const rememberProject = (store: Store, path: string) => store.setItem(LAST_PROJECT_KEY, path);

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
