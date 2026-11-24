// Compare word lists: the project's answers to one of its concept lists
// beside other languages' words for the same concepts, read from their own
// files (src-tauri/src/compare.rs), each with how alike it is to the
// project's word, and the whole table copied as an EDICTOR/LingPy file.
// Nothing is written to the project; loaded lists last until the project
// changes. Rules in ../compare/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { cognates, compareRead, conceptLists, setCognates, conceptListView, errorText, projectLanguage, saveDocument, type ListSummary } from "../api";
import { layout, ldndMatrix, midpointRoot, neighbourJoining, newick, phylip, treeLine, type Lect, type Matrix } from "../compare/tree";
import { standalone } from "./vowels";
import { correspondenceLine, correspondences, correspondenceTsv } from "../compare/correspondences";
import { distanceLine, distances, type Distance } from "../compare/distance";
import { alignAll, alignmentLine, alignmentTsv, cognateForms, kept } from "../compare/alignment";
import { cognacy, cognacyLine, edictorWithCognates, judgeKey, judgementOf, judgementsText, nextJudgement, parseJudgements, type Judgement } from "../compare/cognates";
import { CLOSE, compareLine, compareRows, languagesOf, type OtherForm, type OurConcept, type Row } from "../compare/logic";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { dataGeneration, getState, showToast } from "../store";
import type { PanelDef, PanelView } from "./panel";

export const comparePanel: PanelDef = {
  title: "Compare word lists",
  create(): PanelView {
    const el = h("div", { className: "panel panel-compare" });
    let shownFor = "";
    let lists: ListSummary[] = [];
    let listId = "";
    let concepts: OurConcept[] = [];
    let forms: OtherForm[] = [];
    let rows: Row[] = [];
    let seq = 0;

    const picker = h("select", { className: "cmp-list", title: "Which of the project's concept lists to compare" });
    picker.setAttribute("aria-label", "Concept list");
    const load = h("button", { type: "button", className: "primary", textContent: "Load a word list…", title: "Read another language's words from a CSV or tab-separated table, an EDICTOR file or a CLDF forms.csv" });
    const clear = h("button", { type: "button", textContent: "Clear", title: "Take the loaded lists away" });
    const answeredBox = h("input", { type: "checkbox", checked: true });
    const answered = h("label", { title: "List only the concepts the project has an answer for" }, answeredBox, " Only answered concepts");
    const copy = h("button", { type: "button", textContent: "Copy for EDICTOR", title: "Copy every form, yours and theirs, as an EDICTOR/LingPy wordlist (ID, DOCULECT, CONCEPT, FORM, TOKENS)" });
    const line = h("p", { className: "note cmp-line" });
    /** Cognate judgements, kept with the project (src-tauri/src/cognates.rs). */
    let judged = new Map<string, Judgement>();
    let saving: Promise<unknown> = Promise.resolve();
    const cognacyBox = h("div", { className: "note cmp-cognacy" });
    const distanceBox = h("div", { className: "note cmp-distance", title: "ASJP's measures (Wichmann et al. 2010): LDN is the mean share of sounds two words for the same concept differ in; LDND divides it by the same between words for different concepts, so likeness any two words have by chance is taken out" });
    /** Distances worked out once per set of rows and languages (judging a form does not change them). */
    let distanceMemo: { key: string; value: Distance[] } | null = null;
    /** The rows and languages the tree was drawn for; a change asks for it again. */
    let treeFor = "";
    const judge = (r: Row, language: string, form: string) => {
      const key = judgeKey(language, r.concept.gloss, form);
      const next = nextJudgement(judged.get(key));
      if (next === undefined) judged.delete(key);
      else judged.set(key, next);
      draw();
      const table = judgementsText(judged);
      saving = saving.then(() => setCognates({ table }).catch((e) => showToast({ tone: "error", lines: [`Could not keep the judgement: ${errorText(e)}`] })));
    };
    const table = h("table", { className: "fq-table cmp-table" });
    // Sound correspondences with one language at a time.
    const corrLang = h("select", { className: "cmp-corr-lang", title: "Which language to align with the project's words" });
    corrLang.setAttribute("aria-label", "Language");
    const corrMin = h("select", { className: "cmp-corr-min", title: "Only pairs of words at least this alike are aligned; lower takes in more cognates and more chance resemblances" }, ...[0.3, 0.4, 0.5, 0.6, 0.7].map((v) => h("option", { value: String(v), textContent: `${Math.round(v * 100)}% alike or more`, selected: v === 0.5 })));
    corrMin.setAttribute("aria-label", "Pairs aligned");
    const differBox = h("input", { type: "checkbox", checked: true });
    const differ = h("label", { title: "Leave out a sound that stays the same" }, differBox, " Only where the sounds differ");
    const corrCopy = h("button", { type: "button", textContent: "Copy table", title: "Copy every correspondence with its count and concepts" });
    const corrLine = h("p", { className: "note cmp-corr-line" });
    const corrTable = h("table", { className: "fq-table cmp-corr" });
    const corrBlock = h(
      "section",
      { className: "cmp-corr-block" },
      h("h3", { textContent: "Sound correspondences" }),
      h("p", { className: "note", textContent: "Each concept's closest pair of words, aligned sound by sound and counted: a correspondence that recurs across many concepts is evidence, one met once may be chance." }),
      h("div", { className: "cc-bar" }, corrLang, corrMin, differ, corrCopy),
      corrLine,
      corrTable,
    );

    // One concept's cognates aligned under the project's sounds.
    const alignConcept = h("select", { className: "cmp-align-concept", title: "Which concept's words to align" });
    alignConcept.setAttribute("aria-label", "Concept");
    const closeBox = h("input", { type: "checkbox", checked: true });
    const closeToo = h("label", { title: `Also align forms not judged yet that are ${Math.round(CLOSE * 100)}% alike or more` }, closeBox, " Close ones not judged yet");
    const alignCopy = h("button", { type: "button", textContent: "Copy table", title: "Copy the alignment, a row per language and a column per sound" });
    const alignLine = h("p", { className: "note" });
    const alignTable = h("table", { className: "fq-table cmp-align" });
    const alignBlock = h(
      "section",
      { className: "cmp-align-block" },
      h("h3", { textContent: "Alignment" }),
      h("p", { className: "note", textContent: "A concept's cognates set under the project's word sound by sound, as EDICTOR aligns a cognate set: a sound with nothing across from it joins the next cell, and a column where every word keeps the sound is shaded." }),
      h("div", { className: "cc-bar" }, alignConcept, closeToo, alignCopy),
      alignLine,
      alignTable,
    );

    const drawAlignment = () => {
      const langs = languagesOf(forms);
      const offered = rows.filter((r) => r.ours.length > 0 && langs.some((l) => (r.others.get(l) ?? []).length > 0));
      alignBlock.hidden = offered.length === 0;
      if (offered.length === 0) return;
      const pick = offered.find((r) => r.concept.id === alignConcept.value) ?? offered.find((r) => cognateForms(r, langs, judged, true).length > 0) ?? offered[0]!;
      alignConcept.replaceChildren(...offered.map((r) => h("option", { value: r.concept.id, textContent: `${r.concept.number}. ${r.concept.gloss}`, selected: r === pick })));
      const fs = cognateForms(pick, langs, judged, closeBox.checked);
      const a = alignAll(pick.ours[0]!, fs);
      const k = kept(a);
      alignLine.textContent = alignmentLine(a, fs.filter((f) => f.judged).length);
      alignTable.replaceChildren(
        h("thead", {}, h("tr", {}, h("th", { scope: "col", textContent: "" }), ...a.columns.map((c, i) => h("th", { scope: "col", className: `ipa${a.rows.length > 0 && k[i] === a.rows.length ? " cmp-kept" : ""}`, textContent: c })))),
        h(
          "tbody",
          {},
          ...a.rows.map((r) =>
            h(
              "tr",
              {},
              h("th", { scope: "row", textContent: r.language, title: r.judged ? "Judged cognate" : "Close, not judged yet" }, r.judged ? " ✓" : ""),
              ...r.cells.map((c, i) => h("td", { className: `ipa${c === a.columns[i] ? " cmp-kept" : ""}`, textContent: c })),
            ),
          ),
        ),
      );
      alignCopy.onclick = () =>
        void writeText(alignmentTsv(pick.concept.gloss, a)).then(
          () => showToast({ tone: "info", lines: [`Copied the alignment of «${pick.concept.gloss}».`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
    };

    const draw = () => {
      rows = compareRows(concepts, forms);
      const langs = languagesOf(forms);
      line.textContent = lists.length === 0 ? "The project has no concept list yet: load one in the Elicit panel's Word lists." : compareLine(rows, langs, forms);
      clear.disabled = forms.length === 0;
      copy.disabled = forms.length === 0;
      const shown = answeredBox.checked ? rows.filter((r) => r.ours.length > 0) : rows;
      const words = { y: "judged cognate", n: "judged not cognate" } as const;
      const cell = (r: Row, language: string) =>
        h(
          "td",
          { className: "ipa" },
          ...(r.others.get(language) ?? []).map((c) => {
            const mark = judgementOf(judged, r, language, c.form);
            const alike = c.similarity === null ? "" : `${Math.round(c.similarity * 100)}% alike, by sounds`;
            const b = h(
              "button",
              {
                type: "button",
                className: `cmp-form${(c.similarity ?? 0) >= CLOSE ? " cmp-close" : ""}${mark === undefined ? "" : ` cmp-${mark}`}`,
                disabled: r.ours.length === 0,
                title: r.ours.length === 0 ? "No answer of the project's to judge it against" : `${alike}${alike === "" ? "" : "; "}${mark === undefined ? "not judged" : words[mark]}. Click: cognate, not cognate, not judged.`,
              },
              mark === "y" ? "✓ " : mark === "n" ? "✗ " : "",
              c.form,
              c.similarity === null ? "" : h("small", { textContent: ` ${Math.round(c.similarity * 100)}%` }),
            );
            b.onclick = () => judge(r, language, c.form);
            return b;
          }),
        );
      const memoKey = `${langs.join("\u001f")}|${rows.map((r) => `${r.ours.join(",")}:${langs.map((l) => (r.others.get(l) ?? []).map((c) => c.form).join(",")).join(";")}`).join("|")}`;
      if (distanceMemo?.key !== memoKey) distanceMemo = { key: memoKey, value: distances(rows, langs) };
      distanceBox.replaceChildren(...distanceMemo.value.map((d) => h("div", { textContent: distanceLine(d) })));
      const shares = cognacy(rows, langs, judged);
      cognacyBox.replaceChildren(...(shares.some((c) => c.judged > 0) ? shares.map((c) => h("div", { textContent: cognacyLine(c) })) : []));
      table.replaceChildren(
        h("thead", {}, h("tr", {}, h("th", { scope: "col", textContent: "#" }), h("th", { scope: "col", textContent: "Concept" }), h("th", { scope: "col", textContent: "This project" }), ...langs.map((l) => h("th", { scope: "col", textContent: l })))),
        h(
          "tbody",
          {},
          ...shown.map((r) =>
            h(
              "tr",
              {},
              h("td", { className: "num", textContent: String(r.concept.number) }),
              h("td", { textContent: r.concept.gloss }),
              h("td", { className: "ipa cmp-ours", textContent: r.ours.join(", ") }),
              ...langs.map((l) => cell(r, l)),
            ),
          ),
        ),
      );
      drawCorrespondences();
      drawAlignment();
      const treeKey = `${memoKey}`;
      if (treeKey !== treeFor) {
        treeFor = treeKey;
        tree = null;
      }
      drawTree();
    };

    const drawCorrespondences = () => {
      const langs = languagesOf(forms);
      corrBlock.hidden = langs.length === 0;
      if (langs.length === 0) return;
      const pick = langs.includes(corrLang.value) ? corrLang.value : langs[0]!;
      corrLang.replaceChildren(...langs.map((l) => h("option", { value: l, textContent: l, selected: l === pick })));
      const min = Number(corrMin.value);
      const c = correspondences(rows, pick, min);
      corrLine.textContent = correspondenceLine(c, pick, min);
      corrCopy.onclick = () =>
        void writeText(correspondenceTsv(c, pick)).then(
          () => showToast({ tone: "info", lines: [`Copied ${c.list.length} correspondences with ${pick}.`] }),
          (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
        );
      const shown = differBox.checked ? c.list.filter((x) => !x.same) : c.list;
      corrTable.replaceChildren(
        h("thead", {}, h("tr", {}, h("th", { scope: "col", textContent: "This project" }), h("th", { scope: "col", textContent: pick }), h("th", { scope: "col", textContent: "Count" }), h("th", { scope: "col", textContent: "Met in" }))),
        h(
          "tbody",
          {},
          ...shown.map((x) =>
            h(
              "tr",
              { className: !x.same && x.count >= 2 ? "cmp-recurs" : "" },
              h("td", { className: "ipa", textContent: x.ours }),
              h("td", { className: "ipa", textContent: x.theirs }),
              h("td", { className: "num", textContent: String(x.count) }),
              h("td", { textContent: x.concepts.join(", ") }),
            ),
          ),
        ),
      );
    };

    const loadConcepts = async () => {
      const mine = ++seq;
      if (listId === "") {
        concepts = [];
        draw();
        return;
      }
      try {
        const v = await conceptListView({ listId });
        if (mine !== seq) return;
        concepts = v.concepts.map((c) => ({ id: c.id, number: c.number, gloss: c.gloss, concepticonGloss: c.concepticonGloss, answers: c.answers.map((a) => ({ lemma: a.lemma })) }));
      } catch (e) {
        showToast({ tone: "error", lines: [errorText(e)] });
      }
      draw();
    };

    picker.onchange = () => {
      listId = picker.value;
      void loadConcepts();
    };
    answeredBox.onchange = draw;
    corrLang.onchange = drawCorrespondences;
    corrMin.onchange = drawCorrespondences;
    differBox.onchange = drawCorrespondences;
    alignConcept.onchange = drawAlignment;
    closeBox.onchange = drawAlignment;
    clear.onclick = () => {
      forms = [];
      draw();
    };
    load.onclick = async () => {
      const picked = await pickFile({ multiple: true, title: "Another language's word list", filters: [{ name: "Word lists", extensions: ["csv", "tsv", "tab", "txt", "xlsx", "xls", "ods"] }] });
      const paths = picked === null ? [] : Array.isArray(picked) ? picked : [picked];
      const lines: string[] = [];
      for (const path of paths) {
        try {
          const r = await compareRead({ path });
          forms = forms.concat(r.forms);
          lines.push(`${r.name}: ${r.forms.length} forms of ${languagesOf(r.forms).length} ${languagesOf(r.forms).length === 1 ? "language" : "languages"} (columns ${r.columns.join(", ")})${r.skipped > 0 ? `, ${r.skipped} rows without a concept or form skipped` : ""}.`);
        } catch (e) {
          lines.push(`${path}: ${errorText(e)}`);
        }
      }
      if (lines.length > 0) showToast({ tone: "info", lines });
      draw();
    };
    copy.onclick = async () => {
      const lang = await projectLanguage().catch(() => null);
      void writeText(edictorWithCognates(rows, lang?.name ?? "", judged)).then(
        () => showToast({ tone: "info", lines: ["Copied the word lists as an EDICTOR file. Paste it into a .tsv file and open it at edictor.org, or read it with LingPy."] }),
        (e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }),
      );
    };

    // A neighbour-joining tree of every language, worked out on request (it reads every pair).
    const treeButton = h("button", { type: "button", textContent: "Draw a tree", title: "Join the languages into a tree by LDND, as ASJP does: neighbour joining, rooted at the midpoint" });
    const treeBlock = h("div", { className: "cmp-tree" });
    let tree: { m: Matrix; newick: string } | null = null;
    const SVG = "http://www.w3.org/2000/svg";
    const svgEl = (tag: string, attrs: Record<string, string | number>, text?: string) => {
      const e = document.createElementNS(SVG, tag);
      for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
      if (text !== undefined) e.textContent = text;
      return e;
    };
    const drawTree = () => {
      const langs = languagesOf(forms);
      if (langs.length < 2) {
        treeBlock.replaceChildren();
        return;
      }
      if (tree === null) {
        treeBlock.replaceChildren(h("h3", { className: "cmp-head", textContent: "A tree of the languages" }), treeButton);
        return;
      }
      const t = tree;
      const parts: Node[] = [h("h3", { className: "cmp-head", textContent: "A tree of the languages" })];
      if (t.m.names.length >= 3) {
        const drawn = layout(midpointRoot(neighbourJoining(t.m.names, t.m.d)));
        const ROW = 22;
        const LEFT = 10;
        const WIDTH = 420;
        const labelW = 8 * Math.max(...drawn.leaves.map((l) => l.name.length)) + 20;
        const scale = (WIDTH - LEFT) / (drawn.depth || 1);
        const height = drawn.leaves.length * ROW + 40;
        const svg = svgEl("svg", { class: "cmp-tree-svg", viewBox: `0 0 ${WIDTH + labelW} ${height}`, width: WIDTH + labelW, height, role: "img", "aria-label": "Neighbour-joining tree of the languages" }) as SVGSVGElement;
        const X = (x: number) => LEFT + x * scale;
        const Y = (y: number) => 16 + y * ROW;
        for (const l of drawn.lines) svg.append(svgEl("line", { x1: X(l.x1), y1: Y(l.y1), x2: X(l.x2), y2: Y(l.y2), class: "cmp-branch" }));
        for (const leaf of drawn.leaves) svg.append(svgEl("text", { x: X(leaf.x) + 6, y: Y(leaf.y) + 4, class: "cmp-leaf" }, leaf.name));
        // A scale bar: 0.1 of LDND.
        const bar = 0.1 * scale;
        svg.append(svgEl("line", { x1: LEFT, y1: height - 14, x2: LEFT + bar, y2: height - 14, class: "cmp-branch" }), svgEl("text", { x: LEFT + bar + 6, y: height - 10, class: "cmp-scale" }, "0.1 LDND"));
        const copyNewick = h("button", { type: "button", textContent: "Copy Newick", title: "Copy the tree in Newick, for FigTree, iTOL, SplitsTree or R's ape" });
        copyNewick.onclick = () => void writeText(t.newick).then(() => showToast({ tone: "info", lines: ["Copied the tree in Newick."] }), (e) => showToast({ tone: "error", lines: [errorText(e)] }));
        const copyMatrix = h("button", { type: "button", textContent: "Copy distances", title: "Copy every pair's LDND as a PHYLIP matrix, which SplitsTree and PHYLIP read" });
        copyMatrix.onclick = () => void writeText(phylip(t.m)).then(() => showToast({ tone: "info", lines: ["Copied the distances as a PHYLIP matrix."] }), (e) => showToast({ tone: "error", lines: [errorText(e)] }));
        const save = h("button", { type: "button", textContent: "Save picture…", title: "Save the tree as an SVG picture" });
        save.onclick = () =>
          void (async () => {
            const folder = await pickFile({ directory: true, title: "Where should the picture go?" });
            if (typeof folder !== "string") return;
            try {
              const r = await saveDocument({ folder, title: "language tree", extension: "svg", contents: standalone(svg) });
              showToast({ tone: "info", lines: [`Saved the tree as ${r.path}.`] });
            } catch (e) {
              showToast({ tone: "error", lines: [errorText(e)] });
            }
          })();
        parts.push(h("div", { className: "cc-bar" }, copyNewick, copyMatrix, save, treeButton), h("div", { className: "cmp-tree-box" }, svg));
        treeButton.textContent = "Draw again";
      } else {
        parts.push(treeButton);
      }
      parts.push(h("p", { className: "note", textContent: treeLine(t.m) }));
      treeBlock.replaceChildren(...parts);
    };
    treeButton.onclick = async () => {
      const lang = await projectLanguage().catch(() => null);
      const lects: Lect[] = [{ name: lang?.name || "This project", forms: (r) => r.ours }, ...languagesOf(forms).map((l) => ({ name: l, forms: (r: Row) => (r.others.get(l) ?? []).map((c) => c.form) }))];
      treeButton.disabled = true;
      treeButton.textContent = "Measuring…";
      // Let the button's new words show before the work starts.
      await new Promise((resolve) => setTimeout(resolve, 30));
      const m = ldndMatrix(rows, lects);
      tree = { m, newick: m.names.length >= 3 ? newick(midpointRoot(neighbourJoining(m.names, m.d))) : "" };
      treeButton.disabled = false;
      treeButton.textContent = "Draw a tree";
      drawTree();
    };

    const unprovide = provide("compare", () => [
      { id: "compare:tree", title: "Draw a tree of the compared languages", group: "Compare word lists", words: ["phylogeny", "neighbour joining", "neighbor joining", "newick", "ASJP", "classification", "family tree"], run: () => (treeButton.isConnected ? treeButton.click() : showToast({ tone: "info", lines: ["Load two or more word lists first; the tree needs three languages."] })) },
      { id: "compare:load", title: "Load another language's word list to compare", group: "Compare word lists", words: ["cognate", "comparative", "historical", "edictor", "lingpy", "swadesh"], run: () => load.click() },
    ]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}` : "closed";
      if (key === shownFor) return;
      const sameProject = s.kind === "open" && shownFor.split("|")[0] === s.info.projectId;
      shownFor = key;
      if (s.kind !== "open") {
        el.replaceChildren(emptyState("Compare word lists", "Open a project to compare its word lists.", "The wug is learning its cousins' words."));
        return;
      }
      if (!sameProject) {
        forms = [];
        listId = "";
        el.replaceChildren(h("div", { className: "cc-bar" }, picker, load, clear, answered, copy), line, distanceBox, cognacyBox, h("div", { className: "cmp-rows" }, table, corrBlock, alignBlock, treeBlock));
        judged = new Map();
        void cognates().then(
          (t) => {
            judged = parseJudgements(t);
            draw();
          },
          () => undefined,
        );
      }
      void conceptLists().then(
        (l) => {
          lists = l;
          if (!lists.some((x) => x.id === listId)) listId = lists[0]?.id ?? "";
          picker.replaceChildren(...lists.map((x) => h("option", { value: x.id, textContent: `${x.name} (${x.answered} of ${x.total})`, selected: x.id === listId })));
          void loadConcepts();
        },
        (e) => showToast({ tone: "error", lines: [errorText(e)] }),
      );
    };
    return { element: el, render, dispose: unprovide };
  },
};
