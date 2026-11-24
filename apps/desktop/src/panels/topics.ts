// Topics: the sentences filed under each grammar topic in the Gloss panel
// (src-tauri/src/topics.rs), gathered for writing a grammar: a topic's
// sentences open in the Gloss panel and copy as numbered examples in the
// Copy example format. Rules in ../topics/logic.ts.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { errorText, renameTopic, topicsOverview } from "../api";
import { emptyState, h } from "../dom";
import { provide } from "../palette/registry";
import { dataGeneration, getState, refreshAll, selectSegment, selectText, showToast } from "../store";
import { matchTopics, topicsLine, topicTsv, type Topics } from "../topics/logic";
import { copyAsExamples } from "./concordance";
import { SHOW_PANEL, type PanelDef, type PanelView } from "./panel";

export const topicsPanel: PanelDef = {
  title: "Topics",
  create(): PanelView {
    const el = h("div", { className: "panel panel-topics" });
    let data: Topics | null = null;
    let picked = "";
    let seq = 0;
    let readFor = "";
    const find = h("input", { type: "search", className: "gt-find", placeholder: "Find a topic", spellcheck: false });
    find.setAttribute("aria-label", "Find a topic");
    const line = h("p", { className: "note gt-line" });
    const list = h("div", { className: "gt-list", role: "listbox", ariaLabel: "Topics" });
    const detail = h("div", { className: "gt-detail" });

    const openSentence = async (textId: string, tierId: string, segmentId: string) => {
      await selectText(textId);
      selectSegment({ textId, tierId, segmentId });
      window.dispatchEvent(new CustomEvent(SHOW_PANEL, { detail: "gloss" }));
    };

    const drawDetail = () => {
      const topic = data?.topics.find((t) => t.name === picked) ?? null;
      if (topic === null) {
        detail.replaceChildren();
        return;
      }
      const copy = h("button", { type: "button", textContent: "Copy as examples", title: "Copy these sentences as numbered glossed examples, in the format chosen for Copy example in the Gloss panel (Word, LaTeX or plain text)" });
      copy.onclick = () => void copyAsExamples(topic.sentences).catch((e) => showToast({ tone: "error", lines: [`Could not copy: ${errorText(e)}`] }));
      const table = h("button", { type: "button", textContent: "Copy table", title: "Copy the topic's sentences and translations as a table" });
      table.onclick = () => void writeText(topicTsv(topic)).then(() => showToast({ tone: "info", lines: [`Copied «${topic.name}»'s ${topic.sentences.length} sentences as a table.`] }));
      const name = h("input", { className: "gt-rename", value: topic.name, spellcheck: false, title: "Rename the topic on every sentence, or give the name of another topic to fold this one into it" });
      name.setAttribute("aria-label", "Topic's name");
      const rename = h("button", { type: "button", textContent: "Rename" });
      const doRename = async () => {
        const to = name.value.trim();
        if (to === "" || to === topic.name) return;
        try {
          data = await renameTopic(topic.name, to);
          picked = data.topics.find((t) => t.name.toLowerCase() === to.toLowerCase())?.name ?? to;
          draw();
          showToast({ tone: "info", lines: [`«${topic.name}» is now «${picked}».`] });
          await refreshAll();
        } catch (e) {
          showToast({ tone: "error", lines: [`Could not rename it: ${errorText(e)}`] });
        }
      };
      rename.onclick = () => void doRename();
      name.onkeydown = (e) => {
        if (e.key === "Enter") void doRename();
      };
      detail.replaceChildren(
        h("h3", { textContent: topic.name }),
        h("div", { className: "cc-bar" }, copy, table, name, rename),
        h(
          "ol",
          { className: "gt-sentences" },
          ...topic.sentences.map((s) => {
            const li = h(
              "li",
              { tabIndex: 0, title: "Open this sentence in the Gloss panel" },
              h("span", { className: "gt-text", textContent: s.textTitle }),
              h("span", { className: "gt-content", textContent: s.content }),
              s.translation === null ? null : h("span", { className: "gt-translation", textContent: `‘${s.translation}’` }),
            );
            li.onclick = () => void openSentence(s.textId, s.tierId, s.segmentId);
            li.onkeydown = (e) => {
              if (e.key === "Enter") void openSentence(s.textId, s.tierId, s.segmentId);
            };
            return li;
          }),
        ),
      );
    };

    const draw = () => {
      if (data === null) return;
      line.textContent = topicsLine(data);
      const shown = matchTopics(data.topics, find.value);
      if (picked === "" || !data.topics.some((t) => t.name === picked)) picked = shown[0]?.name ?? "";
      list.replaceChildren(
        ...shown.map((t) => {
          const b = h("button", { type: "button", className: "gt-topic" }, h("span", { textContent: t.name }), h("span", { className: "gt-count", textContent: String(t.sentences.length) }));
          b.setAttribute("role", "option");
          b.setAttribute("aria-selected", String(t.name === picked));
          b.onclick = () => {
            picked = t.name;
            draw();
          };
          return b;
        }),
      );
      drawDetail();
    };
    find.oninput = draw;

    async function load(): Promise<void> {
      const mine = ++seq;
      try {
        const r = await topicsOverview();
        if (mine !== seq) return;
        data = r;
        draw();
      } catch (e) {
        if (mine !== seq) return;
        line.textContent = `Could not read the topics: ${errorText(e)}`;
      }
    }

    const unprovide = provide("topics", () => [{ id: "topics:focus", title: "Find a grammar topic's example sentences", group: "Topics", words: ["grammar", "examples", "chapter", "collection", "tag"], run: () => find.focus() }]);

    const render = () => {
      const s = getState();
      const key = s.kind === "open" ? `${s.info.projectId}|${dataGeneration()}` : "closed";
      if (key === readFor) return;
      const sameProject = s.kind === "open" && readFor.split("|")[0] === s.info.projectId;
      readFor = key;
      if (s.kind !== "open") {
        seq += 1;
        data = null;
        el.replaceChildren(emptyState("Topics", "Open a project to see its topics.", "The wug files its examples in folders."));
        return;
      }
      if (!sameProject) {
        data = null;
        picked = "";
        find.value = "";
        line.textContent = "";
        list.replaceChildren();
        detail.replaceChildren();
        el.replaceChildren(h("div", { className: "cc-bar" }, find), line, h("div", { className: "gt-body" }, list, detail));
      }
      void load();
    };
    return { element: el, render, dispose: unprovide };
  },
};
