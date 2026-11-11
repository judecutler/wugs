// In-app updates as data: the notice's state, what it says, and the
// per-machine setting. The Rust side (src-tauri/src/updater.rs) checks,
// downloads and installs; this module only decides what the person sees.
// Online: every step except "Later" needs the network.

export type Available = {
  version: string;
  currentVersion: string;
  notes: string | null;
  sizeBytes: number | null;
};

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking"; manual: boolean }
  | { kind: "available"; update: Available; showNotes: boolean; message: string | null }
  | { kind: "downloading"; update: Available; done: number; total: number | null }
  /** Downloaded and verified, waiting (for example on a recording) to install. */
  | { kind: "ready"; update: Available; message: string | null }
  | { kind: "installing"; update: Available }
  | { kind: "later"; version: string };

export type UpdateEvent =
  | { type: "check"; manual: boolean }
  | { type: "checked"; update: Available | null }
  | { type: "check-failed" }
  | { type: "notes" }
  | { type: "later" }
  | { type: "download" }
  | { type: "progress"; done: number; total: number | null }
  | { type: "downloaded"; message: string | null }
  | { type: "install" }
  /** A download failed, or an install failed or was refused, with why. */
  | { type: "failed"; message: string };

export const idle: UpdateState = { kind: "idle" };

const busy = (s: UpdateState) => s.kind === "checking" || s.kind === "downloading" || s.kind === "installing";

/** Numeric dotted versions with an optional `-pre.release` tail (semver order). */
export function compareVersions(a: string, b: string): number {
  const split = (v: string) => {
    const [core = "", pre] = v.trim().replace(/^v/, "").split("+")[0]!.split(/-(.*)/s);
    return { core: core.split(".").map((n) => Number.parseInt(n, 10) || 0), pre: pre === undefined || pre === "" ? null : pre.split(".") };
  };
  const x = split(a);
  const y = split(b);
  for (let i = 0; i < Math.max(x.core.length, y.core.length); i++) {
    const d = (x.core[i] ?? 0) - (y.core[i] ?? 0);
    if (d !== 0) return Math.sign(d);
  }
  if (x.pre === null || y.pre === null) return x.pre === y.pre ? 0 : x.pre === null ? 1 : -1;
  for (let i = 0; i < Math.max(x.pre.length, y.pre.length); i++) {
    const p = x.pre[i];
    const q = y.pre[i];
    if (p === undefined || q === undefined) return p === undefined ? -1 : 1;
    if (p === q) continue;
    const pn = /^\d+$/.test(p);
    const qn = /^\d+$/.test(q);
    if (pn && qn) return Math.sign(Number(p) - Number(q));
    if (pn !== qn) return pn ? -1 : 1;
    return p < q ? -1 : 1;
  }
  return 0;
}

export function step(s: UpdateState, e: UpdateEvent): UpdateState {
  switch (e.type) {
    case "check":
      // A manual check never interrupts a download or an offer on screen.
      return busy(s) || s.kind === "available" || s.kind === "ready" ? s : { kind: "checking", manual: e.manual };
    case "checked":
      if (s.kind !== "checking") return s;
      // The plugin only reports newer versions; this keeps a misconfigured
      // manifest from offering the running version or an older one.
      if (e.update === null || compareVersions(e.update.version, e.update.currentVersion) <= 0) return idle;
      return { kind: "available", update: e.update, showNotes: false, message: null };
    case "check-failed":
      return s.kind === "checking" ? idle : s;
    case "notes":
      return s.kind === "available" ? { ...s, showNotes: !s.showNotes } : s;
    case "later":
      return s.kind === "available" || s.kind === "ready" ? { kind: "later", version: s.update.version } : s;
    case "download":
      return s.kind === "available" ? { kind: "downloading", update: s.update, done: 0, total: s.update.sizeBytes } : s;
    case "progress":
      return s.kind === "downloading" ? { ...s, done: e.done, total: e.total ?? s.total } : s;
    case "downloaded":
      return s.kind === "downloading" ? { kind: "ready", update: s.update, message: e.message } : s;
    case "install":
      return s.kind === "ready" ? { kind: "installing", update: s.update } : s;
    case "failed":
      // The verified bytes stay on the Rust side after a refused install,
      // so a retry never downloads again.
      if (s.kind === "installing" || s.kind === "ready") return { kind: "ready", update: s.update, message: e.message };
      return s.kind === "downloading" || s.kind === "available" ? { kind: "available", update: s.update, showNotes: false, message: e.message } : s;
  }
}

/** "42 MB", "8.5 MB", "640 KB" (decimal units, as download pages show them). */
export function formatSize(bytes: number): string {
  if (bytes < 1e6) return `${Math.max(1, Math.round(bytes / 1e3))} KB`;
  const mb = bytes / 1e6;
  return `${mb < 10 ? mb.toFixed(1).replace(/\.0$/, "") : Math.round(mb)} MB`;
}

/** The notice's one line, or null when nothing shows. */
export function noticeText(s: UpdateState): string | null {
  switch (s.kind) {
    case "available": {
      const size = s.update.sizeBytes === null ? "" : ` (${formatSize(s.update.sizeBytes)})`;
      return `wugs ${s.update.version} is available${size}. You have ${s.update.currentVersion}.`;
    }
    case "downloading": {
      const of = s.total === null ? "" : ` of ${formatSize(s.total)}`;
      return `Downloading wugs ${s.update.version}: ${formatSize(s.done)}${of}`;
    }
    case "ready":
      return `wugs ${s.update.version} is downloaded and checked.`;
    case "installing":
      return `Installing wugs ${s.update.version}. wugs restarts when it is done.`;
    default:
      return null;
  }
}

/** The toast after a check the person asked for, or null after an automatic one. */
export function manualResult(manual: boolean, outcome: { kind: "newest"; version: string } | { kind: "failed"; error: string } | { kind: "found" }): string | null {
  if (!manual || outcome.kind === "found") return null;
  if (outcome.kind === "newest") return `wugs ${outcome.version} is the newest version.`;
  return `Could not check for updates (Online): ${outcome.error}`;
}

/** Download progress as 0–1, or null when the size is unknown. */
export const fraction = (s: UpdateState): number | null =>
  s.kind === "downloading" && s.total !== null && s.total > 0 ? Math.min(1, s.done / s.total) : null;

const SETTING = "wugs.updates.check";

/** Checking at start is on unless this machine turned it off. */
export const checksEnabled = (store: Pick<Storage, "getItem">): boolean => store.getItem(SETTING) !== "off";

export function setChecksEnabled(store: Pick<Storage, "setItem">, on: boolean) {
  store.setItem(SETTING, on ? "on" : "off");
}
