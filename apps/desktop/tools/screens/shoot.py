#!/usr/bin/env python3
"""Screenshot every wugs desktop screen in headless Chrome, backend mocked.

Usage: python3 apps/desktop/tools/screens/shoot.py [--out DIR] [--only NAME ...]
"""
import json
import argparse
import io
import re
import socket
import struct
import subprocess
import sys
import time
import urllib.request
import wave
from pathlib import Path

from playwright.sync_api import sync_playwright

HERE = Path(__file__).resolve().parent
DESKTOP = HERE.parents[1]
PORT = 1420
CHROME = "/usr/bin/google-chrome"

# Panel id -> title, in src/panels/registry.ts order.
PANELS = [
    ("timeline", "Timeline"),
    ("media", "Media"),
    ("texts", "Texts"),
    ("transcribe", "Transcribe"),
    ("gloss", "Gloss"),
    ("lexicon", "Lexicon"),
    ("elicit", "Elicit"),
    ("search", "Search"),
    ("concordance", "Concordance"),
    ("frequencies", "Frequencies"),
    ("assist", "Speech assist"),
    ("people", "People"),
    ("conflicts", "Conflicts"),
    ("devices", "Devices"),
    ("kits", "Kits"),
]


def registry_ids():
    text = (DESKTOP / "src/panels/registry.ts").read_text()
    body = text.split("export const PANELS = {", 1)[1].split("}", 1)[0]
    return re.findall(r"^\s*(\w+):", body, re.M)


def serving():
    try:
        with urllib.request.urlopen(url(), timeout=1) as r:
            return r.status == 200
    except OSError:
        return False


def url():
    return f"http://localhost:{PORT}/"


def start_vite():
    if serving():
        return None
    with socket.socket() as s:
        if s.connect_ex(("127.0.0.1", PORT)) == 0:
            sys.exit(f"port {PORT} is taken by something that is not the vite dev server")
    proc = subprocess.Popen(
        ["npx", "vite", "--port", str(PORT), "--strictPort"],
        cwd=DESKTOP,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    for _ in range(100):
        if serving():
            return proc
        if proc.poll() is not None:
            sys.exit("vite exited:\n" + proc.stderr.read().decode())
        time.sleep(0.2)
    proc.kill()
    sys.exit("vite did not start within 20 s")


def silent_wav(seconds=20, rate=16000):
    buf = io.BytesIO()
    with wave.open(buf, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(struct.pack("<h", 0) * (seconds * rate))
    return buf.getvalue()


def bundled_kit(name="tongues"):
    """A real built-in kits/<name>/kit.toml as the shell's Kit shape, so the
    shots always show the layout a fresh install starts with."""
    import tomllib
    k = tomllib.loads((HERE.parents[3] / "kits" / name / "kit.toml").read_text())
    return {
        "id": k["kit"]["id"], "name": k["kit"]["name"], "subtitle": k["kit"].get("subtitle", ""),
        "version": k["kit"]["version"], "description": k["kit"].get("description", ""),
        "tools": k["tools"]["enabled"], "layout": k["layout"]["panels"], "active": k["layout"].get("active", []),
        **({"wug": k["wug"]} if "wug" in k else {}),
        **({"bundle": {
            "license": k.get("package", {}).get("license", ""), "source": k.get("package", {}).get("source", ""),
            "authors": k.get("bundle", {}).get("authors", []), "summary": k.get("bundle", {}).get("summary", ""),
            "requires": [], **({"examples": k["examples"]["project"]} if "examples" in k else {}),
        }} if "package" in k or "bundle" in k else {}),
    }


THEME = None


def init_script(auto_open):
    flag = "true" if auto_open else "false"
    return (
        f"window.__WUGS_SCREENS__ = {{ autoOpen: {flag}, theme: {json.dumps(THEME)}, kit: {json.dumps(bundled_kit())}, scrolls: {json.dumps(bundled_kit('scrolls'))} }};\n"
        + (HERE / "fixtures.js").read_text()
        + "\n"
        + (HERE / "mock-tauri.js").read_text()
    )


class Run:
    def __init__(self, browser, out, only, auto_open):
        self.out = out
        self.only = only
        self.errors = []
        self.context = browser.new_context(viewport={"width": 1440, "height": 900}, device_scale_factor=1)
        self.context.add_init_script(init_script(auto_open))
        wav = silent_wav()
        self.context.route("**/__mock_asset/**", lambda route: route.fulfill(status=200, body=wav, content_type="audio/wav"))
        self.page = self.context.new_page()
        self.page.on("pageerror", lambda e: self.errors.append(f"pageerror: {e}"))
        self.page.on("console", self.console)
        self.page.goto(url())
        self.page.wait_for_selector(".dv-tab", state="attached", timeout=15000)

    def console(self, msg):
        if msg.type == "error":
            self.errors.append(f"console.error: {msg.text}")

    def wanted(self, name):
        return not self.only or any(o in name for o in self.only)

    def shot(self, name, locator=None):
        if not self.wanted(name):
            return
        self.page.wait_for_timeout(250)
        path = self.out / f"{name}.png"
        (locator or self.page).screenshot(path=str(path))
        print(f"  {path}")

    def staged(self, panel_id, width=1200, height=800):
        """The panel alone at a roomy size: its element is moved into a stage
        on top of the page for the shot, then put back where it was."""
        name = f"stage-{panel_id}"
        if not self.wanted(name):
            return
        self.page.evaluate(
            """([id, w, h]) => {
                const el = document.querySelector(`.panel-${id}`);
                const stage = document.createElement('div');
                stage.id = 'wugs-screens-stage';
                stage.style.cssText = `position:fixed;left:0;top:0;width:${w}px;height:${h}px;z-index:99999;display:flex;background:var(--bg, #fff);padding:16px;box-sizing:border-box`;
                const card = document.createElement('div');
                card.className = 'dv-content-container';
                card.style.cssText = 'flex:1;display:flex;min-width:0;min-height:0;background:var(--surface-raised, #fff);border-radius:12px;overflow:hidden';
                stage.append(card);
                window.__wugsStageHome = [el.parentNode, el.nextSibling];
                card.append(el);
                el.style.flex = '1';
                document.body.append(stage);
            }""",
            [panel_id, width, height],
        )
        self.page.wait_for_timeout(300)
        self.shot(name, self.page.locator("#wugs-screens-stage"))
        self.page.evaluate(
            """id => {
                const el = document.querySelector(`.panel-${id}`);
                const [parent, next] = window.__wugsStageHome;
                el.style.flex = '';
                parent.insertBefore(el, next);
                document.getElementById('wugs-screens-stage').remove();
            }""",
            panel_id,
        )

    def toasts(self):
        return self.page.locator(".toast").all_inner_texts()

    def unknown(self):
        return self.page.evaluate("[...window.__WUGS_UNKNOWN__]")

    def front(self, panel_id, title):
        tab = self.page.locator(f".dv-tab[data-tab-panel-id='{panel_id}']")
        if tab.count() == 0:
            # A closed panel comes back through Settings (src/settings/panel.ts).
            self.page.click(".settings-button")
            self.page.click(f".settings-chips button:has-text('Show {title}')")
            self.page.keyboard.press("Escape")
            tab.first.wait_for(state="attached")
            self.page.locator(".settings-overlay").wait_for(state="detached")
        # Dockview activates a tab on pointerdown. Dispatched rather than
        # clicked, so it works however the theme hides or overlaps the strip.
        tab.first.evaluate(
            """t => {
                t.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0, isPrimary: true }));
                t.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, button: 0, isPrimary: true }));
                t.dispatchEvent(new MouseEvent('click', { bubbles: true, button: 0 }));
            }"""
        )
        self.page.mouse.move(720, 890)
        self.page.locator(f".panel-{panel_id}").wait_for(state="visible")


def prepare(run, panel_id):
    page = run.page
    if panel_id == "lexicon":
        page.locator(".panel-lexicon").get_by_text("kani", exact=True).first.click()
        page.locator(".panel-lexicon").get_by_text("reckon", exact=False).first.wait_for()
    elif panel_id == "search":
        box = page.locator(".panel-search input").first
        box.fill("kesi")
        page.locator(".panel-search mark").first.wait_for()
    elif panel_id == "kits":
        # The user kit, so the shot shows the editor and the wug picker.
        page.locator(".panel-kits .kit-name", has_text="Word-list sprint").click()
        page.locator(".panel-kits .kit-wug select").first.wait_for()


def open_workspace(run):
    page = run.page
    # Driven through the store module (the app's own instance), not the
    # chrome, so the harness keeps working while the chrome is restyled.
    page.wait_for_function("window.__WUGS_CALLS__.includes('list_texts')", timeout=15000)
    page.evaluate("async () => (await import('/src/store.ts')).selectText('t-count')")
    try:
        page.wait_for_selector(".tl-sources option[value='src-count']", state="attached")
    except Exception:
        print("\n".join(run.errors), file=sys.stderr)
        raise
    page.select_option(".tl-sources", "src-count")
    page.evaluate(
        "async () => (await import('/src/store.ts')).selectSegment({ textId: 't-count', tierId: 'tier-utt', segmentId: 'u01' })"
    )
    page.locator(".panel-gloss .gl-igt").wait_for()
    page.wait_for_timeout(600)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default="/tmp/screens")
    ap.add_argument("--only", action="append", default=[], help="substring of a screenshot name; repeatable")
    ap.add_argument("--theme", default=None, help="cream, seafoam, clay or night")
    ap.add_argument("--port", type=int, default=1420, help="dev server port; another worktree's server on 1420 would be reused")
    args = ap.parse_args()
    global THEME, PORT
    THEME = args.theme
    PORT = args.port
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    ids = registry_ids()
    known = [p for p, _ in PANELS]
    if ids != known:
        sys.exit(f"panel registry changed: {ids} vs harness {known}; update PANELS in shoot.py")

    vite = start_vite()
    failures = []
    unknown = set()
    try:
        with sync_playwright() as pw:
            browser = pw.chromium.launch(executable_path=CHROME, headless=True)

            run = Run(browser, out, args.only, auto_open=False)
            run.page.wait_for_timeout(500)
            run.shot("00-no-project")
            unknown |= set(run.unknown())
            failures += run.errors

            run = Run(browser, out, args.only, auto_open=True)
            open_workspace(run)
            run.shot("01-workspace")
            for n, (panel_id, title) in enumerate(PANELS, start=2):
                run.front(panel_id, title)
                prepare(run, panel_id)
                run.shot(f"{n:02d}-{panel_id}")
                run.shot(f"zoom-{panel_id}", run.page.locator(f".panel-{panel_id}"))
                run.staged(panel_id)
                for t in run.toasts():
                    failures.append(f"toast after {panel_id}: {t}")

            run.front("timeline", "Timeline")
            run.page.evaluate(
                """async () => (await import('/src/store.ts')).showToast({
                    tone: 'warn', lines: ['3 words kept their analysis; 1 did not.', 'Synced with Village tablet: 42 changes.'],
                    action: { label: 'Undo', run: () => {} } })"""
            )
            run.shot("state-toast")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")

            # Reach for the Timeline group's tabs: they rise and push its content down.
            box = run.page.locator(".panel-timeline").bounding_box()
            if box is not None:
                run.page.mouse.move(box["x"] + 40, box["y"] - 6)
                run.page.wait_for_timeout(600)
                run.shot("state-tabs-risen")
                run.page.mouse.move(box["x"] + box["width"] / 2, box["y"] + 200)
                run.page.wait_for_timeout(800)

            # Track headers: lock the first tier, hide the second, then try to edit the locked one.
            tiers = run.page.locator(".panel-timeline .tl-gutter-tier")
            if tiers.count() >= 2:
                tiers.nth(0).locator(".tl-track-toggle").nth(0).click()
                tiers.nth(1).locator(".tl-track-toggle").nth(1).click()
                canvas = run.page.locator(".panel-timeline .tl-canvas").bounding_box()
                if canvas is not None:
                    run.page.mouse.dblclick(canvas["x"] + 60, canvas["y"] + 150)
                run.page.wait_for_timeout(300)
                run.shot("state-tracks", run.page.locator(".panel-timeline"))
                locked = [t for t in run.toasts() if "locked" in t]
                if not locked:
                    failures.append("editing a locked track gave no message")
                run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
                tiers.nth(0).locator(".tl-track-toggle").nth(0).click()
                tiers.nth(1).locator(".tl-track-toggle").nth(1).click()

            # The kit switcher (roadmap 73): open it, then switch to Scrolls and back.
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.page.click(".kit-switcher")
            run.page.wait_for_timeout(200)
            run.shot("state-kit-switcher")
            run.page.locator(".menu-list [role=menuitemradio]", has_text="Scrolls").click()
            run.page.wait_for_timeout(900)
            run.shot("state-scrolls")
            # A built-in kit shown as a package (roadmap 74).
            run.front("kits", "Kits")
            run.page.locator(".panel-kits .kit-name", has_text="Scrolls").first.click()
            run.page.wait_for_timeout(300)
            run.shot("state-kit-bundle", run.page.locator(".panel-kits"))
            if run.page.locator(".panel-kits .kit-bundle li").count() == 0:
                failures.append("a built-in kit did not show itself as a package")
            # Frequencies (Scrolls): a row opens the Concordance on that word.
            run.front("frequencies", "Frequencies")
            run.page.wait_for_timeout(400)
            run.shot("state-frequencies", run.page.locator(".panel-frequencies"))
            if run.page.locator(".panel-frequencies .fq-table tbody tr").count() == 0:
                failures.append("the frequency list showed no rows")
            run.page.locator(".panel-frequencies .fq-table tbody tr").first.click()
            run.page.wait_for_timeout(600)
            if run.page.locator(".panel-concordance .cc-query").input_value() != "nikani":
                failures.append("a frequency row did not open the Concordance on its word")
            if "Scrolls" not in run.page.locator(".kit-switcher").inner_text():
                failures.append("switching to Scrolls did not change the kit")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.page.click(".kit-switcher")
            run.page.locator(".menu-list [role=menuitemradio]", has_text="Tongues").click()
            run.page.wait_for_timeout(900)
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")

            # Concordance with a query (roadmap 71).
            run.front("concordance", "Concordance")
            run.page.fill(".panel-concordance .cc-query", "kani")
            run.page.locator(".panel-concordance .scopes button", has_text="Morphs").click()
            run.page.wait_for_timeout(500)
            run.shot("state-concordance", run.page.locator(".panel-concordance"))
            if run.page.locator(".panel-concordance .cc-row").count() == 0:
                failures.append("the concordance showed no rows")

            # Media bin (roadmap 66): a card dragged onto the Timeline is used for the text on screen.
            run.front("media", "Media")
            fox = run.page.locator(".panel-media .bin-card", has_text="Fox story")
            fox.wait_for()
            fox.drag_to(run.page.locator(".panel-timeline .tl-body"))
            run.page.wait_for_timeout(600)
            if run.page.locator(".tl-sources").input_value() != "src-fox":
                failures.append("dragging a media card onto the Timeline did not load it")
            run.page.locator(".tl-sources").select_option("src-count")
            run.page.wait_for_timeout(400)
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")

            # Hear it everywhere (roadmap 72): a lexicon sense lists where it is used, each with ▶.
            run.front("lexicon", "Lexicon")
            run.page.locator(".panel-lexicon").get_by_text("kani", exact=True).first.click()
            run.page.locator(".panel-lexicon").get_by_text("reckon", exact=False).first.wait_for()
            uses = run.page.locator(".panel-lexicon .lx-sense-foot .link-button").first
            if uses.count() == 0:
                failures.append("no 'used in N words' button in the lexicon")
            else:
                uses.click()
                run.page.wait_for_timeout(400)
                run.shot("state-lexicon-uses", run.page.locator(".panel-lexicon"))
                if run.page.locator(".panel-lexicon .lx-uses .play-seg").count() == 0:
                    failures.append("the lexicon's uses list has no play buttons")
            if run.page.locator(".panel-transcribe .play-seg").count() == 0:
                failures.append("Transcribe rows have no play buttons")

            # Command palette (Ctrl+K) and the shortcuts sheet (?).
            run.page.locator(".panel-timeline .tl-wrap").focus()
            run.page.keyboard.press("Control+k")
            run.page.wait_for_timeout(200)
            run.page.keyboard.type("split")
            run.page.wait_for_timeout(150)
            run.shot("state-palette")
            if run.page.locator(".palette-row").count() == 0:
                failures.append("the palette found nothing for 'split'")
            run.page.keyboard.press("Escape")
            run.page.keyboard.press("?")
            run.page.wait_for_timeout(200)
            run.shot("state-shortcuts")
            if run.page.locator(".palette-shortcuts").count() == 0:
                failures.append("? did not open the shortcuts sheet")
            run.page.keyboard.press("Escape")

            # The first-run tour over the workspace (roadmap 68): two of its stops.
            run.page.evaluate("async () => (await import('/src/tour/tour.ts')).startTour(() => {})")
            run.page.wait_for_timeout(400)
            run.shot("state-tour")
            run.page.keyboard.press("ArrowRight")
            run.page.keyboard.press("ArrowRight")
            run.page.wait_for_timeout(400)
            run.shot("state-tour-gloss")
            run.page.keyboard.press("Escape")
            if run.page.locator(".tour-layer").count() != 0:
                failures.append("Escape did not end the tour")

            run.page.click(".settings-button")
            # Pattern tiles are CSS masks that load on first use.
            run.page.wait_for_load_state("networkidle")
            run.page.wait_for_timeout(800)
            run.shot("state-settings")
            run.page.locator(".settings-sheet .settings-section").nth(1).scroll_into_view_if_needed()
            run.shot("state-settings-patterns")
            run.page.keyboard.press("Escape")
            run.page.locator(".settings-overlay").wait_for(state="detached")

            run.page.get_by_role("button", name="Feedback").click()
            run.page.locator(".feedback-message").fill("The timeline froze after I imported a long recording.")
            run.page.locator(".feedback-details summary").click()
            run.page.wait_for_timeout(300)
            run.shot("state-feedback")
            run.page.get_by_role("button", name="Send feedback").click()
            run.page.wait_for_timeout(300)
            run.shot("state-feedback-sent")

            unknown |= set(run.unknown())
            failures += run.errors
            browser.close()
    finally:
        if vite is not None:
            vite.terminate()

    print("unknown commands:", ", ".join(sorted(unknown)) or "none")
    if failures:
        print("FAILURES:")
        for f in failures:
            print("  " + f)
        sys.exit(1)


if __name__ == "__main__":
    main()
