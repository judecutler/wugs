// The first screen when no project is open: the kit's wug, who is at the
// keyboard, and the three ways in (create, open, join from another device).
// The workspace stays mounted behind it, so joining shows the Devices panel.
import { h, wugArt } from "./dom";
import type { Kit } from "./kits/logic";

export type WelcomeActions = {
  researcher: string;
  setResearcher(value: string): void;
  create(): void;
  open(): void;
  reopen: { label: string; run(): void } | null;
  join(): void;
  /** Open the sample project: every panel already filled in, to look around. */
  sample(): void;
};

/** The folder's own name, for "Reopen «name»". */
export const folderName = (path: string): string => path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? path;

export function welcomeView(kit: Kit | null, a: WelcomeActions): HTMLElement {
  const who = h("input", {
    id: "welcome-researcher",
    className: "welcome-researcher",
    placeholder: "e.g. your initials",
    value: a.researcher,
    autocomplete: "off",
    spellcheck: false,
  });
  who.oninput = () => a.setResearcher(who.value);
  const name = kit?.name ?? "wugs";
  return h(
    "main",
    { className: "welcome" },
    h(
      "div",
      { className: "welcome-card" },
      wugArt("welcome-wug", `${name} wug`),
      h("h1", { textContent: `Welcome to ${name}` }),
      h("p", { className: "welcome-lede", textContent: kit?.description || "Record, transcribe, gloss and build a dictionary, offline." }),
      h(
        "div",
        { className: "welcome-who" },
        h("label", { htmlFor: "welcome-researcher", textContent: "Your researcher id" }),
        who,
        h("p", { className: "note", textContent: "Every edit you make is signed with it, so a shared project shows who did what." }),
      ),
      h(
        "div",
        { className: "welcome-actions" },
        h("button", { className: "primary", textContent: "Create a project", onclick: a.create }),
        h("button", { textContent: "Open a project folder", onclick: a.open }),
      ),
      h(
        "div",
        { className: "welcome-sample" },
        h("button", { textContent: "Open the sample project", onclick: a.sample }),
        h("p", { className: "note", textContent: "A short made-up recording, already transcribed, glossed and in a dictionary, with a two-minute tour. Change anything; it is yours to try things on." }),
      ),
      a.reopen === null ? null : h("button", { className: "welcome-link", textContent: a.reopen.label, onclick: a.reopen.run }),
      h("button", { className: "welcome-link", textContent: "Join a project from another device", onclick: a.join }),
    ),
  );
}
