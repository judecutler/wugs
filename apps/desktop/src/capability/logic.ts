// What powers each capability, in words (roadmap 53). Pure, so node --test runs it.
// The view comes from the capabilities command; the panel only draws it.

import type { CapabilityView, ProviderRow } from "../api";

export type ChoiceOption = { value: string; label: string; disabled: boolean };

const nameOf = (v: CapabilityView, id: string | null): string | null => (id === null ? null : (v.options.find((o) => o.id === id)?.name ?? id));

/** The entries of this computer's choice: Automatic, then each provider. Unready ones stay listed (picking one is allowed; it says why it cannot run). */
export function choiceOptions(v: CapabilityView): ChoiceOption[] {
  const autoName = v.pin === null ? nameOf(v, v.effective) : bestReady(v.options)?.name ?? null;
  return [
    { value: "automatic", label: autoName === null ? "Automatic (nothing ready yet)" : `Automatic (${autoName})`, disabled: false },
    ...v.options.map((o) => ({ value: o.id, label: o.ready ? o.name : `${o.name} (${o.note.replace(/\.$/, "").toLowerCase()})`, disabled: false })),
  ];
}

/** Automatic's pick among ready providers, as the backend makes it: best score, then unscored, then listed order. */
export function bestReady(options: ProviderRow[]): ProviderRow | null {
  let best: ProviderRow | null = null;
  for (const o of options) {
    if (!o.ready) continue;
    if (best === null) best = o;
    else if (o.score !== null && (best.score === null || o.score < best.score)) best = o;
  }
  return best;
}

/** One line under the picker: what runs now and why, or what is in the way. */
export function statusLine(v: CapabilityView): string {
  if (v.problem !== null) return v.problem;
  const name = nameOf(v, v.effective) ?? "";
  if (v.pin !== null) return `This project is pinned to ${name}: everyone on it uses this for ${v.title.toLowerCase()}.`;
  return `${v.title} uses ${name} on this computer.`;
}

/** What "Pin for this project" would pin: the provider running now (pinning Automatic would not stop a switch). */
export function pinTarget(v: CapabilityView): string | null {
  return v.pin === null ? v.effective : null;
}
