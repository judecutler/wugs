import { createText } from "../api";
import { emptyState, h } from "../dom";
import { attempt, getState, reloadTexts, selectText } from "../store";
import { sliced, type PanelDef } from "./panel";

async function newText(title: string) {
  if (title === "") return;
  await attempt(async () => {
    const textId = await createText({ title });
    await reloadTexts();
    await selectText(textId);
  });
}

export const textsPanel: PanelDef = {
  title: "Texts",
  create() {
    const el = h("div", { className: "panel panel-texts" });
    const slice = () => {
      const s = getState();
      return s.kind === "open" ? `${s.info.projectId}|${s.text?.id ?? ""}|${JSON.stringify(s.texts)}` : "closed";
    };
    return sliced(el, slice, () => {
      const s = getState();
      if (s.kind === "closed") {
        el.replaceChildren(emptyState("No project open", "Open or create a project from the top bar."));
        return;
      }
      const title = h("input", { placeholder: "New text title" });
      const form = h(
        "form",
        {
          className: "new-text",
          onsubmit: (ev: SubmitEvent) => {
            ev.preventDefault();
            void newText(title.value.trim());
          },
        },
        title,
        h("button", { type: "submit", textContent: "New text" }),
      );
      const list =
        s.texts.length === 0
          ? h("p", { className: "empty", textContent: "No texts yet." })
          : h(
              "ul",
              { className: "text-list" },
              ...s.texts.map((t) =>
                h(
                  "li",
                  { className: t.id === s.text?.id ? "selected" : "", onclick: () => void selectText(t.id) },
                  h("span", { textContent: t.title || "(untitled)" }),
                  h("span", { className: "count", textContent: String(t.segments) }),
                ),
              ),
            );
      el.replaceChildren(form, list);
    });
  },
};
