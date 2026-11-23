// The Search panel's Replace mode: the search field is what to find, a second
// field what to write, and a live preview lists every sentence that would
// change, each with a box. Replace writes the ticked ones as ordinary edits
// and the toast offers Undo (the same call with before and after swapped).
import { replaceApply, replacePreview } from "../api";
import { h } from "../dom";
import { attempt, getState, refreshAll, selectSegment, selectText, showToast } from "../store";
import { appliedLines, nounOfTrack, previewNote, replaceLabel, TRACKS, undoOf, type Change, type Find, type Preview, type Track } from "./logic";

export type ReplaceSection = { el: HTMLElement; run: () => void; reset: () => void };

async function open(c: Change) {
  await selectText(c.textId);
  const s = getState();
  if (s.kind === "open" && s.text?.id === c.textId) selectSegment({ textId: c.textId, tierId: c.tierId, segmentId: c.segmentId });
}

/** The sentence with each place shown as what goes and what comes. */
function sentence(c: Change): HTMLElement {
  const out = h("span", { className: "rp-sentence", dir: "auto" });
  for (const p of c.parts ?? [{ keep: c.after, removed: "", added: "" }]) {
    out.append(p.keep);
    if (p.removed !== "") out.append(h("del", { textContent: p.removed }));
    if (p.added !== "") out.append(h("ins", { textContent: p.added }));
  }
  return out;
}

export function replaceSection(findInput: HTMLInputElement): ReplaceSection {
  const withInput = h("input", { type: "text", placeholder: "Replace with", className: "rp-with", spellcheck: false });
  let track: Track = "transcription";
  const trackChips = TRACKS.map(({ track: t, label }) =>
    h("button", {
      type: "button",
      textContent: label,
      onclick: () => {
        track = t;
        paintTracks();
        run();
      },
    }),
  );
  const paintTracks = () => trackChips.forEach((chip, i) => chip.setAttribute("aria-pressed", String(TRACKS[i]?.track === track)));
  const box = (label: string) => {
    const input = h("input", { type: "checkbox", onchange: () => run() });
    return { input, label: h("label", { className: "rp-option" }, input, label) };
  };
  const whole = box("Whole words");
  const matchCase = box("Match case");
  const onlyThis = box("Only the text on screen");
  const note = h("p", { className: "rp-note", role: "status" });
  const list = h("ul", { className: "rp-changes" });
  const all = h("input", { type: "checkbox", checked: true, title: "Tick or untick every sentence" });
  const allLabel = h("label", { className: "rp-option" }, all, "All");
  const go = h("button", { type: "button", className: "primary", textContent: "Replace", disabled: true });
  const foot = h("div", { className: "rp-foot" }, allLabel, go);
  const el = h(
    "div",
    { className: "rp" },
    withInput,
    h("div", { className: "scopes" }, ...trackChips),
    h("div", { className: "rp-options" }, whole.label, matchCase.label, onlyThis.label),
    note,
    list,
    foot,
  );

  let preview: Preview = { changes: [], total: 0 };
  let ticked = new Set<number>();
  let seq = 0;
  let busy = false;

  const find = (): Find => {
    const s = getState();
    const text = onlyThis.input.checked && s.kind === "open" ? (s.text?.id ?? null) : null;
    return { find: findInput.value, with: withInput.value, track, wholeWord: whole.input.checked, matchCase: matchCase.input.checked, text };
  };
  const paintFoot = () => {
    go.textContent = replaceLabel(ticked.size, nounOfTrack(track));
    go.disabled = busy || ticked.size === 0;
    all.checked = ticked.size === preview.changes.length && ticked.size > 0;
    all.indeterminate = ticked.size > 0 && ticked.size < preview.changes.length;
    foot.hidden = preview.changes.length === 0;
  };
  const row = (c: Change, i: number) => {
    const tick = h("input", {
      type: "checkbox",
      checked: ticked.has(i),
      ariaLabel: c.field != null ? `Replace in ${c.textTitle}` : `Replace in this sentence of ${c.textTitle || "Untitled text"}`,
      onchange: () => {
        if (tick.checked) ticked.add(i);
        else ticked.delete(i);
        paintFoot();
      },
    });
    return h(
      "li",
      {},
      tick,
      h(
        "button",
        { type: "button", className: "rp-open", title: c.field === "gloss" ? "A morph's gloss in this word and text" : c.field != null ? "A headword: it changes in the Lexicon" : "Open this sentence", disabled: c.field != null, onclick: () => void open(c) },
        h("span", { className: "hit-kind", textContent: c.textTitle || "Untitled text" }),
        sentence(c),
        c.glossed ? h("span", { className: "rp-glossed", textContent: "glossed", title: "Its words are glossed: after the replace, Retokenize it in Gloss" }) : "",
      ),
    );
  };
  const show = (p: Preview) => {
    preview = p;
    ticked = new Set(p.changes.map((_, i) => i));
    note.textContent = previewNote(p, findInput.value);
    list.replaceChildren(...p.changes.map(row));
    paintFoot();
  };

  let timer: number | undefined;
  function run() {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => void ask(), 200);
  }
  async function ask() {
    const mine = ++seq;
    if (findInput.value === "") {
      show({ changes: [], total: 0 });
      return;
    }
    try {
      const p = await replacePreview({ find: find() });
      if (mine === seq) show(p);
    } catch (e) {
      if (mine !== seq) return;
      show({ changes: [], total: 0 });
      note.textContent = String(e);
    }
  }

  all.onchange = () => {
    ticked = all.checked ? new Set(preview.changes.map((_, i) => i)) : new Set();
    list.querySelectorAll<HTMLInputElement>("li > input[type=checkbox]").forEach((b) => (b.checked = all.checked));
    paintFoot();
  };
  const write = async (changes: Change[], undo: boolean) => {
    busy = true;
    paintFoot();
    try {
      await attempt(async () => {
        const applied = await replaceApply({ changes });
        await refreshAll();
        showToast({
          tone: applied.skipped > 0 ? "warn" : "info",
          lines: appliedLines(applied, undo),
          action: undo || applied.done.length === 0 ? undefined : { label: "Undo", run: () => void write(undoOf(applied.done), true) },
        });
      });
    } finally {
      busy = false;
      void ask();
    }
  };
  go.onclick = () => void write(preview.changes.filter((_, i) => ticked.has(i)).map(({ parts: _, ...c }) => c), false);
  withInput.oninput = () => run();

  return {
    el,
    run,
    reset: () => {
      withInput.value = "";
      track = "transcription";
      whole.input.checked = false;
      matchCase.input.checked = false;
      onlyThis.input.checked = false;
      paintTracks();
      show({ changes: [], total: 0 });
    },
  };
}
