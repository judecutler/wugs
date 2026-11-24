// Checks: what to tidy before a grammar or a deposit. First check: morphs
// glossed more than one way across the project (src-tauri/src/consistency.rs),
// each with its glosses, how often each is used, where one occurs, and a
// button that makes one gloss the only one. Then glossed roots the lexicon
// lacks, rare words a letter away from common ones (../checks/spelling.ts),
// and the characters the project uses.
import { unclearLine, unclearSheet, spanOf, type Unclear, type UnclearRow } from "../checks/unclear";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { localDay } from "../dates";
import { unclearPassages, projectLanguage, saveDocument, glossFill, glossUnglossedMorphs, type Unglossed, glossForms, glossRegloss, archiveReadiness, characterInventory, consentGaps, errorText, glossAddMissing, glossConsistency, glossMissing, glossSegmentations, glossUseBreakdown, glossSenseDrift, glossSettleDrift, glossUnify, lexiconExampleGaps, lexiconList, lexiconMerge, segmentTiming, duplicateSentences, lexiconExamplesFromTexts, soundWords, treeProblems, type TreeProblem } from "../api";
import { leipzigIssues, leipzigLine, type LeipzigIssue } from "../checks/leipzig";
import { duplicateNote, duplicatesLine, likelyDuplicates, type LikelyPair } from "../checks/entries";
import { nearSpellings, pairNote, spellingSummary, type NearPair } from "../checks/spelling";
import { sequenceWords, unusualLine, unusualWords, type Unusual } from "../checks/sequences";
import { breakdownNote, breakdownsLine, breakdownUsedLine, useBreakdownLabel, type Breakdown, type Segmented, readinessLine, readinessNote, timingLine, timingNote, twiceLine, twiceNote, type Duplicates, type Readiness, type Timing, type TimingIssue, type Twice } from "../checks/logic";
import { addedLine, addLabel, consentLine, consentNote, type ConsentGaps, driftSummary, settledLine, type Drift, exampleGapsLine, examplesAddedLine, treesLine, cardNote, findable, glyph, inventoryNote, missingSummary, summary, unifiedLine, useLabel, type CharRow, type Inconsistent, type Inventory, type Missing } from "../checks/logic";
import { askConcordance } from "./concordance";
import { headwordOf } from "../notes/logic";
import { termLabel } from "../people/logic";
import { emptyState, h } from "../dom";
import { glossRuns } from "../gloss/example";
import { marked } from "../gloss/morphs";
import type { MorphType } from "../gloss/morphs";
import { provide } from "../palette/registry";
import { dataGeneration, getState, openEntry, refreshAll, selectSegment, selectText, showToast, touchLexicon } from "../store";
import { SHOW_PANEL, type PanelDef, type PanelView } from "./panel";

function glossNode(gloss: string): HTMLElement {
  return h("span", { className: "ck-gloss" }, ...glossRuns(gloss).map((r) => (r.caps ? h("span", { className: "sc", textContent: r.text }) : document.createTextNode(r.text))));
}

export const checksPanel: PanelDef = {
  title: "Checks",
  create(): PanelView {
    const el = h("div", { className: "panel panel-checks" });
    let rows: Inconsistent[] | null = null;
    let missing: Missing[] = [];
    let drift: Drift[] = [];
    let consent: ConsentGaps | null = null;
    let ready: Readiness | null = null;
    let chars: Inventory | null = null;
    let near: { pairs: NearPair[]; read: number } | null = null;
    let unusual: { words: Unusual[]; running: number } | null = null;
    let doubles: { pairs: LikelyPair[]; entries: number } | null = null;
    let timing: Timing | null = null;
    let unclear: Unclear | null = null;
    let twice: Duplicates | null = null;
    let breakdowns: Segmented[] | null = null;
    let leipzig: { issues: LeipzigIssue[]; forms: number } | null = null;
    let bare: Unglossed[] | null = null;
    /** What is typed in each unglossed morph's field, by key and type. */
    const fillDrafts = new Map<string, string>();
    let gaps: { without: number; fillable: number } | null = null;
    let trees: { sentences: TreeProblem[]; total: number; annotated: number } | null = null;
    let showAllChars = false;
    let key = "";
    let seq = 0;
    let busy = false;
    /** The armed "use for all" button: morph key, kind and gloss. */
    let armed: string | null = null;

    const load = () => {
      const s = getState();
      if (s.kind !== "open") return;
      const mine = ++seq;
      Promise.all([glossConsistency(), glossMissing(), characterInventory(), soundWords({ tierId: null, textId: null }), lexiconExampleGaps(), treeProblems(), glossSenseDrift(), consentGaps().catch(() => null), lexiconList({ query: "" }), segmentTiming().catch(() => null), archiveReadiness().catch(() => null), duplicateSentences().catch(() => null), glossSegmentations().catch(() => null), glossForms().catch(() => null), glossUnglossedMorphs().catch(() => null), unclearPassages().catch(() => null)]).then(
        ([r, m, c, w, g, t, d, k, l, tm, ar, du, bd, gf, ug, un]) => {
          if (mine !== seq) return;
          unclear = un;
          bare = ug;
          leipzig = gf === null ? null : { issues: leipzigIssues(gf), forms: gf.length };
          breakdowns = bd;
          twice = du;
          ready = ar;
          timing = tm;
          doubles = { pairs: likelyDuplicates(l), entries: l.length };
          consent = k;
          drift = d;
          gaps = g;
          trees = t;
          rows = r;
          missing = m;
          chars = c;
          near = { pairs: nearSpellings(w.words), read: w.words.length };
          unusual = unusualWords(w.words);
          paint();
        },
        (e) => {
          if (mine === seq) el.replaceChildren(emptyState("Checks", errorText(e)));
        },
      );
    };

    const unify = async (r: Inconsistent, to: string) => {
      busy = true;
      paint();
      let changed = 0;
      try {
        for (const v of r.variants) if (v.gloss !== to) changed += await glossUnify({ key: r.key, kind: r.kind, from: v.gloss, to });
        showToast({ tone: "info", lines: [unifiedLine(changed, to)] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      armed = null;
      await refreshAll();
      load();
    };

    const useBreakdown = async (v: Breakdown) => {
      busy = true;
      paint();
      try {
        const changed = await glossUseBreakdown({ tokenId: v.tokenId });
        showToast({ tone: "info", lines: [breakdownUsedLine(changed, v.breakdown)] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      armed = null;
      await refreshAll();
      load();
    };

    const regloss = async (i: LeipzigIssue) => {
      busy = true;
      paint();
      try {
        const changed = await glossRegloss({ from: i.form.gloss, to: i.to });
        showToast({ tone: "info", lines: [`Glossed ${changed} ${changed === 1 ? "morph" : "morphs"} «${i.to}» instead of «${i.form.gloss}».`] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      armed = null;
      await refreshAll();
      load();
    };

    const fill = async (u: Unglossed, gloss: string) => {
      busy = true;
      paint();
      try {
        const n = await glossFill({ key: u.key, kind: u.kind, gloss });
        showToast({ tone: "info", lines: [`Glossed ${n} ${n === 1 ? "morph" : "morphs"} «${marked({ surface: u.surface, type: u.kind as MorphType })}» as «${gloss.trim()}».`] });
        fillDrafts.delete(`${u.key}\u0001${u.kind}`);
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      armed = null;
      await refreshAll();
      load();
    };

    /** Morphs with no gloss yet: an example printed now would show «?» under them. */
    const bareSection = (): HTMLElement[] => {
      if (bare === null) return [];
      const total = bare.reduce((n, u) => n + u.count, 0);
      const line = bare.length === 0 ? "Every morph has a gloss." : `${total} ${total === 1 ? "morph has" : "morphs have"} no gloss yet (${bare.length} ${bare.length === 1 ? "spelling" : "spellings"}); a glossed example would show «?» under ${total === 1 ? "it" : "them"}. Type a gloss and press the button to gloss every one of a spelling at once.`;
      const card = (u: Unglossed): HTMLElement => {
        const k = `${u.key}\u0001${u.kind}`;
        const id = `fill\u0001${k}`;
        const shownMorph = marked({ surface: u.surface, type: u.kind as MorphType });
        const field = h("input", { className: "ck-fill", value: fillDrafts.get(k) ?? u.elsewhere[0]?.[0] ?? "", placeholder: "Gloss", spellcheck: false, disabled: busy });
        field.setAttribute("aria-label", `Gloss for ${shownMorph}`);
        field.oninput = () => fillDrafts.set(k, field.value);
        const go = h("button", { type: "button", className: armed === id ? "danger" : "", disabled: busy, textContent: armed === id ? `Gloss ${u.count}: sure?` : `Gloss all ${u.count}`, title: `Give every unglossed «${shownMorph}» (${u.kind}) the gloss typed beside it` });
        go.onclick = () => {
          if (field.value.trim() === "") {
            field.focus();
            return;
          }
          if (armed !== id) {
            fillDrafts.set(k, field.value);
            armed = id;
            paint();
            return;
          }
          void fill(u, field.value);
        };
        const show = h("button", { type: "button", className: "ck-show", textContent: `in «${u.textTitle || "(untitled)"}»`, title: "Open a sentence with this morph" });
        show.onclick = () => selectSegment({ textId: u.textId, tierId: u.tierId, segmentId: u.segmentId });
        const elsewhere = u.elsewhere.length === 0 ? null : h("p", { className: "note", textContent: `Glossed elsewhere as ${u.elsewhere.slice(0, 4).map(([g, n]) => `«${g}» (${n})`).join(", ")}.` });
        return h("li", { className: "ck-card" }, h("div", { className: "ck-head" }, h("span", { className: "ck-morph", textContent: shownMorph }), h("span", { className: "ck-kind", textContent: u.kind }), h("span", { className: "ck-count", textContent: `× ${u.count}` }), show), elsewhere, h("div", { className: "ck-bar" }, field, go));
      };
      return [h("h3", { className: "ck-title", textContent: "Morphs without a gloss" }), h("p", { className: "note", textContent: line }), h("ul", { className: "ck-list" }, ...bare.slice(0, 100).map(card))];
    };

    const leipzigSection = (): HTMLElement[] => {
      if (leipzig === null) return [];
      const card = (i: LeipzigIssue): HTMLElement => {
        const id = `leipzig\u0001${i.form.gloss}`;
        const use = h("button", { type: "button", className: armed === id ? "danger" : "", disabled: busy, textContent: armed === id ? `Change ${i.form.count}: sure?` : `Use «${i.to}» for all ${i.form.count}`, title: `Gloss every morph now glossed «${i.form.gloss}» as «${i.to}»` });
        use.onclick = () => {
          if (armed !== id) {
            armed = id;
            paint();
            return;
          }
          void regloss(i);
        };
        const show = h("button", { type: "button", className: "ck-show", textContent: `in «${i.form.textTitle || "(untitled)"}»`, title: "Open a sentence glossed this way" });
        show.onclick = () => selectSegment({ textId: i.form.textId, tierId: i.form.tierId, segmentId: i.form.segmentId });
        return h(
          "li",
          { className: "ck-card" },
          h("div", { className: "ck-head" }, h("span", { className: "ck-morph ck-from", textContent: i.form.gloss }), " → ", glossNode(i.to), h("span", { className: "ck-count", textContent: `× ${i.form.count}` }), show, use),
          ...i.reasons.map((r) => h("p", { className: "note", textContent: r })),
        );
      };
      return [h("h3", { className: "ck-title", textContent: "Glosses against the Leipzig rules" }), h("p", { className: "note", textContent: leipzigLine(leipzig.issues, leipzig.forms) }), h("ul", { className: "ck-list" }, ...leipzig.issues.slice(0, 100).map(card))];
    };

    const breakdownsSection = (): HTMLElement[] => {
      if (breakdowns === null) return [];
      const card = (s: Segmented): HTMLElement => {
        const variants = s.variants.map((v) => {
          const id = `breakdown\u0001${v.tokenId}`;
          const use = h("button", { type: "button", className: armed === id ? "danger" : "", textContent: useBreakdownLabel(v, s, armed === id), disabled: busy, title: `Break every analysed «${s.form}» into ${v.breakdown} with its glosses` });
          use.onclick = () => {
            if (armed !== id) {
              armed = id;
              paint();
              return;
            }
            void useBreakdown(v);
          };
          const show = h("button", { type: "button", className: "ck-show", textContent: `in «${v.textTitle || "(untitled)"}»`, title: "Open a sentence where it is broken up this way" });
          show.onclick = () => selectSegment({ textId: v.textId, tierId: v.tierId, segmentId: v.segmentId });
          return h("li", { className: "ck-variant" }, h("span", { className: "ck-morph", textContent: v.breakdown }), glossNode(v.glosses), h("span", { className: "ck-count", textContent: `× ${v.count}` }), show, use);
        });
        return h("li", { className: "ck-card" }, h("div", { className: "ck-head" }, h("span", { className: "ck-morph", textContent: s.form })), h("p", { className: "note", textContent: breakdownNote(s) }), h("ul", { className: "ck-variants" }, ...variants));
      };
      return [h("h3", { className: "ck-title", textContent: "Words broken up more than one way" }), h("p", { className: "note", textContent: breakdownsLine(breakdowns) }), h("ul", { className: "ck-list" }, ...breakdowns.slice(0, 100).map(card))];
    };

    /** Make lexicon entries for these roots (all of them when `keys` is empty). */
    const add = async (keys: string[]) => {
      busy = true;
      paint();
      try {
        const r = await glossAddMissing({ keys });
        showToast({ tone: "info", lines: [addedLine(r)] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      armed = null;
      await refreshAll();
      load();
    };

    const settle = async (d: Drift, useLexicon: boolean) => {
      busy = true;
      paint();
      try {
        const changed = await glossSettleDrift({ senseId: d.senseId, textGloss: d.textGloss, useLexicon });
        showToast({ tone: "info", lines: [settledLine(changed, useLexicon, useLexicon ? d.senseGloss : d.textGloss)] });
        if (!useLexicon) touchLexicon();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      armed = null;
      await refreshAll();
      load();
    };

    const readinessSection = (): HTMLElement[] => {
      if (ready === null || ready.total === 0) return [];
      return [
        h("h3", { className: "ck-title", textContent: "Ready for the archive" }),
        h("p", { className: "note", textContent: readinessLine(ready) }),
        h(
          "ul",
          { className: "ck-list" },
          ...ready.texts.slice(0, 200).map((t) => {
            const open = h("button", { type: "button", className: "link-button", textContent: t.title || "(untitled)", title: "Open this text; its details are in the Texts panel, under Details" });
            open.onclick = () => void selectText(t.textId);
            return h("li", { className: "ck-card" }, h("div", { className: "ck-head" }, open, h("span", { className: "note", textContent: ` · ${t.missing.length} to do` })), h("p", { className: "note", textContent: readinessNote(t) }));
          }),
        ),
      ];
    };

    const consentSection = (): HTMLElement[] => {
      if (consent === null || consent.total === 0) return [];
      const people = h("button", { type: "button", textContent: "Open People", title: "Add or edit consent records in the People panel" });
      people.onclick = () => window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "people" }));
      return [
        h("h3", { className: "ck-title", textContent: "Consent records" }),
        h("div", { className: "ck-bar" }, h("p", { className: "note", textContent: consentLine(consent) }), people),
        h(
          "ul",
          { className: "ck-list" },
          ...consent.texts.slice(0, 200).map((t) => {
            const open = h("button", { type: "button", className: "link-button", textContent: t.title || "(untitled)", title: "Open this text" });
            open.onclick = () => void selectText(t.textId);
            return h("li", { className: `ck-card${t.records === 0 ? " ck-slip" : ""}` }, h("div", { className: "ck-head" }, open, h("span", { className: "note", textContent: ` · ${t.records} ${t.records === 1 ? "record" : "records"}` })), h("p", { className: "note", textContent: consentNote(t, termLabel) }));
          }),
        ),
      ];
    };

    const driftCard = (d: Drift): HTMLElement => {
      const id = `drift\u0001${d.senseId}\u0001${d.textGloss}`;
      const entry = h("button", { type: "button", className: "link-button ck-morph", textContent: headwordOf(d), title: "Open this entry in the Lexicon" });
      entry.onclick = () => {
        openEntry(d.entryId);
        window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "lexicon" }));
      };
      const show = h("button", { type: "button", className: "ck-show", textContent: `in «${d.first.textTitle || "(untitled)"}»`, title: "Open a sentence where it is glossed this way" });
      show.onclick = () => selectSegment({ textId: d.first.textId, tierId: d.first.tierId, segmentId: d.first.segmentId });
      const inTexts =
        d.senseGloss === ""
          ? null
          : h("button", {
              type: "button",
              className: armed === id ? "danger" : "",
              disabled: busy,
              textContent: armed === id ? `Change ${d.count}: sure?` : `Use «${d.senseGloss}» in the texts`,
              title: `Gloss ${d.count === 1 ? "this word" : `these ${d.count} words`} as the lexicon does`,
            });
      if (inTexts !== null)
        inTexts.onclick = () => {
          if (d.count > 1 && armed !== id) {
            armed = id;
            paint();
            return;
          }
          void settle(d, true);
        };
      const inLexicon = h("button", { type: "button", disabled: busy, textContent: `Make «${d.textGloss}» the lexicon's gloss`, title: "Change the sense's gloss in the lexicon to the one the texts use" });
      inLexicon.onclick = () => void settle(d, false);
      return h(
        "li",
        { className: "ck-card" },
        h("div", { className: "ck-head" }, entry, h("span", { className: "ck-kind", textContent: d.senseGloss === "" ? "the lexicon has no gloss" : "lexicon: " }), d.senseGloss === "" ? null : glossNode(d.senseGloss)),
        h("div", { className: "ck-variant" }, h("span", { className: "note", textContent: "texts: " }), glossNode(d.textGloss), h("span", { className: "ck-count", textContent: `× ${d.count}` }), show),
        h("div", { className: "ck-bar" }, inTexts, inLexicon),
      );
    };

    const missingCard = (r: Missing): HTMLElement => {
      const glosses = r.variants.map((v) => {
        const show = h("button", { type: "button", className: "ck-show", textContent: `in «${v.textTitle || "(untitled)"}»`, title: "Open a sentence where it is glossed this way" });
        show.onclick = () => selectSegment({ textId: v.textId, tierId: v.tierId, segmentId: v.segmentId });
        return h("li", { className: "ck-variant" }, glossNode(v.gloss), h("span", { className: "ck-count", textContent: `× ${v.count}` }), show);
      });
      const addOne = h("button", { type: "button", textContent: addLabel(r), disabled: busy, title: `A lexicon entry «${r.surface}» with a sense per gloss, every use of it linked` });
      addOne.onclick = () => void add([r.key]);
      return h(
        "li",
        { className: "ck-card" },
        h("div", { className: "ck-head" }, h("span", { className: "ck-morph", textContent: r.surface }), h("span", { className: "ck-kind", textContent: `root · ${r.total}×` }), addOne),
        h("ul", { className: "ck-variants" }, ...glosses),
      );
    };

    const card = (r: Inconsistent): HTMLElement => {
      const variants = r.variants.map((v) => {
        const id = `${r.key}\u0001${r.kind}\u0001${v.gloss}`;
        const use = h("button", { type: "button", className: armed === id ? "danger" : "", textContent: useLabel(v, r, armed === id), disabled: busy, title: `Gloss every «${marked({ surface: r.surface, type: r.kind as MorphType })}» that reads otherwise as «${v.gloss}»` });
        use.onclick = () => {
          if (armed !== id) {
            armed = id;
            paint();
            return;
          }
          void unify(r, v.gloss);
        };
        const show = h("button", { type: "button", className: "ck-show", textContent: `in «${v.textTitle || "(untitled)"}»`, title: "Open a sentence where it is glossed this way" });
        show.onclick = () => selectSegment({ textId: v.textId, tierId: v.tierId, segmentId: v.segmentId });
        return h("li", { className: "ck-variant" }, glossNode(v.gloss), h("span", { className: "ck-count", textContent: `× ${v.count}` }), show, use);
      });
      return h(
        "li",
        { className: `ck-card${r.slip ? " ck-slip" : ""}` },
        h("div", { className: "ck-head" }, h("span", { className: "ck-morph", textContent: marked({ surface: r.surface, type: r.kind as MorphType }) }), h("span", { className: "ck-kind", textContent: r.kind })),
        h("p", { className: "note", textContent: cardNote(r) }),
        h("ul", { className: "ck-variants" }, ...variants),
      );
    };

    /** Where a character occurs: a sentence to open, or the entry using it. */
    const where = (r: CharRow): HTMLElement | null => {
      if (r.place !== null) {
        const p = r.place;
        const show = h("button", { type: "button", className: "ck-show", textContent: `in «${p.textTitle || "(untitled)"}»`, title: p.context });
        show.onclick = () => selectSegment({ textId: p.textId, tierId: p.tierId, segmentId: p.segmentId });
        return show;
      }
      return r.entry === null ? null : h("span", { className: "note", textContent: `in the entry «${r.entry[1]}»` });
    };

    const charCard = (r: CharRow): HTMLElement => {
      const find = findable(r) ? h("button", { type: "button", className: "ck-show", textContent: "Every word with it", title: `Open the Concordance on *${r.ch}*` }) : null;
      if (find !== null) find.onclick = () => askConcordance(`*${r.ch}*`, "word", null, false);
      return h(
        "li",
        { className: "ck-card ck-slip" },
        h("div", { className: "ck-head" }, h("span", { className: "ck-char", textContent: glyph(r) }), h("span", { className: "ck-kind", textContent: `${r.code} · ${r.name}` })),
        ...r.flags.map((f) => h("p", { className: "note", textContent: f })),
        h("div", { className: "ck-variant" }, h("span", { className: "ck-count", textContent: `× ${r.inTexts} in texts${r.inLexicon > 0 ? `, ${r.inLexicon} in the lexicon` : ""}` }), where(r), find),
      );
    };

    const charTable = (all: CharRow[]): HTMLElement => {
      const head = h("tr", {}, ...["", "Code", "Name", "Script", "Texts", "Lexicon"].map((t) => h("th", { textContent: t })));
      const rows = all.map((r) =>
        h(
          "tr",
          { className: r.flags.length > 0 ? "ck-flagged" : "" },
          h("td", { className: "ck-char", textContent: glyph(r) }),
          h("td", { textContent: r.code }),
          h("td", { textContent: r.name }),
          h("td", { textContent: r.script }),
          h("td", { className: "num", textContent: String(r.inTexts) }),
          h("td", { className: "num", textContent: String(r.inLexicon) }),
        ),
      );
      return h("table", { className: "fq-table ck-chars" }, h("thead", {}, head), h("tbody", {}, ...rows));
    };

    const charSection = (): HTMLElement[] => {
      if (chars === null) return [];
      const flagged = chars.rows.filter((r) => r.flags.length > 0);
      const toggle = h("button", { type: "button", textContent: showAllChars ? "Hide the full list" : `Every character (${chars.rows.length})` });
      toggle.onclick = () => {
        showAllChars = !showAllChars;
        paint();
      };
      return [
        h("h3", { className: "ck-title", textContent: "Characters" }),
        h("div", { className: "ck-bar" }, h("p", { className: "note", textContent: inventoryNote(chars) }), chars.rows.length > 0 ? toggle : null),
        h("ul", { className: "ck-list" }, ...flagged.map(charCard)),
        ...(showAllChars ? [charTable(chars.rows)] : []),
      ];
    };

    const pairCard = (p: NearPair): HTMLElement => {
      const open = (word: string) => {
        const b = h("button", { type: "button", className: "ck-show ck-word", textContent: word, title: `Every «${word}» in the Concordance` });
        b.onclick = () => askConcordance(word, "word", null, true);
        return b;
      };
      return h(
        "li",
        { className: "ck-card" },
        h(
          "div",
          { className: "ck-variant" },
          open(p.rare),
          h("span", { className: "ck-count", textContent: `× ${p.rareCount}` }),
          h("span", { className: "note", textContent: "beside" }),
          open(p.common),
          h("span", { className: "ck-count", textContent: `× ${p.commonCount}` }),
        ),
        h("p", { className: "note", textContent: pairNote(p) }),
      );
    };

    /** Fill every sense with no example from its first use in an open text. */
    const fillExamples = async () => {
      busy = true;
      paint();
      try {
        const n = await lexiconExamplesFromTexts();
        showToast({ tone: "info", lines: [examplesAddedLine(n)] });
        touchLexicon();
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      busy = false;
      armed = null;
      await refreshAll();
      load();
    };

    /** Parsed sentences whose tree has no root, several, a cycle or a missing head. */
    const treesSection = (): HTMLElement[] => {
      if (trees === null || trees.annotated === 0) return [];
      const card = (t: TreeProblem) => {
        const open = h("button", { type: "button", className: "link-button", textContent: t.content || "(no text)", title: `Open this sentence of «${t.textTitle}» in the Sentence panel` });
        open.onclick = async () => {
          await selectText(t.textId);
          selectSegment({ textId: t.textId, tierId: t.tierId, segmentId: t.segmentId });
          window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "sentence" }));
        };
        return h("li", { className: "ck-card" }, h("div", { className: "ck-head" }, open, h("span", { className: "note", textContent: ` · ${t.textTitle}` })), ...t.problems.map((p) => h("p", { className: "note", textContent: p })));
      };
      return [
        h("h3", { className: "ck-title", textContent: "Dependency trees" }),
        h("p", { className: "note", textContent: treesLine(trees.total, trees.annotated, trees.sentences.length) }),
        h("ul", { className: "ck-list" }, ...trees.sentences.map(card)),
      ];
    };

    const examplesSection = (): HTMLElement[] => {
      if (gaps === null || gaps.without === 0) return [];
      const fill =
        gaps.fillable === 0
          ? null
          : h("button", {
              type: "button",
              className: armed === "examples" ? "danger" : "",
              disabled: busy,
              textContent: armed === "examples" ? `Add ${gaps.fillable}: sure?` : `Add ${gaps.fillable} from the texts`,
              title: "Each sense with no example sentence gets its first use in a text that is open, with its translation, as Use as example does",
              onclick: () => {
                if (armed !== "examples") {
                  armed = "examples";
                  paint();
                  return;
                }
                void fillExamples();
              },
            });
      return [h("h3", { className: "ck-title", textContent: "Example sentences" }), h("div", { className: "ck-bar" }, h("p", { className: "note", textContent: exampleGapsLine(gaps) }), fill)];
    };

    const merge = async (p: LikelyPair) => {
      busy = true;
      armed = null;
      paint();
      try {
        await lexiconMerge({ keep: p.keep.id, other: p.other.id });
        touchLexicon();
        showToast({ tone: "info", lines: [`Merged ${headwordOf({ headword: p.other.lemma, homograph: p.other.homograph })} into ${headwordOf({ headword: p.keep.lemma, homograph: p.keep.homograph })}: its senses, forms and links are now there.`] });
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      } finally {
        busy = false;
      }
      refreshAll();
    };

    const doubleCard = (p: LikelyPair): HTMLElement => {
      const name = (x: LikelyPair["keep"]) => headwordOf({ headword: x.lemma, homograph: x.homograph });
      const open = (x: LikelyPair["keep"]) => {
        const b = h("button", { type: "button", className: "ck-show ck-word", textContent: name(x), title: `Open «${name(x)}» in the Lexicon (${x.glosses.join("; ")})` });
        b.onclick = () => {
          openEntry(x.id);
          window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "lexicon" }));
        };
        return b;
      };
      const id = `merge:${p.keep.id}:${p.other.id}`;
      const go = h("button", {
        type: "button",
        className: armed === id ? "danger" : "",
        disabled: busy,
        textContent: armed === id ? `Merge «${name(p.other)}» into «${name(p.keep)}»: sure?` : `Merge «${name(p.other)}» into «${name(p.keep)}»`,
        title: "Move the second entry's senses, forms, pronunciations and relations to the first, and delete the second",
      });
      go.onclick = () => {
        if (armed !== id) {
          armed = id;
          paint();
          return;
        }
        void merge(p);
      };
      return h("li", { className: "ck-card" }, h("div", { className: "ck-variant" }, open(p.keep), h("span", { className: "note", textContent: "and" }), open(p.other)), h("p", { className: "note", textContent: duplicateNote(p) }), go);
    };

    const timingCard = (i: TimingIssue): HTMLElement => {
      const open = h("button", { type: "button", className: "link-button", textContent: i.sentence || "(no text)", title: `Open this segment of «${i.textTitle}» on the Timeline` });
      open.onclick = async () => {
        await selectText(i.textId);
        selectSegment({ textId: i.textId, tierId: i.tierId, segmentId: i.segmentId });
        window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "timeline" }));
      };
      const label = { overlap: "Overlaps", short: "Very short", long: "Very long" }[i.kind];
      return h("li", { className: "ck-card" }, h("div", { className: "ck-variant" }, h("strong", { textContent: label }), h("span", { className: "note", textContent: `«${i.textTitle}»` }), open), h("p", { className: "note", textContent: timingNote(i) }));
    };

    const unclearCard = (r: UnclearRow): HTMLElement => {
      const open = h("button", { type: "button", className: "link-button", textContent: r.content, title: `Open this segment of «${r.textTitle}» in Transcribe` });
      open.onclick = async () => {
        await selectText(r.textId);
        selectSegment({ textId: r.textId, tierId: r.tierId, segmentId: r.segmentId });
        window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "transcribe" }));
      };
      return h("li", { className: "ck-card" }, h("div", { className: "ck-variant" }, h("span", { className: "note", textContent: `«${r.textTitle}» ${spanOf(r)}` }), open));
    };

    const printUnclear = async () => {
      if (unclear === null || unclear.rows.length === 0) return;
      try {
        const folder = await pickFolder({ directory: true, title: "Where should the list go?" });
        if (typeof folder !== "string") return;
        const language = await projectLanguage();
        const html = unclearSheet(unclear.rows, { language: language.name, date: localDay(new Date()), generator: "wugs" });
        const r = await saveDocument({ folder, title: `${language.name.trim() || "Project"} unclear passages`, extension: "html", contents: html });
        showToast({ tone: "info", lines: [`Saved the list in ${r.path}. Open it in a browser and print it for the session.`, ...r.notes] });
      } catch (e) {
        showToast({ tone: "error", lines: [`Could not save the list: ${errorText(e)}`] });
      }
    };

    const unclearSection = (): HTMLElement[] =>
      unclear === null
        ? []
        : [
            h("h3", { className: "ck-title", textContent: "Unclear passages" }),
            h("p", { className: "note", textContent: unclearLine(unclear) }),
            ...(unclear.rows.length === 0 ? [] : [h("div", { className: "cc-bar" }, h("button", { type: "button", className: "ck-print-unclear", textContent: "Print a list for the next session…", onclick: () => void printUnclear() })), h("ul", { className: "ck-list" }, ...unclear.rows.slice(0, 200).map(unclearCard))]),
          ];

    const timingSection = (): HTMLElement[] =>
      timing === null
        ? []
        : [h("h3", { className: "ck-title", textContent: "Segment timing" }), h("p", { className: "note", textContent: timingLine(timing) }), h("ul", { className: "ck-list" }, ...timing.issues.map(timingCard))];

    const twiceCard = (t: Twice): HTMLElement => {
      const copies = t.copies.slice(0, 6).map((c, i) => {
        const b = h("button", { type: "button", className: "link-button", textContent: `«${c.textTitle}»`, title: `Open copy ${i + 1} in the Gloss panel` });
        b.onclick = async () => {
          await selectText(c.textId);
          selectSegment({ textId: c.textId, tierId: c.tierId, segmentId: c.segmentId });
          window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "gloss" }));
        };
        return b;
      });
      return h(
        "li",
        { className: "ck-card" },
        h("div", { className: "ck-variant" }, h("strong", { textContent: t.sentence }), h("span", { className: "note", textContent: twiceNote(t) })),
        h("div", { className: "ck-variant" }, ...copies, t.copies.length > 6 ? h("span", { className: "note", textContent: `and ${t.copies.length - 6} more` }) : null),
      );
    };

    const twiceSection = (): HTMLElement[] =>
      twice === null ? [] : [h("h3", { className: "ck-title", textContent: "Sentences written twice" }), h("p", { className: "note", textContent: twiceLine(twice) }), h("ul", { className: "ck-list" }, ...twice.listed.slice(0, 100).map(twiceCard))];

    const doublesSection = (): HTMLElement[] =>
      doubles === null
        ? []
        : [
            h("h3", { className: "ck-title", textContent: "Entries that may be one word" }),
            h("p", { className: "note", textContent: duplicatesLine(doubles.pairs, doubles.entries) }),
            h("ul", { className: "ck-list" }, ...doubles.pairs.map(doubleCard)),
          ];

    const spellingSection = (): HTMLElement[] =>
      near === null
        ? []
        : [
            h("h3", { className: "ck-title", textContent: "Spellings to check" }),
            h("div", { className: "ck-bar" }, h("p", { className: "note", textContent: spellingSummary(near.pairs, near.read) })),
            h("ul", { className: "ck-list" }, ...near.pairs.map(pairCard)),
          ];

    const unusualCard = (u: Unusual): HTMLElement => {
      const b = h("button", { type: "button", className: "ck-show ck-word", textContent: u.word, title: `Every «${u.word}» in the Concordance` });
      b.onclick = () => askConcordance(u.word, "word", null, true);
      return h(
        "li",
        { className: "ck-card" },
        h("div", { className: "ck-variant" }, b, h("span", { className: "ck-count", textContent: `× ${u.count}` })),
        h("p", { className: "note", textContent: `Only word with ${u.sequences.map(sequenceWords).join("; ")}.` }),
      );
    };

    const unusualSection = (): HTMLElement[] =>
      unusual === null
        ? []
        : [h("h3", { className: "ck-title", textContent: "Unusual sound sequences" }), h("p", { className: "note", textContent: unusualLine(unusual) }), h("ul", { className: "ck-list" }, ...unusual.words.map(unusualCard))];

    const paint = () => {
      if (rows === null) {
        el.replaceChildren(emptyState("Checks", "Looking through every gloss…"));
        return;
      }
      const again = h("button", { type: "button", textContent: "Check again", disabled: busy });
      again.onclick = load;
      const addAll = h("button", { type: "button", className: armed === "add-all" ? "danger" : "", disabled: busy, textContent: armed === "add-all" ? `Add ${missing.length}: sure?` : `Add all ${missing.length}` });
      addAll.onclick = () => {
        if (armed !== "add-all") {
          armed = "add-all";
          paint();
          return;
        }
        void add([]);
      };
      el.replaceChildren(
        h("h3", { className: "ck-title", textContent: "Glosses" }),
        h("div", { className: "ck-bar" }, h("p", { className: "note", textContent: summary(rows) }), again),
        h("ul", { className: "ck-list" }, ...rows.map(card)),
        ...breakdownsSection(),
        ...leipzigSection(),
        ...bareSection(),
        h("h3", { className: "ck-title", textContent: "Not in the lexicon" }),
        h("div", { className: "ck-bar" }, h("p", { className: "note", textContent: missingSummary(missing) }), missing.length > 1 ? addAll : null),
        h("ul", { className: "ck-list" }, ...missing.map(missingCard)),
        h("h3", { className: "ck-title", textContent: "Unlike the lexicon" }),
        h("p", { className: "note", textContent: driftSummary(drift) }),
        h("ul", { className: "ck-list" }, ...drift.map(driftCard)),
        ...doublesSection(),
        ...unclearSection(),
        ...timingSection(),
        ...twiceSection(),
        ...examplesSection(),
        ...treesSection(),
        ...consentSection(),
        ...readinessSection(),
        ...spellingSection(),
        ...unusualSection(),
        ...charSection(),
      );
    };

    const draw = () => {
      const s = getState();
      if (s.kind !== "open") {
        key = "";
        rows = null;
        el.replaceChildren(emptyState("No project open", "Open a project to check its glosses."));
        return;
      }
      const wanted = `${s.info.projectId}|${dataGeneration()}`;
      if (wanted === key) return;
      key = wanted;
      if (!busy) load();
    };

    const unprovide = provide("checks", () => [{ id: "checks:again", title: "Check glosses again", group: "Checks", words: ["consistency", "typo", "inconsistent", "lexicon", "missing", "word list", "characters", "alphabet", "orthography", "unicode", "spelling", "typo", "near spellings", "variants"], run: load }]);
    return { element: el, render: draw, dispose: unprovide };
  },
};
