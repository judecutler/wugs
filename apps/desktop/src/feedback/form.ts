// The Send feedback form (roadmap 65): what happened, an optional reply
// address, an optional screenshot or video, and the technical details the
// person can see and leave out. Online; a report written offline waits in
// the outbox (src-tauri/src/feedback.rs).
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { feedbackDeliver, feedbackDetails, feedbackKeep } from "../api";
import { h } from "../dom";
import { showToast } from "../store";
import { fileName, outcomeLines, recordingTip, sendBlocker, systemOf } from "./logic";

const CONTACT_KEY = "wugs.feedback.contact";

let open: HTMLElement | null = null;

/** Opens the form over the window. `page` is what the page knows (kit, theme). */
export function openFeedback(page: Record<string, unknown>): void {
  if (open !== null) {
    open.querySelector("textarea")?.focus();
    return;
  }
  let attachment: string | null = null;
  // Checking the report and copying the attachment; the form closes once kept.
  let keeping = false;
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  const message = h("textarea", { className: "feedback-message", rows: 6, placeholder: "What were you doing, and what went wrong or could be better?" });
  const contact = h("input", { type: "email", className: "feedback-contact", placeholder: "you@example.org", autocomplete: "email", value: localStorage.getItem(CONTACT_KEY) ?? "" });
  const attached = h("span", { className: "feedback-file" });
  const attach = h("button", { textContent: "Attach a screenshot or video…" });
  const unattach = h("button", { textContent: "Remove", hidden: true });
  const includeDetails = h("input", { type: "checkbox", checked: true });
  const detailsText = h("pre", { className: "feedback-details-text" });
  const why = h("p", { className: "feedback-why", role: "status" });
  const send = h("button", { className: "primary", textContent: "Send feedback" });
  const cancel = h("button", { textContent: "Cancel" });

  const refresh = () => {
    const blocker = sendBlocker(message.value, contact.value);
    send.disabled = keeping || blocker !== null;
    cancel.disabled = keeping;
    attach.disabled = keeping;
    send.textContent = keeping ? "Saving…" : "Send feedback";
    why.textContent = message.value.trim() === "" ? "" : (blocker ?? "");
    attached.textContent = attachment === null ? "" : fileName(attachment);
    unattach.hidden = attachment === null;
    attach.textContent = attachment === null ? "Attach a screenshot or video…" : "Choose another…";
  };
  message.oninput = refresh;
  contact.oninput = refresh;
  attach.onclick = async () => {
    const path = await pickFile({
      title: "A screenshot or short video (up to 95 MB)",
      filters: [{ name: "Screenshots and videos", extensions: ["png", "jpg", "jpeg", "gif", "webp", "mp4", "mov", "m4v", "webm", "mkv"] }],
    });
    if (typeof path === "string") attachment = path;
    refresh();
  };
  unattach.onclick = () => {
    attachment = null;
    refresh();
  };

  let overlay: HTMLElement;
  const close = () => {
    overlay.remove();
    if (open === overlay) open = null;
    window.removeEventListener("keydown", onKey, true);
    opener?.focus();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      // The form is in front: nothing behind it reacts to Escape.
      e.stopImmediatePropagation();
      // Escape never throws away what was typed: Cancel does that.
      if (keeping) return;
      if (message.value.trim() === "") close();
      else why.textContent = "To close without sending, click Cancel. What you wrote will be discarded.";
    }
    if (e.key === "Tab") {
      // Keep the keyboard inside the form while it is open.
      const stops = [...card.querySelectorAll<HTMLElement>("textarea, input, button:not(:disabled), summary")].filter((el) => el.offsetParent !== null);
      const first = stops[0];
      const last = stops[stops.length - 1];
      if (first === undefined || last === undefined) return;
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      } else if (!card.contains(document.activeElement)) {
        e.preventDefault();
        first.focus();
      }
    }
  };
  cancel.onclick = close;
  send.onclick = async () => {
    if (sendBlocker(message.value, contact.value) !== null || keeping) return;
    keeping = true;
    refresh();
    localStorage.setItem(CONTACT_KEY, contact.value.trim());
    let id: string;
    try {
      id = await feedbackKeep({ message: message.value, contact: contact.value, details: includeDetails.checked ? page : null, attachment });
    } catch (e) {
      // Checked before anything was kept (too large, unreadable): fix and send again.
      keeping = false;
      refresh();
      why.textContent = String(e);
      return;
    }
    // Kept on this computer: nothing is lost from here, so the form can go.
    close();
    if (attachment !== null) showToast({ tone: "info", lines: ["Sending your feedback and its attachment…"] });
    try {
      const { tone, lines } = outcomeLines(await feedbackDeliver({ id }));
      showToast({ tone, lines });
    } catch (e) {
      showToast({ tone: "warn", lines: ["Your feedback is saved on this computer and will be sent later.", String(e)] });
    }
  };

  void feedbackDetails({ page }).then((d) => {
    detailsText.textContent = JSON.stringify(d, null, 2);
  });

  const card = h(
    "div",
    { className: "feedback-card", role: "dialog", ariaModal: "true", ariaLabel: "Send feedback" },
    h("h2", { textContent: "Send feedback" }),
    h("p", { className: "feedback-lede", textContent: "Tell us what went wrong or what would help. It goes to the people who make wugs, nowhere public." }),
    h("label", { className: "feedback-label", textContent: "What happened?" }, message),
    h("label", { className: "feedback-label", textContent: "Your email, if you would like a reply (optional)" }, contact),
    h("div", { className: "feedback-attach" }, attach, attached, unattach),
    h("p", { className: "feedback-tip", textContent: recordingTip(systemOf(navigator.userAgent)) }),
    h("p", { className: "feedback-caution", textContent: "A screenshot or video shows whatever is on screen, including recordings, names and transcriptions from your project. Check it before you attach it." }),
    h("label", { className: "feedback-check" }, includeDetails, " Include technical details (the wugs version and your system; nothing from your project)"),
    h("details", { className: "feedback-details" }, h("summary", { textContent: "See exactly what is included" }), detailsText),
    why,
    h("div", { className: "feedback-actions" }, cancel, send),
  );
  // No close on a click outside: a stray click must not lose what was typed.
  overlay = h("div", { className: "feedback-overlay" }, card);
  open = overlay;
  // On window, capturing: it runs before any listener on the document.
  window.addEventListener("keydown", onKey, true);
  document.body.append(overlay);
  refresh();
  message.focus();
}
