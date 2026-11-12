// The Help conversation (roadmap 67): the kit's wug answers questions about
// wugs from the user guide, through the help service (src-tauri/src/help.rs).
// Online. Shown in the Help panel, or in a card while no project is open.
// The conversation lives here, not in the panel, so closing and reopening
// the panel keeps it until wugs restarts.
import { helpAsk } from "../api";
import { h } from "../dom";
import { currentWug, wugSvg } from "../theme/wug";
import { imeBusy } from "../transcribe/logic";
import { askBlocker, type Exchange, historyOf, SHARE_KEY, shareOn, sourcesLine, spoken } from "./logic";

const exchanges: Exchange[] = [];
let asking = false;
const views = new Set<() => void>();
const redrawAll = () => views.forEach((draw) => draw());

const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

export type HelpView = {
  element: HTMLElement;
  /** The wug that speaks, where the jump lands. */
  wug: HTMLElement;
  focus(): void;
  dispose(): void;
};

/** Builds a view of the conversation. `feedback` opens the Feedback form. */
export function helpView(feedback: () => void): HelpView {
  const wug = h("span", { className: "help-wug" });
  wug.setAttribute("aria-hidden", "true");
  const log = h("div", { className: "help-log", ariaLive: "polite" });
  const question = h("textarea", { className: "help-question", rows: 2, placeholder: "How do I split a segment? What does the yellow tint mean?", ariaLabel: "Your question" });
  const share = h("input", { type: "checkbox", checked: shareOn(localStorage) });
  const why = h("p", { className: "help-why", role: "status" });
  const ask = h("button", { className: "primary", textContent: "Ask" });
  const toFeedback = h("button", { className: "link-button", textContent: "Ask a person instead (Feedback)" });
  // The answer being said, character by character; the rest of the log is whole.
  let talking: { exchange: Exchange; started: number; frame: number } | null = null;

  const bubble = (text: string, sources: string[] = []) => {
    const src = sourcesLine(sources);
    return h("div", { className: "help-say" }, h("p", { className: "help-text", textContent: text }), src === null ? null : h("p", { className: "help-src", textContent: src }));
  };

  const draw = () => {
    wug.innerHTML = wugSvg(currentWug(), "");
    wug.classList.toggle("talking", asking || talking !== null);
    const rows: HTMLElement[] = [bubble("Hi! Ask me how to do something in wugs. I answer from the user guide, so check the parts I name.")];
    for (const e of exchanges) {
      rows.push(h("p", { className: "help-q", textContent: e.question }));
      if (e.problem !== null) rows.push(h("div", { className: "help-say help-problem" }, h("p", { className: "help-text", textContent: e.problem })));
      else if (e.answer === null) rows.push(h("div", { className: "help-say help-wait" }, h("p", { className: "help-text", textContent: "Let me look in the guide…" })));
      else if (talking?.exchange === e) rows.push(bubble(spoken(e.answer, performance.now() - talking.started)));
      else rows.push(bubble(e.answer, e.sources));
    }
    log.replaceChildren(...rows);
    log.scrollTop = log.scrollHeight;
    ask.disabled = asking || askBlocker(question.value) !== null;
    ask.textContent = asking ? "Asking…" : "Ask";
  };

  const say = (e: Exchange) => {
    if (e.answer === null || reducedMotion()) return;
    const answer = e.answer;
    const started = performance.now();
    const step = () => {
      if (talking === null || talking.exchange !== e) return;
      if (spoken(answer, performance.now() - started) === answer) {
        talking = null;
      } else {
        talking.frame = requestAnimationFrame(step);
      }
      draw();
    };
    talking = { exchange: e, started, frame: requestAnimationFrame(step) };
  };

  const send = async () => {
    const text = question.value.trim();
    const blocker = askBlocker(text);
    if (blocker !== null || asking) {
      if (text !== "") why.textContent = blocker ?? "";
      return;
    }
    why.textContent = "";
    const history = historyOf(exchanges);
    const e: Exchange = { question: text, answer: null, sources: [], problem: null };
    exchanges.push(e);
    asking = true;
    question.value = "";
    redrawAll();
    try {
      const r = await helpAsk({ question: text, history, share: share.checked });
      if (r === null) e.problem = "I can't reach the help service from this version of wugs yet. The user guide and the Feedback button still work.";
      else {
        e.answer = r.answer;
        e.sources = r.sources;
      }
    } catch (err) {
      e.problem = String(err);
    }
    asking = false;
    say(e);
    redrawAll();
  };

  question.oninput = () => {
    why.textContent = "";
    draw();
  };
  question.onkeydown = (ev) => {
    if (ev.key === "Enter" && !ev.shiftKey && !imeBusy(ev, false)) {
      ev.preventDefault();
      void send();
    }
  };
  share.onchange = () => localStorage.setItem(SHARE_KEY, share.checked ? "on" : "off");
  ask.onclick = () => void send();
  toFeedback.onclick = feedback;

  const element = h(
    "div",
    { className: "help-view" },
    h("div", { className: "help-stage" }, wug, log),
    h("div", { className: "help-ask" }, question, ask),
    why,
    h("label", { className: "help-share" }, share, " Share my questions with the people who make wugs, so the guide covers what people ask most (each question, the part of the guide it matched and the wugs version)"),
    h("p", { className: "help-foot" }, "Online: your question, your last few questions and answers here, and the wugs version go to the help service, never anything from your project. ", toFeedback),
  );
  views.add(draw);
  draw();
  return {
    element,
    wug,
    focus: () => question.focus(),
    dispose: () => {
      views.delete(draw);
      if (talking !== null) cancelAnimationFrame(talking.frame);
      talking = null;
    },
  };
}
