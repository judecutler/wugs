// How to cite one text: from its title, its details (who, when, where it
// is kept) and the project's language, as a reference line for a paper's
// example sources and as a BibTeX entry. Pure, so npm-tested.

export type CiteLanguage = { name: string; glottocode: string; iso: string };

const pick = (details: ReadonlyMap<string, string>, keys: readonly string[]): string => {
  for (const k of keys) {
    const v = details.get(k)?.trim();
    if (v) return v;
  }
  return "";
};

/** "Awi Heole; Ilawi Amosa" as people, each "Surname, Given" when written given name first. */
export function people(value: string): string[] {
  return value
    .split(/;|\band\b|&/)
    .map((p) => p.trim().replace(/\s+/g, " "))
    .filter((p) => p !== "");
}

function inverted(name: string): string {
  if (name.includes(",")) return name;
  const parts = name.split(" ");
  if (parts.length < 2) return name;
  return `${parts[parts.length - 1]}, ${parts.slice(0, -1).join(" ")}`;
}

/** The first four-digit year in a date or year detail. */
export function yearOf(value: string): string {
  return /(?<!\d)(\d{4})(?!\d)/.exec(value)?.[1] ?? "";
}

export type Citation = { text: string; bibtex: string };

export function citation(title: string, details: ReadonlyMap<string, string>, language: CiteLanguage): Citation {
  const authors = people(pick(details, ["author", "authors", "speakers", "speaker", "participants"]));
  const role = pick(details, ["author", "authors"]) === "" && authors.length > 0 ? " (speaker" + (authors.length > 1 ? "s" : "") + ")" : "";
  const year = yearOf(pick(details, ["date", "year", "recorded", "created"])) || "n.d.";
  const place = pick(details, ["place", "location"]);
  const project = pick(details, ["project", "collection", "corpus"]);
  const archive = pick(details, ["archive", "publisher", "repository"]);
  const id = pick(details, ["doi", "handle", "url", "idno", "identifier", "id"]);
  const genre = pick(details, ["genre"]);
  const lang = language.name.trim() + (language.glottocode.trim() ? ` [${language.glottocode.trim()}]` : language.iso.trim() ? ` [${language.iso.trim()}]` : "");
  const who = authors.length === 0 ? "" : authors.map((a, i) => (i === 0 ? inverted(a) : a)).join(authors.length === 2 ? " & " : ", ");
  const link = /^10\.\d+\//.test(id) ? `https://doi.org/${id}` : id;
  const what = [genre ? genre[0]!.toUpperCase() + genre.slice(1) : "Text", lang.trim() ? `in ${lang}` : ""].filter((x) => x).join(" ");
  const dot = (v: string) => (v.endsWith(".") ? "" : ".");
  const parts = [
    who ? `${who}${role}. ${year}${dot(year)}` : `${year}${dot(year)}`,
    `${title.trim() || "Untitled"}${dot(title.trim() || "Untitled")}`,
    `${what}${place ? `, ${place}` : ""}.`,
    project ? `${project}.` : "",
    archive ? `${archive}.` : "",
    link ? `${link}` : "",
  ].filter((x) => x !== "");
  const text = parts.join(" ");

  const key =
    ((authors[0] ?? title).split(/[ ,]+/).filter((w) => w).slice(-1)[0] ?? "text")
      .normalize("NFD")
      .replace(/[^A-Za-z0-9]/g, "")
      .toLowerCase() + (year === "n.d." ? "" : year);
  const esc = (v: string) => v.replace(/([&%$#_])/g, "\\$1");
  const fields: [string, string][] = [
    ["author", authors.map(inverted).join(" and ")],
    ["title", `{${title.trim() || "Untitled"}}`],
    ["year", year === "n.d." ? "" : year],
    ["howpublished", [what, place].filter((x) => x).join(", ")],
    ["note", [project, archive].filter((x) => x).join(". ")],
    [/^10\.\d+\//.test(id) ? "doi" : "url", id],
  ];
  const body = fields
    .filter(([, v]) => v !== "" && v !== "{}")
    .map(([k, v]) => `  ${k} = {${k === "url" || k === "doi" ? v : esc(v)}},`)
    .join("\n");
  const bibtex = `@misc{${key || "text"},\n${body}\n}\n`;
  return { text, bibtex };
}
