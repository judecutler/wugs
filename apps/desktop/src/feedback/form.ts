// The Send feedback form (roadmap 65): what happened, an optional reply
// address, an optional screenshot or video, and the technical details the
// person can see and leave out. Online; a report written offline waits in
// the outbox (src-tauri/src/feedback.rs).
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { feedbackDetails, feedbackSend } from "../api";
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
  let sending = false;

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
    send.disabled = sending || blocker !== null;
    send.textContent = sending ? "Sending…" : "Send feedback";
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

  const close = () => {
    open?.remove();
    open = null;
    document.removeEventListener("keydown", onKey, true);
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape" && !sending) {
      e.stopPropagation();
      close();
    }
  };
  cancel.onclick = close;
  send.onclick = async () => {
    if (sendBlocker(message.value, contact.value) !== null || sending) return;
    sending = true;
    refresh();
    localStorage.setItem(CONTACT_KEY, contact.value.trim());
    try {
      const outcome = await feedbackSend({ message: message.value, contact: contact.value, details: includeDetails.checked ? page : null, attachment });
      // A refused report with nothing waiting stays open so it can be fixed.
      if (outcome.sent || outcome.waiting > 0) close();
      const { tone, lines } = outcomeLines(outcome);
      showToast({ tone, lines });
    } catch (e) {
      // Checked before anything was kept (too large, unreadable): fix and retry.
      why.textContent = String(e);
    } finally {
      sending = false;
      if (open !== null) refresh();
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
  open = h("div", { className: "feedback-overlay" }, card);
  document.addEventListener("keydown", onKey, true);
  document.body.append(open);
  refresh();
  message.focus();
}
