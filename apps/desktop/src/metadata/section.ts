// The Texts panel's "Details" section: the selected text's document
// metadata (author, date, genre…), each value saved on Enter or when focus
// leaves it, a row to add a key, and a line showing how to search by it.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, importMetadata, projectLanguage, setTextMetadata, textMetadata } from "../api";
import { citation } from "./cite";
import { h } from "../dom";
import { showToast } from "../store";
import { importLines, keyOf, label, queryHint, suggestions, type MetaView } from "./logic";
import { detailsChanged } from "./scope";

let shown: { text: string; view: MetaView } | null = null;
let loading: string | null = null;
// Typed but not saved, by `<text>|<key>`, so a redraw while typing keeps it.
const drafts = new Map<string, string>();
let newKey = "";
let newValue = "";
let redraw: () => void = () => {};

async function load(text: string, force = false) {
  if (!force && (shown?.text === text || loading === text)) return;
  loading = text;
  try {
    const view = await textMetadata({ textId: text });
    if (loading === text) shown = { text, view };
  } catch {
    if (loading === text) shown = { text, view: { fields: [], keys: [] } };
  }
  if (loading === text) loading = null;
  redraw();
}

async function save(text: string, key: string, value: string) {
  const now = shown?.text === text ? shown.view.fields.find((f) => f.key === key)?.value ?? "" : null;
  if (now !== null && now === value.trim()) return;
  try {
    const view = await setTextMetadata({ textId: text, key, value });
    if (shown?.text === text) shown = { text, view };
    detailsChanged();
  } catch (e) {
    showToast({ tone: "error", lines: [errorText(e)] });
  }
  redraw();
}

/** Bring in a table of metadata; `after` refreshes what depends on it. */
export async function importMetadataTable(path: string) {
  const r = await importMetadata({ path });
  detailsChanged();
  showToast({ tone: r.unmatchedCount > 0 ? "warn" : "info", lines: importLines(r) });
  if (shown !== null) await load(shown.text, true);
}

function valueField(text: string, key: string, value: string): HTMLInputElement {
  const id = `${text}|${key}`;
  const input = h("input", { value: drafts.get(id) ?? value, spellcheck: false, className: "meta-value" });
  input.setAttribute("aria-label", label(key));
  input.dataset.key = key;
  input.oninput = () => drafts.set(id, input.value);
  const commit = () => {
    drafts.delete(id);
    void save(text, key, input.value);
  };
  input.onchange = commit;
  input.onkeydown = (ev: KeyboardEvent) => {
    if (ev.key === "Enter" && !ev.isComposing) commit();
  };
  return input;
}

/** Copy how to cite the text, as a reference line or as BibTeX. */
async function copyCitation(text: { id: string; title: string }, bibtex: boolean) {
  try {
    const view = shown?.text === text.id ? shown.view : await textMetadata({ textId: text.id });
    const lang = await projectLanguage();
    const c = citation(text.title, new Map(view.fields.map((f) => [f.key, f.value])), lang);
    await writeText(bibtex ? c.bibtex : c.text);
    showToast({ tone: "info", lines: [bibtex ? "Copied a BibTeX entry for this text." : `Copied: ${c.text}`, "It is built from the details above: add an archive, a handle or DOI, a speaker or a date there to make it fuller."] });
  } catch (e) {
    showToast({ tone: "error", lines: [errorText(e)] });
  }
}

/** The section for one text; `draw` is called again when its data arrives. */
export function detailsSection(text: { id: string; title: string }, draw: () => void): HTMLElement {
  redraw = draw;
  void load(text.id);
  const view = shown?.text === text.id ? shown.view : null;
  const rows: HTMLElement[] = (view?.fields ?? []).map((f) =>
    h(
      "div",
      { className: "meta-row" },
      h("label", { className: "meta-key", textContent: label(f.key), title: `Search: ${queryHint(f.key, f.value)}` }),
      valueField(text.id, f.key, f.value),
      h("button", {
        type: "button",
        className: "meta-remove",
        textContent: "×",
        title: `Clear ${label(f.key).toLowerCase()}`,
        tabIndex: -1,
        onclick: () => void save(text.id, f.key, ""),
      }),
    ),
  );
  const listId = "meta-key-suggestions";
  const keyInput = h("input", { value: newKey, placeholder: "Name", size: 14, title: "A name, like genre, year or speaker", spellcheck: false });
  keyInput.setAttribute("list", listId);
  keyInput.setAttribute("aria-label", "New detail's name");
  keyInput.dataset.key = "+key";
  keyInput.oninput = () => (newKey = keyInput.value);
  const valueInput = h("input", { value: newValue, placeholder: "Value", spellcheck: false, className: "meta-value" });
  valueInput.setAttribute("aria-label", "New detail's value");
  valueInput.dataset.key = "+value";
  valueInput.oninput = () => (newValue = valueInput.value);
  const add = () => {
    const key = keyOf(newKey);
    if (key === null) {
      showToast({ tone: "warn", lines: ["A detail's name needs a letter first, like genre or year."] });
      return;
    }
    if (newValue.trim() === "") {
      valueInput.focus();
      return;
    }
    const value = newValue;
    newKey = "";
    newValue = "";
    void save(text.id, key, value).then(() => {
      // Keep typing: focus goes back to the empty name field.
      document.querySelector<HTMLInputElement>('.meta-add input[data-key="+key"]')?.focus();
    });
  };
  for (const el of [keyInput, valueInput]) {
    el.onkeydown = (ev: KeyboardEvent) => {
      if (ev.key === "Enter" && !ev.isComposing) add();
    };
  }
  const datalist = h("datalist", { id: listId }, ...(view === null ? [] : suggestions(view)).map((k) => h("option", { value: k })));
  return h(
    "section",
    { className: "meta" },
    h("h3", { textContent: `Details of «${text.title || "untitled"}»` }),
    view === null
      ? h("p", { className: "meta-note", textContent: "Loading…" })
      : rows.length === 0
        ? h("p", { className: "meta-note", textContent: "Who wrote or spoke it, when, what kind of text: corpus searches can then pick texts by these, as in :: match.text_genre=\"news\"." })
        : h("div", { className: "meta-rows" }, ...rows),
    h("div", { className: "meta-row meta-add" }, keyInput, valueInput, h("button", { type: "button", textContent: "Add", onclick: add }), datalist),
    h(
      "div",
      { className: "meta-cite" },
      h("button", { type: "button", textContent: "Copy citation", title: "How to cite this text in a paper (speakers, year, title, language, place, archive, handle), from its details", onclick: () => void copyCitation(text, false) }),
      h("button", { type: "button", textContent: "Copy BibTeX", title: "The same citation as a BibTeX entry for LaTeX, Zotero or JabRef", onclick: () => void copyCitation(text, true) }),
    ),
  );
}
