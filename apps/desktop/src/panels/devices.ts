// Devices: this device's identity, pairing (show a code, or join with
// one), the roster with "mark lost", and sync with paired devices.
import { open as pickDirectory } from "@tauri-apps/plugin-dialog";
import {
  deviceIdentity,
  deviceMarkLost,
  devicesList,
  discoverPeers,
  errorText,
  mintDeviceIdentity,
  onPairFinished,
  onSyncServed,
  pairJoin,
  pairStart,
  relocationAcceptMove,
  syncListen,
  syncNow,
  type DeviceRow,
  type DevicesView,
  type IdentityInfo,
  type ListenInfo,
} from "../api";
import { h } from "../dom";
import { refuseProjectChange } from "../capture/session.ts";
import { attempt, clearRelocation, enterProject, getState, refreshAll, researcher, showToast } from "../store";
import type { PanelDef } from "./panel";

const DEVICE_NAME_KEY = "wugs.deviceName";

type Pairing = { projectId: string; digits: string; port: number; expiresAt: number; guest: boolean };

/** Inputs live as long as the panel, so a redraw never wipes what is being typed. */
type Inputs = { name: HTMLInputElement; joinAddr: HTMLInputElement; code: HTMLInputElement; syncAddr: HTMLInputElement; guest: HTMLInputElement };

function makeInputs(): Inputs {
  const name = h("input", {
    placeholder: "e.g. Field laptop",
    value: localStorage.getItem(DEVICE_NAME_KEY) ?? "",
    title: "How other devices list this one",
  });
  name.oninput = () => localStorage.setItem(DEVICE_NAME_KEY, name.value);
  return {
    name,
    joinAddr: h("input", { placeholder: "host:port, e.g. 192.168.1.20:41234" }),
    code: h("input", { placeholder: "1234 5678", inputMode: "numeric", className: "code-input" }),
    syncAddr: h("input", { placeholder: "host:port" }),
    guest: h("input", { type: "checkbox" }),
  };
}

// One Devices state for the app, so pairing and listening survive the panel
// being closed or re-docked; the panel only draws it.
const devices = {
  identity: null as IdentityInfo | { error: string } | null,
  view: null as DevicesView | null,
  pairing: null as Pairing | null,
  listening: null as (ListenInfo & { path: string }) | null,
  peers: null as string[] | null,
  confirmLost: null as string | null,
  busy: false,
};
const redraws = new Set<() => void>();
const redraw = () => {
  for (const f of redraws) f();
};

const openProjectId = (): string | null => {
  const s = getState();
  return s.kind === "open" ? s.info.projectId : null;
};

const openProjectPath = (): string | null => {
  const s = getState();
  return s.kind === "open" ? s.info.path : null;
};

// Pairing belongs to the project it was started for; listening to the
// folder, because the backend stops it when another folder (with another
// device id) opens, even one of the same project.
const pairingNow = () => (devices.pairing?.projectId === openProjectId() ? devices.pairing : null);
const listeningNow = () => (devices.listening?.path === openProjectPath() ? devices.listening : null);

const deviceName = () => localStorage.getItem(DEVICE_NAME_KEY)?.trim() || "unnamed device";

async function reloadDevices() {
  if (getState().kind !== "open") {
    devices.view = null;
    redraw();
    return;
  }
  await attempt(async () => {
    devices.view = await devicesList();
    redraw();
  });
}

async function busy(action: () => Promise<void>) {
  if (devices.busy) return;
  devices.busy = true;
  redraw();
  await attempt(action);
  devices.busy = false;
  redraw();
}

/** Redraw hook for the shell's relocation banner, which shares `busy`. */
export function onDevicesChange(f: () => void): void {
  redraws.add(f);
}

async function mintFresh(projectId: string) {
  devices.view = await mintDeviceIdentity();
  clearRelocation(projectId);
  // The listener announced the old id, so the backend stopped it.
  devices.listening = null;
  showToast({ tone: "info", lines: ["This folder now has its own device id. The next sync tells the other devices."] });
}

/** Second step of "I moved it" when the original is still on disk. */
let confirmingMove = false;

/** Shown by the shell the moment a project opens as a copy of another folder,
 * wherever the Devices tab is. */
export function relocationBanner(): HTMLElement | null {
  const s = getState();
  const relocation = s.kind === "open" ? s.info.relocation : null;
  if (s.kind !== "open" || relocation === null) {
    confirmingMove = false;
    return null;
  }
  const projectId = s.info.projectId;
  const copy = relocation.originalStillPresent;
  const acceptMove = () =>
    void busy(async () => {
      await relocationAcceptMove();
      confirmingMove = false;
      clearRelocation(projectId);
    });
  const moved = h("button", {
    disabled: devices.busy,
    textContent: confirmingMove ? "Keep one device id" : "I moved it",
    title: "Keep this folder's device id and remember the new location.",
    onclick: () => {
      if (copy && !confirmingMove) {
        confirmingMove = true;
        redraw();
      } else acceptMove();
    },
  });
  const fresh = h("button", {
    className: copy ? "primary" : "",
    disabled: devices.busy,
    textContent: "Create fresh device identity",
    title: "It is a copy: give this folder its own device id. Its history stays; new work is written under the new id.",
    onclick: () => void busy(() => mintFresh(projectId)),
  });
  const message = copy
    ? `This folder is a copy of ${relocation.from}, which still exists.`
    : `This folder was moved or copied from ${relocation.from}.`;
  return h(
    "div",
    { className: "banner banner-warn" },
    h("p", { textContent: message }),
    ...(confirmingMove ? [h("p", { textContent: "The original still exists. Keep sharing one device id anyway?" })] : []),
    ...(copy ? [fresh, moved] : [moved, fresh]),
  );
}

function syncToast(lines: string[], warnings: string[]) {
  showToast({ tone: warnings.length > 0 ? "warn" : "info", lines: [...lines, ...warnings] });
}

async function syncWith(addr: string) {
  await busy(async () => {
    const r = await syncNow({ addr });
    syncToast(
      [`Synced with ${r.peerLabel ?? addr}: ${r.pulledOps} ops in, ${r.answered} requests answered${r.peerGuest ? " (guest, read-only)" : ""}.`],
      r.warnings,
    );
    // Only pulled ops change what this device shows; a refresh for nothing
    // would redraw panels under someone mid-edit.
    if (r.pulledOps > 0) await refreshAll();
    await reloadDevices();
  });
}

/** Register the backend's pairing and sync events once, at start. */
export async function wireDeviceEvents() {
  await onPairFinished((e) => {
    devices.pairing = null;
    if (e.kind === "enrolled") {
      showToast({ tone: "info", lines: [`Paired ${e.device.label || "a new device"}${e.device.guest ? " as a guest" : ""}.`] });
    } else {
      showToast({ tone: "error", lines: [`Pairing did not complete: ${e.error}`] });
    }
    void reloadDevices();
  });
  await onSyncServed((e) => {
    syncToast([`${e.peerLabel ?? "A device"} synced with this one: ${e.outcome}, ${e.pulledOps} ops in.`], e.warnings);
    if (e.pulledOps > 0) void refreshAll();
    void reloadDevices();
  });
  await attempt(async () => {
    try {
      devices.identity = await deviceIdentity();
    } catch (e) {
      devices.identity = { error: errorText(e) };
    }
  });
  redraw();
}

function identitySection({ name }: Inputs): HTMLElement {
  const id = devices.identity;
  const keyLine =
    id === null
      ? h("span", { className: "muted", textContent: "loading…" })
      : "error" in id
        ? h("span", { className: "error-text", textContent: id.error })
        : h(
            "span",
            {},
            h("code", { className: "fingerprint", textContent: id.fingerprint, title: id.publicKey }),
            " ",
            h("span", {
              className: id.store === "keychain" ? "badge" : "badge badge-warn",
              textContent: id.store === "keychain" ? "OS keychain" : "key file",
              title: id.fallbackReason ?? "The device key is in the OS keychain.",
            }),
          );
  return h(
    "section",
    {},
    h("h3", { textContent: "This device" }),
    h("div", { className: "field-row" }, h("label", { textContent: "Name" }), name),
    h("div", { className: "field-row" }, h("label", { textContent: "Key" }), keyLine),
    id !== null && "fallbackReason" in id && id.fallbackReason !== null ? h("p", { className: "note", textContent: id.fallbackReason }) : null,
  );
}

function joinSection({ joinAddr: addr, code }: Inputs): HTMLElement {
  const form = h(
    "form",
    {
      className: "stack",
      onsubmit: (ev: SubmitEvent) => {
        ev.preventDefault();
        void join(addr.value, code);
      },
    },
    h("div", { className: "field-row" }, h("label", { textContent: "Address" }), addr),
    h("div", { className: "field-row" }, h("label", { textContent: "Code" }), code),
    h("button", { type: "submit", disabled: devices.busy, textContent: "Choose a folder and join" }),
  );
  return h(
    "section",
    {},
    h("h3", { textContent: "Join a project" }),
    h("p", { className: "note", textContent: "On a device already in the project, open Devices and show a pairing code. Its screen gives the address." }),
    form,
  );
}

async function join(addr: string, code: HTMLInputElement) {
  if (refuseProjectChange()) return;
  const who = researcher();
  if (who === "") {
    showToast({ tone: "error", lines: ["Enter your researcher id first."] });
    return;
  }
  const path = await pickDirectory({ directory: true, title: "Choose an empty folder for the joined project" });
  if (typeof path !== "string") return;
  await busy(async () => {
    const joined = await pairJoin({ path, researcher: who, addr, code: code.value, label: deviceName() });
    await enterProject(joined.project);
    code.value = "";
    showToast({
      tone: "info",
      lines: [`Joined as ${joined.standing === "guest" ? "a guest (read-only)" : "a member"}. Sync with the other device to bring the project over.`],
    });
    await reloadDevices();
  });
}

function pairSection({ guest }: Inputs): HTMLElement {
  const p = pairingNow();
  if (p !== null) {
    const left = Math.max(0, Math.ceil((p.expiresAt - Date.now()) / 1000));
    return h(
      "section",
      {},
      h("h3", { textContent: "Pairing code" }),
      h("div", { className: "pair-code", textContent: `${p.digits.slice(0, 4)} ${p.digits.slice(4)}` }),
      h("p", {
        className: "note",
        textContent: `On the new device, join with this code and this computer's network address, port ${p.port}. ${left > 0 ? `Expires in ${left} s.` : "Expired."}${p.guest ? " It joins as a guest." : ""}`,
      }),
    );
  }
  return h(
    "section",
    {},
    h("h3", { textContent: "Pair a new device" }),
    h("label", { className: "check" }, guest, " Guest (reads open material only, never sends)"),
    h("button", {
      disabled: devices.busy,
      textContent: "Show pairing code",
      onclick: () =>
        void busy(async () => {
          const projectId = openProjectId() ?? "";
          const offer = await pairStart({ label: deviceName(), guest: guest.checked });
          devices.pairing = { projectId, digits: offer.digits, port: offer.port, expiresAt: Date.now() + offer.expiresIn * 1000, guest: guest.checked };
          await reloadDevices();
        }),
    }),
  );
}

function deviceRow(d: DeviceRow): HTMLElement {
  const badges = [
    d.thisDevice ? h("span", { className: "badge", textContent: "this device" }) : null,
    d.guest ? h("span", { className: "badge", textContent: "guest" }) : null,
    d.standing === "revoked" ? h("span", { className: "badge badge-muted", textContent: "lost" }) : null,
    d.contested
      ? h("span", {
          className: "badge badge-hot",
          textContent: "contested",
          title: "This key's history forked, so it may be in two hands. It cannot sync until a member marks it lost.",
        })
      : null,
  ];
  let action: HTMLElement | null = null;
  if (!d.thisDevice && d.standing !== "revoked") {
    action =
      devices.confirmLost === d.key
        ? h(
            "span",
            { className: "confirm" },
            `Mark ${d.label || "this device"} lost? It can never sync again. `,
            h("button", {
              className: "danger",
              textContent: "Mark lost",
              onclick: () =>
                void busy(async () => {
                  devices.view = await deviceMarkLost({ key: d.key });
                  devices.confirmLost = null;
                }),
            }),
            " ",
            h("button", {
              textContent: "Cancel",
              onclick: () => {
                devices.confirmLost = null;
                redraw();
              },
            }),
          )
        : h("button", {
            textContent: "Mark lost…",
            onclick: () => {
              devices.confirmLost = d.key;
              redraw();
            },
          });
  }
  return h(
    "tr",
    { className: d.standing === "revoked" ? "revoked" : "" },
    h("td", {}, h("div", { textContent: d.label || "(unnamed)" }), h("code", { className: "fingerprint", textContent: d.fingerprint, title: d.key })),
    h("td", {}, h("div", { className: "badges" }, ...badges, action)),
  );
}

function rosterSection(view: DevicesView): HTMLElement {
  const banner =
    view.copyWarning === null
      ? null
      : h(
          "div",
          { className: "banner banner-warn" },
          h("p", { textContent: view.copyWarning }),
          h("button", {
            disabled: devices.busy,
            textContent: "Create fresh device identity",
            title: "Give this folder its own device id. Its history stays; new work is written under the new id.",
            onclick: () => {
              const s = getState();
              if (s.kind === "open") void busy(() => mintFresh(s.info.projectId));
            },
          }),
        );
  const body = !view.paired
    ? h("p", { className: "note", textContent: "Not paired yet. Showing a pairing code makes this device the project's first member." })
    : h(
        "table",
        { className: "dense devices" },
        h("thead", {}, h("tr", {}, h("th", { textContent: "Device" }), h("th"))),
        h("tbody", {}, ...view.devices.map(deviceRow)),
      );
  return h("section", {}, h("h3", { textContent: "Devices in this project" }), banner, body);
}

function syncSection({ syncAddr: addr }: Inputs): HTMLElement {
  const l = listeningNow();
  const reach =
    l === null
      ? h("button", {
          disabled: devices.busy,
          textContent: "Make this device reachable",
          title: "Listen for sync from paired devices and announce this project on the local network",
          onclick: () =>
            void busy(async () => {
              const path = openProjectPath() ?? "";
              devices.listening = { ...(await syncListen({ label: deviceName() })), path };
              await reloadDevices();
            }),
        })
      : h(
          "p",
          { className: "note" },
          `Reachable for sync on port ${l.port}.`,
          l.discoveryError === null ? " Announced on the local network." : ` Not announced (${l.discoveryError}); others can still type the address.`,
        );
  const peers =
    devices.peers === null
      ? null
      : devices.peers.length === 0
        ? h("p", { className: "note", textContent: "No devices of this project found on the network." })
        : h(
            "ul",
            { className: "peers" },
            ...devices.peers.map((p) => h("li", {}, h("code", { textContent: p }), " ", h("button", { disabled: devices.busy, textContent: "Sync", onclick: () => void syncWith(p) }))),
          );
  return h(
    "section",
    {},
    h("h3", { textContent: "Sync" }),
    reach,
    h(
      "form",
      {
        className: "inline",
        onsubmit: (ev: SubmitEvent) => {
          ev.preventDefault();
          if (addr.value.trim() !== "") void syncWith(addr.value);
        },
      },
      addr,
      h("button", { type: "submit", disabled: devices.busy, textContent: "Sync now" }),
      h("button", {
        type: "button",
        disabled: devices.busy,
        textContent: "Find devices",
        onclick: () =>
          void busy(async () => {
            devices.peers = await discoverPeers();
          }),
      }),
    ),
    peers,
  );
}

export const devicesPanel: PanelDef = {
  title: "Devices",
  create() {
    const el = h("div", { className: "panel panel-devices" });
    const inputs = makeInputs();
    const draw = () => {
      const s = getState();
      const view = devices.view;
      const own = view?.devices.find((d) => d.thisDevice);
      const canHost = view !== null && (!view.paired || own?.standing === "member");
      const open = s.kind === "open" && view !== null;
      el.replaceChildren(
        identitySection(inputs),
        ...(open ? [rosterSection(view), canHost ? pairSection(inputs) : "", syncSection(inputs)] : []),
        joinSection(inputs),
      );
    };
    redraws.add(draw);
    const tick = window.setInterval(() => {
      if (devices.pairing === null) return;
      if (Date.now() > devices.pairing.expiresAt + 1000) devices.pairing = null;
      draw();
    }, 1000);
    let project: string | null = null;
    return {
      element: el,
      render() {
        const s = getState();
        const next = s.kind === "open" ? s.info.projectId : null;
        if (next !== project) {
          project = next;
          devices.peers = null;
          void reloadDevices();
        }
        draw();
      },
      dispose() {
        redraws.delete(draw);
        window.clearInterval(tick);
      },
    };
  },
};
