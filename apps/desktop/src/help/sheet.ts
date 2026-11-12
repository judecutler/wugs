// The Help sheet (roadmap 67): ask a question about wugs and get an answer
// from the user guide, through the help service (src-tauri/src/help.rs).
// Online. A question is kept for the "most asked" report only when the
// person ticked the box.
import { helpAsk } from "../api";
import { h } from "../dom";
import { askBlocker, type Exchange, historyOf, SHARE_KEY, shareOn, sourcesLine } from "./logic";

let open: HTMLElement | null = null;

/** Opens the sheet over the window. `feedback` opens the Feedback form. */
export function openHelp(feedback: () => void): void {
  if (open !== null) {
    open.querySelector("textarea")?.focus();
    return;
  }
  const exchanges: Exchange[] = [];
  let asking = false;
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  const log = h("div", { className: "help-log", ariaLive: "polite" });
  const question = h("textarea", { className: "help-question", rows: 3, placeholder: "How do I split a segment? What does the yellow tint mean?" });
  const share = h("input", { type: "checkbox", checked: shareOn(localStorage) });
  const why = h("p", { className: "feedback-why", role: "status" });
  const ask = h("button", { className: "primary", textContent: "Ask" });
  const close = h("button", { textContent: "Close" });
  const toFeedback = h("button", { className: "link-button", textContent: "Ask a person instead (Feedback)" });

  const draw = () => {
    log.replaceChildren(
      ...(exchanges.length === 0
        ? [h("p", { className: "help-empty", textContent: "Answers come from the wugs user guide. Ask about any panel, button or task." })]
        : exchanges.flatMap((e) => {
            const src = sourcesLine(e.sources);
            return [
              h("p", { className: "help-q", textContent: e.question }),
              e.problem !== null
                ? h("p", { className: "help-a help-problem", textContent: e.problem })
                : e.answer === null
                  ? h("p", { className: "help-a help-wait", textContent: "Looking in the guide…" })
                  : h("div", { className: "help-a" }, h("p", { className: "help-text", textContent: e.answer }), src === null ? null : h("p", { className: "help-src", textContent: src })),
            ];
          })),
    );
    log.scrollTop = log.scrollHeight;
    ask.disabled = asking || askBlocker(question.value) !== null;
    ask.textContent = asking ? "Asking…" : "Ask";
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
    draw();
    try {
      const r = await helpAsk({ question: text, history, share: share.checked });
      if (r === null) e.problem = "This version of wugs is not connected to the help service yet. The user guide and the Feedback button still work.";
      else {
        e.answer = r.answer;
        e.sources = r.sources;
      }
    } catch (err) {
      e.problem = String(err);
    }
    asking = false;
    draw();
    question.focus();
  };

  let overlay: HTMLElement;
  const shut = () => {
    overlay.remove();
    if (open === overlay) open = null;
    window.removeEventListener("keydown", onKey, true);
    opener?.focus();
  };
  const onKey = (ev: KeyboardEvent) => {
    if (ev.key === "Escape") {
      ev.stopImmediatePropagation();
      shut();
    }
    if (ev.key === "Enter" && !ev.shiftKey && !ev.isComposing && document.activeElement === question) {
      ev.preventDefault();
      void send();
    }
    if (ev.key === "Tab") {
      const stops = [...card.querySelectorAll<HTMLElement>("textarea, input, button:not(:disabled)")];
      const first = stops[0];
      const last = stops[stops.length - 1];
      if (first === undefined || last === undefined) return;
      if (ev.shiftKey && document.activeElement === first) {
        ev.preventDefault();
        last.focus();
      } else if (!ev.shiftKey && document.activeElement === last) {
        ev.preventDefault();
        first.focus();
      } else if (!card.contains(document.activeElement)) {
        ev.preventDefault();
        first.focus();
      }
    }
  };
  question.oninput = () => {
    why.textContent = "";
    draw();
  };
  share.onchange = () => localStorage.setItem(SHARE_KEY, share.checked ? "on" : "off");
  ask.onclick = () => void send();
  close.onclick = shut;
  toFeedback.onclick = () => {
    shut();
    feedback();
  };

  const card = h(
    "div",
    { className: "feedback-card help-card", role: "dialog", ariaModal: "true", ariaLabel: "Help" },
    h("h2", { textContent: "Help" }),
    h("p", { className: "feedback-lede", textContent: "Ask how to do something in wugs. Online: your question goes to the wugs help service, never anything from your project." }),
    log,
    h("label", { className: "feedback-label" }, "Your question", question),
    h("label", { className: "feedback-check" }, share, " Share my questions with the people who make wugs, so the guide covers what people ask most (the question only)"),
    why,
    h("div", { className: "feedback-actions" }, toFeedback, close, ask),
  );
  overlay = h("div", { className: "feedback-overlay" }, card);
  open = overlay;
  window.addEventListener("keydown", onKey, true);
  document.body.append(overlay);
  draw();
  question.focus();
}
