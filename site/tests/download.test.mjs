import assert from "node:assert/strict";
import { test } from "node:test";
import { detectSystem, installers, primaryChoice, size, version } from "../download.js";

test("the visitor's system comes from what the browser reports", () => {
  assert.equal(detectSystem({ userAgentData: { platform: "Windows" } }), "windows");
  assert.equal(detectSystem({ platform: "MacIntel", userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5)" }), "mac");
  assert.equal(detectSystem({ platform: "Linux x86_64", userAgent: "X11; Linux x86_64" }), "linux");
  assert.equal(detectSystem({ platform: "iPhone", userAgent: "iPhone; CPU iPhone OS 18_0 like Mac OS X" }), "mobile");
  assert.equal(detectSystem({ platform: "Linux armv8l", userAgent: "Linux; Android 15" }), "mobile");
  assert.equal(detectSystem({ platform: "MacIntel", userAgent: "Macintosh", maxTouchPoints: 5 }), "mobile");
  assert.equal(detectSystem({ platform: "Linux aarch64", userAgent: "X11; Linux aarch64" }), "unknown");
  assert.equal(detectSystem({}), "unknown");
});

const release = {
  tag_name: "v0.1.0",
  assets: [
    { name: "wugs_0.1.0_x64-setup.exe", size: 9e7, browser_download_url: "w" },
    { name: "wugs_0.1.0_x64-setup.exe.sig", size: 400, browser_download_url: "ws" },
    { name: "wugs_0.1.0_universal.dmg", size: 2e8, browser_download_url: "m" },
    { name: "wugs.app.tar.gz", size: 2e8, browser_download_url: "mt" },
    { name: "wugs_0.1.0_amd64.AppImage", size: 1e8, browser_download_url: "l" },
    { name: "wugs_0.1.0_amd64.deb", size: 9e7, browser_download_url: "d" },
    { name: "latest.json", size: 900, browser_download_url: "j" },
  ],
};

test("each system gets its installer and never an updater file", () => {
  const f = installers(release);
  assert.equal(f.windows.browser_download_url, "w");
  assert.equal(f.mac.browser_download_url, "m");
  assert.equal(f.linux.browser_download_url, "l");
  assert.equal(f.linuxDeb.browser_download_url, "d");
});

test("a release without installers, or none at all, offers nothing", () => {
  assert.deepEqual(installers({ assets: [] }), { windows: null, mac: null, linux: null, linuxDeb: null });
  assert.deepEqual(installers(null), { windows: null, mac: null, linux: null, linuxDeb: null });
});

test("sizes and versions read plainly", () => {
  assert.equal(size(92_400_000), "92 MB");
  assert.equal(size(1_300_000_000), "1.3 GB");
  assert.equal(size(0), "");
  assert.equal(version("v0.1.0"), "0.1.0");
});

test("the main button offers the visitor's installer, or says why it cannot", () => {
  const files = installers(release);
  assert.equal(primaryChoice("windows", files, "0.1.0", "R").href, "w");
  assert.equal(primaryChoice("windows", files, "0.1.0", "R").text, "Download for Windows");
  const empty = primaryChoice("windows", installers({ assets: [] }), "0.1.0", "R");
  assert.equal(empty.href, "R");
  assert.match(empty.meta, /still being prepared/);
  assert.equal(primaryChoice("unknown", files, "0.1.0", "R").href, "#systems");
  assert.match(primaryChoice("mobile", installers({ assets: [] }), "0.1.0", "R").meta, /still being prepared/);
  assert.equal(primaryChoice("mobile", files, "0.1.0", "R").text, "See all downloads");
});
