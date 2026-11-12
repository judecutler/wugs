// Which installer this visitor needs, from the newest GitHub release.
// Pure functions first (tested by tests/download.test.mjs), then the page code.

/** The visitor's system, from what the browser says about itself. */
export function detectSystem(nav) {
  const hint = `${nav.userAgentData?.platform ?? ""} ${nav.platform ?? ""} ${nav.userAgent ?? ""}`;
  if (/android|iphone|ipad|ipod/i.test(hint)) return "mobile";
  if (/win/i.test(hint)) return "windows";
  if (/mac/i.test(hint)) return "mac";
  if (/linux|x11|cros/i.test(hint)) return "linux";
  return "unknown";
}

/** The installers in a release, by system. The release also carries updater
 * files (.sig, .tar.gz, latest.json), which are never offered here. */
export function installers(release) {
  const assets = release?.assets ?? [];
  const find = (test) => assets.find((a) => test(a.name.toLowerCase())) ?? null;
  return {
    windows: find((n) => n.endsWith("-setup.exe")),
    mac: find((n) => n.endsWith(".dmg")),
    linux: find((n) => n.endsWith(".appimage")),
    linuxDeb: find((n) => n.endsWith(".deb")),
  };
}

export const SYSTEM_NAMES = { windows: "Windows", mac: "macOS", linux: "Linux" };

/** "92 MB" from bytes. */
export function size(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "";
  return bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${Math.max(1, Math.round(bytes / 1e6))} MB`;
}

/** The version a tag names: "v0.1.0" → "0.1.0". */
export const version = (tag) => String(tag ?? "").replace(/^v/i, "");

// ---- page ----------------------------------------------------------------

const REPO = "judecutler/wugs";
const RELEASES = `https://github.com/${REPO}/releases`;

function render(release) {
  const system = detectSystem(navigator);
  const files = installers(release);
  const primary = document.querySelector("[data-primary]");
  const meta = document.querySelector("[data-meta]");
  const others = document.querySelector("[data-others]");
  if (!primary || !meta || !others) return;

  if (release === null) {
    primary.textContent = "See releases on GitHub";
    primary.href = RELEASES;
    meta.textContent = "The first public version is on its way.";
    return;
  }

  const v = version(release.tag_name);
  const mine = system in SYSTEM_NAMES ? files[system] : null;
  if (mine) {
    primary.textContent = `Download for ${SYSTEM_NAMES[system]}`;
    primary.href = mine.browser_download_url;
    meta.textContent = [`Version ${v}`, size(mine.size), system === "mac" ? "Apple silicon and Intel" : ""].filter(Boolean).join(" · ");
  } else {
    primary.textContent = system === "mobile" ? "See all downloads" : "Choose your system below";
    primary.href = "#systems";
    meta.textContent = system === "mobile" ? `Version ${v} is for Windows, macOS and Linux. Open this page on your laptop.` : `Version ${v}`;
  }

  const rows = [
    ["windows", "Windows 10 or 11", files.windows, "Installer"],
    ["mac", "macOS 11 or newer", files.mac, "Disk image"],
    ["linux", "Linux", files.linux, "AppImage"],
    ["linux", "Debian, Ubuntu", files.linuxDeb, ".deb package"],
  ].filter(([, , asset]) => asset);
  others.replaceChildren(
    ...rows.map(([, label, asset, kind]) => {
      const a = document.createElement("a");
      a.className = "system";
      a.href = asset.browser_download_url;
      a.innerHTML = `<span class="system-name"></span><span class="system-kind"></span>`;
      a.querySelector(".system-name").textContent = label;
      a.querySelector(".system-kind").textContent = [kind, size(asset.size)].filter(Boolean).join(" · ");
      return a;
    }),
  );
  if (rows.length === 0) others.textContent = "This release has no installers yet.";

  // Open the first-run steps for the visitor's own system.
  document.querySelector(`details[data-system="${system}"]`)?.setAttribute("open", "");
}

async function main() {
  let release = null;
  try {
    const r = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: "application/vnd.github+json" } });
    if (r.ok) release = await r.json();
  } catch {
    // Offline or rate-limited: the buttons fall back to the releases page.
  }
  render(release);
}

if (typeof document !== "undefined") main();
