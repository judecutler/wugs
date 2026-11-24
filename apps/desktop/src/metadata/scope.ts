// The scope picker the corpus panels share: the whole project, one text, or
// the texts a detail picks ("Genre: news"). Details' values are fetched in
// the background and cached briefly; a picker fills from the cache at once
// and again when fresh values arrive.
import { corpusTracks, metadataValues, speakerValues } from "../api";
import { h } from "../dom";
import { getState } from "../store";
import { parseScope, speakerOptions, trackValue, whereOptions, type MetaValues, type Scope } from "./logic";

let cache: { project: string; at: number; values: MetaValues[]; tracks: { id: string; name: string }[]; speakers: [string, [string, number][]][] } | null = null;
let asking: Promise<void> | null = null;
const waiting = new Set<() => void>();

function refresh(project: string) {
  if (asking !== null) return;
  asking = Promise.all([metadataValues().catch(() => [] as MetaValues[]), corpusTracks().catch(() => []), speakerValues().catch(() => [] as [string, [string, number][]][])])
    .then(([values, tracks, speakers]) => {
      cache = { project, at: Date.now(), values, tracks, speakers };
    })
    .finally(() => {
      asking = null;
      for (const again of [...waiting]) again();
    });
}

/**
 * Fill `select` with the scopes, keeping `value` when it is still offered
 * (else the whole project); returns the value chosen. `again` is called once
 * fresher details arrive, so the panel can fill the picker again.
 */
export function fillScopes(select: HTMLSelectElement, value: string, again: () => void, whole = "Whole project"): string {
  const s = getState();
  if (s.kind !== "open") {
    select.replaceChildren(h("option", { value: "", textContent: whole }));
    return "";
  }
  const project = s.info.projectId;
  if (cache === null || cache.project !== project || Date.now() - cache.at > 3000) {
    waiting.add(again);
    refresh(project);
  } else {
    waiting.delete(again);
  }
  const values = cache?.project === project ? cache.values : [];
  const groups = whereOptions(values, s.texts.length);
  const options: HTMLElement[] = [h("option", { value: "", textContent: whole })];
  if (s.texts.length > 0) options.push(h("optgroup", { label: "One text" }, ...s.texts.map((t) => h("option", { value: t.id, textContent: t.title || t.id }))));
  for (const g of groups) options.push(h("optgroup", { label: "Texts by detail" }, ...g.options.map((o) => h("option", { value: o.value, textContent: o.label }))));
  // One speaker's track, when the project has more than one (a CHAT file brings one per speaker).
  const tracks = cache?.project === project ? cache.tracks : [];
  if (tracks.length > 1) options.push(h("optgroup", { label: "One track" }, ...tracks.map((t) => h("option", { value: trackValue(t.id, t.name), textContent: t.name || t.id }))));
  // The tracks whose speaker (People panel) has a detail: sex, age group, village…
  const speakers = speakerOptions(cache?.project === project ? cache.speakers : []);
  if (speakers.length > 0) options.push(h("optgroup", { label: "Speakers by detail" }, ...speakers.map((o) => h("option", { value: o.value, textContent: o.label }))));
  select.replaceChildren(...options);
  const offered = [...select.querySelectorAll("option")].some((o) => o.value === value);
  select.value = offered ? value : "";
  return select.value;
}

/** What a picker's value asks the commands for. */
export const scopeOf = (value: string): Scope => parseScope(value);

/** Forget the cached details (after an edit or an import), so pickers fetch them again. */
export function detailsChanged(): void {
  cache = null;
}
