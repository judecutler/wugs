// Synthetic backend answers for the screenshot harness, shaped after
// src/api.ts. Every name, word and recording here is made up: the language
// ("Tokari"), the people and the waveform. Loaded before mock-tauri.js.
(() => {
  const PROJECT = "01JC8R2V7Q5N3M4K6P9T0W1XYZ";
  const PROJECT_PATH = "/projects/field-trip-2026";
  const DAY = Date.UTC(2026, 10, 9, 14, 30);

  const projectInfo = { projectId: PROJECT, path: PROJECT_PATH, texts: 3, conflicts: 2, relocation: null };

  const texts = [
    { id: "t-count", title: "Counting to ten", kind: "elicitation", segments: 10, recording: "src-count" },
    { id: "t-fox", title: "The fox and the well", kind: "narrative", segments: 24, recording: "src-fox" },
    { id: "t-market", title: "Greetings at the market", kind: "conversation", segments: 7, recording: null },
  ];

  const tiers = [
    { id: "tier-utt", name: "utterance", kind: "time-aligned", role: "transcription", language: "x-tok", utterance: true },
    { id: "tier-tr", name: "translation", kind: "time-aligned", role: "translation", language: "en" },
  ];

  const seg = (id, tier, startMs, endMs, content, pos, machine = null) => ({ id, tier, startMs, endMs, content, pos, machine });
  const utterances = [
    seg("u01", "tier-utt", 620, 2480, "nikani ŋai toɾuma kesi", "a0"),
    seg("u02", "tier-utt", 2900, 4100, "kesi mopa", "a1"),
    seg("u03", "tier-utt", 4500, 5600, "liwa", "a2"),
    seg("u04", "tier-utt", 6000, 7300, "senu paki", "a3", { model: "wav2vec2-espeak", confidence: 0.81 }),
    seg("u05", "tier-utt", 7800, 8900, "ɾata", "a4", { model: "wav2vec2-espeak", confidence: 0.37 }),
    seg("u06", "tier-utt", 9400, 10700, "wuno tekal", "a5"),
    seg("u07", "tier-utt", 11200, 12600, "tekal ŋai", "a6"),
    seg("u08", "tier-utt", 13000, 14200, "", "a7"),
    seg("u09", "tier-utt", 14700, 16100, "", "a8"),
    seg("u10", "tier-utt", 16500, 18000, "nikani tekal", "a9"),
  ];
  const translations = [
    seg("r01", "tier-tr", 620, 2480, "I count: one, two, three.", "b0"),
    seg("r02", "tier-tr", 2900, 4100, "Three, four.", "b1"),
    seg("r03", "tier-tr", 4500, 5600, "Five.", "b2"),
    seg("r04", "tier-tr", 6000, 7300, "Six, seven.", "b3"),
    seg("r06", "tier-tr", 9400, 10700, "Nine, ten.", "b4"),
  ];
  const countingText = { id: "t-count", title: "Counting to ten", tiers, segments: [...utterances, ...translations] };

  const otherText = (t) => {
    const segments = Array.from({ length: t.segments }, (_, i) =>
      seg(`${t.id}-s${i}`, "tier-utt", 1000 + i * 2400, 3000 + i * 2400 - ((i * 337 + (i % 4) * 280) % 900), ["mopa tela wiripa", "kanu sele", "ŋai toɾu tela", "wiripa ni-kani"][i % 4], `c${i}`),
    );
    if (t.id !== "t-market") return { id: t.id, title: t.title, tiers: [tiers[0]], segments };
    // The market conversation was transcribed twice, for the Agreement panel:
    // the second hearing moves some boundaries, hears two words otherwise,
    // misses one turn and adds a short one.
    const second = { id: "tier-second", name: "second hearing", kind: "time-aligned", role: "transcription", language: "x-tok" };
    const heard = { 1: "kanu sela", 4: "ŋai toɾo tela" };
    const again = segments
      .filter((_, i) => i !== 5)
      .map((x, i0) => {
        const i = i0 >= 5 ? i0 + 1 : i0;
        return seg(`${t.id}-b${i}`, "tier-second", x.startMs + ((i * 53) % 90) - 30, x.endMs + ((i * 37) % 70) - 20, heard[i] ?? x.content, `d${i}`);
      });
    again.push(seg(`${t.id}-b-extra`, "tier-second", 17_600, 17_950, "ee", "d9"));
    return { id: t.id, title: t.title, tiers: [tiers[0], second], segments: [...segments, ...again] };
  };

  // ------------------------------------------------------------ recordings

  const SOURCES = [
    { id: "src-count", label: "Counting session, 3 Nov", kind: "audio", access: null, hasMedia: true, details: [["date", "2026-11-03"], ["equipment", "Zoom H5, lapel mic"], ["place", "Kelani village"]] },
    { id: "src-fox", label: "Fox story (phone memo)", kind: "audio", access: "restricted", hasMedia: true, details: [] },
    { id: "src-notes", label: "Notebook scan, p. 12", kind: "image", access: null, hasMedia: false, details: [] },
  ];
  const SAMPLE_RATE = 16000;
  const DURATION_MS = 20000;
  const FRAMES = (SAMPLE_RATE * DURATION_MS) / 1000;

  // Deterministic noise, so every run draws the same waveform.
  const noise = (n) => {
    const x = Math.sin(n * 12.9898 + 78.233) * 43758.5453;
    return x - Math.floor(x);
  };

  /** Speech-like envelope: syllable bursts inside each utterance, room tone between. */
  function envelope(tMs) {
    let amp = 0.012 + 0.006 * noise(Math.floor(tMs / 7));
    for (const u of utterances) {
      if (tMs < u.startMs - 40 || tMs > u.endMs + 60) continue;
      const local = tMs - u.startMs;
      const syllable = 190 + 40 * noise(u.startMs);
      const phase = (local % syllable) / syllable;
      const burst = Math.pow(Math.sin(Math.PI * Math.min(Math.max(phase, 0), 1)), 1.6);
      const which = Math.floor(local / syllable);
      const loud = 0.35 + 0.45 * noise(which * 31 + u.startMs);
      const edge = Math.min(1, Math.max(0, (tMs - u.startMs + 40) / 80), Math.max(0, (u.endMs + 60 - tMs) / 120));
      amp = Math.max(amp, burst * loud * edge);
    }
    return amp;
  }

  function peaksQuery({ startFrame, endFrame, pixels }) {
    const end = Math.min(endFrame, FRAMES);
    const bucketFrames = Math.max(1, Math.ceil((endFrame - startFrame) / pixels));
    const mins = [];
    const maxs = [];
    for (let f = startFrame; f < end; f += bucketFrames) {
      const tMs = ((f + bucketFrames / 2) / SAMPLE_RATE) * 1000;
      const a = envelope(tMs) * 32767;
      const jitter = 0.75 + 0.25 * noise(f);
      maxs.push(Math.round(a * jitter));
      mins.push(-Math.round(a * (0.7 + 0.3 * noise(f + 1))));
    }
    return { bucketFrames, firstFrame: startFrame, mins, maxs };
  }

  function acousticAnalysis({ chunks }) {
    return chunks.map((index) => {
      const startMs = index * 10000;
      const stepMs = 10;
      const n = Math.max(0, Math.min(1000, Math.floor((DURATION_MS - startMs) / stepMs)));
      const pitch = [], intensity = [], f1 = [], f2 = [], f3 = [];
      for (let i = 0; i < n; i++) {
        const t = startMs + i * stepMs;
        const a = envelope(t);
        const voiced = a > 0.12;
        pitch.push(voiced ? 180 + 40 * Math.sin(t / 300) + 10 * noise(i) : null);
        intensity.push(40 + 40 * Math.min(1, a * 1.4));
        f1.push(voiced ? 550 + 150 * Math.sin(t / 170) : null);
        f2.push(voiced ? 1500 + 350 * Math.sin(t / 230) : null);
        f3.push(voiced ? 2600 + 150 * Math.sin(t / 410) : null);
      }
      return { index, startMs, stepMs, pitch, intensity, f1, f2, f3 };
    });
  }

  // A synthetic spectrogram in the Rust layout (src-tauri/src/spectrogram.rs):
  // 2500 columns of 100 rows, half-dB steps above -120 dB, with formant bands
  // where the fixture's voice is and a noisy hiss elsewhere.
  function spectrogram({ index }) {
    const rows = 100, cols = 2500, out = new Uint8Array(rows * cols);
    for (let c = 0; c < cols; c++) {
      const t = index * 10000 + (c + 0.5) * 4;
      const a = t < DURATION_MS ? envelope(t) : 0;
      const voiced = a > 0.12;
      const formants = [550 + 150 * Math.sin(t / 170), 1500 + 350 * Math.sin(t / 230), 2600 + 150 * Math.sin(t / 410)];
      for (let r = 0; r < rows; r++) {
        const hz = (r + 0.5) * 50;
        let db = -95 + 6 * noise(c * 131 + r);
        if (voiced) for (const [k, f] of formants.entries()) db = Math.max(db, -30 - 6 * k - ((hz - f) / 120) ** 2 * 6);
        else if (a > 0.05 && hz > 3000) db = Math.max(db, -55 + 4 * noise(c + r * 7));
        out[c * rows + r] = Math.max(0, Math.min(255, Math.round((db + 120) * 2)));
      }
    }
    return out.buffer;
  }

  // ------------------------------------------------------------ glossing

  const morph = (id, surface, type, gloss, senseLabel = null, es = null) => ({
    id,
    surface,
    type,
    gloss,
    glossesByLang: es === null ? {} : { es },
    sense: senseLabel === null ? null : `sense-${id}`,
    senseLabel,
  });
  const glossView = (segmentId) => {
    if (segmentId !== "u01") {
      const s = utterances.find((u) => u.id === segmentId) ?? utterances[1];
      const forms = s.content.split(" ").filter((w) => w !== "");
      return {
        segmentId,
        // A word starting ni- gets a guess read from known morphs (never analysed in this spelling).
        words: forms.map((form, i) => ({
          id: `${segmentId}-w${i}`,
          form,
          morphs: [],
          suggestions: form.startsWith("ni") && form.length > 3
            ? [{ morphs: [{ surface: "ni", type: "prefix", gloss: "1SG", glossesByLang: {}, sense: null }, { surface: form.slice(2), type: "root", gloss: "count", glossesByLang: {}, sense: "sense-kani" }], count: 0, guessed: true }]
            : [],
          othersUnanalysed: 2,
          findings: [],
        })),
        readsAs: forms,
        hidden: [],
        alternatives: [],
        segmentNotes: [],
      };
    }
    return {
      segmentId,
      words: [
        {
          id: "w1",
          form: "nikani",
          morphs: [morph("m1", "ni", "prefix", "1SG", null), morph("m2", "kani", "root", "count", "kani – count", "contar")],
          suggestions: [],
          othersUnanalysed: 0,
          findings: [],
        },
        { id: "w2", form: "ŋai", morphs: [morph("m3", "ŋai", "root", "one", "ŋai – one", "uno")], suggestions: [], othersUnanalysed: 0, findings: [] },
        {
          id: "w3",
          form: "toɾuma",
          morphs: [morph("m4", "toɾu", "root", "two", "toɾu – two", "dos"), morph("m5", "ma", "clitic", "FOC", null)],
          suggestions: [],
          othersUnanalysed: 0,
          findings: ["The morphs (toɾu=ma) do not spell the word (toɾuma) the same way."],
        },
        {
          id: "w4",
          form: "kesi",
          morphs: [],
          suggestions: [{ morphs: [{ surface: "kesi", type: "root", gloss: "three", glossesByLang: { es: "tres" }, sense: "sense-kesi", }], count: 3 }],
          othersUnanalysed: 4,
          findings: [],
        },
      ],
      readsAs: ["nikani", "ŋai", "toɾuma", "kesi"],
      hidden: [],
      alternatives: [],
      segmentNotes: segmentId === "u01" ? [{ id: "ann_1", body: "Speaker counts on her fingers here; check «ŋai» with her." }] : [],
    };
  };

  // ------------------------------------------------------------ lexicon

  // Sentences a tagger tagged, and those a person has checked since (the Sentence panel).
  const TAGGED = ["u01", "u02", "u03", "u04"];
  const checkedTags = new Set();
  const checkStatus = () => ({ analysed: TAGGED.length, unchecked: TAGGED.filter((id) => !checkedTags.has(id)), sentences: utterances.map((u) => u.id) });

  let paradigmTemplatesFixture = [
  {
    id: "noun-class",
    label: "Noun class × number",
    rows: { feature: "noun-class", label: "Noun class", values: [{ value: "1", label: "1" }, { value: "3", label: "3" }, { value: "7", label: "7" }] },
    cols: { feature: "number", label: "Number", values: [{ value: "sg", label: "singular" }, { value: "pl", label: "plural" }] },
  },
];
const LEXICON = [
    ["lx-kani", "kani", null, "v", ["count", "reckon"]],
    ["lx-ni", "ni-", null, "prefix", ["1SG"]],
    ["lx-ma", "=ma", null, "clitic", ["FOC"]],
    ["lx-ngai", "ŋai", null, "num", ["one"]],
    ["lx-toru", "toɾu", null, "num", ["two"]],
    ["lx-kesi", "kesi", null, "num", ["three"]],
    ["lx-mopa", "mopa", null, "num", ["four"]],
    ["lx-liwa", "liwa", null, "num", ["five", "hand"]],
    ["lx-senu", "senu", null, "num", ["six"]],
    ["lx-tekal", "tekal", null, "num", ["ten"]],
    ["lx-tela1", "tela", 1, "n", ["river"]],
    ["lx-tela2", "tela", 2, "v", ["wade across"]],
    ["lx-wiripa", "wiripa", null, "n", ["fox"]],
    ["lx-toruma", "toruma", null, "", [""]],
    // Typed twice: Checks' "Entries that may be one word" pairs it with liwa.
    ["lx-lima", "lima", null, "n", ["hand"]],
  ].map(([id, lemma, homograph, posOfSpeech, glosses]) => ({ id, lemma, homograph, posOfSpeech, glosses, used: !["lx-wiripa", "lx-toruma", "lx-tela2", "lx-lima"].includes(id) }));

  const DOMAINS = [
    { id: "dom-8", code: "8", label: "States", description: "Use this domain for general words referring to states and conditions.", depth: 0 },
    { id: "dom-8.1", code: "8.1", label: "Quantity", description: "Use this domain for words referring to the quantity or amount of something.", depth: 1 },
    { id: "dom-8.1.1", code: "8.1.1", label: "Number", description: "Use this domain for words referring to numbers.", depth: 2, words: 2 },
    { id: "dom-8.1.1.1", code: "8.1.1.1", label: "Cardinal numbers", description: "Use this domain for the cardinal numbers.", depth: 3, words: 7 },
    { id: "dom-8.1.1.2", code: "8.1.1.2", label: "Ordinal numbers", description: "Use this domain for the ordinal numbers.", depth: 3 },
    { id: "dom-1.3", code: "1.3", label: "Water", description: "Use this domain for general words referring to water.", depth: 1 },
    { id: "dom-1.6.1.1", code: "1.6.1.1", label: "Mammal", description: "Use this domain for general words referring to mammals.", depth: 3 },
  ];
  const domainRef = (id) => {
    const d = DOMAINS.find((x) => x.id === id);
    return { id: d.id, code: d.code, label: d.label };
  };

  function lexiconEntry({ id }) {
    const row = LEXICON.find((r) => r.id === id) ?? LEXICON[0];
    const sameLemma = LEXICON.filter((r) => r.lemma === row.lemma && r.id !== row.id).map((r) => [r.id, r.homograph]);
    if (row.id === "lx-kani") {
      return {
        id: row.id,
        lemma: "kani",
        citation: "kani",
        posOfSpeech: "v",
        homograph: null,
        notes: "Perhaps related to kan 'hand'; ask Mosa about the long vowel.",
        sameLemma,
        allomorphs: [["frm-kan", "kan"], ["frm-hani", "hani"]],
        pronunciations: [
          { id: "prn-1", form: "ˈka.ni", audio: { sourceId: "src-1", label: "elicitation 1", startMs: 1200, endMs: 1850 } },
          { id: "prn-2", form: "ˈkaːni", audio: null },
        ],
        relations: [
          { id: "rel-1", relation: "synonym", hereSense: null, otherId: "lx-tela-1", otherLemma: "tela", otherHomograph: 1, otherSense: null },
          { id: "rel-2", relation: "compare", hereSense: 2, otherId: "lx-mopa", otherLemma: "mopa", otherHomograph: null, otherSense: null },
        ],
        senses: [
          {
            id: "sense-kani-1",
            gloss: "count",
            glossesByLang: { es: "contar" },
            definition: "To say numbers in order, aloud or under one's breath, or to find how many things there are.",
            usage: "With ni- when the speaker counts aloud: nikani ŋai toɾu.",
            notes: "",
            domains: [domainRef("dom-8.1.1"), domainRef("dom-8.1.1.1")],
            occurrences: 3,
            examples: [
              {
                id: "exm-1",
                sentence: "nikani ŋai toɾu",
                translation: "I count three",
                reference: "",
                from: { textId: "text-1", textTitle: "Counting to five", tierId: "tier-utt", segmentId: "seg-3", startMs: 3000, endMs: 4200 },
              },
              { id: "exm-2", sentence: "kani ta haru", translation: "count the stones", reference: "Notebook 2, p. 14", from: null },
            ],
            picture: { blob: "blake3:" + "ab".repeat(32), name: "counting.svg", path: "/project/blobs/counting.svg" },
          },
          { id: "sense-kani-2", gloss: "reckon", glossesByLang: {}, definition: "To think something likely.", usage: "", notes: "Only heard from younger speakers.", domains: [], occurrences: 1, examples: [] },
        ],
      };
    }
    return {
      id: row.id,
      lemma: row.lemma,
      citation: row.lemma,
      posOfSpeech: row.posOfSpeech,
      homograph: row.homograph,
      notes: "",
      sameLemma,
      allomorphs: [],
      pronunciations: [],
      relations: [],
      senses: row.glosses.map((g, i) => ({ id: `sense-${row.id}-${i}`, gloss: g, glossesByLang: {}, definition: "", usage: "", notes: "", domains: row.posOfSpeech === "num" ? [domainRef("dom-8.1.1.1")] : [], occurrences: i === 0 ? 2 : 0, examples: [] })),
    };
  }

  // ------------------------------------------------------------ elicitation

  const SWADESH = "I you we this that who what not all many one two big long small woman man person fish bird dog louse tree seed leaf root bark skin flesh blood bone grease egg horn tail feather hair head ear eye nose mouth tooth tongue claw foot knee hand belly neck breast heart liver drink eat bite see hear know sleep die kill swim fly walk come lie sit stand give say sun moon star water rain stone sand earth cloud smoke fire ash burn path mountain red green yellow white black night hot cold full new good round dry name".split(" ");
  const ANSWERS = {
    one: ["ŋai", "lx-ngai"],
    two: ["toɾu", "lx-toru"],
    I: ["ni", "lx-ni"],
    water: ["tela", "lx-tela1"],
    hand: ["liwa", "lx-liwa"],
    fish: ["sele", "lx-sele"],
    dog: ["kanu", "lx-kanu"],
    sun: ["mara", "lx-mara"],
    moon: ["pelu", "lx-pelu"],
    eye: ["miti", "lx-miti"],
    head: ["kopu", "lx-kopu"],
    tree: ["wala", "lx-wala"],
    stone: ["tuku", "lx-tuku"],
    fire: ["ahi", "lx-ahi"],
    eat: ["nam", "lx-nam"],
    drink: ["unu", "lx-unu"],
    see: ["kite", "lx-kite"],
    come: ["mai", "lx-mai"],
    give: ["ho", "lx-ho"],
    red: ["mea", "lx-mea"],
    night: ["poi", "lx-poi"],
    name: ["ina", "lx-ina"],
    bird: ["manu", "lx-manu"],
  };
  const SKIPPED = { louse: "No word offered; speaker unsure.", grease: "Asked twice, two different answers. Come back to it." };
  const concepts = SWADESH.map((gloss, i) => {
    const a = ANSWERS[gloss];
    return {
      id: `con-${i + 1}`,
      number: i + 1,
      gloss,
      concepticonGloss: gloss.toUpperCase(),
      skipped: gloss in SKIPPED,
      skipNote: SKIPPED[gloss] ?? "",
      answers: a === undefined ? [] : [{ senseId: `sense-${a[1]}`, lexemeId: a[1], lemma: a[0], homograph: a[1] === "lx-tela1" ? 1 : null, gloss, sound: a[1] === "lx-tela1" || a[1] === "lx-kani" ? { sourceId: "src-count", startMs: 620, endMs: 1300 } : null }],
    };
  });
  const promptView = {
    list: { id: "valency-prompts", name: "Valency prompts", total: 4, answered: 1, textId: "t-count" },
    prompts: [
      { id: "1", number: 1, prompt: "The dog sleeps.", answers: [{ segmentId: "u02", tierId: "tier-utt", content: "wiripa nani" }] },
      { id: "2", number: 2, prompt: "The dog bit the man.", answers: [] },
      { id: "3", number: 3, prompt: "The man gave the dog a bone.", answers: [] },
      { id: "4", number: 4, prompt: "The bone was given to the dog.", answers: [] },
    ],
  };
  const listSummary = {
    id: "cl-swadesh-100",
    name: "Swadesh 1955 (100)",
    citation: "Swadesh, Morris. 1955. Towards greater accuracy in lexicostatistic dating. Via Concepticon.",
    total: concepts.length,
    answered: concepts.filter((c) => c.answers.length > 0).length,
    skipped: concepts.filter((c) => c.skipped).length,
  };

  // ------------------------------------------------------------ kits

  const tongues = window.__WUGS_SCREENS__?.kit ?? {
    id: "org.wugs.tongues",
    name: "Tongues",
    subtitle: "Fieldwork",
    version: "0.0.1",
    description: "Language documentation: record, slice, transcribe, gloss, build the lexicon.",
    tools: ["timeline", "media", "texts", "transcribe", "gloss", "lexicon", "elicit", "search", "assist", "people"],
    layout: [
      { panel: "timeline" },
      { panel: "media", place: "left", of: "timeline", width: 260 },
      { panel: "texts", place: "tab", of: "media" },
      { panel: "lexicon", place: "right", of: "timeline", width: 320 },
      { panel: "elicit", place: "tab", of: "lexicon" },
      { panel: "search", place: "tab", of: "lexicon" },
      { panel: "assist", place: "tab", of: "lexicon" },
      { panel: "people", place: "tab", of: "lexicon" },
      { panel: "devices", place: "tab", of: "lexicon" },
      { panel: "transcribe", place: "below", of: "timeline", height: 260 },
      { panel: "gloss", place: "right", of: "transcribe" },
      { panel: "conflicts", place: "right", of: "gloss", width: 320 },
    ],
    active: ["texts", "timeline"],
  };
  const sprint = {
    id: "word-list-sprint",
    name: "Word-list sprint",
    subtitle: "Elicitation",
    version: "0.2.0",
    description: "Just the word lists, the lexicon and who said what, for a quick elicitation session.",
    tools: ["elicit", "lexicon", "search", "people"],
    layout: [
      { panel: "elicit" },
      { panel: "lexicon", place: "right", of: "elicit", width: 420 },
      { panel: "search", place: "tab", of: "lexicon" },
      { panel: "people", place: "below", of: "elicit", height: 240 },
    ],
    active: ["elicit"],
    wug: { body: "#7a6cd8", accessories: ["crown", "scroll", "glasses"], colours: { "crown-shape": "#ffd135" } },
  };
  const kitsView = {
    active: tongues.id,
    kits: [
      { kit: tongues, bundled: true },
      ...(window.__WUGS_SCREENS__.scrolls ? [{ kit: window.__WUGS_SCREENS__.scrolls, bundled: true }] : []),
      { kit: sprint, bundled: false },
    ],
    broken: [{ file: "old-transcription.toml", problems: ["line 14, column 1: unknown section [scripts]; kits cannot carry scripts", "line 3, column 6: id must be letters, digits, dots and dashes"] }],
  };
  const kitCode = ({ id }) =>
    id === sprint.id
      ? `[kit]\nid = "word-list-sprint"\nname = "Word-list sprint"\nsubtitle = "Elicitation"\nversion = "0.2.0"\ndescription = "Just the word lists, the lexicon and who said what."\n\n[tools]\nenabled = ["elicit", "lexicon", "search", "people"]\n\n[layout]\npanels = [\n  { panel = "elicit" },\n  { panel = "lexicon", place = "right", of = "elicit", width = 420 },\n  { panel = "search", place = "tab", of = "lexicon" },\n  { panel = "people", place = "below", of = "elicit", height = 240 },\n]\nactive = ["elicit"]\n\n[wug]\nbody = "#7a6cd8"\naccessories = ["crown", "scroll", "glasses"]\ncolours = { crown-shape = "#ffd135" }\n`
      : `[kit]\nid = "org.wugs.tongues"\nname = "Tongues"\nsubtitle = "Fieldwork"\n`;

  // ------------------------------------------------------------ people

  const coverable = [
    { id: PROJECT, kind: "project", label: "The whole project" },
    { id: "src-count", kind: "recording", label: "Counting session, 3 Nov" },
    { id: "src-fox", kind: "recording", label: "Fox story (phone memo)" },
    { id: "t-count", kind: "text", label: "Counting to ten" },
  ];
  const sessionsLog = {
    people: [["p-ama", "Ama Torvesi"], ["p-demo", "D. Mo"], ["p-kelo", "Kelo Marandi"]],
    sessions: [
      {
        id: "ses-2",
        date: "2026-11-20",
        place: "Kelani village, Ama's house",
        method: "recording",
        notes: "Recorded the fox story twice; the second telling is cleaner.\nAsk Ama next time: is «toɾu» also 'well' for a spring?",
        people: [["p-ama", "Ama Torvesi"], ["p-demo", "D. Mo"]],
        texts: [["t-fox", "The fox and the well"]],
      },
      { id: "ses-1", date: "2026-11-18", place: "Kelani school", method: "elicitation", notes: "Numbers one to ten, then body parts.", people: [["p-kelo", "Kelo Marandi"], ["p-demo", "D. Mo"]], texts: [["t-count", "Counting to ten"]] },
    ],
  };

  const peopleView = {
    coverable,
    people: [
      {
        id: "p-ama",
        name: "Ama Torvesi",
        roles: ["speaker"],
        details: [["born", "1952"], ["from", "Kelani village"], ["languages", "Kanu (first), Tok Pisin, English"]],
        deleted: false,
        consents: [
          {
            id: "c-ama-1",
            covers: PROJECT,
            terms: [["archive-ok", true], ["web-ok", false], ["community-only", null], ["AI-ok", true]],
            date: "2026-11-03",
            evidenceName: "consent-form-signed.pdf",
            hasEvidence: true,
          },
        ],
      },
      {
        id: "p-kelo",
        name: "Kelo Marandi",
        roles: ["consultant", "translator"],
        details: [],
        deleted: false,
        consents: [
          { id: "c-kelo-1", covers: "src-fox", terms: [["archive-ok", true], ["web-ok", null], ["community-only", true], ["AI-ok", false]], date: "2026-11-05", evidenceName: null, hasEvidence: false },
        ],
      },
      { id: "p-demo", name: "D. Mo", roles: ["researcher"], details: [], deleted: false, consents: [] },
    ],
  };

  // ------------------------------------------------------------ devices, assist, capabilities

  const devicesView = {
    paired: true,
    copyWarning: null,
    devices: [
      { key: "ed25519:7f3a9c", fingerprint: "K7QF 2M9X 4TRA", label: "Field laptop", standing: "member", guest: false, contested: false, thisDevice: true },
      { key: "ed25519:b21e44", fingerprint: "P3WZ 8NHC 1QLE", label: "Village tablet", standing: "member", guest: false, contested: false, thisDevice: false },
      { key: "ed25519:0c88d1", fingerprint: "HX4D 6YVB 9MSU", label: "Supervisor (read-only)", standing: "guest", guest: true, contested: false, thisDevice: false },
      { key: "ed25519:e5a017", fingerprint: "R2KT 5JGF 7CWN", label: "Lost phone", standing: "revoked", guest: false, contested: false, thisDevice: false },
    ],
  };

  const assistModels = [
    {
      id: "wav2vec2-espeak",
      name: "wav2vec 2.0 phones (espeak IPA)",
      about: "A phone recognizer exported to ONNX. Writes the phones it hears in any language as IPA. Expect to correct rarer segments.",
      license: "Apache-2.0",
      attribution: "Model © Meta Platforms (Xu, Baevski & Auli 2022); ONNX export by sadda-speech",
      homepage: "https://huggingface.co/sadda-speech/wav2vec2-espeak-ctc",
      capability: "phone-transcription",
      runsHere: true,
      approxMb: 640,
      installed: { files: [{ name: "model.onnx", bytes: 1262311424, sha256: "4be1f0c7a9d2e83b5c6a1f7e9d0b2c4a6e8f1a3b5c7d9e0f2a4b6c8d0e1f3a5b", published: false }, { name: "vocab.json", bytes: 5120, sha256: "9a0c1e3f5b7d9f1a3c5e7a9b1d3f5a7c9e1b3d5f7a9c1e3b5d7f9a1c3e5b7d9f", published: false }] },
      download: null,
    },
    {
      id: "zipa-cr-small",
      name: "ZIPA-CR small (IPA phones)",
      about: "A 64M-parameter Zipformer CTC phone recognizer in its INT8 ONNX release. A third of the memory of wav2vec2.",
      license: "CC-BY-4.0",
      attribution: "ZIPA by Zhu, Samir, Chodroff and Mortensen (2025), CC BY 4.0",
      homepage: "https://huggingface.co/anyspeech/zipa-small-crctc-500k",
      capability: "phone-transcription",
      runsHere: true,
      approxMb: 75,
      installed: null,
      download: null,
    },
  ];

  const capabilities = [
    {
      id: "phone-transcription",
      title: "Phone transcription",
      machine: "automatic",
      pin: null,
      effective: "wav2vec2-espeak",
      problem: null,
      options: [
        { id: "wav2vec2-espeak", name: "wav2vec 2.0 phones (espeak IPA)", kind: "model", ready: true, note: "Downloaded", score: 0.21 },
        { id: "zipa-cr-small", name: "ZIPA-CR small (IPA phones)", kind: "model", ready: false, note: "Not downloaded", score: 0.228 },
      ],
    },
    {
      id: "acoustic-analysis",
      title: "Acoustic analysis",
      machine: "automatic",
      pin: null,
      effective: "praat",
      problem: null,
      options: [{ id: "praat", name: "Praat", kind: "program", ready: true, note: "Praat 6.4.27", score: null }],
    },
    {
      id: "segmentation",
      title: "Segmentation",
      machine: "automatic",
      pin: null,
      effective: "voice-activity",
      problem: null,
      options: [{ id: "voice-activity", name: "Voice activity", kind: "built-in", ready: true, note: "Built in", score: null }],
    },
    {
      id: "morphosyntax",
      title: "Tagging and parsing",
      machine: "automatic",
      pin: null,
      effective: null,
      problem: "UDPipe needs a model for the language (a .udpipe file from the UDPipe models page); choose one. Or set up spaCy or Stanza, or choose the UDPipe 2 web service.",
      options: [
        { id: "udpipe", name: "UDPipe on this computer", kind: "program", ready: false, note: "UDPipe needs a model for the language (a .udpipe file from the UDPipe models page); choose one.", score: null },
        { id: "spacy", name: "spaCy on this computer", kind: "program", ready: false, note: "spaCy needs a pipeline for the language (such as en_core_web_sm); type its name.", score: null },
        { id: "stanza", name: "Stanza on this computer", kind: "program", ready: false, note: "Stanza needs the language's code (such as en, de or grc); type it.", score: null },
        { id: "udpipe-lindat", name: "UDPipe 2 (LINDAT web service)", kind: "web service", ready: true, note: "Sends the text to ÚFAL in Prague, who may keep it.", score: null },
      ],
    },
  ];

  // ------------------------------------------------------------ conflicts

  const conflicts = [
    {
      target: "sense-lx-tela1-0",
      field: "gloss",
      title: "Gloss of «tela»",
      place: "Lexicon",
      textId: null,
      segmentId: null,
      editable: true,
      heads: ["h-tela-a", "h-tela-b"],
      versions: [
        { raw: '"river"', label: "river", atMs: DAY - 3 * 3600_000, mine: true, winner: true },
        { raw: '"stream"', label: "stream", atMs: DAY - 2 * 3600_000, mine: false, winner: false },
      ],
    },
    {
      target: "u06",
      field: "content",
      title: "Transcription of a segment",
      place: "Text «Counting to ten»",
      textId: "t-count",
      segmentId: "u06",
      editable: true,
      heads: ["h-u06-a", "h-u06-b"],
      versions: [
        { raw: '"wuno tekal"', label: "wuno tekal", atMs: DAY - 26 * 3600_000, mine: false, winner: true },
        { raw: '"wunu tekal"', label: "wunu tekal", atMs: DAY - 27 * 3600_000, mine: true, winner: false },
      ],
    },
  ];

  // ------------------------------------------------------------ search

  function search({ query }) {
    const q = String(query).trim();
    if (q === "") return [];
    const mark = (s) => s.split(q).join(`[${q}]`);
    const hits = utterances
      .filter((u) => u.content.includes(q))
      .map((u) => ({ target: u.id, kind: "segment", field: "content", snippet: mark(u.content), textId: "t-count", textTitle: "Counting to ten", tierId: "tier-utt", tierName: "utterance", startMs: u.startMs, endMs: u.endMs }));
    hits.push({ target: "t-fox-s3", kind: "segment", field: "content", snippet: `wiripa [${q}] tela`, textId: "t-fox", textTitle: "The fox and the well", tierId: "tier-utt", tierName: "utterance", startMs: 8200, endMs: 9400 });
    hits.push({ target: "lx-kesi", kind: "lexeme", field: "lemma", snippet: `[${q}]`, textId: null, textTitle: null, tierId: null, tierName: null, startMs: null, endMs: null });
    hits.push({ target: "sense-lx-kesi-0", kind: "sense", field: "gloss", snippet: "three (from [kesi])", textId: null, textTitle: null, tierId: null, tierName: null, startMs: null, endMs: null });
    return hits;
  }

  // ------------------------------------------------------------ the table

  const glossEdit = ({ segmentId }) => ({ view: glossView(segmentId), notes: [] });

  window.__WUGS_FIXTURES__ = {
    open_project: () => projectInfo,
    create_project: () => projectInfo,
    open_sample: () => projectInfo,
    concordance: ({ query }) => ({
      total: 3,
      hits: [
        { textId: "t-count", textTitle: "Counting to ten", segmentId: "u01", tierId: "tier-utt", startMs: 620, endMs: 2480, words: ["nikani", "ŋai", "toruma", "kesi"], at: 0, breakdown: "ni-kani", glosses: "1SG-count" },
        { textId: "t-fox", textTitle: "The fox and the well", segmentId: "f03", tierId: "tier-utt", startMs: 5100, endMs: 7900, words: ["wuno", "tekal", "nikani", "ra", "ŋai", "liwa"], at: 2, breakdown: "ni-kani", glosses: "1SG-count" },
        { textId: "t-greet", textTitle: "Greetings at the market", segmentId: "g07", tierId: "tier-utt", startMs: 12000, endMs: 13600, words: ["senu", "paki", "nikani"], at: 2, breakdown: "ni-kani", glosses: "1SG-count" },
      ],
    }),
    list_texts: () => texts,
    link_recording: () => null,
    concordance_query: () => ({
      total: 1,
      hits: [{ textId: "t-count", textTitle: "Counting to ten", segmentId: "u01", tierId: "tier-utt", startMs: 620, endMs: 2480, words: ["nikani", "ŋai", "toruma", "kesi"], at: 0, len: 1, breakdown: null, glosses: null, tags: ["VERB", "NUM", "NUM", "NUM"] }],
    }),
    // Find and replace: three invented sentences, one glossed.
    replace_preview: ({ find }) => {
      const sentences = [
        ["t1", "Counting to six (tutorial)", "wʌn tuː θɹiː", true],
        ["t1", "Counting to six (tutorial)", "fɔɹ faɪv", false],
        ["t2", "Market day", "θɹiː kʊkiːz ænd θɹiː keɪks", false],
      ];
      const changes = [];
      for (const [textId, textTitle, before, glossed] of sentences) {
        const count = find.find === "" ? 0 : before.split(find.find).length - 1;
        if (count === 0) continue;
        const pieces = before.split(find.find);
        const parts = pieces.map((keep, i) => (i < pieces.length - 1 ? { keep, removed: find.find, added: find.with } : { keep, removed: "", added: "" }));
        changes.push({ textId, textTitle, tierId: "tier-u", segmentId: `seg-${changes.length}`, before, after: pieces.join(find.with), count, glossed, parts });
      }
      return { changes, total: changes.length };
    },
    replace_apply: ({ changes }) => ({ done: changes, skipped: 0 }),
    // A synthetic speaker's vowels: five qualities, six tokens each, spread deterministically.
    acoustic_voice: () => ({
      voice: "typical",
      voices: [
        { id: "typical", name: "Typical (pitch 75–600 Hz, formants to 5500 Hz)", pitchFloor: 75, pitchCeiling: 600, formantCeiling: 5500 },
        { id: "low", name: "Low voice (pitch 50–300 Hz, formants to 5000 Hz)", pitchFloor: 50, pitchCeiling: 300, formantCeiling: 5000 },
      ],
    }),
    measure_points: () => {
      const means = { i: [300, 2300], e: [450, 2000], a: [750, 1300], o: [480, 900], u: [320, 800] };
      const points = [];
      let k = 0;
      for (const [label, [f1, f2]] of Object.entries(means)) {
        for (let j = 0; j < 6; j++) {
          const w = Math.sin(++k * 1.7);
          const v = Math.cos(k * 2.3);
          // Its pitch: a tone per vowel quality, so the Pitch contours panel draws shapes.
          const shape = { i: [1, 0], e: [-1, 0], a: [0, 0], o: [0, 1], u: [1, -1] }[label];
          const contour = Array.from({ length: 10 }, (_, t) => (k % 7 === 0 && t > 7 ? null : 150 + 30 * shape[0] * (t / 9 - 0.5) + 25 * shape[1] * Math.sin((t / 9) * Math.PI) + 4 * w));
          points.push({ segment: `v${k}`, label, startMs: k * 400, endMs: k * 400 + 180, f0: 120 + 10 * w, f1: f1 + 40 * w, f2: f2 + 120 * v + 60 * w, f3: 2700, contour });
        }
      }
      points.push({ segment: "v-silent", label: "a", startMs: 13000, endMs: 13200, f0: null, f1: null, f2: null, f3: null, contour: Array(10).fill(null) });
      return { points, untimed: 1, analysedBy: "Acoustic analysis (built in)" };
    },
    gloss_consistency: () => [
      { surface: "ma", kind: "clitic", key: "ma", total: 9, slip: true, variants: [
        { gloss: "FOC", count: 7, textId: "t-count", textTitle: "Counting to ten", tierId: "tier-utt", segmentId: "u01" },
        { gloss: "foc.", count: 2, textId: "t-fox", textTitle: "The fox and the well", tierId: "tier-utt", segmentId: "u01" },
      ] },
      { surface: "ka", kind: "suffix", key: "ka", total: 5, slip: false, variants: [
        { gloss: "PL", count: 4, textId: "t-count", textTitle: "Counting to ten", tierId: "tier-utt", segmentId: "u01" },
        { gloss: "3SG", count: 1, textId: "t-greet", textTitle: "Greetings at the market", tierId: "tier-utt", segmentId: "u01" },
      ] },
    ],
    gloss_unify: () => 1,
    // The character inventory: a Cyrillic «а» typed among Latin letters, and two apostrophes.
    typology_profile: ({ glottocode }) => {
      const f = (id, question, value, label, others) => ({ id, question, value, label, others: others.map(([v, l, count]) => ({ value: v, label: l, count })), othersCoded: others.reduce((n, o) => n + o[2], 0), codes: others.map(([v, l]) => ({ value: v, label: l })).sort((x, y) => x.value.localeCompare(y.value)) });
      return {
        glottocode,
        name: "Invented Austronesian",
        codedAs: null,
        codedName: null,
        comparedWith: "Austronesian languages",
        comparedCount: 312,
        edition: "Grambank 1.0 (CC BY 4.0): Skirgård et al. 2023, Science Advances",
        page: `https://grambank.clld.org/languages/${glottocode}`,
        features: [
          f("GB020", "Are there definite or specific articles?", null, null, [["1", "present", 201], ["0", "absent", 98]]),
          f("GB021", "Do indefinite nominals commonly have indefinite articles?", null, null, [["0", "absent", 180], ["1", "present", 110]]),
          f("GB024", "What is the order of numeral and noun in the NP?", null, null, [["1", "Num-N", 150], ["2", "N-Num", 120], ["3", "both", 30]]),
          f("GB025", "What is the order of adnominal demonstrative and noun?", null, null, [["2", "N-Dem", 210], ["1", "Dem-N", 70], ["3", "both", 20]]),
          f("GB026", "Can adnominal property words occur discontinuously?", null, null, []),
        ],
      };
    },
    // Invented Austronesian answers placed where Austronesian languages are spoken, Madagascar to Rapa Nui.
    typology_map: ({ glottocode, feature }) => {
      const places = [[-19, 47], [-6, 106], [-8, 115], [1, 104], [14, 121], [10, 124], [7, 125], [-2, 120], [-4, 122], [2, 128], [-6, 147], [-9, 160], [-17, 168], [-18, 178], [-13, -172], [-17, -150], [21, -157], [-27, -109], [23, 121], [5, 116], [-4, 138], [-10, 124], [0, 110], [13, 144], [7, 134], [-21, 166], [-38, 175], [-44, 171]];
      const points = places.map(([lat, lon], i) => ({ glottocode: `aust${1000 + i}`, name: `Language ${i + 1}`, lat, lon, value: i % 7 === 3 ? "?" : i % 3 === 0 ? "0" : "1", me: false }));
      points.push({ glottocode, name: "Invented Austronesian", lat: 9, lon: 123, value: null, me: true });
      return {
        feature,
        question: "Are there definite or specific articles?",
        codes: [{ value: "0", label: "absent" }, { value: "1", label: "present" }],
        comparedWith: "Austronesian languages",
        points,
        unplaced: 3,
        land: window.__WUGS_SCREENS__.land,
        edition: "Grambank 1.0 (CC BY 4.0): Skirgård et al. 2023, Science Advances",
      };
    },
    orthography_get: () => ({
      profile: "# Kanu practical spelling\nGrapheme\tIPA\tFrequency\nsh\tʃ\t12\nng\tŋ\t9\nch\tt͡ʃ\t7\nj\tdʒ\t4\ny\tj\t6\n^h\tʔ\t3\ne\tɛ\t20\no\tɔ\t18\nx\tNULL\t1\n",
      check: { columns: ["IPA"], rules: 9, problems: [] },
      tracks: [
        { id: "tier-utt", name: "utterance", segments: 64 },
        { id: "tier-ipa", name: "IPA", segments: 52 },
      ],
    }),
    orthography_save: () => ({ columns: ["IPA"], rules: 9, problems: [] }),
    orthography_draft: () => "Grapheme\tIPA\tFrequency\na\ta\t30\ne\te\t20\n",
    orthography_preview: () => ({
      check: { columns: ["IPA"], rules: 9, problems: [] },
      segments: 64,
      changes: [
        { before: "Kesi mopa", after: "kɛsi mɔpa" },
        { before: "shanga yeho", after: "ʃaŋa jɛhɔ" },
        { before: "Hachi jolo", after: "ʔat͡ʃi dʒɔlɔ" },
        { before: "tela senu", after: "tɛla sɛnu" },
      ],
      unknown: [{ text: "q", count: 2, example: "qelo" }],
    }),
    orthography_convert: () => ({ track: "tier-ipa2", trackName: "IPA (converted)", texts: 3, created: 60, updated: 0, unchanged: 0, kept: 0, empty: 4, unknown: [] }),
    typology_answers: () => [
      { id: "GB020", value: "1", note: "«ang» before common nouns (Kanu notebook 3, p. 12)" },
      { id: "GB024", value: "2", note: "" },
      { id: "GB026", value: "?", note: "ask about colour words" },
    ],
    set_typology_answer: ({ id, value, note }) => ({ id, value, note }),
    sound_words: ({ tierId }) => ({
      tracks: [
        { id: "tier-utt", name: "utterance", words: 64 },
        { id: "tier-ipa", name: "IPA", words: 52 },
      ],
      words: (tierId === "tier-utt" ? ["kesi mopa", "tela senu"] : ["tʰaka", "ŋaːla", "t͡ʃimbo", "ᵐbeli", "kʼosu", "ɓaɾu", "ǃxoa", "ɛʔɛ", "jɔɣu", "Wiri", "łuwe", "ɲĩda", "fɪsu", "pata", "bata", "tata", "kata", "pati", "bati", "suka", "ʃuka", "smaka", "kunta", "palks", "spina"]).flatMap((w, i) => w.split(" ").map((word) => ({ word, count: 9 - (i % 7) })))
        // Every track (the Checks panel): a slip and a variant beside common words.
        .concat(tierId === null ? [{ word: "tʰakka", count: 1 }, { word: "fisu", count: 1 }] : []),
      running: tierId === "tier-utt" ? 64 : 52,
      omitted: 0,
    }),
    character_inventory: () => {
      const place = (context) => ({ textId: "t-count", textTitle: "Counting to ten", tierId: "tier-utt", segmentId: "u02", context });
      const row = (ch, code, name, script, kind, inTexts, inLexicon, flags = [], where = null) => ({ ch, code, name, script, kind, inTexts, inLexicon, place: where, entry: null, flags });
      return {
        mainScript: "Latin",
        notNfc: 0,
        rows: [
          row("а", "U+0430", "cyrillic small letter a", "Cyrillic", "letter", 1, 0, ["A Cyrillic letter among Latin ones: often typed by mistake for a look-alike.", "Used only once."], place("kesi mоpа")),
          row("'", "U+0027", "apostrophe", "Common", "other", 3, 0, ["Looks like «ʼ» (modifier letter apostrophe), which the project also uses."], place("ta'a tela")),
          row("ʼ", "U+02BC", "modifier letter apostrophe", "Common", "letter", 14, 6, ["Looks like «'» (apostrophe), which the project also uses."], place("taʼa senu")),
          row("a", "U+0061", "latin small letter a", "Latin", "letter", 212, 48),
          row("k", "U+006B", "latin small letter k", "Latin", "letter", 96, 21),
          row("ŋ", "U+014B", "latin small letter eng", "Latin", "letter", 40, 9),
          row("ɾ", "U+027E", "latin small letter r with fishhook", "Latin", "letter", 18, 4),
          row("\u0301", "U+0301", "combining acute accent", "Inherited", "mark", 7, 2),
        ],
      };
    },
    lexicon_example_gaps: () => ({ without: 6, fillable: 4 }),
    sound_changes: () => "",
    set_sound_changes: ({ rules }) => rules.trim(),
    word_sketch: ({ lemma }) => ({
      lemma: lemma.trim().toLowerCase(),
      count: 214,
      parsed: 1180,
      groups: [
        { relation: "obj", direction: "dependent", total: 96, items: [["bread", 31, 11.2, 31], ["fish", 22, 10.6, 22], ["rice", 14, 9.9, 14], ["meat", 9, 9.1, 9], ["fruit", 6, 8.4, 6]].map(([l, c, d, a]) => ({ lemma: l, count: c, logDice: d, after: a })) },
        { relation: "nsubj", direction: "dependent", total: 71, items: [["child", 18, 10.1, 0], ["dog", 12, 9.6, 0], ["they", 15, 8.2, 0], ["we", 11, 7.9, 0]].map(([l, c, d, a]) => ({ lemma: l, count: c, logDice: d, after: a })) },
        { relation: "advmod", direction: "dependent", total: 23, items: [["quickly", 8, 9.4, 7], ["together", 6, 9.0, 6], ["already", 5, 8.1, 1]].map(([l, c, d, a]) => ({ lemma: l, count: c, logDice: d, after: a })) },
        { relation: "xcomp", direction: "head", total: 17, items: [["want", 9, 9.8, 0], ["go", 5, 7.7, 0], ["start", 3, 7.2, 0]].map(([l, c, d, a]) => ({ lemma: l, count: c, logDice: d, after: a })) },
        { relation: "obl:tmod", direction: "dependent", total: 7, items: [["morning", 4, 8.8, 4], ["night", 3, 8.3, 3]].map(([l, c, d, a]) => ({ lemma: l, count: c, logDice: d, after: a })) },
      ],
    }),
    archive_readiness: () => ({
      total: 3,
      ready: 1,
      language: ["iso639_3"],
      texts: [
        { textId: "t-market", title: "Greetings at the market", missing: ["date", "place", "speakers", "recording", "consent", "translation"], sentences: 7, transcribed: 7, translated: 2 },
        { textId: "t-fox", title: "The fox and the well", missing: ["genre", "transcription"], sentences: 24, transcribed: 20, translated: 20 },
      ],
    }),
    consent_gaps: () => ({
      texts: [
        { textId: "t-market", title: "Greetings at the market", records: 0, no: [], communityOnly: false },
        { textId: "t-fox", title: "The fox and the well", records: 1, no: ["web-ok"], communityOnly: true },
      ],
      total: 3,
      records: 2,
    }),
    gloss_sense_drift: () => [
      { senseId: "sense-lx-tela1-0", entryId: "lx-tela1", headword: "tela", homograph: 1, senseGloss: "river", textGloss: "stream", count: 3, first: { gloss: "stream", count: 3, textId: "t-fox", textTitle: "The fox and the well", tierId: "tier-utt", segmentId: "f03" } },
    ],
    gloss_settle_drift: (a) => (a.useLexicon ? 3 : 1),
    all_notes: () => ({
      sentences: [
        { noteId: "n1", body: "Is wu- the third person or a plural? Ask again.", textId: "t-count", textTitle: "Counting to ten", tierId: "tier-utt", segmentId: "u01", content: "wu no" },
        { noteId: "n2", body: "Speaker laughed here.", textId: "t-count", textTitle: "Counting to ten", tierId: "tier-utt", segmentId: "u02", content: "fɔɹ faɪv" },
      ],
      entries: [{ entryId: "lx-tela1", headword: "tela", homograph: 1, sense: 1, gloss: "river", body: "Also a stream? Check with the fishers." }],
    }),
    tree_problems: () => ({
      sentences: [{ textId: "t-count", textTitle: "Counting to ten", tierId: "tier-utt", segmentId: "u02", content: "fɔɹ faɪv", problems: ["The tree has 2 roots: words 1 and 2."] }],
      total: 1,
      annotated: 4,
    }),
    lexicon_examples_from_texts: () => 4,
    gloss_missing: () => [
      { surface: "haru", key: "haru", total: 6, variants: [
        { gloss: "stone", count: 5, textId: "t-fox", textTitle: "The fox and the well", tierId: "tier-utt", segmentId: "u01" },
        { gloss: "hill", count: 1, textId: "t-greet", textTitle: "Greetings at the market", tierId: "tier-utt", segmentId: "u01" },
      ] },
      { surface: "pelu", key: "pelu", total: 2, variants: [{ gloss: "fox", count: 2, textId: "t-fox", textTitle: "The fox and the well", tierId: "tier-utt", segmentId: "u01" }] },
    ],
    gloss_add_missing: () => ({ entries: 2, senses: 3, linked: 8 }),
    abbreviation_meanings: ({ abbreviations }) => abbreviations.map((a) => (a === "INCL" ? "inclusive (speaker and hearer)" : null)),
    set_abbreviation_meaning: () => null,
    dispersion: () => ({
      words: 1240,
      hits: 14,
      dp: 0.31,
      dpNorm: 0.36,
      texts: [
        { textId: "t-count", title: "Counting to ten", words: 64, hits: 5, positions: [0, 9, 18, 40, 61], meta: [["genre", "procedural"]] },
        { textId: "t-fox", title: "The fox and the well", words: 812, hits: 8, positions: [12, 40, 41, 200, 333, 520, 700, 790], meta: [["genre", "narrative"]] },
        { textId: "t-greet", title: "Greetings at the market", words: 364, hits: 1, positions: [180], meta: [["genre", "conversation"]] },
        { textId: "t-empty", title: "Untranscribed session", words: 0, hits: 0, positions: [], meta: [] },
      ],
    }),
    keywords: ({ minCount }) => {
      // Computed by the same formulas as src-tauri/src/keywords.rs.
      const c = 23, d = 42, n = c + d;
      const t = (o, e) => (o > 0 && e > 0 ? o * Math.log(o / e) : 0);
      const row = (item, a, b) => {
        const k = a + b, other = n - k;
        let ll = 2 * (t(a, (c * k) / n) + t(b, (d * k) / n) + t(c - a, (c * other) / n) + t(d - b, (d * other) / n));
        if (a / c < b / d) ll = -ll;
        const fa = Math.max(a, 0.5) / c, fb = Math.max(b, 0.5) / d;
        return { item, focus: a, reference: b, logLikelihood: ll, logRatio: Math.log2(fa / fb), percentDiff: ((fa - fb) * 100) / fb, simpleMaths: ((a * 1e6) / c + 1) / ((b * 1e6) / d + 1) };
      };
      const all = [row("nikani", 9, 2), row("toruma", 4, 0), row("liwa", 1, 0), row("ŋai", 3, 16), row("kesi", 0, 11)];
      const rows = all.filter((r) => (r.logLikelihood >= 0 ? r.focus : r.reference) >= (minCount ?? 1));
      return { focusTokens: c, referenceTokens: d, types: 6, omitted: 0, critical: 9.14, rows };
    },
    collocates: ({ node }) => ({
      node: node === "" ? 0 : 23,
      tokens: 65,
      types: 5,
      rows: [
        { item: "ŋai", together: 14, left: 2, right: 12, freq: 19, logLikelihood: 31.42, mutualInformation: 2.11, logDice: 11.11, tScore: 1.2 },
        { item: "kesi", together: 6, left: 6, right: 0, freq: 11, logLikelihood: 9.87, mutualInformation: 1.53, logDice: 10.53, tScore: 1.2 },
        { item: "toruma", together: 4, left: 1, right: 3, freq: 7, logLikelihood: 5.02, mutualInformation: 1.48, logDice: 10.48, tScore: 1.2 },
        { item: "liwa", together: 1, left: 0, right: 1, freq: 1, logLikelihood: 2.6, mutualInformation: 2.9, logDice: 11.9, tScore: 1.2 },
        { item: "tekama", together: 1, left: 1, right: 0, freq: 4, logLikelihood: -0.31, mutualInformation: 0.0, logDice: 9.0, tScore: 1.2 },
      ],
    }),
    frequencies: ({ kind, size }) =>
      kind === "lemma"
        ? { tokens: 52, types: 3, words: 65, unlabelled: 13, rows: [{ item: "kani", count: 23, texts: 3 }, { item: "ŋai", count: 19, texts: 3 }, { item: "kesi", count: 10, texts: 2 }] }
        : size > 1
        ? { tokens: 58, types: 9, words: 65, once: 5, rows: [{ item: "nikani ŋai", count: 12, texts: 3 }, { item: "ŋai kesi", count: 7, texts: 2 }, { item: "kesi toruma", count: 4, texts: 1 }, { item: "toruma liwa", count: 2, texts: 1 }] }
        : ({
      tokens: kind === "gloss" ? 29 : 65,
      types: kind === "gloss" ? 3 : 6,
      rows:
        kind === "gloss"
          ? [{ item: "1SG", count: 14, texts: 3 }, { item: "count", count: 9, texts: 2 }, { item: "FOC", count: 6, texts: 2 }]
          : kind !== "word"
          ? [{ item: "one", count: 9, texts: 2 }, { item: "two", count: 7, texts: 2 }]
          : [
              { item: "nikani", count: 23, texts: 3, lexicon: "analysed", dp: 0.08 },
              { item: "ŋai", count: 19, texts: 3, lexicon: "entry", dp: 0.14 },
              { item: "kesi", count: 11, texts: 2, lexicon: "entry", dp: 0.41 },
              { item: "toruma", count: 7, texts: 1, lexicon: "none", dp: 0.86 },
              { item: "tekama", count: 4, texts: 2, lexicon: "entry", dp: 0.52 },
              { item: "liwa", count: 1, texts: 1, lexicon: "none", dp: 0.93 },
            ],
        }),
    get_text: ({ textId }) => (textId === "t-count" ? countingText : otherText(texts.find((t) => t.id === textId) ?? texts[1])),
    conflict_count: () => conflicts.length,
    list_sources: () => SOURCES,
    source_set_detail: ({ source, key, value }) => {
      const s = SOURCES.find((x) => x.id === source);
      s.details = [...s.details.filter(([k]) => k !== key), ...(value ? [[key, value]] : [])].sort();
      return SOURCES;
    },
    peaks_info: ({ sourceId }) => ({ frames: FRAMES, sampleRate: SAMPLE_RATE, channels: 1, durationMs: DURATION_MS, mediaPath: `${PROJECT_PATH}/blobs/${sourceId}.wav` }),
    peaks_query: peaksQuery,
    media_overview: () => {
      const slice = peaksQuery({ startFrame: 0, endFrame: FRAMES, pixels: 120 });
      return { state: "ready", durationMs: DURATION_MS, mins: slice.mins, maxs: slice.maxs };
    },
    speech_spans: () => utterances.map((u) => [u.startMs + 30, u.endMs - 40]),
    acoustic_analysis: acousticAnalysis,
    spectrogram,
    load_layout: () => null,
    save_layout: () => null,
    kits_list: () => kitsView,
    feedback_details: ({ page }) => ({ wugs: "0.1.0", os: "windows", arch: "x86_64", ...page }),
    feedback_retry: () => ({ sent: false, waiting: 0, problem: null, configured: true }),
    feedback_keep: () => "01JC0000000000000000000000",
    feedback_deliver: () => ({ sent: true, waiting: 0, problem: null, configured: true }),
    help_ask: () => ({
      answer: "Select the segment, move the playhead to where it should be cut, and press S (or click Split at the playhead in the Timeline toolbar). The transcription stays on the first part.",
      sources: ["Segment the recording › Split, merge, and nudge"],
    }),
    kit_code: kitCode,
    kit_set_active: ({ id }) => ((kitsView.active = id), null),
    capture_reset: () => 0,
    capture_recovered_list: () => [{ id: "take-20261108-1742", bytes: 5_760_044, durationMs: 60_000, modifiedMs: DAY - 20 * 3600_000 }],
    device_identity: () => ({ publicKey: "ed25519:7f3a9c51d0e8b2a4c6f19e3d7b5a0c2e4f6a8b1d3c5e7f9a0b2c4d6e8f1a3b5c", fingerprint: "K7QF 2M9X 4TRA", store: "keychain", fallbackReason: null }),
    devices_list: () => devicesView,
    discover_peers: () => ["192.168.4.21:47011"],
    assist_models: () => assistModels,
    capabilities: () => capabilities,
    corrections_summary: () => ({ ready: 12, recordedNo: 1, restricted: 2, unchecked: 3, unusable: 0, emptied: 1, partial: null }),
    praat_status: () => ({ path: "/usr/bin/praat", version: "6.4.27", chosen: false, problem: null }),
    praat_pending: () => false,
    morphosyntax_setup: () => ({ local: { program: "/usr/bin/udpipe", model: null }, spacy: { python: null, pipeline: null, about: null }, stanza: { python: null, pipeline: null, about: null }, webOk: false }),
    morphosyntax_choose_stanza: ({ python, language }) => ({ python, pipeline: language, about: language ? `Stanza 1.10.1, ${language} (tokenize, pos, lemma, depparse)` : null }),
    morphosyntax_choose_spacy: ({ python, pipeline }) => ({ python, pipeline, about: pipeline ? `spaCy 3.8.18, ${pipeline} 3.8.0 (tok2vec, tagger, parser, attribute_ruler, lemmatizer, ner)` : null }),
    project_language: () => ({ name: "Kanu", glottocode: "abcd1234", iso: "" }),
    set_project_language: ({ language }) => ({ name: language.name.trim(), glottocode: language.glottocode.trim().toLowerCase(), iso: language.iso.trim().toLowerCase() }),
    glottolog_find: ({ query }) =>
      query.toLowerCase().startsWith("tur")
        ? [
            { glottocode: "nucl1301", iso: "tur", name: "Turkish", dialect: false, family: "Turkic", macroarea: "Eurasia", language: "" },
            { glottocode: "turk1304", iso: "tuk", name: "Turkmen", dialect: false, family: "Turkic", macroarea: "Eurasia", language: "" },
            { glottocode: "cypr1248", iso: "", name: "Cypriot Turkish", dialect: true, family: "Turkic", macroarea: "Eurasia", language: "Turkish" },
          ]
        : [],
    metadata_values: () => [
      { key: "genre", values: [["narrative", 2], ["procedural", 1]] },
      { key: "speaker", values: [["A. Speaker (synthetic)", 3]] },
    ],
    text_metadata: () => ({
      fields: [
        { key: "genre", value: "procedural" },
        { key: "speaker", value: "A. Speaker (synthetic)" },
        { key: "date", value: "2024-06-12" },
      ],
      keys: [{ key: "genre", texts: 3 }, { key: "speaker", texts: 3 }, { key: "date", texts: 2 }, { key: "place", texts: 1 }],
    }),
    update_check: () => ({ currentVersion: "0.0.1", available: null }),
    gloss_get: ({ segmentId }) => glossView(segmentId),
    gloss_add_note: glossEdit,
    gloss_set_note: glossEdit,
    sentence_check_status: () => checkStatus(),
    sentence_mark_checked: ({ segmentIds }) => {
      for (const id of segmentIds.length === 0 ? TAGGED : segmentIds) checkedTags.add(id);
      return checkStatus();
    },
    // A tagged sentence for the Sentence panel: ni-kani ŋai toɾu-ma kesi, "I count one, two, three".
    sentence_get: ({ segmentId }) => ({
      segmentId,
      content: "nikani ŋai toɾuma kesi.",
      annotated: true,
      emptyNodes: 0,
      problems: [],
      machine: checkedTags.has(segmentId) ? null : "udpipe kanu-test.udpipe",
      words: [
        { token: "w1", form: "nikani", lemma: "kani", upos: "VERB", xpos: "v", feats: "Person=1|Number=Sing", head: 0, deprel: "root" },
        { token: "w2", form: "ŋai", lemma: "ŋai", upos: "NUM", xpos: "num", feats: "NumType=Card", head: 1, deprel: "obj" },
        { token: "w3", form: "toɾuma", lemma: "toɾu", upos: "NUM", xpos: "num", feats: "NumType=Card", head: 2, deprel: "conj" },
        { token: "w4", form: "kesi", lemma: "kesi", upos: "NUM", xpos: "num", feats: "NumType=Card", head: 2, deprel: "conj" },
        { token: "w5", form: ".", lemma: ".", upos: "PUNCT", xpos: "", feats: "", head: 1, deprel: "punct" },
      ],
    }),
    gloss_tokenize: glossEdit,
    gloss_retokenize: glossEdit,
    gloss_apply: glossEdit,
    gloss_set: glossEdit,
    gloss_pick_sense: glossEdit,
    gloss_lexicon: ({ surface }) =>
      LEXICON.filter((r) => r.lemma.replace(/[-=]/g, "") === surface).map((r) => ({ lexeme: r.id, sense: `sense-${r.id}-0`, lemma: r.lemma, gloss: r.glosses[0] })),
    lexicon_list: ({ query }) => {
      const q = String(query ?? "").toLowerCase();
      return LEXICON.filter((r) => q === "" || r.lemma.includes(q) || r.glosses.some((g) => g.includes(q)));
    },
    lexicon_entry: lexiconEntry,
    gloss_word_types: () => ({
      rows: [
        { form: "nikani", unanalysed: 4, uses: 9, suggestion: { morphs: [{ surface: "ni", type: "prefix", gloss: "1SG", glossesByLang: {}, sense: null }, { surface: "kani", type: "root", gloss: "count", glossesByLang: {}, sense: null }], count: 5, guessed: false } },
        { form: "kanira", unanalysed: 3, uses: 3, suggestion: { morphs: [{ surface: "kani", type: "root", gloss: "count", glossesByLang: {}, sense: null }, { surface: "ra", type: "suffix", gloss: "PL", glossesByLang: {}, sense: null }], count: 0, guessed: true } },
        { form: "wuno", unanalysed: 2, uses: 2, suggestion: null },
      ],
      unbrokenWords: 14,
      unbrokenSentences: 3,
      types: 40,
    }),
    gloss_break_all: () => 3,
    gloss_apply_type: () => ({ applied: 4, texts: 2 }),
    project_affixes: () => {
      const ex = { word: "ni-kani-ra", textId: "t-count", tierId: "tier-utt", segmentId: "seg-1" };
      const a = (surface, kind, gloss, slot, uses, roots, positions, categories) => ({ surface, kind, gloss, uses, roots, positions, slot, categories, example: ex });
      return {
        words: 52,
        withoutRoot: 1,
        affixes: [
          a("ni", "prefix", "1SG", -1, 14, 6, [[-1, 14]], [["verb", 14]]),
          a("ka", "prefix", "2SG", -1, 6, 4, [[-1, 6]], [["verb", 6]]),
          a("ra", "suffix", "PST", 1, 11, 5, [[1, 11]], [["verb", 11]]),
          a("si", "suffix", "PL", 1, 9, 7, [[1, 9]], [["noun", 9]]),
          a("ma", "clitic", "FOC", 2, 7, 6, [[2, 5], [1, 2]], [["verb", 4], ["noun", 3]]),
        ],
        orders: [
          { first: 2, second: 4, count: 5 },
          { first: 0, second: 2, count: 4 },
          { first: 2, second: 3, count: 2 },
          { first: 4, second: 2, count: 1 },
        ],
      };
    },
    vocabulary_growth: () => {
      // A synthetic curve that flattens: 317 running words over three texts.
      const points = [];
      for (let t = 0; t <= 317; t += 8) points.push({ tokens: t, types: Math.round(2.5 * t ** 0.6) });
      points.push({ tokens: 317, types: Math.round(2.5 * 317 ** 0.6) });
      return { points, tokens: 317, types: Math.round(2.5 * 317 ** 0.6), texts: [{ title: "Counting to ten", at: 0 }, { title: "The fox and the well", at: 65 }, { title: "Greetings at the market", at: 279 }] };
    },
    project_progress: () => ({
      texts: [
        { id: "t-count", title: "Counting to ten", recordingMs: 62_000, sentences: 10, transcribed: 10, transcribedMs: 41_000, words: 65, glossedWords: 52, morphs: 97, translated: 9, meta: [["genre", "procedural"], ["speaker", "A. Speaker"]] },
        { id: "t-fox", title: "The fox and the well", recordingMs: 754_000, sentences: 24, transcribed: 17, transcribedMs: 402_000, words: 214, glossedWords: 60, morphs: 131, translated: 11, meta: [["genre", "narrative"], ["speaker", "B. Speaker"]] },
        { id: "t-market", title: "Greetings at the market", recordingMs: null, sentences: 7, transcribed: 7, transcribedMs: 0, words: 38, glossedWords: 0, morphs: 0, translated: 7, meta: [["genre", "conversation"]] },
      ],
      recordedMs: 816_000,
      recordingsMissing: 0,
      entries: 22,
      senses: 25,
      sensesWithoutGloss: 1,
      people: 2,
      consentRecords: 2,
    }),
    lexicon_add: () => ({ entry: lexiconEntry({ id: LEXICON[0].id }), notes: [] }),
    // A paradigm answered for whatever axes the panel asks: every cell of the
    // grid, the first one filled, so the coverage line has something to count.
    lexicon_paradigm: ({ id, axes }) => {
      const cells = [];
      const walk = (i, f) => {
        if (i === axes.length) return void cells.push({ features: { ...f }, forms: [] });
        for (const v of axes[i].values) walk(i + 1, { ...f, [axes[i].feature]: v });
      };
      walk(0, {});
      if (cells.length > 0) cells[0].forms.push({ id: "form-1", form: "kani", features: cells[0].features });
      return { lexeme: id, cells, others: [] };
    },
    paradigm_templates: () => paradigmTemplatesFixture,
    save_paradigm_template: ({ template }) => {
      paradigmTemplatesFixture = [...paradigmTemplatesFixture.filter((t) => t.id !== template.id), template];
      return paradigmTemplatesFixture;
    },
    delete_paradigm_template: ({ id }) => (paradigmTemplatesFixture = paradigmTemplatesFixture.filter((t) => t.id !== id)),
    domains_list: () => DOMAINS,
    typing_shortcuts: () => ";n ŋ\nb' ɓ\n;e ɛ",
    set_typing_shortcuts: ({ table }) => table,
    concept_lists_bundled: () => [
      { source: "Swadesh-1955-100", name: "Swadesh 1955 (100)", citation: "Swadesh 1955, via Concepticon", count: 100, listId: listSummary.id },
      { source: "Swadesh-1952-200", name: "Swadesh 1952 (200)", citation: "Swadesh 1952, via Concepticon", count: 200, listId: null },
      { source: "Tadmor-2009-100", name: "Leipzig-Jakarta (100)", citation: "Tadmor 2009, via Concepticon", count: 100, listId: null },
    ],
    concept_lists: () => [listSummary],
    segment_timing: () => ({
      timed: 42,
      overlaps: 1,
      short: 1,
      long: 0,
      issues: [
        { kind: "overlap", textId: "t-count", textTitle: "Counting to ten", tierId: "tier-utt", segmentId: "u03", sentence: "kesi mopa", startMs: 4100, endMs: 5600, other: "u02", overlapMs: 180 },
        { kind: "short", textId: "t-count", textTitle: "Counting to ten", tierId: "tier-utt", segmentId: "u07", sentence: "a", startMs: 9000, endMs: 9080, other: null, overlapMs: null },
      ],
    }),
    sentence_lists: () => [{ ...promptView.list }],
    sentence_list_view: () => promptView,
    sentence_answer: ({ prompt, sentence }) => {
      const r = promptView.prompts.find((x) => x.id === prompt);
      r.answers.push({ segmentId: `se-${prompt}-${r.answers.length}`, tierId: "tier-utt", content: sentence });
      promptView.list.answered = promptView.prompts.filter((x) => x.answers.length > 0).length;
      return promptView;
    },
    cognates: () => "y\tKelani\tone\tŋaːi",
    set_cognates: ({ table }) => table,
    word_order: () => {
      const ex = (sentence) => ({ textId: "t-count", tierId: "tier-utt", segmentId: "u02", sentence });
      const o = (label, code, count, sentence = null) => ({ label, code, count, example: sentence === null ? null : ex(sentence) });
      return {
        parsed: 120,
        patterns: [
          { id: "81A", title: "Order of subject, object and verb", orders: [o("SOV", "1", 31, "nani wiripa kani"), o("SVO", "2", 3, "nani kani wiripa"), o("VSO", "3", 0), o("VOS", "4", 0), o("OVS", "5", 1, "wiripa kani nani"), o("OSV", "6", 0)] },
          { id: "83A", title: "Order of object and verb", orders: [o("OV", "1", 52, "wiripa kani"), o("VO", "2", 6, "kani wiripa")] },
          { id: "85A", title: "Order of adposition and noun phrase", orders: [o("Postpositions", "1", 18, "tela ri"), o("Prepositions", "2", 14, "ri tela")] },
          { id: "87A", title: "Order of adjective and noun", orders: [o("Adjective-Noun", "1", 2, "kesi tela"), o("Noun-Adjective", "2", 1, "tela kesi")] },
        ],
      };
    },
    phoible_compare: () => ({
      name: "Kelani",
      own: [{ id: "9001", source: "upsid", name: "Kelani", phonemes: ["p", "b", "t", "d", "k", "ɡ", "ʔ", "m", "n", "ŋ", "s", "h", "l", "ɾ", "w", "j", "t̠ʃ", "a", "e", "i", "o", "u", "ə"] }],
      comparedWith: "Austronesian languages",
      relatives: 48,
      shares: [["a", 48], ["i", 48], ["u", 47], ["k", 47], ["m", 47], ["n", 46], ["t", 45], ["p", 44], ["ŋ", 44], ["s", 43], ["l", 41], ["ʔ", 33], ["ə", 27], ["ɓ", 2], ["ǃ", 0]].map(([phoneme, languages]) => ({ phoneme, languages })),
      edition: "PHOIBLE 2.0 (CC BY-SA 3.0)",
    }),
    phoible_map: ({ phoneme }) => {
      const places = [[-19, 47], [-6, 106], [-8, 115], [1, 104], [14, 121], [10, 124], [-2, 120], [-6, 147], [-9, 160], [-17, 178], [-13, -172], [-27, -109], [23, 121], [-38, 175]];
      const points = places.map(([lat, lon], i) => ({ glottocode: `aust${2000 + i}`, name: `Language ${i + 1}`, lat, lon, value: i % 4 === 0 ? "1" : "0", me: false }));
      points.push({ glottocode: "kela1234", name: "Kelani", lat: 9, lon: 123, value: "0", me: true });
      return { feature: phoneme, question: `Is /${phoneme}/ in the inventory?`, codes: [{ value: "0", label: `no /${phoneme}/` }, { value: "1", label: `/${phoneme}/` }], comparedWith: "Austronesian languages", points, unplaced: 1, land: window.__WUGS_SCREENS__.land, edition: "PHOIBLE 2.0 (CC BY-SA 3.0)" };
    },
    compare_read: ({ path }) => ({
      name: path.split("/").pop(),
      columns: ["DOCULECT", "CONCEPT", "FORM"],
      skipped: 1,
      forms: [
        ["Kelani", "one", "ŋaːi"], ["Kelani", "two", "tolu"], ["Kelani", "I", "nii"], ["Kelani", "water", "wai"], ["Kelani", "hand", "lima"], ["Kelani", "fish", "sela"],
        ["Moru", "one", "isa"], ["Moru", "two", "doru"], ["Moru", "water", "tela"], ["Moru", "hand", "kamay"], ["Moru", "fish", "ikan"],
      ].map(([language, concept, form]) => ({ language, concept, form })),
    }),
    concept_list_view: () => ({ summary: listSummary, concepts }),
    domain_questions: ({ domainId }) => {
      const d = DOMAINS.find((x) => x.id === domainId) ?? DOMAINS[2];
      return {
        id: d.id,
        code: d.code,
        label: d.label,
        description: d.description,
        questions: [
          { q: "(1) What words refer to numbers?", words: "number, numeral, digit" },
          { q: "(2) What words are used when counting?", words: "count, add up, tally" },
        ],
        words: [{ senseId: "sense-lx-kani-0", lexemeId: "lx-kani", lemma: "kani", homograph: null, gloss: "count" }],
      };
    },
    conflicts_list: () => conflicts,
    people_view: () => peopleView,
    field_sessions_view: () => sessionsLog,
    field_session_add: ({ date }) => {
      sessionsLog.sessions.unshift({ id: `ses-${sessionsLog.sessions.length + 1}`, date, place: "", method: "", notes: "", people: [], texts: [] });
      return sessionsLog;
    },
    field_session_set: ({ session, field, value }) => {
      const s = sessionsLog.sessions.find((x) => x.id === session);
      s[field] = String(value).trim();
      return sessionsLog;
    },
    field_session_person: ({ session, person, add }) => {
      const s = sessionsLog.sessions.find((x) => x.id === session);
      const who = sessionsLog.people.find(([id]) => id === person);
      s.people = add ? [...s.people, who] : s.people.filter(([id]) => id !== person);
      return sessionsLog;
    },
    field_session_delete: ({ session }) => {
      sessionsLog.sessions = sessionsLog.sessions.filter((x) => x.id !== session);
      return sessionsLog;
    },
    person_set_detail: ({ person, key, value }) => {
      const p = peopleView.people.find((x) => x.id === person);
      p.details = [...p.details.filter(([k]) => k !== key), ...(value ? [[key, value]] : [])].sort();
      return peopleView;
    },
    search,
  };
})();
