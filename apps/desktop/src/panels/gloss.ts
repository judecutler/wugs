// The Gloss panel (TONGUES-UI build order step 4, roadmap item 21): the selected
// segment as Leipzig-style interlinear text. Words, morphs, glosses and the free
// translation line up under the transcription. The project's earlier analyses
// are one keypress away: Enter on an empty word accepts the ghost suggestion.
import {
  addSegment, addTier, editSegmentContent, errorText, glossAddLexeme, glossApply, glossApplyAll, glossChooseList, glossGet, glossLexicon, glossPickSense, glossRestore, glossRetokenize, glossSet, glossTokenize,
  type Alternative, type GlossView, type HiddenEdit, type LexiconMatch, type MorphView, type WordView,
} from "../api";
import { analysisOf, breakdownEffect, mergeAnalysis } from "../gloss/analysis";
import { alternativeNote, hiddenNote } from "../gloss/hidden";
import { breakdownKey, breakdownCell, glossCell, glossKey, langCell, nextCell, nextWordCell } from "../gloss/keys";
import { normalizeLangTag } from "../gloss/lang";
import { glossParts } from "../gloss/leipzig";
import { matchLabel, offerText, shouldOfferLexeme } from "../gloss/lexicon";
import { breakdownChanged, formatBreakdown, marked, parseBreakdown, type MorphType } from "../gloss/morphs";
import { tokenState } from "../gloss/state";
import { applyNeedsConfirm, applyOthersLabel, describe } from "../gloss/suggest";
import { forms } from "../gloss/tokenizer";
import { needsTranslationTrack, translationLines } from "../gloss/translation";
import { emptyState, h } from "../dom";
import { dataGeneration, getSelection, getState, refreshConflicts, refreshText, showToast, subscribeSelection, touchLexicon } from "../store";
import { createComposition, formatRange, imeBusy } from "../transcribe/logic";
import type { PanelDef, PanelView } from "./panel";

const LANG_KEY = "wugs.gloss-lang";

/** Where focus goes once the next draw has put the cells in place. */
type Focus = { cell: string } | { firstGlossOf: string };

export const glossPanel: PanelDef = {
  title: "Gloss",
  create(): PanelView {
    const el = h("div", { className: "panel panel-gloss" });

    let key = "";
    let generation = -1;
    let view: GlossView | null = null;
    let viewText = "";
    /** What was last sent, by cell id, so a Tab and the blur it causes write once. */
    const sent = new Map<string, string>();
    let stamp = "";
    let loadSeq = 0;
    let queue: Promise<unknown> = Promise.resolve();
    /** Typed but not yet saved, by cell id. Survives a redraw. */
    const drafts = new Map<string, string>();
    const errors = new Map<string, string>();
    const lexicon = new Map<string, LexiconMatch[]>();
    let offer: string | null = null;
    let lang: string | null = localStorage.getItem(LANG_KEY);
    let askingLang = false;
    let pending: Focus | null = null;
    let menu: { morphId: string; options: LexiconMatch[]; highlighted: number; host: HTMLElement } | null = null;
    /** The word whose "apply to the others" waits for its confirming second click. */
    let armedOthers: string | null = null;

    const composition = createComposition({ set: (cb) => window.setTimeout(cb, 0), clear: (id) => window.clearTimeout(id) });
    el.addEventListener("compositionstart", () => composition.start());
    el.addEventListener("compositionend", () => {
      composition.end();
      window.setTimeout(() => draw(), 0);
    });

    // ---------------------------------------------------------------- selection

    function selected() {
      const s = getState();
      const sel = getSelection();
      if (s.kind !== "open" || s.text === null || sel === null || sel.textId !== s.text.id) return null;
      const segment = s.text.segments.find((x) => x.id === sel.segmentId);
      return segment === undefined ? null : { text: s.text, segment };
    }

    function empty(title: string, body: string, caption?: string): void {
      view = null;
      key = "";
      stamp = "";
      el.replaceChildren(emptyState(title, body, caption));
    }

    // ---------------------------------------------------------------- data

    function load(textId: string, segmentId: string): void {
      const mine = ++loadSeq;
      key = `${textId}|${segmentId}`;
      generation = dataGeneration();
      void glossGet({ textId, segmentId }).then(
        (v) => {
          if (mine !== loadSeq) return;
          view = v;
          viewText = textId;
          draw();
        },
        (e) => {
          if (mine === loadSeq) showToast({ tone: "error", lines: [errorText(e)] });
        },
      );
    }

    /** Edits run one after another, so the view that comes back last is the newest. */
    function run<T extends { view: GlossView; notes?: string[] }>(job: () => Promise<T>, done?: (r: T) => void): Promise<T | null> {
      const next = queue.then(async () => {
        try {
          const r = await job();
          // An edit that lands after the user moved to another segment still saved; it just is not shown.
          if (view !== null && r.view.segmentId === view.segmentId) view = r.view;
          done?.(r);
          if (r.notes !== undefined && r.notes.length > 0) showToast({ tone: "warn", lines: r.notes });
          draw();
          return r;
        } catch (e) {
          showToast({ tone: "error", lines: [errorText(e)] });
          draw();
          return null;
        }
      });
      queue = next;
      return next;
    }

    /** The segment the visible view belongs to: edits go there even if the selection has just moved on. */
    const ids = () => (view === null ? null : { textId: viewText, segmentId: view.segmentId });

    async function matchesFor(surface: string): Promise<LexiconMatch[]> {
      const hit = lexicon.get(surface);
      if (hit !== undefined) return hit;
      const found = await glossLexicon({ surface });
      lexicon.set(surface, found);
      return found;
    }

    // ---------------------------------------------------------------- focus

    function order(v: GlossView): string[] {
      return v.words.flatMap((w) => [breakdownCell(w.id), ...w.morphs.map((m) => glossCell(m.id)), ...(lang === null ? [] : w.morphs.map((m) => langCell(m.id)))]);
    }

    const cellEl = (id: string) => el.querySelector<HTMLInputElement>(`[data-cell="${CSS.escape(id)}"]`);

    function focusCell(id: string | null): void {
      if (id === null) return;
      const input = cellEl(id);
      if (input === null) pending = { cell: id };
      else {
        input.focus();
        input.select();
      }
    }

    function applyPending(v: GlossView): void {
      const p = pending;
      pending = null;
      if (p === null) return;
      if ("cell" in p) {
        const input = cellEl(p.cell);
        input?.focus();
        input?.select();
        return;
      }
      const word = v.words.find((w) => w.id === p.firstGlossOf);
      const first = word?.morphs[0];
      focusCell(first === undefined ? nextWordCell(order(v), breakdownCell(p.firstGlossOf), 1) : glossCell(first.id));
    }

    // ---------------------------------------------------------------- actions

    function acceptSuggestion(word: WordView): void {
      const at = ids();
      const ghost = word.suggestions[0];
      if (at === null || ghost === undefined || view === null) return;
      const nextWord = nextWordCell(order(view), breakdownCell(word.id), 1);
      pending = nextWord === null ? { cell: breakdownCell(word.id) } : { cell: nextWord };
      void run(() => glossApply({ ...at, tokenId: word.id, morphs: ghost.morphs }));
    }

    /** Apply a typed breakdown. False when it does not parse (the error shows under the field). */
    function applyBreakdown(word: WordView, then: Focus | null): boolean {
      const at = ids();
      const id = breakdownCell(word.id);
      const draft = drafts.get(id);
      if (at === null || draft === undefined) return true;
      const parsed = parseBreakdown(draft);
      if (!parsed.ok) {
        errors.set(id, parsed.error);
        showError(id);
        return false;
      }
      errors.delete(id);
      if (!breakdownChanged(draft, word.morphs.map((m) => ({ surface: m.surface, type: m.type as MorphType })))) {
        drafts.delete(id);
        return true;
      }
      if (breakdownEffect(parsed.morphs, word.morphs.length) === "keep") {
        drafts.delete(id);
        errors.set(id, "Kept the analysis. Use Clear analysis to remove it.");
        draw(true);
        return true;
      }
      if (then !== null) pending = then;
      void run(() => glossApply({ ...at, tokenId: word.id, morphs: mergeAnalysis(parsed.morphs, analysisOf(word)) }), () => drafts.delete(id));
      return true;
    }

    function commitGloss(m: MorphView, language: string | null): Promise<unknown> {
      const at = ids();
      const id = language === null ? glossCell(m.id) : langCell(m.id);
      const stored = language === null ? (m.gloss ?? "") : (m.glossesByLang[language] ?? "");
      const draft = drafts.get(id);
      if (at === null || draft === undefined || draft === stored) {
        drafts.delete(id);
        return Promise.resolve();
      }
      if (sent.get(id) === draft) return queue;
      sent.set(id, draft);
      return run(() => glossSet({ ...at, morphId: m.id, gloss: draft, lang: language }), () => {
        if (drafts.get(id) === draft) drafts.delete(id);
      }).finally(() => sent.delete(id));
    }

    async function offerAfterCommit(morphId: string): Promise<boolean> {
      const m = view?.words.flatMap((w) => w.morphs).find((x) => x.id === morphId);
      if (m === undefined) return false;
      const matches = await matchesFor(m.surface);
      if (!shouldOfferLexeme({ surface: m.surface, type: m.type, gloss: m.gloss ?? "", sense: m.sense, matches: matches.length })) return false;
      offer = m.id;
      pending = { cell: glossCell(m.id) };
      draw();
      return true;
    }

    function createLexeme(m: MorphView): void {
      const at = ids();
      if (at === null) return;
      offer = null;
      lexicon.clear();
      void run(async () => {
        const r = await glossAddLexeme({ ...at, morphId: m.id, lemma: m.surface, gloss: m.gloss ?? "" });
        touchLexicon();
        return r;
      });
    }

    function pick(m: MorphView, option: LexiconMatch): void {
      const at = ids();
      if (at === null || view === null) return;
      closeMenu();
      drafts.delete(glossCell(m.id));
      const next = nextCell(order(view), glossCell(m.id), 1);
      if (next !== null) pending = { cell: next };
      void run(() => glossPickSense({ ...at, morphId: m.id, senseId: option.sense, gloss: option.gloss }));
    }

    function restoreButton(hidden: HiddenEdit, on: "morph" | "word"): HTMLElement {
      const b = h("button", { type: "button", className: "gl-hidden", textContent: `\u21ba ${hidden.label}`, title: hiddenNote(hidden, on) });
      b.onclick = () => {
        const at = ids();
        if (at !== null) void run(() => glossRestore({ ...at, parentId: hidden.parent, memberId: hidden.member }));
      };
      return b;
    }

    /** "Another device analysed this word as …" with the action that takes that version instead. */
    function alternativeNotice(alt: Alternative, on: "word" | "segment"): HTMLElement {
      const label = on === "word" ? "Use that instead" : "Use the other word breakdown";
      const b = h("button", { type: "button", className: "gl-others", textContent: label, title: "Shows the other device's version on every device. What is shown now stays restorable; nothing is deleted." });
      b.onclick = () => {
        const at = ids();
        if (at !== null) void run(() => glossChooseList({ ...at, parentId: alt.parent, members: alt.members })).then(() => refreshConflicts());
      };
      return h("div", { className: "gl-notice" }, h("span", { textContent: alternativeNote(alt, on) }), b);
    }

    // ---------------------------------------------------------------- lexicon autocomplete

    function closeMenu(): void {
      menu?.host.replaceChildren();
      menu = null;
    }

    function paintMenu(): void {
      if (menu === null) return;
      const open = menu;
      open.host.replaceChildren(
        ...open.options.map((o, i) => {
          const row = h("div", { className: i === open.highlighted ? "gl-opt on" : "gl-opt", textContent: matchLabel(o), title: "Link this sense and use its gloss" });
          row.onmousedown = (ev) => {
            ev.preventDefault();
            const m = view?.words.flatMap((w) => w.morphs).find((x) => x.id === open.morphId);
            if (m !== undefined) pick(m, o);
          };
          return row;
        }),
      );
    }

    async function openMenu(m: MorphView, input: HTMLInputElement, host: HTMLElement): Promise<void> {
      const all = await matchesFor(m.surface);
      if (document.activeElement !== input) return;
      const typed = input.value.trim().toLowerCase();
      const options = all.filter((o) => typed === "" || o.gloss.toLowerCase().startsWith(typed));
      menu?.host.replaceChildren();
      menu = options.length === 0 ? null : { morphId: m.id, options, highlighted: -1, host };
      paintMenu();
    }

    // ---------------------------------------------------------------- DOM

    function showError(id: string): void {
      const slot = el.querySelector<HTMLElement>(`[data-error="${CSS.escape(id)}"]`);
      if (slot !== null) slot.textContent = errors.get(id) ?? "";
    }

    function glossInput(m: MorphView, language: string | null, v: GlossView): HTMLElement {
      const id = language === null ? glossCell(m.id) : langCell(m.id);
      const stored = language === null ? (m.gloss ?? "") : (m.glossesByLang[language] ?? "");
      const value = drafts.get(id) ?? stored;
      const input = h("input", { className: "gl-input", value, spellcheck: false, placeholder: language ?? "gloss", title: language === null ? "Gloss. Tab and Enter move on; Enter on a match links its lexicon sense." : `Gloss in ${language}` });
      input.dataset.cell = id;
      const paint = h("span", { className: "gl-view", title: "Click to edit" });
      const repaint = (text: string) => {
        paint.replaceChildren(...(text === "" ? [h("span", { className: "gl-ph", textContent: language ?? "gloss" })] : glossParts(text).map((p) => h("span", { className: p.smallCaps ? "sc" : "", textContent: p.text }))));
      };
      repaint(value);
      paint.onmousedown = (ev) => {
        ev.preventDefault();
        input.focus();
      };
      const host = h("div", { className: "gl-menu" });
      const wrap = h("div", { className: "gl-cell" }, paint, input, language === null ? host : null);
      if (language === null && m.sense !== null) {
        wrap.classList.add("linked");
        paint.title = `Linked to ${m.senseLabel ?? "a lexicon sense"}`;
      }

      input.oninput = () => {
        drafts.set(id, input.value);
        repaint(input.value);
        if (language === null) void openMenu(m, input, host);
      };
      input.onfocus = () => {
        if (language === null) void openMenu(m, input, host);
      };
      input.onblur = () => {
        closeMenu();
        void commitGloss(m, language);
      };
      input.onkeydown = (ev) => {
        const action = glossKey(ev, composition.active, { menuOpen: menu !== null && menu.morphId === m.id, highlighted: menu?.highlighted ?? -1, offer: offer === m.id });
        if (action === null) return;
        ev.preventDefault();
        const cells = order(v);
        const step = (to: "next" | "prev") => focusCell(nextCell(cells, id, to === "next" ? 1 : -1));
        switch (action) {
          case "menu-down":
          case "menu-up":
            if (menu !== null) {
              const n = menu.options.length;
              menu.highlighted = action === "menu-down" ? (menu.highlighted + 1) % n : (menu.highlighted - 1 + n) % n;
              paintMenu();
            }
            break;
          case "pick":
            if (menu !== null) pick(m, menu.options[menu.highlighted]!);
            break;
          case "create-lexeme":
            createLexeme(m);
            break;
          case "dismiss":
            offer = null;
            closeMenu();
            draw(true);
            break;
          case "revert":
            drafts.delete(id);
            input.value = stored;
            repaint(stored);
            break;
          case "next":
          case "prev":
            if (offer === m.id) offer = null;
            void commitGloss(m, language);
            step(action);
            break;
          case "commit-next":
          case "commit-prev":
            closeMenu();
            if (action === "commit-prev") {
              void commitGloss(m, language);
              step("prev");
            } else {
              void commitGloss(m, language).then(async () => {
                if (language === null && (await offerAfterCommit(m.id))) return;
                step("next");
              });
            }
            break;
        }
      };
      const offerRow =
        language === null && offer === m.id
          ? h("div", { className: "gl-offer", textContent: `${offerText(m.surface, m.gloss ?? "")}. Enter to add, Tab to skip.` })
          : null;
      return h("div", { className: "gl-slot" }, h("div", { className: "gl-morph", textContent: marked({ surface: m.surface, type: m.type as MorphType }) }), wrap, offerRow);
    }

    function wordColumn(w: WordView, v: GlossView): HTMLElement {
      const id = breakdownCell(w.id);
      const current = w.morphs.map((m) => ({ surface: m.surface, type: m.type as MorphType }));
      const ghost = w.suggestions[0] ?? null;
      const ghostText = ghost === null ? null : describe(ghost, w.suggestions);
      const input = h("input", {
        className: "gl-breakdown",
        value: drafts.get(id) ?? formatBreakdown(current),
        spellcheck: false,
        placeholder: ghostText === null ? "köpek -ler" : `${ghostText.breakdown}   ${ghostText.glosses}`,
        title: "The word's morphs: -ler suffix, ka- prefix, =ki clitic, re~ reduplication, <in> infix, ∅ zero. Enter on an empty field accepts the grey suggestion.",
      });
      input.dataset.cell = id;
      input.oninput = () => {
        drafts.set(id, input.value);
        errors.delete(id);
        showError(id);
      };
      input.onblur = () => void applyBreakdown(w, null);
      input.onkeydown = (ev) => {
        const value = input.value;
        const action = breakdownKey(ev, composition.active, { empty: value.trim() === "", hasGhost: ghost !== null, changed: breakdownChanged(value, current) });
        if (action === null) return;
        ev.preventDefault();
        const cells = order(v);
        switch (action) {
          case "accept-suggestion":
            acceptSuggestion(w);
            break;
          case "apply-next":
            applyBreakdown(w, { firstGlossOf: w.id });
            break;
          case "apply-prev":
            if (applyBreakdown(w, null)) focusCell(nextWordCell(cells, id, -1));
            break;
          case "next":
            focusCell(nextCell(cells, id, 1));
            break;
          case "prev":
            focusCell(nextCell(cells, id, -1));
            break;
          case "revert":
            drafts.delete(id);
            errors.delete(id);
            input.value = formatBreakdown(current);
            showError(id);
            break;
        }
      };

      const chips: HTMLElement[] = [];
      if (ghostText !== null && ghost !== null) {
        const chip = h("button", { type: "button", className: "gl-ghost", title: `${ghostText.usesLabel}. Enter in the empty field accepts it.` }, h("span", { textContent: ghostText.breakdown }), h("span", { className: "gl-ghost-gloss", textContent: ghostText.glosses }), h("span", { className: "gl-uses", textContent: ghost.count > 1 ? `×${ghost.count}` : "" }));
        chip.onclick = () => acceptSuggestion(w);
        chips.push(chip);
        for (const alt of w.suggestions.slice(1)) {
          const d = describe(alt, w.suggestions);
          const b = h("button", { type: "button", className: "gl-alt", textContent: `${d.breakdown}  ${d.glosses}`, title: d.usesLabel });
          b.onclick = () => {
            const at = ids();
            if (at !== null) void run(() => glossApply({ ...at, tokenId: w.id, morphs: alt.morphs }));
          };
          chips.push(b);
        }
      }
      const others = applyOthersLabel(w.form, w.othersUnanalysed, armedOthers === w.id);
      const link = w.morphs.length > 0 && others !== null ? h("button", { type: "button", className: "gl-others", textContent: others, title: "Gives these words this analysis. Words that already have one are left alone." }) : null;
      if (link !== null) link.onclick = () => applyOthers(w);
      const clear = w.morphs.length > 0 ? h("button", { type: "button", className: "gl-others", textContent: "Clear analysis", title: "Takes this word's morphs and glosses out of view. Nothing is deleted from the project's history." }) : null;
      if (clear !== null) {
        clear.onclick = () => {
          const at = ids();
          if (at !== null) void run(() => glossApply({ ...at, tokenId: w.id, morphs: [] }));
        };
      }

      const flagged = w.findings.length > 0;
      const form = h("div", { className: flagged ? "gl-form gl-flag" : "gl-form", textContent: w.form, title: flagged ? w.findings.join("\n") : "" });
      return h(
        "div",
        { className: "gl-word" },
        form,
        input,
        h("div", { className: "gl-error", textContent: errors.get(id) ?? "" }),
        h("div", { className: "gl-morphs" }, ...w.morphs.map((m) => glossInput(m, null, v))),
        lang === null ? null : h("div", { className: "gl-morphs gl-lang-row" }, ...w.morphs.map((m) => glossInput(m, lang, v))),
        ...chips,
        link,
        clear,
        ...v.hidden.filter((x) => x.parent === w.id).map((x) => restoreButton(x, "morph")),
        ...v.alternatives.filter((x) => x.parent === w.id).map((x) => alternativeNotice(x, "word")),
      );
    }
    /** Runs on the edit queue, and reads the word when its turn comes, so a gloss typed just
     * before the click is part of what gets applied. */
    function applyOthers(w: WordView): void {
      if (applyNeedsConfirm(w.othersUnanalysed) && armedOthers !== w.id) {
        armedOthers = w.id;
        draw(true);
        return;
      }
      armedOthers = null;
      void run(
        async () => {
          const at = ids();
          const word = view?.words.find((x) => x.id === w.id);
          if (at === null || word === undefined || word.morphs.length === 0) throw new Error("This word has no analysis to apply any more.");
          return glossApplyAll({ ...at, form: word.form, morphs: analysisOf(word) });
        },
        (r) => showToast({ tone: "info", lines: [r.applied === 0 ? "Nothing left to apply." : `Applied to ${r.applied} other «${w.form}» in ${r.texts} ${r.texts === 1 ? "text" : "texts"}.`] }),
      );
    }

    function langControl(): HTMLElement {
      if (lang !== null) {
        const off = h("button", { type: "button", className: "gl-lang-off", textContent: "×", title: "Hide the second gloss row" });
        off.onclick = () => {
          lang = null;
          localStorage.removeItem(LANG_KEY);
          draw(true);
        };
        return h("span", { className: "gl-lang" }, h("span", { textContent: `second row: ${lang}` }), off);
      }
      if (askingLang) {
        const tag = h("input", { className: "gl-lang-input", placeholder: "es, fr, zh-hant", spellcheck: false, title: "A language tag. Glosses in it are stored as gloss:<tag>." });
        tag.onkeydown = (ev) => {
          if (imeBusy(ev, composition.active)) return;
          if (ev.key === "Escape") {
            askingLang = false;
            draw(true);
          } else if (ev.key === "Enter") {
            ev.preventDefault();
            const t = normalizeLangTag(tag.value);
            if (t === null) {
              tag.title = "Letters, digits and hyphens, 2 to 35 characters.";
              tag.classList.add("bad");
              return;
            }
            lang = t;
            askingLang = false;
            localStorage.setItem(LANG_KEY, t);
            draw(true);
          }
        };
        window.setTimeout(() => tag.focus());
        return tag;
      }
      const add = h("button", { type: "button", textContent: "+ language", title: "Show a second gloss row for one more analysis language" });
      add.onclick = () => {
        askingLang = true;
        draw(true);
      };
      return add;
    }

    function translationBlock(text: NonNullable<ReturnType<typeof selected>>): HTMLElement {
      const { segment, text: detail } = text;
      const box = h("div", { className: "gl-free" }, h("h3", { textContent: "Free translation" }));
      if (needsTranslationTrack(detail.tiers)) {
        const add = h("button", { type: "button", textContent: "Add translation track" });
        add.onclick = () => {
          void addTier({ template: "translation", name: null }).then(() => refreshText(), (e) => showToast({ tone: "error", lines: [errorText(e)] }));
        };
        box.append(h("p", { className: "note", textContent: "No translation track yet. A translation lives on its own track, lined up in time with the utterance." }), add);
        return box;
      }
      if (segment.startMs === null || segment.endMs === null) {
        box.append(h("p", { className: "note", textContent: "This segment has no times, so no translation can line up with it. Give it times on the Timeline." }));
        return box;
      }
      for (const line of translationLines(segment, detail.segments, detail.tiers)) {
        const cell = `t:${line.tier.id}`;
        const stored = line.segment?.content ?? "";
        const input = h("input", { className: "gl-free-input", value: drafts.get(cell) ?? stored, spellcheck: false, placeholder: line.segment === null ? "Type a translation. It becomes a segment over this one." : "" });
        input.dataset.cell = cell;
        const commit = () => {
          const draft = drafts.get(cell);
          if (draft === undefined || draft === stored) return;
          drafts.delete(cell);
          const job =
            line.segment !== null
              ? editSegmentContent({ textId: detail.id, segmentId: line.segment.id, content: draft }).then((r) => {
                  if (r.findings.length > 0) showToast({ tone: "warn", lines: r.findings });
                })
              : draft.trim() === ""
                ? Promise.resolve()
                : addSegment({ textId: detail.id, tierId: line.tier.id, startMs: segment.startMs, endMs: segment.endMs, content: draft }).then(() => undefined);
          void job.then(() => refreshConflicts()).then(() => refreshText(), (e) => {
            drafts.set(cell, draft);
            showToast({ tone: "error", lines: [errorText(e)] });
          });
        };
        input.oninput = () => drafts.set(cell, input.value);
        input.onblur = commit;
        input.onkeydown = (ev) => {
          if (imeBusy(ev, composition.active)) return;
          if (ev.key === "Enter") {
            ev.preventDefault();
            commit();
          } else if (ev.key === "Escape") {
            drafts.delete(cell);
            input.value = stored;
          }
        };
        box.append(h("label", { className: "gl-free-row" }, h("span", { className: "gl-free-tier", textContent: line.tier.name }), input));
      }
      return box;
    }

    // ---------------------------------------------------------------- draw

    function draw(force = false): void {
      if (composition.active) return;
      const s = getState();
      if (s.kind === "closed") return empty("No project open", "Open a project to gloss. Words, morphs and meanings line up here.", "The wug has nothing to say yet.");
      if (s.text === null) return empty("Gloss", s.texts.length === 0 ? "Create a text and transcribe a segment, then break it into words here." : "Select a text to gloss it.", "The wug cocked its head and waited.");
      const sel = selected();
      if (sel === null) return empty("Gloss", "Select a segment on the Timeline or in Transcribe. Its words appear here, ready to break into morphs and meanings.", "The wug is waiting for a word.");
      const wanted = `${sel.text.id}|${sel.segment.id}`;
      if (key !== wanted || generation !== dataGeneration()) {
        if (key !== wanted) {
          view = null;
          stamp = "";
          el.replaceChildren();
        }
        load(sel.text.id, sel.segment.id);
      }
      if (view === null) return;
      const v = view;
      const roles = sel.text.tiers.map((t) => `${t.id}:${t.role ?? ""}:${t.name}`).join(",");
      const translations = JSON.stringify(translationLines(sel.segment, sel.text.segments, sel.text.tiers).map((l) => [l.tier.id, l.segment?.id, l.segment?.content]));
      const next = JSON.stringify([key, sel.segment.content, sel.segment.startMs, sel.segment.endMs, v, lang, askingLang, offer, armedOthers, roles, translations]);
      const focused = el.contains(document.activeElement) && document.activeElement instanceof HTMLInputElement ? document.activeElement : null;
      const keep = focused === null ? null : { cell: focused.dataset.cell ?? "", from: focused.selectionStart, to: focused.selectionEnd };
      if (!force && next === stamp) {
        applyPending(v);
        return;
      }
      stamp = next;
      closeMenu();

      const state = tokenState(sel.segment.content, v.words.map((w) => w.form));
      const head = h("div", { className: "gl-head" }, h("span", { className: "gl-time", textContent: formatRange(sel.segment.startMs, sel.segment.endMs) }), h("span", { className: "gl-spacer" }), langControl());
      const transcription = h("div", { className: "gl-transcription", textContent: sel.segment.content === "" ? "No transcription yet." : sel.segment.content, title: "Read-only here. Edit it in Transcribe." });
      const body: HTMLElement[] = [];
      const at = { textId: sel.text.id, segmentId: sel.segment.id };
      const words = forms(sel.segment.content);
      if (state === "empty") {
        body.push(h("p", { className: "note", textContent: "Nothing to break into words yet. Type what was said in Transcribe, then come back. The wug is listening." }));
      } else if (state === "untokenized") {
        const go = h("button", { type: "button", className: "gl-primary", textContent: "Break into words", title: "Splits on spaces. Punctuation at the edges stays out of the words." });
        go.onclick = () => void run(() => glossTokenize({ ...at, forms: words }));
        body.push(h("div", { className: "gl-notice" }, h("span", { textContent: `${words.length} ${words.length === 1 ? "word" : "words"} to break out.` }), go));
      } else if (state === "changed") {
        const go = h("button", { type: "button", className: "gl-primary", textContent: "Retokenize", title: "Words that did not change keep their morphs and glosses." });
        go.onclick = () => void run(() => glossRetokenize({ ...at, forms: words }));
        body.push(h("div", { className: "gl-notice" }, h("span", { textContent: "Transcription changed since these words were broken out." }), go));
      }
      body.push(...v.alternatives.filter((x) => x.parent === v.segmentId).map((x) => alternativeNotice(x, "segment")));
      const hiddenWords = v.hidden.filter((x) => x.parent === v.segmentId);
      if (hiddenWords.length > 0) {
        body.push(h("div", { className: "gl-notice gl-hidden-words" }, h("span", { textContent: "Edits from another device on words no longer in this segment:" }), ...hiddenWords.map((x) => restoreButton(x, "word"))));
      }
      if (v.words.length > 0) body.push(h("div", { className: "gl-igt" }, ...v.words.map((w) => wordColumn(w, v))));
      el.replaceChildren(head, transcription, ...body, translationBlock(sel));

      if (pending !== null) applyPending(v);
      else if (keep !== null && keep.cell !== "") {
        const again = cellEl(keep.cell);
        if (again !== null) {
          again.focus();
          if (keep.from !== null && keep.to !== null) again.setSelectionRange(keep.from, keep.to);
        }
      }
    }

    const unsubscribe = subscribeSelection(() => draw());
    return { element: el, render: () => draw(), dispose: unsubscribe };
  },
};
