// The Tutorial button (owner, 2026-11-12): in the top bar until the tutorial
// project is first opened, from there or anywhere else; then it jumps into
// the Project menu, where "Open the tutorial project" lives from then on.

export const FILED_KEY = "wugs.tutorial.filed";

type Store = Pick<Storage, "getItem" | "setItem">;

/** Whether the Tutorial button has gone into the Project menu. */
export const tutorialFiled = (s: Store): boolean => s.getItem(FILED_KEY) === "1";

export function fileTutorial(s: Store): void {
  try {
    s.setItem(FILED_KEY, "1");
  } catch {
    // Storage refused: the button stays and goes next time.
  }
}
