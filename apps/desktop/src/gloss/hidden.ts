// Hidden edits (GLOSSING-MODEL "Merge semantics"): work another device did on
// a word or morph that the current analysis leaves out. Nothing is deleted; the
// panel says what is hidden and offers to put it back.

/** A field name as a person would say it. */
export function fieldLabel(field: string): string {
  if (field.startsWith("gloss:")) return `${field.slice("gloss:".length)} gloss`;
  switch (field) {
    case "sense":
      return "lexicon link";
    case "morphs":
      return "analysis";
    case "form":
      return "spelling";
    default:
      return field;
  }
}

/** What another device's losing analysis of a word, or breakdown of the segment, was. */
export function alternativeNote(alt: { surfaces: string[]; glosses: string[] }, on: "word" | "segment"): string {
  if (on === "segment") {
    return alt.surfaces.length === 0 ? "Another device left this segment without words." : `Another device broke this segment into «${alt.surfaces.join(" ")}».`;
  }
  if (alt.surfaces.length === 0) return "Another device cleared this word's analysis.";
  const glossed = alt.glosses.some((g) => g !== "");
  const gloss = glossed ? ` '${alt.glosses.map((g) => (g === "" ? "?" : g)).join("-")}'` : "";
  return `Another device analysed this word as «${alt.surfaces.join("-")}${gloss}».`;
}

/** The tooltip of a hidden edit's restore control. */
export function hiddenNote(h: { label: string; fields: string[] }, on: "morph" | "word"): string {
  const what = h.fields.map(fieldLabel).join(", ");
  const why = on === "morph" ? "this word's current analysis leaves that morph out" : "the current words of this segment leave it out";
  return `Another device edited the ${what} of the ${on} «${h.label}», and ${why}. Restore shows it again; nothing was deleted.`;
}
