// The Help conversation (roadmap 67): the kit's wug answers questions about
// wugs from the user guide, through the help service (src-tauri/src/help.rs).
// Online. Shown in the Help panel, or in a card while no project is open.
// The conversation lives here, not in the panel, so closing and reopening
// the panel keeps it until wugs restarts.
//
// The log is built row by row and never redrawn whole: while the wug "says"
// an answer only that bubble's text changes, and a screen reader hears each
// finished answer once, from a separate status line.
import { helpAsk } from "../api";
import { h } from "../dom";
import { currentWug, wugSvg } from "../theme/wug";
import { imeBusy } from "../transcribe/logic";
import { askBlocker, type Exchange, historyOf, SHARE_KEY, shareOn, sourcesLine, spoken } from "./logic";

const exchanges: Exchange[] = [];
let asking = false;
/** Every open view, told when an exchange changes. */
const views = new Set<(e: Exchange) => void>();
const changed = (e: Exchange) => views.forEach((tell) => tell(e));

const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const GREETING = "Hi! Ask me how to do something in wugs. I answer from the user guide.";

export type HelpView = {
  element: HTMLElement;
  /** The wug that speaks, where the jump lands. */
  wug: HTMLElement;
  focus(): void;
  dispose(): void;
};

/** Builds a view of the conversation. `feedback` opens the Feedback form. */
export function helpView(feedback: () => void): HelpView {
  let disposed = false;
  const wug = h("span", { className: "help-wug" });
  wug.setAttribute("aria-hidden", "true");
  let drawnLook = "";
  // Drawn only when the kit's wug changes, so its CSS animations keep running.
  const drawWug = () => {
    const look = JSON.stringify(currentWug());
    if (look === drawnLook) return;
    drawnLook = look;
    wug.innerHTML = wugSvg(currentWug(), "");
  };
  const log = h("div", { className: "help-log" });
  const announce = h("p", { className: "visually-hidden", role: "status" });
  const question = h("textarea", { className: "help-question", rows: 2, placeholder: "How do I split a segment? What does the yellow tint mean?", ariaLabel: "Your question" });
  const share = h("input", { type: "checkbox", checked: shareOn(localStorage) });
  const why = h("p", { className: "help-why", role: "status" });
  const ask = h("button", { className: "primary", textContent: "Ask" });
  const toFeedback = h("button", { className: "link-button", textContent: "Give wugs feedback" });
  /** Each exchange's answer row, so a change touches only that row. */
  const rows = new Map<Exchange, HTMLElement>();
  let typing: { exchange: Exchange; started: number; frame: number } | null = null;

  // The log follows new text only while the person is reading its end.
  const atBottom = () => log.scrollHeight - log.scrollTop - log.clientHeight < 24;
  const keepBottom = (was: boolean) => {
    if (was) log.scrollTop = log.scrollHeight;
  };

  const bubble = (text: string, sources: string[] = [], extra = "") => {
    const src = sourcesLine(sources);
    return h("div", { className: `help-say ${extra}`.trim() }, h("p", { className: "help-text", textContent: text }), src === null ? null : h("p", { className: "help-src", textContent: src }));
  };

  const answerRow = (e: Exchange): HTMLElement => {
    if (e.problem !== null) return bubble(e.problem, [], "help-problem");
    if (e.answer === null) return bubble("Let me look in the guide…", [], "help-wait");
    return bubble(e.answer, e.sources);
  };

  const status = () => {
    wug.classList.toggle("talking", asking || typing !== null);
    ask.disabled = asking || askBlocker(question.value) !== null;
    ask.textContent = asking ? "Asking…" : "Ask";
  };

  const say = (e: Exchange, row: HTMLElement) => {
    if (e.answer === null || reducedMotion() || disposed) return;
    const answer = e.answer;
    const text = row.querySelector(".help-text")?.firstChild;
    const src = row.querySelector(".help-src");
    if (!(text instanceof Text)) return;
    src?.setAttribute("hidden", "");
    text.data = "";
    const step = () => {
      if (disposed || typing?.exchange !== e) return;
      const was = atBottom();
      text.data = spoken(answer, performance.now() - typing.started);
      keepBottom(was);
      if (text.data === answer) {
        typing = null;
        src?.removeAttribute("hidden");
        status();
      } else {
        typing.frame = requestAnimationFrame(step);
      }
    };
    typing = { exchange: e, started: performance.now(), frame: requestAnimationFrame(step) };
  };

  // An exchange this view does not show yet is added; a known one is redrawn in place.
  const tell = (e: Exchange) => {
    const was = atBottom();
    const row = rows.get(e);
    const next = answerRow(e);
    rows.set(e, next);
    if (row === undefined) log.append(h("p", { className: "help-q", textContent: e.question }), next);
    else {
      row.replaceWith(next);
      say(e, next);
    }
    keepBottom(was);
    status();
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
    changed(e);
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
    changed(e);
  };

  question.oninput = () => {
    why.textContent = "";
    status();
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

  drawWug();
  log.append(bubble(GREETING));
  for (const e of exchanges) {
    const row = answerRow(e);
    rows.set(e, row);
    log.append(h("p", { className: "help-q", textContent: e.question }), row);
  }
  const element = h(
    "div",
    { className: "help-view" },
    h("div", { className: "help-stage" }, wug, log),
    announce,
    h("div", { className: "help-ask" }, question, ask),
    why,
    h("label", { className: "help-share" }, share, " Share my questions with the people who make wugs, so the guide covers what people ask most (each question, the part of the guide it matched and the wugs version)"),
    h("p", { className: "help-foot" }, "Still having trouble? ", toFeedback, " and we'll fix it as soon as possible."),
    h("p", { className: "help-foot" }, "Online: your question, your last few questions and answers here, and the wugs version go to the help service, never anything from your project."),
  );
  // Announced once per finished answer (the views share one conversation, so each says it).
  const announceDone = (e: Exchange) => {
    if (e.answer !== null || e.problem !== null) announce.textContent = e.answer ?? e.problem ?? "";
  };
  const tellAll = (e: Exchange) => {
    tell(e);
    announceDone(e);
  };
  views.add(tellAll);
  status();
  return {
    element,
    wug,
    focus: () => {
      drawWug();
      question.focus();
    },
    dispose: () => {
      disposed = true;
      views.delete(tellAll);
      if (typing !== null) cancelAnimationFrame(typing.frame);
      typing = null;
    },
  };
}
