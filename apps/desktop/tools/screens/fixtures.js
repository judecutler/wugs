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

  const otherText = (t) => ({
    id: t.id,
    title: t.title,
    tiers: [tiers[0]],
    segments: Array.from({ length: t.segments }, (_, i) =>
      seg(`${t.id}-s${i}`, "tier-utt", 1000 + i * 2400, 3000 + i * 2400, ["mopa tela wiripa", "kanu sele", "ŋai toɾu tela", "wiripa ni-kani"][i % 4], `c${i}`),
    ),
  });

  // ------------------------------------------------------------ recordings

  const SOURCES = [
    { id: "src-count", label: "Counting session, 3 Nov", kind: "audio", access: null, hasMedia: true },
    { id: "src-fox", label: "Fox story (phone memo)", kind: "audio", access: "restricted", hasMedia: true },
    { id: "src-notes", label: "Notebook scan, p. 12", kind: "image", access: null, hasMedia: false },
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
      return {
        segmentId,
        words: s.content.split(" ").filter((w) => w !== "").map((form, i) => ({ id: `${segmentId}-w${i}`, form, morphs: [], suggestions: [], othersUnanalysed: 2, findings: [] })),
        hidden: [],
        alternatives: [],
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
      hidden: [],
      alternatives: [],
    };
  };

  // ------------------------------------------------------------ lexicon

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
  ].map(([id, lemma, homograph, posOfSpeech, glosses]) => ({ id, lemma, homograph, posOfSpeech, glosses }));

  const DOMAINS = [
    { id: "dom-8", code: "8", label: "States", description: "Use this domain for general words referring to states and conditions.", depth: 0 },
    { id: "dom-8.1", code: "8.1", label: "Quantity", description: "Use this domain for words referring to the quantity or amount of something.", depth: 1 },
    { id: "dom-8.1.1", code: "8.1.1", label: "Number", description: "Use this domain for words referring to numbers.", depth: 2 },
    { id: "dom-8.1.1.1", code: "8.1.1.1", label: "Cardinal numbers", description: "Use this domain for the cardinal numbers.", depth: 3 },
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
        sameLemma,
        senses: [
          {
            id: "sense-kani-1",
            gloss: "count",
            glossesByLang: { es: "contar" },
            definition: "To say numbers in order, aloud or under one's breath, or to find how many things there are.",
            usage: "With ni- when the speaker counts aloud: nikani ŋai toɾu.",
            domains: [domainRef("dom-8.1.1"), domainRef("dom-8.1.1.1")],
            occurrences: 3,
          },
          { id: "sense-kani-2", gloss: "reckon", glossesByLang: {}, definition: "To think something likely.", usage: "", domains: [], occurrences: 1 },
        ],
      };
    }
    return {
      id: row.id,
      lemma: row.lemma,
      citation: row.lemma,
      posOfSpeech: row.posOfSpeech,
      homograph: row.homograph,
      sameLemma,
      senses: row.glosses.map((g, i) => ({ id: `sense-${row.id}-${i}`, gloss: g, glossesByLang: {}, definition: "", usage: "", domains: row.posOfSpeech === "num" ? [domainRef("dom-8.1.1.1")] : [], occurrences: i === 0 ? 2 : 0 })),
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
      answers: a === undefined ? [] : [{ senseId: `sense-${a[1]}`, lexemeId: a[1], lemma: a[0], homograph: a[1] === "lx-tela1" ? 1 : null, gloss }],
    };
  });
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
  const peopleView = {
    coverable,
    people: [
      {
        id: "p-ama",
        name: "Ama Torvesi",
        roles: ["speaker"],
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
        deleted: false,
        consents: [
          { id: "c-kelo-1", covers: "src-fox", terms: [["archive-ok", true], ["web-ok", null], ["community-only", true], ["AI-ok", false]], date: "2026-11-05", evidenceName: null, hasEvidence: false },
        ],
      },
      { id: "p-demo", name: "D. Mo", roles: ["researcher"], deleted: false, consents: [] },
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
    collocates: ({ node }) => ({
      node: node === "" ? 0 : 23,
      tokens: 65,
      types: 5,
      rows: [
        { item: "ŋai", together: 14, left: 2, right: 12, freq: 19, logLikelihood: 31.42, mutualInformation: 2.11 },
        { item: "kesi", together: 6, left: 6, right: 0, freq: 11, logLikelihood: 9.87, mutualInformation: 1.53 },
        { item: "toruma", together: 4, left: 1, right: 3, freq: 7, logLikelihood: 5.02, mutualInformation: 1.48 },
        { item: "liwa", together: 1, left: 0, right: 1, freq: 1, logLikelihood: 2.6, mutualInformation: 2.9 },
        { item: "tekama", together: 1, left: 1, right: 0, freq: 4, logLikelihood: -0.31, mutualInformation: 0.0 },
      ],
    }),
    frequencies: ({ kind }) => ({
      tokens: kind === "gloss" ? 29 : 65,
      types: kind === "gloss" ? 3 : 6,
      rows:
        kind === "gloss"
          ? [{ item: "1SG", count: 14, texts: 3 }, { item: "count", count: 9, texts: 2 }, { item: "FOC", count: 6, texts: 2 }]
          : [
              { item: "nikani", count: 23, texts: 3 },
              { item: "ŋai", count: 19, texts: 3 },
              { item: "kesi", count: 11, texts: 2 },
              { item: "toruma", count: 7, texts: 1 },
              { item: "tekama", count: 4, texts: 2 },
              { item: "liwa", count: 1, texts: 1 },
            ],
    }),
    get_text: ({ textId }) => (textId === "t-count" ? countingText : otherText(texts.find((t) => t.id === textId) ?? texts[1])),
    conflict_count: () => conflicts.length,
    list_sources: () => SOURCES,
    peaks_info: ({ sourceId }) => ({ frames: FRAMES, sampleRate: SAMPLE_RATE, channels: 1, durationMs: DURATION_MS, mediaPath: `${PROJECT_PATH}/blobs/${sourceId}.wav` }),
    peaks_query: peaksQuery,
    media_overview: () => {
      const slice = peaksQuery({ startFrame: 0, endFrame: FRAMES, pixels: 120 });
      return { state: "ready", durationMs: DURATION_MS, mins: slice.mins, maxs: slice.maxs };
    },
    speech_spans: () => utterances.map((u) => [u.startMs + 30, u.endMs - 40]),
    acoustic_analysis: acousticAnalysis,
    load_layout: () => null,
    save_layout: () => null,
    kits_list: () => kitsView,
    feedback_details: ({ page }) => ({ wugs: "0.1.0", os: "windows", arch: "x86_64", ...page }),
    feedback_retry: () => ({ sent: false, waiting: 0, problem: null, configured: true }),
    feedback_keep: () => "01JC0000000000000000000000",
    feedback_deliver: () => ({ sent: true, waiting: 0, problem: null, configured: true }),
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
    update_check: () => ({ currentVersion: "0.0.1", available: null }),
    gloss_get: ({ segmentId }) => glossView(segmentId),
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
    domains_list: () => DOMAINS,
    concept_lists_bundled: () => [
      { source: "Swadesh-1955-100", name: "Swadesh 1955 (100)", citation: "Swadesh 1955, via Concepticon", count: 100, listId: listSummary.id },
      { source: "Swadesh-1952-200", name: "Swadesh 1952 (200)", citation: "Swadesh 1952, via Concepticon", count: 200, listId: null },
      { source: "Tadmor-2009-100", name: "Leipzig-Jakarta (100)", citation: "Tadmor 2009, via Concepticon", count: 100, listId: null },
    ],
    concept_lists: () => [listSummary],
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
    search,
  };
})();
