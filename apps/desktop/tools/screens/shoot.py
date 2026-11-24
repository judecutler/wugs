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
    ("sentence", "Sentence"),
    ("interlinear", "Interlinear"),
    ("lexicon", "Lexicon"),
    ("elicit", "Elicit"),
    ("search", "Search"),
    ("concordance", "Concordance"),
    ("frequencies", "Frequencies"),
    ("collocates", "Collocates"),
    ("similar", "Similar words"),
    ("keywords", "Keywords"),
    ("checks", "Checks"),
    ("vowels", "Vowel space"),
    ("contours", "Pitch contours"),
    ("durations", "Durations"),
    ("agreement", "Agreement"),
    ("turns", "Turn-taking"),
    ("sessions", "Sessions"),
    ("sounds", "Sounds"),
    ("signs", "Signs"),
    ("facsimile", "Facsimile"),
    ("typology", "Typology"),
    ("progress", "Progress"),
    ("affixes", "Affixes"),
    ("wordtypes", "Words to gloss"),
    ("notes", "Notes"),
    ("topics", "Topics"),
    ("wordsketch", "Word sketch"),
    ("wordorder", "Word order"),
    ("sentences", "Elicit sentences"),
    ("soundchanges", "Sound changes"),
    ("compare", "Compare word lists"),
    ("orthography", "Orthography"),
    ("assist", "Speech assist"),
    ("people", "People"),
    ("conflicts", "Conflicts"),
    ("devices", "Devices"),
    ("kits", "Kits"),
    ("help", "Help"),
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
        f"window.__WUGS_SCREENS__ = {{ autoOpen: {flag}, theme: {json.dumps(THEME)}, kit: {json.dumps(bundled_kit())}, scrolls: {json.dumps(bundled_kit('scrolls'))}, glyphs: {json.dumps(bundled_kit('glyphs'))}, land: {json.dumps((HERE.parents[3] / "datasets" / "naturalearth" / "land-110m.path").read_text().strip())} }};\n"
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
        picture = (HERE.parent.parent / "public" / "brand" / "logo.svg").read_bytes()
        # A file the fixtures name as a picture (`.svg`) is the brand wug; any other is a silent recording.
        self.context.route(
            "**/__mock_asset/**",
            lambda route: route.fulfill(status=200, body=picture, content_type="image/svg+xml")
            if route.request.url.endswith("svg")
            else route.fulfill(status=200, body=wav, content_type="audio/wav"),
        )
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
    elif panel_id == "vowels":
        page.locator(".panel-vowels button", has_text="Plot").click()
        page.locator(".panel-vowels .vw-plot").wait_for()
        # The picture Save picture… writes, beside the shots, to look at by itself.
        svg = page.evaluate("async () => (await import('/src/panels/vowels.ts')).standalone(document.querySelector('.panel-vowels .vw-plot'))")
        (run.out / "vowels-picture.svg").write_text(svg, encoding="utf-8")
    elif panel_id == "contours":
        page.locator(".panel-contours button", has_text="Draw").click()
        page.locator(".panel-contours .ct-plot").wait_for()
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
            # The Timeline as a picture for a paper.
            run.page.evaluate("window.__WUGS_PICK__ = '/tmp/figures'")
            run.page.locator(".panel-timeline .tl-toolbar button", has_text="Save picture").click()
            run.page.wait_for_timeout(500)
            run.page.evaluate("window.__WUGS_PICK__ = null")
            saved = run.page.evaluate("window.__WUGS_SAVED__") or {}
            if saved.get("extension") != "png" or not saved.get("contents", "").startswith("data:image/png;base64,") or len(saved.get("contents", "")) < 5000:
                failures.append(f"Save picture did not save the Timeline as a PNG: {str(saved)[:120]}")
            else:
                import base64 as _b64
                (run.out / "state-timeline-picture.png").write_bytes(_b64.b64decode(saved["contents"].split(",", 1)[1]))
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
                # The spectrogram under the waveform, zoomed to a stretch of speech.
                run.page.locator(".panel-timeline .tl-toolbar button", has_text="Spectrogram").click()
                run.page.wait_for_timeout(900)
                run.shot("state-spectrogram", run.page.locator(".panel-timeline"))
                if "active" not in (run.page.locator(".panel-timeline .tl-toolbar button", has_text="Spectrogram").get_attribute("class") or ""):
                    failures.append("the Spectrogram toggle did not turn on")
                # Narrowband asks for its own chunks.
                run.page.evaluate("window.__WUGS_NARROW__ = false")
                run.page.locator(".panel-timeline select.tl-band").select_option("narrow")
                run.page.wait_for_timeout(600)
                if not run.page.evaluate("window.__WUGS_NARROW__"):
                    failures.append("Narrowband did not ask for narrowband chunks")
                run.page.locator(".panel-timeline select.tl-band").select_option("broad")
                run.page.locator(".panel-timeline .tl-toolbar button", has_text="Spectrogram").click()
                if run.page.locator(".panel-timeline select.tl-band").is_visible():
                    failures.append("the band picker stayed with the spectrogram off")
                # Pitch on: the voice picker shows beside the overlay toggles.
                run.page.locator(".panel-timeline .praat-bar button", has_text="Pitch").click()
                run.page.wait_for_timeout(700)
                run.shot("state-pitch-voice", run.page.locator(".panel-timeline"))
                if run.page.locator(".panel-timeline .praat-voice").count() != 1:
                    failures.append("turning Pitch on showed no voice picker")
                # The cursor readout: the values under the pointer in the waveform band.
                canvas = run.page.locator(".panel-timeline canvas").first
                box = canvas.bounding_box()
                run.page.mouse.move(box["x"] + box["width"] * 0.3, box["y"] + 60)
                run.page.wait_for_timeout(300)
                run.shot("state-pitch-readout", run.page.locator(".panel-timeline"))
                run.page.locator(".panel-timeline .praat-bar button", has_text="Pitch").click()

            # The Language row offers Glottolog's matches as a name is typed.
            run.front("texts", "Texts")
            run.page.locator(".panel-texts input[aria-label='Language: Name']").fill("tur")
            run.page.locator(".panel-texts .glotto-match").first.wait_for()
            run.shot("state-glottolog", run.page.locator(".panel-texts"))
            if run.page.locator(".panel-texts .glotto-match").count() != 3:
                failures.append("typing a language name offered no Glottolog matches")
            run.page.locator(".panel-texts .glotto-match").first.click()
            run.page.wait_for_timeout(300)
            if run.page.locator(".panel-texts .glotto-match").count() != 0:
                failures.append("picking a Glottolog match left the list open")
            # Articles from a Wikipedia as texts.
            run.page.locator(".panel-texts button", has_text="From Wikipedia").click()
            run.page.locator(".panel-texts input.wiki-code").fill("sw")
            run.page.locator(".panel-texts textarea.wiki-titles").fill("Mlima Kilimanjaro\nMto Nile")
            run.page.locator(".panel-texts input.wiki-random").fill("3")
            run.shot("state-texts-wikipedia", run.page.locator(".panel-texts"))
            run.page.locator(".panel-texts .wiki-form button", has_text="Add the articles").click()
            run.page.wait_for_timeout(400)
            asked = run.page.evaluate("window.__WUGS_WIKI__") or {}
            if asked.get("wiki") != "sw" or asked.get("titles") != ["Mlima Kilimanjaro", "Mto Nile"] or asked.get("random") != 3:
                failures.append(f"From Wikipedia did not ask for the articles typed: {asked}")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.page.locator(".panel-texts button", has_text="From Wikipedia").click()
            # Pairing the text with another that translates it.
            run.page.locator(".panel-texts button", has_text="Pair with a translation").click()
            run.page.locator(".panel-texts .align-form button", has_text="Pair the sentences").click()
            run.page.locator(".panel-texts .align-pairs tr").first.wait_for()
            run.shot("state-texts-align", run.page.locator(".panel-texts"))
            if "Write 3 translations" not in run.page.locator(".panel-texts .align-form").inner_text():
                failures.append("Pair with a translation did not offer to write the three pairs")
            run.page.locator(".panel-texts button", has_text="Pair with a translation").click()
            # A reader to print ends with the words the text uses.
            run.page.evaluate("window.__WUGS_PICK__ = '/tmp/reader'")
            run.page.locator(".panel-texts .share-row", has_text="Reader to print").locator("button", has_text="Save").click()
            run.page.wait_for_timeout(800)
            run.page.evaluate("window.__WUGS_PICK__ = null")
            reader = (run.page.evaluate("window.__WUGS_SAVED__") or {}).get("contents", "")
            if "Words in this text" not in reader:
                failures.append(f"the reader did not end with the text's words: {run.toasts()}")
            else:
                (run.out / "doc-reader.html").write_text(reader)
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # A spelling dictionary of the language, for LibreOffice and Firefox.
            run.page.evaluate("window.__WUGS_PICK__ = '/tmp/spell'")
            run.page.locator(".panel-texts button", has_text="Spelling dictionary").click()
            run.page.wait_for_timeout(400)
            run.page.evaluate("window.__WUGS_PICK__ = null")
            if not any("spelling dictionary of 212 words" in t for t in run.toasts()):
                failures.append(f"Texts did not save a spelling dictionary: {run.toasts()}")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")

            # Find and replace in the Search panel: a live preview with the change marked.
            run.front("search", "Search")
            run.page.locator(".panel-search .rp-toggle").click()
            run.page.locator(".panel-search input[type=search]").fill("θ")
            run.page.locator(".panel-search .rp-with").fill("t")
            run.page.locator(".panel-search .rp-changes ins").first.wait_for()
            run.shot("state-replace", run.page.locator(".panel-search"))
            if run.page.locator(".panel-search .rp-changes li").count() != 2:
                failures.append("the replace preview did not list the two sentences with θ")
            run.page.locator(".panel-search .rp-foot button", has_text="Replace in 2").click()
            run.page.locator(".toast", has_text="Replaced in 2 sentences").wait_for()
            run.page.locator(".panel-search .rp-toggle").click()

            # The kit switcher (roadmap 73): open it, then switch to Scrolls and back.
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.page.click(".kit-switcher")
            run.page.wait_for_timeout(200)
            run.shot("state-kit-switcher")
            # Glyphs, the historical kit, opens its own tools.
            run.page.locator(".menu-list [role=menuitemradio]", has_text="Glyphs").click()
            run.page.wait_for_timeout(900)
            run.shot("state-glyphs")
            if run.page.locator(".panel-soundchanges").count() == 0:
                failures.append("Glyphs did not open the Sound changes panel")
            # The facsimile: boxes on the page; a box selects its line, a drag marks the next.
            run.front("facsimile", "Facsimile")
            boxes = run.page.locator(".panel-facsimile .fx-area")
            boxes.first.wait_for()
            if boxes.count() != 2:
                failures.append("Facsimile: page 1 did not show its two marked lines")
            boxes.nth(1).click()
            run.page.locator(".panel-facsimile .fx-area.fx-on").wait_for()
            run.page.locator(".panel-facsimile .fx-help label", has_text="each box a new line").locator("input").uncheck()
            page_box = run.page.locator(".panel-facsimile .fx-page img").bounding_box()
            if page_box:
                run.page.mouse.move(page_box["x"] + page_box["width"] * 0.1, page_box["y"] + page_box["height"] * 0.4)
                run.page.mouse.down()
                run.page.mouse.move(page_box["x"] + page_box["width"] * 0.6, page_box["y"] + page_box["height"] * 0.48, steps=4)
                run.page.mouse.up()
                run.page.wait_for_timeout(300)
            run.shot("state-facsimile", run.page.locator(".panel-facsimile"))
            if boxes.count() != 2:
                failures.append("Facsimile: dragging moved the selected line's box rather than adding one: " + str(boxes.count()))
            # With "each box a new line", a box drawn makes a new line on the page.
            run.page.locator(".panel-facsimile .fx-help label", has_text="each box a new line").locator("input").check()
            before = boxes.count()
            page_box = run.page.locator(".panel-facsimile .fx-page img").bounding_box()
            if page_box:
                run.page.mouse.move(page_box["x"] + page_box["width"] * 0.1, page_box["y"] + page_box["height"] * 0.3)
                run.page.mouse.down()
                run.page.mouse.move(page_box["x"] + page_box["width"] * 0.7, page_box["y"] + page_box["height"] * 0.36, steps=4)
                run.page.mouse.up()
                run.page.wait_for_timeout(300)
            if boxes.count() != before + 1:
                failures.append("Facsimile: a box drawn as a new line did not add one")
            run.shot("state-facsimile-new-line", run.page.locator(".panel-facsimile"))
            run.page.locator(".panel-facsimile .fx-help label", has_text="each box a new line").locator("input").uncheck()
            run.page.click(".kit-switcher")
            run.page.wait_for_timeout(200)
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
            # One speaker's track among the places to count.
            if run.page.locator(".panel-frequencies select.fq-scope optgroup[label='One track'] option").count() != 2:
                failures.append("The scope picker did not offer each track")
            # The Zipf plot, opened.
            run.page.locator(".panel-frequencies .fq-zipf summary").click()
            run.page.locator(".panel-frequencies .fq-zipf-plot").wait_for()
            run.shot("state-frequencies-zipf", run.page.locator(".panel-frequencies"))
            if run.page.locator(".panel-frequencies .fq-zipf-dot").count() < 3 or "Zipf's exponent" not in run.page.locator(".panel-frequencies .fq-zipf").inner_text():
                failures.append("the Zipf plot drew no points or no exponent")
            run.page.locator(".panel-frequencies .fq-zipf summary").click()
            # A reverse list lines words up at their ends.
            run.page.locator(".panel-frequencies .fq-sort").select_option("ending")
            run.page.wait_for_timeout(200)
            run.shot("state-frequencies-endings", run.page.locator(".panel-frequencies"))
            if run.page.locator(".panel-frequencies .fq-table.fq-endings").count() != 1:
                failures.append("By ending did not line the words up at their ends")
            run.page.locator(".panel-frequencies .fq-sort").select_option("spread")
            run.page.wait_for_timeout(200)
            run.shot("state-frequencies-spread", run.page.locator(".panel-frequencies"))
            first = run.page.locator(".panel-frequencies .fq-table tbody tr td.fq-item").first.text_content()
            if first != "nikani":
                failures.append(f"frequencies: the most evenly spread word is not first ({first!r})")
            run.page.locator(".panel-frequencies .fq-sort").select_option("count")
            # Only the words the lexicon lacks, each with Add.
            run.page.locator(".panel-frequencies .fq-missing input").check()
            run.page.wait_for_timeout(200)
            run.shot("state-frequencies-missing", run.page.locator(".panel-frequencies"))
            if run.page.locator(".panel-frequencies .fq-table tbody tr").count() != 3:
                failures.append("Only words not in the lexicon did not leave the three missing words")
            run.page.locator(".panel-frequencies .fq-lexicon button", has_text="Add").first.click()
            run.page.wait_for_timeout(300)
            if run.page.locator(".panel-frequencies .fq-table tbody tr").count() != 2:
                failures.append("a word added with Add stayed among the words not in the lexicon")
            # Words on the rise and falling over the years.
            run.page.locator(".panel-frequencies .fq-trends summary").click()
            run.page.locator(".panel-frequencies .fq-trend-list tr").first.wait_for()
            run.shot("state-frequencies-trends", run.page.locator(".panel-frequencies"))
            if run.page.locator(".panel-frequencies .fq-trend-list tr").count() != 3:
                failures.append("Over time did not list the rising and falling words")
            run.page.locator(".panel-frequencies .fq-trends summary").click()
            # The rest at once, after a second click.
            run.page.locator(".panel-frequencies .fq-add-all").click()
            run.shot("state-frequencies-add-all", run.page.locator(".panel-frequencies"))
            run.page.locator(".panel-frequencies .fq-add-all", has_text="Click again").click()
            run.page.wait_for_timeout(300)
            if run.page.locator(".panel-frequencies .fq-table tbody tr").count() != 1 or run.page.evaluate("window.__WUGS_ADDED_WORDS__") != 1:
                failures.append("Add the words not in the lexicon did not add the one unglossed word left")
            run.page.locator(".panel-frequencies .fq-missing input").uncheck()
            run.page.locator(".panel-frequencies .fq-table tbody tr").first.click()
            run.page.wait_for_timeout(600)
            if run.page.locator(".panel-concordance .cc-query").input_value() != "nikani":
                failures.append("a frequency row did not open the Concordance on its word")
            run.front("frequencies", "Frequencies")
            run.page.locator(".panel-frequencies .fq-size").select_option("2")
            run.page.wait_for_timeout(400)
            run.shot("state-frequencies-ngrams", run.page.locator(".panel-frequencies"))
            if "nikani ŋai" not in run.page.locator(".panel-frequencies .fq-table").inner_text():
                failures.append("two-word sequences did not show")
            # Texts picked by a detail are offered beside the texts.
            scope = run.page.locator(".panel-frequencies .fq-scope")
            if "Genre: narrative (2 texts)" not in scope.inner_text():
                failures.append("the scope picker does not offer texts by detail")
            else:
                scope.select_option(label="Genre: narrative (2 texts)")
                run.page.wait_for_timeout(400)
                run.shot("state-frequencies-detail", run.page.locator(".panel-frequencies"))
            if "Speaker:" in scope.inner_text():
                failures.append("a detail every text shares is offered as a scope")
            # A lemma list: a row opens the CQP query that finds its lines.
            scope.select_option(index=0)
            run.page.locator(".panel-frequencies .scopes button", has_text="Lemmas").click()
            run.page.wait_for_timeout(400)
            run.shot("state-frequencies-lemmas", run.page.locator(".panel-frequencies"))
            run.page.locator(".panel-frequencies .fq-table tbody tr").first.click()
            run.page.wait_for_timeout(600)
            if run.page.locator(".panel-concordance .cc-query").input_value() != '[lemma="kani"]':
                failures.append("a lemma row did not open the Concordance on its CQP query")
            # A tagged line can show each word's tag beside it.
            tags = run.page.locator(".panel-concordance button", has_text="Tags")
            tags.wait_for()
            tags.click()
            run.page.wait_for_timeout(200)
            run.shot("state-concordance-tags", run.page.locator(".panel-concordance"))
            if "nikani_VERB" not in run.page.locator(".panel-concordance .cc-rows").inner_text():
                failures.append("Tags did not show each word's tag")
            tags.click()
            # Collocates (Scrolls): a word's company, a row opens its Concordance.
            run.front("collocates", "Collocates")
            run.page.locator(".panel-collocates .co-node").fill("nikani")
            run.page.locator(".panel-collocates .co-node").press("Enter")
            run.page.wait_for_timeout(400)
            run.shot("state-collocates", run.page.locator(".panel-collocates"))
            if run.page.locator(".panel-collocates .co-table tbody tr").count() == 0:
                failures.append("collocates showed no rows")
            # Two words' collocates side by side.
            run.page.locator(".panel-collocates .co-versus").fill("kesi")
            run.page.locator(".panel-collocates .co-versus").press("Enter")
            run.page.wait_for_timeout(400)
            run.shot("state-collocates-compare", run.page.locator(".panel-collocates"))
            if run.page.locator(".panel-collocates .ws-lean-b").count() != 1 or "against «kesi»" not in run.page.locator(".panel-collocates .fq-count").inner_text():
                failures.append("Collocates did not compare two words")
            # The network: the two words with what they share between them.
            run.page.locator(".panel-collocates button", has_text="Network").click()
            run.page.wait_for_timeout(300)
            run.shot("state-collocates-network", run.page.locator(".panel-collocates"))
            if run.page.locator(".panel-collocates .co-net-shared").count() != 1:
                failures.append("the collocation network did not set the shared collocate between the words")
            run.page.locator(".panel-collocates button", has_text="Network").click()
            run.page.locator(".panel-collocates .co-versus").fill("")
            # Similar words (Scrolls): the words used in the same company.
            run.front("similar", "Similar words")
            run.page.locator(".panel-similar .co-node").fill("kesi")
            run.page.locator(".panel-similar .co-node").press("Enter")
            run.page.locator(".panel-similar .sm-table tbody tr").first.wait_for()
            run.shot("state-similar", run.page.locator(".panel-similar"))
            if run.page.locator(".panel-similar .sm-table tbody tr").count() != 4:
                failures.append("Similar words did not list the fixture's four words")
            # Keywords (Scrolls): a text's words against the rest.
            run.front("keywords", "Keywords")
            run.page.wait_for_timeout(400)
            run.shot("state-keywords", run.page.locator(".panel-keywords"))
            if run.page.locator(".panel-keywords .kw-table tbody tr").count() == 0:
                failures.append("keywords showed no rows")
            alike = run.page.locator(".panel-keywords .kw-alike-row")
            alike.first.scroll_into_view_if_needed()
            run.shot("state-keywords-alike", run.page.locator(".panel-keywords"))
            if alike.count() != 2 or "Greetings at the market" not in alike.first.inner_text():
                failures.append("Keywords did not rank the texts most like the one picked")
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
            # A line opened to the sentences around it.
            run.page.locator(".panel-concordance .cc-row .cc-more").first.click()
            run.page.locator(".panel-concordance .cc-around-me").first.wait_for()
            run.shot("state-concordance-around", run.page.locator(".panel-concordance"))
            if run.page.locator(".panel-concordance .cc-around p").count() < 2:
                failures.append("the opened line showed no sentences around it")
            run.page.locator(".panel-concordance .cc-row .cc-more").first.click()
            # Near: only the lines with ŋai right next to the match.
            run.page.fill(".panel-concordance .cc-near-word", "ŋai")
            run.page.select_option(".panel-concordance .cc-near select >> nth=0", "1")
            run.page.wait_for_timeout(200)
            run.shot("state-concordance-near", run.page.locator(".panel-concordance"))
            near_rows = run.page.locator(".panel-concordance .cc-row").count()
            if near_rows != 1 or "1 line of 3 has «ŋai» right before or after it" not in run.page.locator(".panel-concordance .cc-count").inner_text():
                failures.append(f"Concordance Near did not keep the one line with ŋai next to the match: {near_rows} rows")
            run.page.fill(".panel-concordance .cc-near-word", "")
            # Sorting the lines into categories of one's own.
            run.page.locator(".panel-concordance .cc-sort-toggle").click()
            run.page.locator(".panel-concordance .cc-cats-input").wait_for()
            run.page.fill(".panel-concordance .cc-cats-input", "literal, figurative")
            run.page.locator(".panel-concordance .cc-cats-input").press("Enter")
            run.page.locator(".panel-concordance select.cc-cat").first.wait_for()
            run.page.locator(".panel-concordance select.cc-cat").first.select_option("figurative")
            run.page.wait_for_timeout(300)
            run.shot("state-concordance-categories", run.page.locator(".panel-concordance"))
            if "figurative 1 (33%)" not in run.page.locator(".panel-concordance .cc-cats-note").inner_text():
                failures.append(f"Concordance did not count a line sorted into a category: {run.page.locator('.panel-concordance .cc-cats-note').inner_text()}")
            # The sorted lines compared across texts.
            run.page.locator(".panel-concordance select.cc-cat").nth(1).select_option("literal")
            run.page.wait_for_timeout(300)
            run.page.select_option(".panel-concordance .cc-cross-by", "text")
            run.page.locator(".panel-concordance .cc-cross-table").wait_for()
            run.shot("state-concordance-cross", run.page.locator(".panel-concordance"))
            if run.page.locator(".panel-concordance .cc-cross-table tbody tr").count() != 2:
                failures.append("the categories were not compared text by text")
            run.page.locator(".panel-concordance .cc-sort-toggle").click()
            # Translations under the lines, read from each text.
            run.page.locator(".panel-concordance .cc-under-toggle").click()
            run.page.wait_for_timeout(500)
            run.shot("state-concordance-translations", run.page.locator(".panel-concordance"))
            unders = run.page.locator(".panel-concordance .cc-under").all_inner_texts()
            if "I count: one, two, three." not in unders:
                failures.append(f"Concordance did not show the translation under a line: {unders}")
            run.page.locator(".panel-concordance button", has_text="Copy rows").click()
            run.page.wait_for_timeout(300)
            copied = run.page.evaluate("() => window.__WUGS_CLIPBOARD__") or {}
            if "\ttranslation" not in (copied.get("text") or "") or "I count: one, two, three." not in (copied.get("html") or ""):
                failures.append("Concordance: Copy rows did not carry the translations shown")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.page.locator(".panel-concordance .cc-under-toggle").click()
            # Every line's sentence saved as a sound file.
            run.page.evaluate("window.__WUGS_PICK__ = '/tmp/clips'")
            run.page.locator(".panel-concordance button", has_text="Save clips").click()
            run.page.wait_for_timeout(400)
            run.page.evaluate("window.__WUGS_PICK__ = null")
            asked_clips = run.page.evaluate("window.__WUGS_HITCLIPS__") or {}
            if len(asked_clips.get("hits") or []) != 3 or asked_clips.get("query") != "kani" or not any("Saved 2 sentences" in t for t in run.toasts()):
                failures.append(f"Concordance did not save its lines as clips: {asked_clips} {run.toasts()}")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # Sorted by the word before: that word is marked in each line.
            run.page.locator(".panel-concordance select.cc-sort").first.select_option("left")
            run.page.wait_for_timeout(200)
            run.shot("state-concordance-sorted", run.page.locator(".panel-concordance"))
            if run.page.locator(".panel-concordance .cc-sorted").count() == 0:
                failures.append("Concordance: the word sorted by was not marked")
            run.page.locator(".panel-concordance select.cc-sort").first.select_option("text")
            run.page.locator(".panel-concordance button", has_text="Copy rows").click()
            run.page.wait_for_timeout(300)
            copied = run.page.evaluate("() => window.__WUGS_CLIPBOARD__")
            if not copied or "<b>" not in (copied.get("html") or "") or "left\tkeyword" not in (copied.get("text") or ""):
                failures.append("Concordance: Copy rows did not give a KWIC table and its text")
            elif "\tgenre\t" not in copied.get("text", "") or "procedural" not in copied.get("text", ""):
                failures.append("Concordance: Copy rows did not carry the texts' details")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # Thinning a short concordance keeps every line.
            before = run.page.locator(".panel-concordance .cc-row").count()
            run.page.select_option(".panel-concordance .cc-thin", "50")
            if run.page.locator(".panel-concordance .cc-row").count() != before:
                failures.append("thinning a short concordance dropped lines")
            run.page.select_option(".panel-concordance .cc-thin", "0")
            # The lines' sentences as numbered examples for a paper.
            run.page.locator(".panel-concordance .cc-bar button", has_text="Copy as examples").click()
            run.page.wait_for_timeout(800)
            if "as numbered examples" not in " ".join(run.toasts()):
                failures.append("Copy as examples said nothing")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # Keep the search with the project, then run it again from the list.
            run.page.locator(".panel-concordance .cc-bar button", has_text="Keep search").click()
            run.page.fill(".panel-concordance .cc-keep-name", "Counting verbs")
            run.page.locator(".panel-concordance .cc-keep-bar button.primary").click()
            run.page.wait_for_timeout(300)
            if run.page.locator(".panel-concordance .cc-saved option[value='r:Counting verbs']").count() != 1:
                failures.append("Keep search did not list the search")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.shot("state-concordance-saved", run.page.locator(".panel-concordance"))
            # The lines' sentences filed under a grammar topic.
            run.page.locator(".panel-concordance .cc-bar button", has_text="File under a topic").click()
            run.page.fill(".panel-concordance .cc-topic-name", "Numerals")
            run.shot("state-concordance-topic", run.page.locator(".panel-concordance"))
            run.page.locator(".panel-concordance .cc-topic-bar button.primary").click()
            run.page.wait_for_timeout(500)
            if "under «Numerals»" not in " ".join(run.toasts()):
                failures.append(f"File under a topic said nothing: {run.toasts()}")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # The Plot view: where in each text, and how evenly.
            run.page.locator(".panel-concordance .cc-bar button", has_text="Plot").click()
            run.page.wait_for_timeout(500)
            run.shot("state-concordance-plot", run.page.locator(".panel-concordance"))
            run.page.select_option(".panel-concordance .cc-plot-by", "genre")
            run.page.wait_for_timeout(200)
            run.shot("state-concordance-plot-genre", run.page.locator(".panel-concordance"))
            if "procedural" not in run.page.locator(".panel-concordance .cc-rows").inner_text():
                failures.append("the plot did not sum the texts by genre")
            run.page.select_option(".panel-concordance .cc-plot-by", "time:date")
            run.page.wait_for_timeout(200)
            run.shot("state-concordance-plot-time", run.page.locator(".panel-concordance"))
            decades = run.page.locator(".panel-concordance .cc-plot-title").all_text_contents()
            if decades[:2] != ["1960s", "1970s"] or decades[-1] != "2010s":
                failures.append("the plot over time did not list the decades in order: " + str(decades))
            run.page.select_option(".panel-concordance .cc-plot-by", "map")
            run.page.locator(".panel-concordance svg.cc-map").wait_for()
            run.shot("state-concordance-plot-map", run.page.locator(".panel-concordance"))
            if run.page.locator(".panel-concordance svg.cc-map circle").count() != 3:
                failures.append("the plot on a map did not draw a dot per site")
            run.page.select_option(".panel-concordance .cc-plot-by", "")
            if run.page.locator(".panel-concordance .cc-plot-row").count() != 3:
                failures.append("the concordance plot did not show one row per text with words")
            run.page.locator(".panel-concordance .cc-bar button", has_text="Plot").click()
            # Clusters: the word sequences that hold the item.
            run.page.locator(".panel-concordance .cc-bar button", has_text="Clusters").click()
            run.page.wait_for_timeout(300)
            run.shot("state-concordance-clusters", run.page.locator(".panel-concordance"))
            if run.page.locator(".panel-concordance .cc-cluster").count() == 0:
                failures.append("Clusters listed nothing")
            run.page.locator(".panel-concordance .cc-cluster").first.click()
            run.page.wait_for_timeout(500)
            if " " not in run.page.locator(".panel-concordance .cc-query").input_value():
                failures.append("a cluster did not open its lines")
            run.page.fill(".panel-concordance .cc-query", "")
            # A word never analysed gets a guess read from known morphs.
            run.front("gloss", "Gloss")
            run.page.evaluate("async () => (await import('/src/store.ts')).selectSegment({ textId: 't-count', tierId: 'tier-utt', segmentId: 'u10' })")
            run.page.wait_for_timeout(600)
            run.shot("state-gloss-guess", run.page.locator(".panel-gloss"))
            if run.page.locator(".panel-gloss .gl-guessed").count() == 0:
                failures.append("a word starting ni- did not show its guess")
            if run.page.locator(".panel-gloss .gl-maybe", has_text="walks").count() != 1:
                failures.append("an unanalysed word did not show what the translations suggest")
            # A gloss being typed says what its abbreviations mean.
            run.page.evaluate("async () => (await import('/src/store.ts')).selectSegment({ textId: 't-count', tierId: 'tier-utt', segmentId: 'u01' })")
            run.page.wait_for_timeout(600)
            cell = run.page.locator(".panel-gloss input.gl-input[placeholder='gloss']").first
            cell.focus()
            cell.fill("3SG.XYZ")
            run.page.locator(".panel-gloss .gl-hint:not(:empty)").first.wait_for()
            run.shot("state-gloss-hint", run.page.locator(".panel-gloss"))
            if "SG singular · XYZ ?" not in run.page.locator(".panel-gloss .gl-hint:not(:empty)").first.inner_text():
                failures.append("the gloss hint did not explain SG and flag XYZ")
            cell.press("Escape")
            # Sentences like this one, with their translations, under the notes.
            like = run.page.locator(".panel-gloss .gl-like")
            like.scroll_into_view_if_needed()
            run.shot("state-gloss-like", run.page.locator(".panel-gloss"))
            if like.locator(".gl-like-row").count() != 2 or "Then I counted one by one." not in like.inner_text():
                failures.append("the Gloss panel did not list sentences like this one")
            # Interlinear: only the sentences still to gloss, and Next to gloss.
            run.front("interlinear", "Interlinear")
            run.page.wait_for_timeout(600)
            run.page.locator(".panel-interlinear .il-show").select_option("gloss")
            run.page.wait_for_timeout(300)
            run.shot("state-interlinear-todo", run.page.locator(".panel-interlinear"))
            if run.page.locator(".panel-interlinear .il-row").count() == 0:
                failures.append("Only sentences still to gloss showed none")
            run.page.locator(".panel-interlinear .il-show").select_option("topic:Numerals")
            run.page.wait_for_timeout(300)
            if run.page.locator(".panel-interlinear .il-row").count() != 1:
                failures.append(f"On «Numerals» did not list its one sentence: {run.page.locator('.panel-interlinear .il-row').count()}")
            run.page.locator(".panel-interlinear .il-show").select_option("gloss")
            run.page.wait_for_timeout(300)
            run.page.locator(".panel-interlinear .il-bar button", has_text="Next to gloss").click()
            run.page.wait_for_timeout(400)
            if run.page.locator(".panel-interlinear .il-selected").count() != 1:
                failures.append("Next to gloss selected no sentence")
            # Sentences without a translation, and Next to translate.
            run.page.locator(".panel-interlinear .il-show").select_option("translate")
            run.page.wait_for_timeout(300)
            run.shot("state-interlinear-translate", run.page.locator(".panel-interlinear"))
            if run.page.locator(".panel-interlinear .il-bar button", has_text="Next to translate").count() != 1:
                failures.append("Sentences without a translation did not offer Next to translate")
            if run.page.locator(".panel-interlinear .il-tr").count() != 0:
                failures.append("Sentences without a translation listed a translated one")
            run.page.locator(".panel-interlinear .il-show").select_option("all")
            # Lines marked on a page image show their crop above them.
            run.page.locator(".panel-interlinear .il-crop").first.wait_for()
            run.shot("state-interlinear-crops", run.page.locator(".panel-interlinear"))
            if run.page.locator(".panel-interlinear .il-crop").count() != 3:
                failures.append("Interlinear did not show the three marked lines' crops: " + str(run.page.locator(".panel-interlinear .il-crop").count()))
            # Checks: the character inventory, a Cyrillic letter among Latin ones flagged.
            run.front("checks", "Checks")
            run.page.locator(".panel-checks .ck-char").first.wait_for()
            examples = run.page.locator(".panel-checks .ck-title", has_text="Example sentences")
            examples.scroll_into_view_if_needed()
            run.shot("state-checks-examples", run.page.locator(".panel-checks"))
            if "Add 4 from the texts" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Checks did not offer examples from the texts")
            leip = run.page.locator(".panel-checks .ck-title", has_text="Glosses against the Leipzig rules")
            leip.locator("xpath=following-sibling::ul[1]").scroll_into_view_if_needed()
            run.shot("state-checks-leipzig", run.page.locator(".panel-checks"))
            if "Use «PL» for all 7" not in run.page.locator(".panel-checks").inner_text() or "Use «go.out» for all 3" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Checks did not offer the Leipzig fixes")
            bare = run.page.locator(".panel-checks .ck-title", has_text="Morphs without a gloss")
            bare.locator("xpath=following-sibling::ul[1]").scroll_into_view_if_needed()
            run.shot("state-checks-unglossed", run.page.locator(".panel-checks"))
            if run.page.locator(".panel-checks .ck-fill").first.input_value() != "PST" or "8 morphs have no gloss yet" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Checks did not list the morphs without a gloss with the gloss they have elsewhere")
            broken = run.page.locator(".panel-checks .ck-title", has_text="Words broken up more than one way")
            broken.locator("xpath=following-sibling::ul[1]").scroll_into_view_if_needed()
            run.shot("state-checks-breakdowns", run.page.locator(".panel-checks"))
            use = run.page.locator(".panel-checks button", has_text="Use «ni-kani» for all 5")
            use.click()
            run.page.locator(".panel-checks button", has_text="Change 1: sure?").click()
            run.page.wait_for_timeout(400)
            if run.page.evaluate("() => window.__WUGS_BREAKDOWN__") != "tok-a":
                failures.append("Checks did not copy the chosen breakdown")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            unlike = run.page.locator(".panel-checks .ck-title", has_text="Unlike the lexicon")
            unlike.locator("xpath=following-sibling::ul[1]").scroll_into_view_if_needed()
            run.shot("state-checks-unlike", run.page.locator(".panel-checks"))
            if "Use «river» in the texts" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Checks did not offer the lexicon's gloss")
            consent = run.page.locator(".panel-checks .ck-title", has_text="Consent records")
            consent.locator("xpath=following-sibling::ul[1]").scroll_into_view_if_needed()
            run.shot("state-checks-consent", run.page.locator(".panel-checks"))
            if "no to «Publish online»" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Checks did not say what a consent record limits")
            archive = run.page.locator(".panel-checks .ck-title", has_text="Ready for the archive")
            archive.locator("xpath=following-sibling::ul[1]").scroll_into_view_if_needed()
            run.shot("state-checks-archive", run.page.locator(".panel-checks"))
            if "Missing: date, place, speakers, recording and consent record" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Checks did not say what a text lacks for the archive")
            trees = run.page.locator(".panel-checks .ck-title", has_text="Dependency trees")
            trees.locator("xpath=following-sibling::ul[1]").scroll_into_view_if_needed()
            run.shot("state-checks-trees", run.page.locator(".panel-checks"))
            if "1 of 4 parsed sentences has" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Checks did not list the tree problem")
            timing = run.page.locator(".panel-checks .ck-title", has_text="Segment timing")
            timing.locator("xpath=following-sibling::ul[1]").scroll_into_view_if_needed()
            run.shot("state-checks-timing", run.page.locator(".panel-checks"))
            if "1 overlaps the one before it" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Checks did not list segment timing")
            twice = run.page.locator(".panel-checks .ck-title", has_text="Sentences written twice")
            twice.scroll_into_view_if_needed()
            run.shot("state-checks-twice", run.page.locator(".panel-checks"))
            if "1 sentence is written more than once" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Checks did not list the sentence written twice")
            doubles = run.page.locator(".panel-checks .ck-title", has_text="Entries that may be one word")
            doubles.locator("xpath=following-sibling::ul[1]").scroll_into_view_if_needed()
            run.shot("state-checks-doubles", run.page.locator(".panel-checks"))
            if "Merge «lima» into «liwa»" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Checks did not offer to merge two entries that may be one word")
            spelling = run.page.locator(".panel-checks .ck-title", has_text="Spellings to check")
            spelling.scroll_into_view_if_needed()
            run.shot("state-spellings", run.page.locator(".panel-checks"))
            if "tʰakka" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Spellings to check did not list a rare word beside a common one")
            unusual = run.page.locator(".panel-checks .ck-title", has_text="Unusual sound sequences")
            unusual.scroll_into_view_if_needed()
            run.shot("state-unusual-sequences", run.page.locator(".panel-checks"))
            if "Only word with kk" not in run.page.locator(".panel-checks").inner_text():
                failures.append("Unusual sound sequences did not list tʰakka's kk")
            run.page.locator(".panel-checks button", has_text="Every character").click()
            run.page.locator(".panel-checks .ck-chars").scroll_into_view_if_needed()
            run.shot("state-characters", run.page.locator(".panel-checks"))
            if run.page.locator(".panel-checks .ck-chars tbody tr").count() != 8:
                failures.append("Every character did not list the inventory")
            # Words to gloss: one analysis typed for every unanalysed use of a form.
            run.front("wordtypes", "Words to gloss")
            run.page.locator(".panel-wordtypes .wt-row").first.wait_for()
            # A likely meaning from the translations fills the root's gloss.
            run.page.locator(".panel-wordtypes .wt-meanings button", has_text="eats").click()
            run.shot("state-wordtypes-meaning", run.page.locator(".panel-wordtypes"))
            if run.page.locator('.panel-wordtypes [data-key="wt-g:wuno"]').input_value() != "eats" or run.page.locator('.panel-wordtypes [data-key="wt-b:wuno"]').input_value() != "wuno":
                failures.append("Words to gloss: a likely meaning did not fill the fields")
            run.page.locator('.panel-wordtypes [data-key="wt-b:wuno"]').fill("wu- no")
            run.page.locator('.panel-wordtypes [data-key="wt-g:wuno"]').fill("3SG eat")
            run.shot("state-wordtypes", run.page.locator(".panel-wordtypes"))
            run.page.locator('.panel-wordtypes [data-key="wt-g:wuno"]').press("Enter")
            run.page.wait_for_timeout(500)
            if "Analysed 4 uses of «wuno»" not in " ".join(run.toasts()):
                failures.append("Words to gloss did not apply an analysis")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # Progress: the vocabulary growth curve under the table.
            run.front("progress", "Progress")
            run.page.locator(".panel-progress .pg-steps li").first.wait_for()
            if run.page.locator(".panel-progress .pg-steps li").count() < 5:
                failures.append("Progress did not list what to do next")
            makeup = run.page.locator(".panel-progress .pg-makeup")
            makeup.wait_for()
            makeup.scroll_into_view_if_needed()
            run.shot("state-makeup", run.page.locator(".panel-progress"))
            if "narrative" not in makeup.inner_text():
                failures.append("Progress did not break the corpus down by genre")
            places = run.page.locator(".panel-progress .pg-places")
            places.locator(".pg-site").first.wait_for()
            places.scroll_into_view_if_needed()
            run.shot("state-progress-places", run.page.locator(".panel-progress"))
            if places.locator(".pg-site").count() != 2 or "2 texts of 3 placed, at 2 sites. 1 text names a place in words only" not in places.inner_text():
                failures.append(f"Progress did not map where the texts were recorded: {places.inner_text()}")
            growth = run.page.locator(".panel-progress .pg-chart")
            growth.wait_for()
            growth.scroll_into_view_if_needed()
            run.shot("state-growth", run.page.locator(".panel-progress"))
            if "flattening" not in run.page.locator(".panel-progress .pg-growth").inner_text():
                failures.append("Progress did not describe the vocabulary growth")
            spk = run.page.locator(".panel-progress .pg-speaker-table")
            spk.scroll_into_view_if_needed()
            run.shot("state-speakers", run.page.locator(".panel-progress .pg-speakers"))
            if "2.37 (30 of 41)" not in spk.inner_text() or "II" not in spk.inner_text():
                failures.append(f"Progress did not give the child's MLU and stage: {spk.inner_text()}")
            # The work log, day by day.
            work = run.page.locator(".panel-progress .pg-work")
            work.scroll_into_view_if_needed()
            run.shot("state-worklog", work)
            if "61 glosses" not in work.inner_text() or run.page.locator(".panel-progress .pg-work-table tbody tr").count() != 3:
                failures.append(f"Progress did not show the work day by day: {work.inner_text()[:200]}")
            # Word sketch: a lemma's company by relation.
            run.front("wordsketch", "Word sketch")
            run.page.locator(".panel-wordsketch .ws-lemma:not(.ws-versus)").fill("eat")
            run.page.locator(".panel-wordsketch .ws-lemma:not(.ws-versus)").press("Enter")
            run.page.locator(".panel-wordsketch .ws-group").first.wait_for()
            run.shot("state-wordsketch", run.page.locator(".panel-wordsketch"))
            if "Objects of «eat»" not in run.page.locator(".panel-wordsketch").inner_text():
                failures.append("Word sketch did not name the objects")
            # Sketch difference: eat against drink.
            run.page.locator(".panel-wordsketch .ws-versus").fill("drink")
            run.page.locator(".panel-wordsketch .ws-versus").press("Enter")
            run.page.locator(".panel-wordsketch .ws-diff").first.wait_for()
            run.shot("state-wordsketch-diff", run.page.locator(".panel-wordsketch"))
            if "against «drink»" not in run.page.locator(".panel-wordsketch .ws-line").inner_text() or run.page.locator(".panel-wordsketch tr.ws-lean-b").count() == 0:
                failures.append("Word sketch did not set eat against drink")
            run.page.locator(".panel-wordsketch .ws-versus").fill("")
            # People: a person's details, and everyone copied as a table.
            run.front("people", "People")
            run.page.locator(".panel-people .pp-details summary").first.click()
            ed = run.page.locator(".panel-people .pp-detail").nth(1)
            ed.fill("female")
            ed.press("Enter")
            run.page.locator(".panel-people .pp-note").first.click()
            run.page.wait_for_timeout(300)
            run.shot("state-people-details", run.page.locator(".panel-people"))
            if "Details (4)" not in run.page.locator(".panel-people .pp-details summary").first.inner_text():
                failures.append("People did not keep a person's detail")
            run.page.locator(".panel-people .pp-copy").click()
            run.page.wait_for_timeout(300)
            clip = run.page.evaluate("window.__WUGS_CLIPBOARD__") or {}
            if "Born (year)" not in clip.get("text", "") or "1952" not in clip.get("text", ""):
                failures.append(f"People did not copy everyone as a table: {clip}")
            # A consent form printed for the first person, with their name and the project's language.
            run.page.evaluate("window.__WUGS_PICK__ = '/tmp/forms'")
            run.page.locator(".panel-people .pp-person .pp-print").first.click()
            run.page.wait_for_timeout(400)
            run.page.evaluate("window.__WUGS_PICK__ = null")
            saved = run.page.evaluate("window.__WUGS_SAVED__") or {}
            form = saved.get("contents", "")
            if "Consent to record and share: Kanu" not in form or "Deposit in an archive" not in form or not saved.get("title", "").startswith("Consent form "):
                failures.append(f"People did not print a consent form: {saved.get('title')}")
            else:
                (run.out / "doc-consent-form.html").write_text(form)
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # Elicit sentences: answer a prompt; focus moves to the next.
            run.front("sentences", "Elicit sentences")
            field = run.page.locator(".panel-sentences input.se-answer[data-prompt='2']")
            field.wait_for()
            field.fill("wiripa kani nani")
            field.press("Enter")
            run.page.wait_for_timeout(300)
            run.shot("state-sentences", run.page.locator(".panel-sentences"))
            if "2 of 4 prompts answered" not in run.page.locator(".panel-sentences .se-line").inner_text():
                failures.append("Elicit sentences did not count the new answer")
            if run.page.evaluate("document.activeElement && document.activeElement.dataset.prompt") != "3":
                failures.append("Elicit sentences did not move to the next prompt")
            # A list made from frames: each entry's gloss in each frame, the headword as a hint.
            run.page.locator(".panel-sentences button", has_text="From frames").click()
            run.page.locator(".panel-sentences .se-frame-text").fill("I see the ___.\n___")
            run.page.wait_for_timeout(200)
            run.shot("state-sentences-frames", run.page.locator(".panel-sentences"))
            frame_note = run.page.locator(".panel-sentences .se-frames .note").inner_text()
            run.page.locator(".panel-sentences button", has_text="Make the list").click()
            run.page.wait_for_timeout(400)
            made = run.page.evaluate("window.__WUGS_MADE__") or {}
            first = (made.get("prompts") or [{}])[0]
            if "in 2 frames" not in frame_note or not first.get("prompt", "").startswith("I see the ") or not first.get("hint"):
                failures.append(f"Elicit sentences did not make a list from frames: {frame_note} {first}")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # Word order: WALS orders counted in the parsed sentences.
            # Durations: a text whose labels repeat, so the strip plot and the comparison show.
            run.page.evaluate("async () => { const st = await import('/src/store.ts'); window.__keptSelection = st.getSelection(); await st.selectText('t-fox'); }")
            run.front("durations", "Durations")
            run.page.locator(".panel-durations .du-plot").wait_for()
            run.page.wait_for_timeout(300)
            run.shot("state-durations", run.page.locator(".panel-durations"))
            result = run.page.locator(".panel-durations .du-result")
            if result.count() != 1 or "Welch" not in (result.text_content() or ""):
                failures.append("Durations: two labels with several segments each were not compared")
            run.page.locator(".panel-durations .du-rate").scroll_into_view_if_needed()
            run.shot("state-durations-rate", run.page.locator(".panel-durations"))
            if "syllables a second" not in run.page.locator(".panel-durations .du-rate").inner_text():
                failures.append("Durations did not give the speaking rate")
            run.page.evaluate("async () => { const st = await import('/src/store.ts'); await st.selectText('t-count'); st.selectSegment(window.__keptSelection); }")
            run.page.wait_for_timeout(300)
            # Vowel space trajectories: each label's arrow from 20% to 80%, the diphthong's long.
            run.front("vowels", "Vowel space")
            if run.page.locator(".panel-vowels .vw-plot").count() == 0:
                run.page.locator(".panel-vowels button", has_text="Plot").click()
                run.page.locator(".panel-vowels .vw-plot").wait_for()
            run.page.locator(".panel-vowels label", has_text="Trajectories").locator("input").check()
            run.page.locator(".panel-vowels .vw-path").first.wait_for()
            run.shot("state-vowels-trajectories", run.page.locator(".panel-vowels"))
            if run.page.locator(".panel-vowels .vw-path").count() != 6:
                failures.append("Vowel space did not draw a trajectory per label")
            run.page.locator(".panel-vowels label", has_text="Trajectories").locator("input").uncheck()
            # Normalized by Watt and Fabricius: the triangle's corners named under the plot.
            run.page.locator(".panel-vowels select.vw-method").select_option("watt-fabricius")
            run.page.wait_for_timeout(200)
            run.shot("state-vowels-watt", run.page.locator(".panel-vowels"))
            if "The triangle's corners" not in run.page.locator(".panel-vowels").inner_text() or "F1 (F/S)" not in run.page.locator(".panel-vowels .vw-plot").text_content():
                failures.append("Vowel space did not normalize by Watt and Fabricius")
            # A second speaker's track over the first, each normalized on their own.
            run.page.locator(".panel-vowels select.vw-method").select_option("lobanov")
            other = run.page.locator(".panel-vowels select.vw-other option").nth(1).get_attribute("value")
            run.page.locator(".panel-vowels select.vw-other").select_option(other)
            run.page.locator(".panel-vowels .vw-label.vw-two").first.wait_for()
            run.shot("state-vowels-two", run.page.locator(".panel-vowels"))
            if run.page.locator(".panel-vowels .vw-key").count() != 1 or "Each speaker is normalized" not in run.page.locator(".panel-vowels").inner_text():
                failures.append("Vowel space did not draw a second speaker over the first")
            run.page.locator(".panel-vowels select.vw-other").select_option("")
            run.page.locator(".panel-vowels select.vw-method").select_option("hz")
            # Agreement: the market conversation's two hearings compared.
            run.page.evaluate("async () => { const st = await import('/src/store.ts'); window.__keptSelection = st.getSelection(); await st.selectText('t-market'); }")
            run.front("agreement", "Agreement")
            run.page.locator(".panel-agreement .ag-table").wait_for()
            run.page.wait_for_timeout(300)
            run.shot("state-agreement", run.page.locator(".panel-agreement"))
            summary = run.page.locator(".panel-agreement .ag-summary").inner_text()
            if "6 pairs" not in summary or "identical in 4 of 6" not in summary or "Sound error rate" not in summary:
                failures.append(f"Agreement: the two hearings were not paired as expected: {summary!r}")
            if run.page.locator(".panel-agreement tr.ag-alone").count() != 2:
                failures.append("Agreement: the missed turn and the added one were not listed alone")
            # Turn-taking: the market conversation's two tracks read as two speakers.
            run.front("turns", "Turn-taking")
            run.page.locator(".panel-turns .tt-plot").wait_for()
            run.page.wait_for_timeout(300)
            run.shot("state-turns", run.page.locator(".panel-turns"))
            if "changes of speaker" not in run.page.locator(".panel-turns .ag-summary").inner_text():
                failures.append("Turn-taking did not count changes of speaker")
            run.page.evaluate("async () => { const st = await import('/src/store.ts'); await st.selectText('t-count'); st.selectSegment(window.__keptSelection); }")
            run.page.wait_for_timeout(300)
            # Sessions: a new session dated today goes on top, its notes ready to type in.
            run.front("sessions", "Sessions")
            run.page.locator(".panel-sessions .ss-card").first.wait_for()
            run.page.locator(".panel-sessions button", has_text="New session").click()
            run.page.wait_for_timeout(400)
            run.shot("state-sessions-new", run.page.locator(".panel-sessions"))
            if run.page.locator(".panel-sessions .ss-card").count() != 3:
                failures.append("Sessions: New session did not add a card")
            elif run.page.evaluate("() => document.activeElement?.classList.contains('ss-notes')") is not True:
                failures.append("Sessions: the new session's notes were not focused")
            # Typology: a feature's map, across the Pacific, the project's answer ringed.
            run.front("typology", "Typology")
            run.page.locator(".panel-typology .ty-map-toggle").first.wait_for()
            run.page.locator(".panel-typology .ty-map-toggle").first.click()
            run.page.locator(".panel-typology .tm-map").wait_for()
            run.page.locator(".panel-typology .tm-wrap").scroll_into_view_if_needed()
            run.shot("state-typology-map", run.page.locator(".panel-typology"))
            if run.page.locator(".panel-typology .tm-dot").count() != 29 or run.page.locator(".panel-typology .tm-me").count() != 1:
                failures.append("Typology: the map did not place every language and ring the project's")
            # Affixes: the order of two affixes met in one word.
            run.front("affixes", "Affixes")
            order = run.page.locator(".panel-affixes .af-order")
            order.locator("tbody tr").first.wait_for()
            order.scroll_into_view_if_needed()
            run.shot("state-affixes-order", run.page.locator(".panel-affixes"))
            if order.locator("tr.af-flag").count() != 2:
                failures.append("Affixes: the pairs met in both orders or in one slot are not marked")
            forms = run.page.locator(".panel-affixes .af-set")
            forms.first.scroll_into_view_if_needed()
            run.shot("state-affixes-forms", run.page.locator(".panel-affixes"))
            if forms.count() != 1 or "vowel harmony" not in (forms.first.text_content() or ""):
                failures.append("Affixes: -si and -su PL were not found conditioned by the stem's vowel")
            gb = run.page.locator(".panel-affixes .af-grambank li")
            gb.first.scroll_into_view_if_needed()
            run.shot("state-affixes-grambank", run.page.locator(".panel-affixes"))
            if [(x or "")[:5] for x in gb.all_text_contents()] != ["GB044", "GB080", "GB083"]:
                failures.append("Affixes: the glosses did not speak for GB044, GB080 and GB083: " + str(gb.all_text_contents()))
            guess = run.page.locator(".panel-affixes .af-guess")
            guess.locator("tbody tr").first.wait_for()
            guess.scroll_into_view_if_needed()
            run.shot("state-affixes-guessed", run.page.locator(".panel-affixes"))
            if not (guess.locator("tbody th").first.text_content() or "").startswith("∅ -la -li"):
                failures.append("Affixes: the endings ∅ -la -li were not guessed from the words")
            run.front("wordorder", "Word order")
            run.page.locator(".panel-wordorder .wo-card").first.wait_for()
            run.shot("state-wordorder", run.page.locator(".panel-wordorder"))
            text = run.page.locator(".panel-wordorder").inner_text()
            if "Use SOV as the answer" not in text or "Too few to tell" not in text:
                failures.append("Word order did not suggest SOV or hold back on too few")
            gb = run.page.locator(".panel-wordorder .wo-grambank", has_text="GB133")
            if gb.count() != 1:
                failures.append("Word order did not offer the Grambank answers SOV gives")
            else:
                gb.click()
                run.page.wait_for_timeout(300)
                if not any("Grambank GB131, GB132, GB133" in t for t in run.toasts()):
                    failures.append(f"Word order did not write the Grambank answers: {run.toasts()}")
                run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # Sound changes: example rules over the lexicon's headwords.
            run.front("soundchanges", "Sound changes")
            run.page.locator(".panel-soundchanges button", has_text="Example rules").click()
            run.page.wait_for_timeout(400)
            run.shot("state-soundchanges", run.page.locator(".panel-soundchanges"))
            if "changed" not in run.page.locator(".panel-soundchanges .sca-line").inner_text():
                failures.append("Sound changes did not run the example rules")
            if run.page.locator(".panel-soundchanges .sca-table tbody tr").count() == 0:
                failures.append("Sound changes listed no changed word")
            # Typed words with their attested forms: how many the rules get right.
            run.page.locator(".panel-soundchanges select.sca-source").select_option("typed")
            run.page.locator(".panel-soundchanges textarea.sca-words").fill("kapa > kab\nsatu > hat\npiti > pid\nmoke > mog")
            run.page.wait_for_timeout(500)
            run.shot("state-soundchanges-expected", run.page.locator(".panel-soundchanges"))
            if "out as expected" not in run.page.locator(".panel-soundchanges .sca-line").inner_text() or run.page.locator(".panel-soundchanges tr.sca-miss").count() == 0:
                failures.append("Sound changes did not check typed words against their expected forms")
            # Over a track: every line of the utterance track through the rules, then written.
            block = run.page.locator(".panel-soundchanges .sca-track-block")
            block.scroll_into_view_if_needed()
            block.locator("button", has_text="Preview").click()
            block.locator("tbody tr").first.wait_for()
            run.shot("state-soundchanges-track", run.page.locator(".panel-soundchanges"))
            if "lines change" not in block.inner_text():
                failures.append("Sound changes did not preview the track")
            block.locator("button", has_text="Write the track").click()
            run.page.locator(".toast", has_text="after the sound changes").wait_for()
            run.page.locator(".panel-soundchanges select.sca-source").select_option("lexicon")
            # Compare word lists: another language's list beside the answers.
            run.front("compare", "Compare word lists")
            run.page.evaluate("window.__WUGS_PICK__ = ['/tmp/kelani-moru.tsv']")
            run.page.locator(".panel-compare button", has_text="Load a word list").click()
            run.page.locator(".panel-compare .cmp-close").first.wait_for()
            run.page.evaluate("window.__WUGS_PICK__ = null")
            if "11 forms of 2 languages" not in " ".join(run.toasts()):
                failures.append("Compare word lists did not report what it read")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # A judgement: click Kelani's word for "I" to mark it cognate.
            run.page.locator(".panel-compare .cmp-table button.cmp-form", has_text="nii").click()
            run.page.wait_for_timeout(200)
            run.shot("state-compare", run.page.locator(".panel-compare"))
            if "Kelani: 2 of 2 judged concepts cognate" not in run.page.locator(".panel-compare .cmp-cognacy").inner_text():
                failures.append("Compare word lists did not count the cognate judgements")
            if "Kelani" not in run.page.locator(".panel-compare .cmp-table thead").inner_text():
                failures.append("Compare word lists did not add a column for the loaded language")
            if "look alike" not in run.page.locator(".panel-compare .cmp-line").inner_text():
                failures.append("Compare word lists did not say how alike the lists are")
            if "Kelani:" not in run.page.locator(".panel-compare .cmp-distance").inner_text():
                failures.append("Compare word lists did not give each language's distance")
            run.page.locator(".panel-compare .cmp-corr-block").scroll_into_view_if_needed()
            run.shot("state-compare-correspondences", run.page.locator(".panel-compare"))
            if "pairs of words at least 50% alike" not in run.page.locator(".panel-compare .cmp-corr-line").inner_text():
                failures.append("Compare word lists did not align the pairs of words")
            if run.page.locator(".panel-compare .cmp-corr tbody tr").count() == 0:
                failures.append("Compare word lists listed no sound correspondence")
            align = run.page.locator(".panel-compare .cmp-align-block")
            align.scroll_into_view_if_needed()
            run.shot("state-compare-alignment", run.page.locator(".panel-compare"))
            if align.locator(".cmp-align tbody tr").count() == 0:
                failures.append("Compare word lists aligned no cognate under the project's word")
            # A tree of the languages, once three more lects are loaded.
            run.page.evaluate("window.__WUGS_PICK__ = ['/tmp/tree-lects.tsv']")
            run.page.locator(".panel-compare button", has_text="Load a word list").click()
            run.page.wait_for_timeout(400)
            run.page.evaluate("window.__WUGS_PICK__ = null")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.page.locator(".panel-compare .cmp-tree button", has_text="Draw a tree").click()
            run.page.locator(".panel-compare .cmp-tree-svg").wait_for()
            run.page.locator(".panel-compare .cmp-tree").scroll_into_view_if_needed()
            run.shot("state-compare-tree", run.page.locator(".panel-compare .cmp-tree"))
            leaves = run.page.locator(".panel-compare .cmp-tree-svg .cmp-leaf").all_text_contents()
            if not {"Rano", "Teva", "Tevu"} <= set(leaves) or "Kelani" in leaves:
                failures.append(f"Compare word lists drew the wrong tree: {leaves} {run.page.locator('.panel-compare .cmp-tree').inner_text()}")
            run.page.locator(".panel-compare .cmp-tree button", has_text="Copy Newick").click()
            run.page.wait_for_timeout(200)
            clip = run.page.evaluate("window.__WUGS_CLIPBOARD__") or {}
            if not clip.get("text", "").startswith("((") or "Rano" not in clip.get("text", ""):
                failures.append(f"Copy Newick did not copy the tree: {clip}")
            # Topics: a grammar topic's sentences.
            run.front("topics", "Topics")
            run.page.locator(".panel-topics .gt-sentences li").first.wait_for()
            run.shot("state-topics", run.page.locator(".panel-topics"))
            if run.page.locator(".panel-topics .gt-topic").count() != 2 or run.page.locator(".panel-topics .gt-sentences li").count() != 2:
                failures.append("Topics did not list its topics and the first one's sentences")
            # Notes: every note in one list, questions only.
            run.front("notes", "Notes")
            run.page.locator(".panel-notes .nt-item").first.wait_for()
            run.shot("state-notes", run.page.locator(".panel-notes"))
            run.page.locator(".panel-notes .nt-questions input").check()
            if run.page.locator(".panel-notes .nt-item").count() != 2:
                failures.append("Notes did not keep only the questions")
            run.page.locator(".panel-notes .nt-where", has_text="tela").click()
            run.page.wait_for_timeout(500)
            if "tela" not in run.page.locator(".panel-lexicon .lx-entry").inner_text():
                failures.append("Notes did not open the entry in the Lexicon")
            # Sounds: the IPA track's sounds on the chart.
            run.front("sounds", "Sounds")
            run.page.locator(".panel-sounds .sd-consonants").wait_for()
            run.shot("state-sounds", run.page.locator(".panel-sounds"))
            if run.page.locator(".panel-sounds .sd-vgroup").count() < 5:
                failures.append("Sounds did not place the vowels")
            if run.page.locator(".panel-sounds .sd-list").count() < 3:
                failures.append("Sounds did not list affricates, clicks or unplaced letters")
            # A made-up word's neighbours: pata, bata, tata, kata are one sound from «gata».
            like = run.page.locator(".panel-sounds input.sd-like-word")
            like.fill("gata")
            like.scroll_into_view_if_needed()
            run.shot("state-sounds-likeness", run.page.locator(".panel-sounds"))
            line = run.page.locator(".panel-sounds .sd-like-line").inner_text()
            if "Not a word of the texts" not in line or run.page.locator(".panel-sounds .sd-like-near button").count() < 4:
                failures.append(f"Word likeness did not find the made-up word's neighbours: {line}")
            together = run.page.locator(".panel-sounds .sd-together")
            together.scroll_into_view_if_needed()
            run.shot("state-sounds-together", run.page.locator(".panel-sounds"))
            if together.locator("tbody tr").count() < 3 or "pairs of a vowel and the next vowel" not in run.page.locator(".panel-sounds .sd-together-note").inner_text():
                failures.append("Sounds together drew no table of vowel pairs")
            run.page.locator(".panel-sounds .sd-bar button", has_text="Copy chart").click()
            run.page.wait_for_timeout(300)
            clip = run.page.evaluate("window.__WUGS_CLIPBOARD__") or {}
            if "<table" not in (clip.get("html") or "") or "Plosive" not in clip.get("text", ""):
                failures.append(f"Sounds did not copy the chart: {clip}")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.page.locator(".panel-sounds .sd-contrast").first.click()
            run.page.locator(".panel-sounds .sd-pairs li").first.scroll_into_view_if_needed()
            run.shot("state-sounds-pairs", run.page.locator(".panel-sounds"))
            if run.page.locator(".panel-sounds .sd-pairs li").count() < 2:
                failures.append("Minimal pairs did not list the pairs of the first contrast")
            run.page.locator(".panel-sounds .sd-bar button", has_text="Environments").click()
            run.page.wait_for_timeout(200)
            run.page.locator(".panel-sounds .sd-env-note").scroll_into_view_if_needed()
            run.shot("state-sounds-environments", run.page.locator(".panel-sounds"))
            if "share" not in run.page.locator(".panel-sounds .sd-env-note").inner_text():
                failures.append("Environments did not say where a minimal pair's sounds both occur")
            if run.page.locator(".panel-sounds .sd-env tbody tr").count() == 0:
                failures.append("Environments listed no environment")
            # Features: +voice −sonorant picks out the voiced obstruents.
            feats = run.page.locator(".panel-sounds .sd-features")
            feats.scroll_into_view_if_needed()
            run.page.locator(".panel-sounds .sd-feat", has_text="voi").click()
            run.page.locator(".panel-sounds .sd-feat", has_text="son").click()
            run.page.locator(".panel-sounds .sd-feat", has_text="son").click()
            run.page.wait_for_timeout(200)
            run.page.locator(".panel-sounds .sd-feat-line").scroll_into_view_if_needed()
            run.shot("state-sounds-features", run.page.locator(".panel-sounds"))
            if "[+voice, −sonorant]" not in run.page.locator(".panel-sounds .sd-feat-line").inner_text():
                failures.append("Features did not pick out [+voice, −sonorant]")
            run.page.locator(".panel-sounds .sd-bar button", has_text="Clear").click()
            allo = run.page.locator(".panel-sounds .sd-h", has_text="Possible allophones")
            allo.scroll_into_view_if_needed()
            run.shot("state-sounds-allophones", run.page.locator(".panel-sounds"))
            if allo.count() != 1:
                failures.append("Sounds did not show Possible allophones")
            # Two sounds never in one environment get a rule to try.
            run.page.locator(".panel-sounds select[aria-label='First sound']").select_option("ɾ")
            run.page.wait_for_timeout(150)
            run.page.locator(".panel-sounds select[aria-label='Second sound']").select_option("l")
            run.page.wait_for_timeout(200)
            run.page.locator(".panel-sounds .sd-rule").scroll_into_view_if_needed()
            run.shot("state-sounds-rule", run.page.locator(".panel-sounds"))
            if "→ [ɾ]" not in run.page.locator(".panel-sounds .sd-rule").inner_text():
                failures.append("Environments did not suggest a rule for ɾ and l")
            clusters = run.page.locator(".panel-sounds .sd-h", has_text="Consonant clusters")
            clusters.locator("xpath=following-sibling::div[1]").scroll_into_view_if_needed()
            run.shot("state-sounds-clusters", run.page.locator(".panel-sounds"))
            if "up to 2 consonants" not in run.page.locator(".panel-sounds").inner_text():
                failures.append("Sounds did not say how many consonants start and end words")
            run.page.locator(".panel-sounds .sd-where >> nth=-1").scroll_into_view_if_needed()
            run.shot("state-sounds-where", run.page.locator(".panel-sounds"))
            if run.page.locator(".panel-sounds .sd-shape").count() < 3:
                failures.append("Sounds did not show word shapes")
            # Tone melodies on a track written with tone marks, then back to IPA.
            run.page.locator(".panel-sounds select[aria-label='Track']").select_option("tier-tone")
            tones = run.page.locator(".panel-sounds .sd-tones")
            tones.wait_for()
            tones.scroll_into_view_if_needed()
            run.shot("state-sounds-tones", run.page.locator(".panel-sounds"))
            tone_note = run.page.locator(".panel-sounds .sd-tone-note").inner_text()
            if "never met: L.H" not in tone_note or tones.locator("tbody tr").count() < 6:
                failures.append(f"Sounds did not read the tone melodies: {tone_note}")
            # Tone apart: á and à count as the vowel a everywhere but the melodies.
            run.page.locator(".panel-sounds label", has_text="Tone apart").locator("input").check()
            run.page.wait_for_timeout(400)
            glyphs = run.page.locator(".panel-sounds .sd-glyph").all_inner_texts()
            if "á" in glyphs or "a" not in glyphs or run.page.locator(".panel-sounds .sd-tones").count() != 1:
                failures.append(f"Tone apart did not read á as a: {glyphs[:12]}")
            run.page.locator(".panel-sounds label", has_text="Tone apart").locator("input").uncheck()
            run.page.wait_for_timeout(300)
            run.page.locator(".panel-sounds select[aria-label='Track']").select_option("tier-ipa")
            run.page.locator(".panel-sounds .sd-h", has_text="Word shapes").wait_for()
            run.page.locator(".panel-sounds button", has_text="Compare with PHOIBLE").click()
            run.page.locator(".panel-sounds .sd-credit").wait_for()
            run.page.locator(".panel-sounds .sd-phoible-note").scroll_into_view_if_needed()
            run.shot("state-sounds-phoible", run.page.locator(".panel-sounds"))
            if "PHOIBLE has 1 inventory of Kelani" not in run.page.locator(".panel-sounds .sd-phoible-note").inner_text():
                failures.append("Sounds did not compare with PHOIBLE")
            # A phoneme chip opens a map of which relatives have it.
            run.page.locator(".panel-sounds .sd-map-chip").first.click()
            run.page.locator(".panel-sounds .tm-map").wait_for()
            run.page.locator(".panel-sounds .tm-wrap").scroll_into_view_if_needed()
            run.shot("state-sounds-phoible-map", run.page.locator(".panel-sounds"))
            if run.page.locator(".panel-sounds .tm-dot").count() != 15:
                failures.append("Sounds: the phoneme map did not place every language")
            run.page.locator(".panel-sounds .sd-map-chip.is-open").click()
            run.page.locator(".panel-sounds .sd-syllable-note").scroll_into_view_if_needed()
            run.shot("state-sounds-syllables", run.page.locator(".panel-sounds"))
            if "consonants before the vowel" not in run.page.locator(".panel-sounds .sd-syllable-note").inner_text():
                failures.append("Sounds did not give the syllable template")
            # Orthography: a practical spelling previewed as IPA.
            run.front("orthography", "Orthography")
            run.page.locator(".panel-orthography .or-grid").wait_for()
            run.page.locator(".panel-orthography button", has_text="Preview").click()
            run.page.locator(".panel-orthography .or-changes").wait_for()
            run.page.locator(".panel-orthography .or-changes").scroll_into_view_if_needed()
            run.shot("state-orthography", run.page.locator(".panel-orthography"))
            if run.page.locator(".panel-orthography .or-changes tbody tr").count() < 2:
                failures.append("Orthography did not preview the conversion")
            # Pronunciations for the lexicon from the profile: counted, then written.
            run.page.locator(".panel-orthography button", has_text="See which entries").click()
            run.page.locator(".panel-orthography button", has_text="Add pronunciations to 41 entries").wait_for()
            run.page.locator(".panel-orthography button", has_text="Add pronunciations to 41 entries").scroll_into_view_if_needed()
            run.shot("state-orthography-pronounce", run.page.locator(".panel-orthography"))
            run.page.locator(".panel-orthography button", has_text="Add pronunciations to 41 entries").click()
            run.page.wait_for_timeout(300)
            if not run.page.evaluate("() => window.__WUGS_PRONOUNCED__"):
                failures.append("Orthography did not write the lexicon's pronunciations")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # Elicit: answers heard in the Timeline's selected segment.
            run.front("elicit", "Elicit")
            run.page.locator(".panel-elicit .el-sound input").check()
            run.page.wait_for_timeout(300)
            run.shot("state-elicit-sound", run.page.locator(".panel-elicit"))
            if run.page.locator(".panel-elicit .el-sound-hint").inner_text().strip() == "":
                failures.append("Sound from the Timeline said nothing about where the sound comes from")
            run.page.locator(".panel-elicit .el-sound input").uncheck()
            # Entries already in the lexicon offered as answers, each to tick or untick.
            run.page.locator(".panel-elicit button", has_text="From the lexicon").click()
            run.page.locator(".panel-elicit .el-matches").wait_for()
            run.shot("state-elicit-from-lexicon", run.page.locator(".panel-elicit"))
            run.page.locator(".panel-elicit .el-match-rows input").nth(1).uncheck()
            run.page.locator(".panel-elicit button", has_text="Link the ticked entry").click()
            run.page.wait_for_timeout(300)
            linked = run.page.evaluate("() => window.__WUGS_LINKED__") or []
            if linked != [["sense-lx-kulit", "con-3"]] or run.page.locator(".panel-elicit .el-matches").count() != 0:
                failures.append(f"Elicit: From the lexicon did not link only the ticked entry: {linked}")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # Adding a word list offers a table of one's own beside the bundled lists.
            run.page.locator(".panel-elicit button", has_text="Add a word list").click()
            run.page.locator(".panel-elicit button", has_text="Load a table").wait_for()
            run.shot("state-elicit-add", run.page.locator(".panel-elicit"))
            run.page.locator(".panel-elicit button", has_text="Close").click()
            # The Project menu lists folders opened lately, but not the one open now.
            run.page.evaluate("localStorage.setItem('wugs.recentProjects', JSON.stringify(['/home/ana/Kanu fieldwork 2025', '/home/ana/Pilot recordings']))")
            run.page.locator(".project-menu").click()
            run.page.locator(".menu-item", has_text="Pilot recordings").wait_for()
            run.shot("state-project-recent", run.page.locator(".menu-list").first)
            run.page.keyboard.press("Escape")
            # Transcribe: the project's typing shortcuts.
            run.front("transcribe", "Transcribe")
            # The selected line marked on a page image shows its crop above the field.
            run.page.evaluate("async () => (await import('/src/store.ts')).selectSegment({ textId: 't-count', tierId: 'tier-utt', segmentId: 'u02' })")
            run.page.locator(".panel-transcribe .tx-crop").wait_for()
            run.shot("state-transcribe-crop", run.page.locator(".panel-transcribe"))
            run.page.locator(".panel-transcribe button", has_text="Shortcuts").click()
            run.page.locator(".panel-transcribe .tx-shortcuts textarea").wait_for()
            run.shot("state-transcribe-shortcuts", run.page.locator(".panel-transcribe"))
            if "3 typing shortcuts on" not in run.page.locator(".panel-transcribe .tx-shortcuts").inner_text():
                failures.append("The Shortcuts box did not count the project's shortcuts")
            run.page.evaluate("window.__WUGS_PICK__ = '/tmp/keyboards'")
            run.page.locator(".panel-transcribe .tx-shortcuts button", has_text="Keyman").click()
            run.page.wait_for_timeout(400)
            run.page.evaluate("window.__WUGS_PICK__ = null")
            saved = run.page.evaluate("window.__WUGS_SAVED__") or {}
            if saved.get("extension") != "kmn" or "U+003B + 'n' > U+014B" not in saved.get("contents", "") or saved.get("title") != "wugs_kanu":
                failures.append(f"Make a Keyman keyboard did not save the shortcuts as a .kmn: {str(saved)[:200]}")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.page.locator(".panel-transcribe button", has_text="Shortcuts").click()
            # Word completion: the commonest project word the typed start begins.
            tx = run.page.locator(".panel-transcribe .tx-field")
            tx.click()
            tx.press("End")
            tx.type(" kanu")
            run.page.locator(".panel-transcribe .tx-complete").wait_for()
            run.shot("state-transcribe-complete", run.page.locator(".panel-transcribe"))
            tx.press("ArrowRight")
            if not tx.input_value().endswith(" kanula"):
                failures.append(f"→ did not take the completion: {tx.input_value()}")
            tx.press("Escape")
            run.front("elicit", "Elicit")
            # Semantic domains: how many words each holds, and the next one still empty.
            run.page.locator(".panel-elicit button", has_text="Semantic domains").click()
            run.page.locator(".panel-elicit .el-coverage").wait_for()
            run.shot("state-elicit-domains", run.page.locator(".panel-elicit"))
            if "9 words in 2 of 7 domains" not in run.page.locator(".panel-elicit .el-coverage").inner_text():
                failures.append("Semantic domains did not say how many domains hold words")
            run.page.locator(".panel-elicit button", has_text="Word lists").click()
            # The entry list can show only the entries with work left, with counts.
            run.front("lexicon", "Lexicon")
            run.page.locator(".panel-lexicon .lx-show").select_option("gloss")
            run.page.wait_for_timeout(200)
            run.shot("state-lexicon-todo", run.page.locator(".panel-lexicon"))
            if run.page.locator(".panel-lexicon .lx-list li").count() != 1:
                failures.append("Needs a gloss did not list the one entry without a gloss")
            # Entries still to record, matched one for one with a recording's segments.
            run.page.locator(".panel-lexicon .lx-show").select_option("sound")
            run.page.locator(".panel-lexicon button", has_text="Match to a recording").click()
            run.page.locator(".panel-lexicon .lx-match-bar select").first.select_option("t-count")
            run.page.locator(".panel-lexicon table.lx-match tbody tr").first.wait_for()
            run.shot("state-lexicon-match", run.page.locator(".panel-lexicon"))
            if "words and" not in run.page.locator(".panel-lexicon .lx-entry").inner_text():
                failures.append("Match to a recording did not say how the words and segments line up")
            run.page.locator(".panel-lexicon .lx-entry button", has_text="Close").click()
            run.page.locator(".panel-lexicon .lx-show").select_option("pos:num")
            run.page.wait_for_timeout(200)
            # The listed entries as a checklist to print.
            run.page.evaluate("window.__WUGS_PICK__ = '/tmp/lists'")
            run.page.locator(".panel-lexicon .lx-print").click()
            run.page.wait_for_timeout(400)
            run.page.evaluate("window.__WUGS_PICK__ = null")
            sheet = (run.page.evaluate("window.__WUGS_SAVED__") or {}).get("contents", "")
            if "entries to check" not in sheet or sheet.count('<td class="ok">') != 7:
                failures.append("Lexicon did not print the listed entries as a checklist")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            if run.page.locator(".panel-lexicon .lx-list li").count() != 7:
                failures.append("the num category did not list its seven entries")
            # Loanwords, with where they come from.
            run.page.locator(".panel-lexicon .lx-show").select_option("loans")
            run.page.wait_for_timeout(200)
            run.shot("state-lexicon-loans", run.page.locator(".panel-lexicon"))
            if "1 loanword" not in run.page.locator(".panel-lexicon .lx-loans").inner_text() or "Malay 1" not in run.page.locator(".panel-lexicon .lx-loans").inner_text():
                failures.append("Lexicon did not list the loanwords with their source")
            run.page.locator(".panel-lexicon .lx-show").select_option("all")
            # Notes on the entry and on each meaning.
            run.page.locator(".panel-lexicon").get_by_text("kani", exact=True).first.click()
            notes = run.page.locator(".panel-lexicon textarea[data-key='notes:lx-kani']")
            notes.wait_for()
            notes.scroll_into_view_if_needed()
            run.shot("state-lexicon-notes", run.page.locator(".panel-lexicon .lx-entry"))
            if "Ask Mosa" not in notes.input_value():
                failures.append("The entry's notes were not shown")
            if "Moru kaŋi" not in run.page.locator(".panel-lexicon textarea[data-key='et:lx-kani']").input_value():
                failures.append("The entry's etymology was not shown")
            if run.page.locator(".panel-lexicon textarea[data-key='nsen:sense-kani-2']").input_value() != "Only heard from younger speakers.":
                failures.append("A sense's notes were not shown")
            # Two entries share a lemma: number them, or merge the other into this one.
            run.page.locator(".panel-lexicon .lx-list li", has_text="river").first.click()
            run.page.locator(".panel-lexicon .gl-notice button", has_text="Merge").first.wait_for()
            run.shot("state-lexicon-twins", run.page.locator(".panel-lexicon .lx-head"))
            # Paradigm templates of the project's own: pick one, then open the form for a new one.
            run.front("lexicon", "Lexicon")
            run.page.locator(".panel-lexicon").get_by_text("kani", exact=True).first.click()
            pdg = run.page.locator(".panel-lexicon .lx-pdg-head select")
            pdg.wait_for()
            pdg.select_option("noun-class")
            run.page.locator(".panel-lexicon .lx-pdg th", has_text="singular").first.wait_for()
            run.page.locator(".panel-lexicon .lx-pdg-head button", has_text="Copy table").click()
            run.page.wait_for_timeout(300)
            clip = run.page.evaluate("window.__WUGS_CLIPBOARD__")
            if not clip or "<table" not in (clip.get("html") or "") or "\t" not in clip.get("text", ""):
                failures.append(f"Copy table did not put the paradigm on the clipboard: {clip}")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.shot("state-paradigm-own", run.page.locator(".panel-lexicon .lx-paradigm"))
            pdg.select_option(label="New template…")
            run.page.locator(".panel-lexicon .lx-pdg-template").wait_for()
            run.page.locator('.panel-lexicon [data-key="pdg-form:name"]').fill("Case × number")
            run.page.locator('.panel-lexicon [data-key="pdg-form:rows"]').fill("Case")
            run.page.locator('.panel-lexicon [data-key="pdg-form:rowValues"]').fill("nom=nominative, acc=accusative, gen=genitive")
            run.shot("state-paradigm-form", run.page.locator(".panel-lexicon .lx-paradigm"))
            run.page.locator(".panel-lexicon .lx-pdg-template button", has_text="Add template").click()
            run.page.locator(".panel-lexicon .lx-pdg th", has_text="genitive").first.wait_for()
            if run.page.locator(".panel-lexicon .lx-pdg-template").count() != 0:
                failures.append("the paradigm template form stayed open after Add template")
            run.page.locator(".panel-lexicon .lx-pdg-head select").select_option("person-number")
            # The entry's forms in the texts, under the grid.
            run.page.locator(".panel-lexicon .lx-pdg-attested button", has_text="Forms in the texts").click()
            run.page.locator(".panel-lexicon .lx-pdg-attested li").first.wait_for()
            run.page.locator(".panel-lexicon .lx-pdg-attested").scroll_into_view_if_needed()
            run.shot("state-paradigm-attested", run.page.locator(".panel-lexicon .lx-paradigm"))
            if "nikani" not in run.page.locator(".panel-lexicon .lx-pdg-attested").inner_text():
                failures.append("Forms in the texts did not list the entry's forms")
            # Sentence: a tagger's analysis stays marked until checked; Checked, next moves on.
            run.front("sentence", "Sentence")
            run.page.locator(".panel-sentence .st-machine").wait_for()
            run.shot("state-sentence-check", run.page.locator(".panel-sentence"))
            run.page.locator(".panel-sentence .st-machine button", has_text="Checked, next").click()
            run.page.wait_for_timeout(600)
            if run.page.locator(".panel-sentence .st-check", has_text="3 of 4 tagged sentences").count() != 1:
                failures.append("Checked, next did not count the sentence as checked")
            # The tree copied for LaTeX and saved as a picture.
            run.page.locator(".panel-sentence .st-export button", has_text="Copy for LaTeX").click()
            run.page.wait_for_timeout(300)
            clip = run.page.evaluate("window.__WUGS_CLIPBOARD__") or {}
            if "\\begin{dependency}" not in clip.get("text", "") or "\\deproot" not in clip.get("text", ""):
                failures.append(f"Copy for LaTeX did not copy a tikz-dependency figure: {clip}")
            run.page.evaluate("window.__WUGS_PICK__ = '/tmp/trees'")
            run.page.locator(".panel-sentence .st-export button", has_text="Save picture").click()
            run.page.wait_for_timeout(400)
            run.page.evaluate("window.__WUGS_PICK__ = null")
            saved = run.page.evaluate("window.__WUGS_SAVED__") or {}
            if saved.get("extension") != "svg" or "<svg" not in saved.get("contents", "") or "fill=" not in saved.get("contents", ""):
                failures.append(f"Save picture did not save the tree as an SVG: {str(saved)[:200]}")
            run.shot("state-sentence-export", run.page.locator(".panel-sentence"))
            run.front("interlinear", "Interlinear")
            # The abbreviations the text's glosses use, with Leipzig meanings and the project's own.
            run.page.locator(".panel-interlinear .il-bar button", has_text="Abbreviations").click()
            run.page.wait_for_timeout(300)
            run.shot("state-interlinear-abbreviations", run.page.locator(".panel-interlinear"))
            if run.page.locator(".panel-interlinear .il-abbr-row").count() == 0:
                failures.append("the Abbreviations list showed none")
            run.page.locator(".panel-interlinear .il-bar button", has_text="Abbreviations").click()

            # Media bin (roadmap 66): a card dragged onto the Timeline is used for the text on screen.
            run.front("media", "Media")
            fox = run.page.locator(".panel-media .bin-card", has_text="Fox story")
            fox.wait_for()
            # A recording's details for the archive.
            run.page.locator(".panel-media .bin-card", has_text="Counting session").locator(".bin-details-open").click()
            rb = run.page.locator(".panel-media .bin-detail[data-key='recorded_by']")
            rb.fill("J. Cutler")
            rb.press("Enter")
            run.page.locator(".panel-media .bin-details-head h3").click()
            run.page.wait_for_timeout(300)
            run.page.locator(".panel-media .bin-details").scroll_into_view_if_needed()
            run.shot("state-media-details", run.page.locator(".panel-media"))
            if "Date, place and 2 more" not in run.page.locator(".panel-media .bin-card", has_text="Counting session").inner_text():
                failures.append("Media did not keep a recording's detail")
            run.page.locator(".panel-media .bin-details-head button", has_text="Close").click()
            # Details read from the file names by a pattern.
            run.page.locator(".panel-media .bin-from-names summary").click()
            run.page.locator(".panel-media .bin-pattern").fill("{genre} ({recorded_by})")
            run.page.wait_for_timeout(200)
            run.page.locator(".panel-media .bin-from-names").scroll_into_view_if_needed()
            run.shot("state-media-names", run.page.locator(".panel-media"))
            names_note = run.page.locator(".panel-media .bin-from-names .note").nth(1).inner_text()
            run.page.locator(".panel-media .bin-from-names button", has_text="Fill details").click()
            run.page.wait_for_timeout(400)
            if "1 of 2 file names fit" not in names_note or not any("Filled 2 details" in t for t in run.toasts()):
                failures.append(f"Media did not fill details from the file names: {names_note} {run.toasts()}")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            run.page.locator(".panel-media .bin-pattern").fill("{date}_{participants}_{genre}")
            run.page.locator(".panel-media .bin-from-names summary").click()
            # Recording levels: peak, speech, room and clips per recording.
            run.page.locator(".panel-media .bin-levels-box summary").click()
            run.page.locator(".panel-media .bin-levels-box button", has_text="Check levels").click()
            run.page.locator(".panel-media .bin-levels tbody tr").nth(1).wait_for()
            run.page.locator(".panel-media .bin-levels-box").scroll_into_view_if_needed()
            run.shot("state-media-levels", run.page.locator(".panel-media"))
            levels_text = run.page.locator(".panel-media .bin-levels-box").inner_text()
            if "1 of 2 recordings have something to look at" not in levels_text or "Clips 4 times, first at 0:07" not in levels_text:
                failures.append(f"Media did not read the recording levels: {levels_text[:300]}")
            run.page.locator(".panel-media .bin-levels-box summary").click()
            fox.drag_to(run.page.locator(".panel-timeline .tl-body"))
            run.page.wait_for_timeout(600)
            if run.page.locator(".tl-sources").input_value() != "src-fox":
                failures.append("dragging a media card onto the Timeline did not load it")
            run.page.locator(".tl-sources").select_option("src-count")
            run.page.wait_for_timeout(400)
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")

            # Speech assist: every transcribed segment exported as a speech dataset.
            run.front("assist", "Speech assist")
            ds = run.page.locator(".panel-assist .as-dataset")
            ds.wait_for()
            ds.scroll_into_view_if_needed()
            run.shot("state-assist-dataset", run.page.locator(".panel-assist"))
            if "Export 40 clips" not in ds.inner_text():
                failures.append(f"Speech assist did not offer the speech dataset: {ds.inner_text()}")
            else:
                run.page.evaluate("window.__WUGS_PICK__ = '/tmp/speech'")
                ds.click()
                run.page.wait_for_timeout(400)
                run.page.evaluate("window.__WUGS_PICK__ = null")
                if not any("Exported 40 clips" in t for t in run.toasts()):
                    failures.append(f"Speech assist did not export the dataset: {run.toasts()}")
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
            # The Texts panel's publishing buttons are found by name too.
            run.page.keyboard.press("Control+k")
            run.page.wait_for_timeout(200)
            run.page.keyboard.type("spell checker")
            run.page.wait_for_timeout(150)
            if run.page.locator(".palette-row", has_text="Save a spelling dictionary").count() == 0:
                failures.append("the palette did not find the spelling dictionary")
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

            # Tap to segment: Enter while playing marks a start, then makes a segment.
            run.front("timeline", "Timeline")
            run.page.evaluate("window.__WUGS_CALLS__.length = 0")
            run.page.locator(".panel-timeline .tl-wrap").focus()
            run.page.keyboard.press("Home")
            run.page.keyboard.press("Space")
            run.page.wait_for_timeout(500)
            if run.page.evaluate("async () => (await import('/src/playback.ts')).isPlaying()"):
                run.page.keyboard.press("Enter")
                run.page.wait_for_timeout(900)
                run.page.keyboard.press("Enter")
                run.page.wait_for_timeout(300)
                run.page.keyboard.press("Space")
                if "add_segment" not in run.page.evaluate("window.__WUGS_CALLS__"):
                    failures.append("Tap to segment: two presses of Enter while playing made no segment")
            else:
                print("  (headless playback did not start; tap to segment not checked)")
            run.page.keyboard.press("Escape")
            run.page.evaluate("async () => (await import('/src/store.ts')).showToast(null)")
            # Help behind another tab, so pressing Help makes the wug jump.
            run.front("timeline", "Timeline")
            run.page.get_by_role("button", name="Help", exact=True).click()
            run.page.wait_for_timeout(280)
            run.shot("state-help-jump")
            run.page.wait_for_timeout(700)
            run.page.locator(".help-panel .help-question").fill("How do I split a segment at the playhead?")
            run.page.get_by_role("button", name="Ask", exact=True).click()
            run.page.locator(".help-panel .help-src").wait_for(timeout=10_000)
            run.page.locator(".help-panel .help-question").fill("And undo it?")
            run.shot("state-help")

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
