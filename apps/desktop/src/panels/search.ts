import { search, type SearchRow } from "../api";
import { emptyState, h } from "../dom";
import { hear, playButton } from "../hear/hear";
import { groupRows, moveActive, navigable, navTargets, SCOPES, whereLabel, type Scope } from "../search/logic";
import { attempt, getState, selectSegment, selectText } from "../store";
import { sliced, type PanelDef } from "./panel";

/** Snippets mark hits with [brackets]; show them as <mark> without HTML parsing. */
function snippet(text: string): HTMLElement {
  const out = h("span", { className: "snippet" });
  for (const [i, part] of text.split(/\[|\]/).entries()) out.append(i % 2 === 1 ? h("mark", { textContent: part }) : part);
  return out;
}

/** Open the hit's text, then select the exact segment so the timeline and Transcribe follow. */
/** A segment with times can be heard from Search (roadmap 72). */
const hearable = (r: SearchRow) => navigable(r) && r.kind === "segment" && r.textId !== null && r.tierId !== null && r.startMs !== null && r.endMs !== null;

async function jump(r: SearchRow) {
  if (r.textId === null) return;
  await selectText(r.textId);
  const s = getState();
  if (r.kind === "segment" && r.tierId !== null && s.kind === "open" && s.text?.id === r.textId) {
    selectSegment({ textId: r.textId, tierId: r.tierId, segmentId: r.target });
  }
}

export const searchPanel: PanelDef = {
  title: "Search",
  create() {
    const el = h("div", { className: "panel panel-search" });
    const input = h("input", { type: "search", placeholder: "Search texts and the lexicon" });
    const results = h("div", { className: "results" });
    let scope: Scope = "all";
    const chips = SCOPES.map(({ scope: s, label }) =>
      h("button", {
        type: "button",
        textContent: label,
        onclick: () => {
          scope = s;
          paintChips();
          void run();
        },
      }),
    );
    const paintChips = () => chips.forEach((chip, i) => chip.setAttribute("aria-pressed", String(SCOPES[i]?.scope === scope)));
    const scopes = h("div", { className: "scopes" }, ...chips);

    let seq = 0;
    let targets: SearchRow[] = [];
    let active = -1;
    const run = async () => {
      const query = input.value;
      const mine = ++seq;
      await attempt(async () => {
        const rows = query.trim() === "" ? [] : await search({ query, scope });
        if (mine === seq) show(rows, query);
      });
    };
    const paintActive = () => {
      for (const li of results.querySelectorAll("li.active")) li.classList.remove("active");
      const li = active < 0 ? null : results.querySelector(`li[data-index="${active}"]`);
      li?.classList.add("active");
      li?.scrollIntoView({ block: "nearest" });
    };
    const row = (r: SearchRow) => {
      const where = whereLabel(r);
      const go = navigable(r);
      const li = h(
        "li",
        { className: go ? "clickable" : "", title: r.target, onclick: go ? () => void jump(r) : null },
        hearable(r)
          ? playButton("Open this segment and play it (Shift+Enter)", () => void hear({ textId: r.textId!, tierId: r.tierId!, segmentId: r.target, startMs: r.startMs, endMs: r.endMs }))
          : h("span", { className: "play-spacer", ariaHidden: "true" }),
        h("span", { className: "hit-kind", textContent: where === "" ? `${r.kind}.${r.field}` : where }),
        snippet(r.snippet),
      );
      if (go) li.dataset.index = String(targets.indexOf(r));
      return li;
    };
    const show = (rows: SearchRow[], query: string) => {
      if (query.trim() === "") {
        targets = [];
        results.replaceChildren();
        return;
      }
      const groups = groupRows(rows);
      targets = navTargets(groups);
      active = targets.length === 0 ? -1 : 0;
      const body = groups.flatMap((g) => [
        h("div", { className: "hit-group", textContent: g.title }),
        h("ul", { className: "hits" }, ...g.rows.map(row)),
      ]);
      results.replaceChildren(...(rows.length === 0 ? [h("p", { className: "empty", textContent: "No matches." })] : body));
      paintActive();
    };
    let timer: number | undefined;
    input.oninput = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => void run(), 150);
    };
    input.onkeydown = (ev) => {
      if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
        ev.preventDefault();
        active = moveActive(active, ev.key, targets.length);
        paintActive();
      } else if (ev.key === "Enter") {
        const hit = targets[active];
        if (hit === undefined) return;
        ev.preventDefault();
        // Shift+Enter plays the hit when it can be heard; Enter goes to it.
        if (ev.shiftKey && hearable(hit)) void hear({ textId: hit.textId!, tierId: hit.tierId!, segmentId: hit.target, startMs: hit.startMs, endMs: hit.endMs });
        else void jump(hit);
      }
    };
    return sliced(el, () => getState().kind, (kind) => {
      if (kind === "closed") {
        el.replaceChildren(emptyState("Search", "Open a project to search its texts and lexicon."));
        return;
      }
      input.value = "";
      scope = "all";
      targets = [];
      active = -1;
      paintChips();
      results.replaceChildren();
      el.replaceChildren(input, scopes, results);
    });
  },
};
