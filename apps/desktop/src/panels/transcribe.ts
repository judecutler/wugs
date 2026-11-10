// The transcription loop (TONGUES-UI build order step 3): one tier's
// segments in time order, the selected one editable in place. Enter saves
// and plays the next, so hearing and typing never leave the keyboard.
import { editSegmentContent, errorText, type SegmentRow, type TierRow } from "../api";
import { emptyState, h } from "../dom";
import * as playback from "../playback";
import { getSelection, getState, refreshConflicts, refreshText, selectSegment, showToast, subscribeSelection } from "../store";
import { IPA_GROUPS } from "../transcribe/ipa";
import { createFieldController, type FieldController } from "../transcribe/field";
import { createComposition, defaultTier, formatRange, insertAtCaret, keyAction, neighbor, renderWindow, tierSegments } from "../transcribe/logic";
import type { PanelDef, PanelView } from "./panel";

/** Rows drawn at once. A text tier rarely has more; beyond this the list follows the selection. */
const WINDOW_ROWS = 200;
const PALETTE_KEY = "wugs.ipa-palette";

export const transcribePanel: PanelDef = {
  title: "Transcribe",
  create(): PanelView {
    const el = h("div", { className: "panel panel-transcribe" });
    const tierSelect = h("select", { className: "tx-tier", title: "The track being transcribed. It follows the Timeline: selecting a segment there switches to its track." });
    const hint = h("span", { className: "tx-hint" });
    const paletteToggle = h("button", { textContent: "IPA", title: "Show or hide the IPA symbol palette" });
    const keys = h("span", { className: "tx-keys", textContent: "Enter next · Shift+Enter back · Tab replay · Esc revert", title: "Tab replays the segment, so it stays in the field. Shift+Tab leaves the field. Ctrl/Cmd+Enter also replays." });
    const toolbar = h("div", { className: "tx-toolbar" }, tierSelect, paletteToggle, keys, hint);
    const list = h("div", { className: "tx-list" });
    const palette = h("div", { className: "tx-palette" });
    const field = h("input", { className: "tx-field", spellcheck: false, title: "Enter: save, next and play. Shift+Enter: previous. Tab or Ctrl/Cmd+Enter: replay. Esc: revert. Shift+Tab leaves the field." });

    const selTime = h("span", { className: "tx-time" });
    const selBadge = h("span", { className: "tx-unsaved", title: "The last save failed. Press Enter to retry." });
    /** The selected segment's row. It stays in the list across redraws so the field is never detached mid-composition. */
    const selRow = h("div", { className: "tx-row selected" }, selTime, field, selBadge);

    let ready = false;
    let tierId: string | null = null;
    let textId = "";
    const controllers = new Map<string, FieldController>();
    let ctl: FieldController | null = null;
    let stamp = "";
    let focusNext = false;
    let moving = false;

    const stored = (id: string): SegmentRow | undefined => {
      const s = getState();
      return s.kind === "open" ? s.text?.segments.find((seg) => seg.id === id) : undefined;
    };

    /** One controller per text, so a draft survives a switch away and back. */
    function controllerFor(id: string): FieldController {
      let c = controllers.get(id);
      if (c === undefined) {
        c = createFieldController({
          edit: async (segmentId, content) => {
            const result = await editSegmentContent({ textId: id, segmentId, content });
            if (result.findings.length > 0) showToast({ tone: "warn", lines: result.findings });
            return result;
          },
          onFailure: (e) => showToast({ tone: "error", lines: [errorText(e)] }),
          onSettled: async () => {
            try {
              await refreshConflicts();
            } catch (e) {
              showToast({ tone: "error", lines: [errorText(e)] });
            }
            await refreshText();
          },
        });
        controllers.set(id, c);
      }
      return c;
    }

    const commitField = (): void => void ctl?.commit();

    function revertField(): void {
      if (ctl === null || ctl.segmentId === null) return;
      ctl.revert(stored(ctl.segmentId)?.content);
      field.value = ctl.value;
    }

    function play(seg: SegmentRow | undefined): void {
      if (seg?.startMs != null && seg.endMs != null) playback.playRange(seg.startMs, seg.endMs);
    }

    function go(step: 1 | -1): void {
      const s = getState();
      if (ctl === null || ctl.segmentId === null || s.kind !== "open" || s.text === null || tierId === null) return;
      commitField();
      const next = neighbor(tierSegments(s.text.segments, tierId), ctl.segmentId, step);
      if (next === null) return;
      choose(next);
      play(next);
    }

    /** A selection made here keeps focus in the field. */
    function choose(seg: SegmentRow): void {
      focusNext = true;
      selectSegment({ textId, tierId: seg.tier, segmentId: seg.id });
    }

    const composition = createComposition({ set: (cb) => window.setTimeout(cb, 0), clear: (id) => window.clearTimeout(id) });
    field.addEventListener("compositionstart", () => composition.start());
    field.addEventListener("compositionend", () => composition.end());
    field.onkeydown = (ev) => {
      const action = keyAction(ev, composition.active);
      if (action === null) return;
      ev.preventDefault();
      if (action === "commit-next") go(1);
      else if (action === "commit-prev") go(-1);
      else if (action === "revert") revertField();
      else if (ctl?.segmentId != null) play(stored(ctl.segmentId));
    };
    field.oninput = () => ctl?.type(field.value);
    field.onblur = () => {
      if (!moving) commitField();
    };

    function empty(title: string, body: string, caption?: string): void {
      ready = false;
      void ctl?.leave();
      ctl = null;
      stamp = "";
      el.replaceChildren(emptyState(title, body, caption));
    }

    function buildPalette(): void {
      palette.replaceChildren(
        ...IPA_GROUPS.map((g) =>
          h(
            "div",
            { className: "tx-group" },
            h("span", { className: "tx-group-label", textContent: g.label }),
            ...g.symbols.map((sym) => {
              const b = h("button", { type: "button", className: "tx-sym", textContent: sym.combining ? `◌${sym.ch}` : sym.ch, title: sym.name });
              // Keep the caret: a click must not take focus from the field.
              b.onmousedown = (ev) => ev.preventDefault();
              b.onclick = () => {
                if (ctl?.segmentId == null) return;
                const end = field.value.length;
                const r = insertAtCaret(field.value, field.selectionStart ?? end, field.selectionEnd ?? end, sym.ch);
                field.value = r.value;
                ctl.type(r.value);
                field.setSelectionRange(r.caret, r.caret);
                field.focus();
              };
              return b;
            }),
          ),
        ),
      );
    }
    buildPalette();
    const showPalette = (on: boolean) => {
      palette.hidden = !on;
      paletteToggle.classList.toggle("active", on);
      localStorage.setItem(PALETTE_KEY, on ? "1" : "0");
    };
    showPalette(localStorage.getItem(PALETTE_KEY) !== "0");
    paletteToggle.onclick = () => showPalette(palette.hidden);

    tierSelect.onchange = () => {
      commitField();
      tierId = tierSelect.value;
      draw(true);
    };

    function fillTiers(tiers: TierRow[]): void {
      tierSelect.replaceChildren(...tiers.map((t) => h("option", { value: t.id, textContent: t.name })));
      if (tierId !== null) tierSelect.value = tierId;
    }

    /** Redraw from the store. `force` redraws even when the tier's segments are unchanged. */
    function draw(force: boolean): void {
      const s = getState();
      if (s.kind === "closed") {
        empty("No project open", "Open a project to transcribe. The Timeline panel shows what it looks like.", "The wug is waiting for its first recording.");
        return;
      }
      const text = s.text;
      if (text === null) {
        empty("Transcribe", s.texts.length === 0 ? "Create a text, then mark segments on the Timeline." : "Select a text to transcribe it.", "The wug cocked its head and waited.");
        return;
      }
      if (text.tiers.length === 0) {
        empty("No tracks yet", "Draw a segment on the Timeline and the first track appears. Then come back and type what was said.", "The wug is listening.");
        return;
      }
      if (text.id !== textId) {
        void ctl?.leave();
        textId = text.id;
        tierId = null;
        ctl = controllerFor(text.id);
        stamp = "";
      }
      if (tierId === null || !text.tiers.some((t) => t.id === tierId)) {
        tierId = defaultTier(text.tiers)?.id ?? null;
        fillTiers(text.tiers);
      } else if (tierSelect.options.length !== text.tiers.length) fillTiers(text.tiers);
      if (tierId === null) return;

      const c = ctl ?? controllerFor(text.id);
      ctl = c;
      const ordered = tierSegments(text.segments, tierId);
      const sel = getSelection();
      const selId = sel !== null && sel.textId === text.id ? (ordered.find((x) => x.id === sel.segmentId)?.id ?? null) : null;
      const next = `${tierId}|${selId}|${JSON.stringify(ordered)}|${playback.hasSource()}|${c.unsavedIds().join(",")}`;
      const hadFocus = document.activeElement === field;

      if (selId === null) {
        void c.leave();
      } else if (c.segmentId !== selId) {
        c.show(selId, ordered.find((x) => x.id === selId)?.content);
      } else {
        c.sync(ordered.find((x) => x.id === selId)?.content);
      }
      if (field.value !== c.value) field.value = c.value;
      if (!force && ready && next === stamp) return;
      stamp = next;

      hint.textContent = playback.hasSource() ? "" : "Load a recording on the Timeline to hear segments.";
      const selIndex = selId === null ? 0 : ordered.findIndex((x) => x.id === selId);
      const [from, to] = renderWindow(ordered.length, selIndex, WINDOW_ROWS);
      moving = true;
      const rows = ordered.slice(from, to).map((seg) => {
        const label = c.label(seg.id, seg.content);
        if (seg.id === selId) {
          selRow.className = label.unsaved ? "tx-row selected unsaved" : "tx-row selected";
          selTime.textContent = formatRange(seg.startMs, seg.endMs);
          selBadge.textContent = label.unsaved ? "not saved, press Enter to retry" : "";
          return selRow;
        }
        const row = h(
          "div",
          { className: label.unsaved ? "tx-row unsaved" : "tx-row" },
          h("span", { className: "tx-time", textContent: formatRange(seg.startMs, seg.endMs) }),
          h("span", { className: "tx-text", textContent: label.text === "" ? "·" : label.text }),
          ...(label.unsaved ? [h("span", { className: "tx-unsaved", textContent: "not saved", title: "The last save failed. Select this segment and press Enter to retry." })] : []),
        );
        row.onmousedown = (ev) => { ev.preventDefault(); choose(seg); };
        return row;
      });
      const items: HTMLElement[] =
        ordered.length === 0
          ? [emptyState("This track is empty", "Drag across the waveform on the Timeline to mark a segment. It shows up here to type into.", "The wug is listening.")]
          : [...(from > 0 || to < ordered.length ? [h("div", { className: "tx-note", textContent: `Showing ${from + 1}–${to} of ${ordered.length}, around the selection.` })] : []), ...rows];
      const at = items.indexOf(selRow);
      if (at !== -1 && selRow.parentNode === list) {
        // Rebuild the rows around the selected one: moving it would drop focus and cancel an IME composition.
        for (const child of [...list.children]) if (child !== selRow) child.remove();
        selRow.before(...items.slice(0, at));
        selRow.after(...items.slice(at + 1));
      } else {
        list.replaceChildren(...items);
      }
      if (!ready) {
        ready = true;
        el.replaceChildren(toolbar, list, palette);
      }
      if (selId !== null && (focusNext || hadFocus)) field.focus();
      moving = false;
      if (selId !== null && focusNext) list.querySelector(".selected")?.scrollIntoView({ block: "nearest" });
      focusNext = false;
    }

    const unsubscribe = subscribeSelection(() => {
      if (!ready) return;
      const sel = getSelection();
      const s = getState();
      if (sel !== null && s.kind === "open" && s.text?.id === sel.textId && sel.tierId !== tierId && s.text.tiers.some((t) => t.id === sel.tierId)) {
        commitField();
        tierId = sel.tierId;
        tierSelect.value = sel.tierId;
      }
      draw(true);
      // A selection from the timeline scrolls here too, without taking focus.
      list.querySelector(".selected")?.scrollIntoView({ block: "nearest" });
    });

    return {
      element: el,
      render: () => draw(false),
      dispose: () => {
        unsubscribe();
        commitField();
      },
    };
  },
};
