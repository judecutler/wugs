// The Sentence panel's "Tag and parse" bar: which tagger runs (the
// `morphosyntax` capability's chooser), what it still needs, and one button
// that tags the text's untagged sentences.
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { capabilityChoose, errorText, morphosyntaxAllowWeb, morphosyntaxChoose, morphosyntaxChooseSpacy, morphosyntaxChooseStanza, morphosyntaxRun, morphosyntaxSetup, type CapabilityView, type MorphosyntaxSetup } from "../api";
import { choiceOptions, statusLine } from "../capability/logic";
import { h } from "../dom";
import { capability, getState, refreshAll, refreshCapabilities, showToast } from "../store";
import { setupFor, spacyNote, stanzaNote, taggedLines, WEB_NOTE } from "./logic";

let setup: { project: string; value: MorphosyntaxSetup } | null = null;
let loading = false;
let running: string | null = null;
let model = "";
type PyTool = "spacy" | "stanza";
const EMPTY = { python: null, pipeline: null, about: null };
/** Each Python tagger's model field while it is typed (null: show the chosen one). */
const pipelineDraft: Record<PyTool, string | null> = { spacy: null, stanza: null };
let checking: PyTool | null = null;
let open = false;
let redraw: () => void = () => {};

async function loadSetup(project: string, force = false) {
  if (loading || (!force && setup?.project === project)) return;
  loading = true;
  try {
    setup = { project, value: await morphosyntaxSetup() };
  } catch {
    setup = { project, value: { local: { program: null, model: null }, spacy: EMPTY, stanza: EMPTY, webOk: false } };
  }
  loading = false;
  redraw();
}

async function act(project: string, step: () => Promise<unknown>) {
  try {
    await step();
  } catch (e) {
    showToast({ tone: "error", lines: [errorText(e)] });
  }
  await refreshCapabilities();
  await loadSetup(project, true);
}

async function choose(project: string, what: "program" | "model") {
  const path = await pickFile({
    title: what === "program" ? "Choose the UDPipe program" : "Choose a UDPipe model (.udpipe) for this language",
    filters: what === "model" ? [{ name: "UDPipe models", extensions: ["udpipe"] }, { name: "All files", extensions: ["*"] }] : [],
  });
  if (typeof path !== "string") return;
  const now = setup?.value.local ?? { program: null, model: null };
  await act(project, () => morphosyntaxChoose(what === "program" ? { program: path, model: now.model } : { program: now.program, model: path }));
}

const TOOL = {
  spacy: { name: "spaCy", field: "spaCy pipeline", placeholder: "Pipeline, such as en_core_web_sm", check: "Load the pipeline once to check it, then use it", note: spacyNote },
  stanza: { name: "Stanza", field: "Stanza language", placeholder: "Language code, such as de or grc", check: "Load the language's models once to check they are downloaded, then use them", note: stanzaNote },
} as const;

async function choosePy(project: string, tool: PyTool, python: string | null, model: string | null) {
  checking = tool;
  redraw();
  await act(project, async () => {
    await (tool === "spacy" ? morphosyntaxChooseSpacy({ python, pipeline: model }) : morphosyntaxChooseStanza({ python, language: model }));
    pipelineDraft[tool] = null;
  });
  checking = null;
  redraw();
}

async function pickPython(project: string, tool: PyTool) {
  const path = await pickFile({ title: `Choose the Python that has ${TOOL[tool].name} (a virtual environment's python works too)` });
  if (typeof path !== "string") return;
  const now = setup?.value[tool] ?? EMPTY;
  await choosePy(project, tool, path, now.pipeline);
}

/** A Python tagger's rows: its Python, the model's name with Use, and what the check found. */
function pyRows(project: string, tool: PyTool): HTMLElement[] {
  const sp = setup?.project === project ? setup.value[tool] : null;
  const busy = checking !== null;
  const field = h("input", { value: pipelineDraft[tool] ?? sp?.pipeline ?? "", placeholder: TOOL[tool].placeholder, spellcheck: false, size: 30, disabled: busy });
  field.setAttribute("aria-label", TOOL[tool].field);
  field.oninput = () => (pipelineDraft[tool] = field.value);
  const use = () => void choosePy(project, tool, sp?.python ?? null, field.value.trim() === "" ? null : field.value.trim());
  field.onkeydown = (e) => {
    if (e.key === "Enter" && !e.isComposing) use();
  };
  return [
    h(
      "div",
      { className: "st-tag-row" },
      h("button", { type: "button", textContent: sp?.python ? `${TOOL[tool].name}'s Python…` : `Choose Python for ${TOOL[tool].name}…`, title: sp?.python ?? "python3 or python on PATH when not chosen", disabled: busy, onclick: () => void pickPython(project, tool) }),
      field,
      h("button", { type: "button", textContent: checking === tool ? "Checking…" : "Use", title: TOOL[tool].check, disabled: busy, onclick: use }),
    ),
    h("p", { className: "note", textContent: TOOL[tool].note(sp) }),
  ];
}

async function run(textId: string, title: string) {
  if (running !== null) return;
  running = textId;
  redraw();
  try {
    const r = await morphosyntaxRun({ textId, model });
    showToast({ tone: r.differ > 0 ? "warn" : "info", lines: taggedLines(r, title) });
    await refreshAll();
  } catch (e) {
    showToast({ tone: "error", lines: [errorText(e)] });
  }
  running = null;
  redraw();
}

/** The bar for the text on screen; `draw` is called again when its data arrives. */
export function tagBar(text: { id: string; title: string }, draw: () => void): HTMLElement {
  redraw = draw;
  const s = getState();
  if (s.kind !== "open") return h("span");
  const project = s.info.projectId;
  void loadSetup(project);
  const v: CapabilityView | null = capability("morphosyntax");
  const button = h("button", {
    type: "button",
    className: "primary",
    textContent: running === text.id ? "Tagging…" : "Tag and parse this text",
    title: "Lemmas, parts of speech, features and a dependency tree for every sentence of this text that has none yet, from the tagger chosen here. Sentences with tags already, and words you glossed, are kept.",
    disabled: running !== null || v === null || v.effective === null,
    onclick: () => void run(text.id, text.title),
  });
  const toggle = h("button", { type: "button", className: "st-tag-toggle", textContent: open ? "Hide tagger" : "Tagger…", title: "Which tagger runs, and what it needs", onclick: () => ((open = !open), draw()) });
  const parts: HTMLElement[] = [h("div", { className: "st-tag-row" }, button, toggle)];
  if (v !== null && (open || v.effective === null)) {
    const picker = h(
      "select",
      { disabled: v.pin !== null || running !== null, title: v.pin !== null ? "This project is pinned to a tagger." : "Automatic uses UDPipe on this computer when it is set up, else spaCy, else Stanza; the web service runs only when chosen." },
      ...choiceOptions(v).map((o) => h("option", { value: o.value, textContent: o.label, selected: o.value === v.machine })),
    );
    picker.setAttribute("aria-label", "Tagger");
    picker.onchange = () => void act(project, () => capabilityChoose({ capability: "morphosyntax", choice: picker.value }));
    parts.push(h("div", { className: "st-tag-row" }, picker), h("p", { className: v.problem === null ? "note" : "note st-problem", textContent: statusLine(v) }));
    const rows = setupFor(v);
    const local = setup?.project === project ? setup.value.local : null;
    if (rows === "udpipe" || rows === "local") {
      parts.push(
        h(
          "div",
          { className: "st-tag-row" },
          h("button", { type: "button", textContent: local?.program ? "UDPipe program…" : "Choose UDPipe…", title: local?.program ?? "Found on PATH when not chosen", onclick: () => void choose(project, "program") }),
          h("button", { type: "button", textContent: local?.model ? "Model…" : "Choose a model…", title: local?.model ?? "A .udpipe model for the language, from the UDPipe models page", onclick: () => void choose(project, "model") }),
        ),
      );
    }
    if (rows === "spacy" || rows === "local") parts.push(...pyRows(project, "spacy"));
    if (rows === "stanza" || rows === "local") parts.push(...pyRows(project, "stanza"));
    if (rows === "web") {
      const webOk = setup?.project === project && setup.value.webOk;
      const allow = h("input", { type: "checkbox", checked: webOk, onchange: () => void act(project, () => morphosyntaxAllowWeb({ allow: allow.checked })) });
      const modelField = h("input", { value: model, placeholder: "Language or model (else the project's)", spellcheck: false, size: 28, title: "A UDPipe model name or prefix (english-ewt), or a language name or code (en, tur, Turkish). Empty: the project's language from the Texts panel." });
      modelField.setAttribute("aria-label", "UDPipe model or language");
      modelField.oninput = () => (model = modelField.value);
      parts.push(
        h("label", { className: "note st-tag-allow" }, allow, ` Allow this project's open texts to go to the web service. ${WEB_NOTE}`),
        h("div", { className: "st-tag-row" }, modelField),
      );
    }
  }
  return h("section", { className: "st-tag" }, ...parts);
}
