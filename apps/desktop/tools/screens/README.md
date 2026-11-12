# Screenshot harness

Runs the real desktop frontend in headless Chrome with the Tauri backend replaced by fixtures, and saves a PNG of every screen. Use it to check styling changes without building the Tauri app or having a display.

## Run it

From the repository root:

```sh
python3 apps/desktop/tools/screens/shoot.py                 # every screen into /tmp/screens/
python3 apps/desktop/tools/screens/shoot.py --out /tmp/a    # another folder
python3 apps/desktop/tools/screens/shoot.py --only gloss --only toast
python3 apps/desktop/tools/screens/shoot.py --port 1430      # beside a dev server of another checkout
```

`--only` keeps the screenshots whose file name contains the text. You can give it more than once. The script starts `npx vite --port 1420 --strictPort` in `apps/desktop`, or reuses a dev server already on that port, whichever checkout it serves. Give `--port` another number when a dev server of another worktree holds 1420. It needs Python Playwright and Chrome at `/usr/bin/google-chrome`.

## What it writes

| File | What it shows |
| --- | --- |
| `00-no-project.png` | The first start, with no project open. |
| `01-workspace.png` | The default Tongues layout, with «Counting to ten» open, its recording loaded and the first segment selected. |
| `NN-<panel>.png` | The whole window with that panel brought to the front, one for each panel in `src/panels/registry.ts`. |
| `zoom-<panel>.png` | Only that panel, at the size the layout gives it. |
| `stage-<panel>.png` | Only that panel, at 1200 × 800. The panel's element is moved onto a stage for the shot and then put back. |
| `state-toast.png` | A warning toast with an Undo action. |

Before the shots, the Lexicon panel opens the entry «kani» and the Search panel searches for «kesi».

The run fails, with a non-zero exit code, if the page throws an error, logs a console error, or shows a toast after a panel opens. At the end it prints every backend command that has no fixture.

## What is mocked

`mock-tauri.js` defines `window.__TAURI_INTERNALS__` before the app loads. This is the object `@tauri-apps/api` calls.

- `invoke` answers from the table in `fixtures.js`. A command with no fixture logs a warning, returns `null` and goes on the unknown list.
- `plugin:event|listen`, `unlisten` and `emit` work in memory. To send the app an event from a test, call `window.__WUGS_EMIT__(name, payload)`.
- `plugin:window|*` returns `null`. File dialogs (`plugin:dialog|open` and `save`) return `null`, which the app treats as a cancelled dialog.
- `convertFileSrc` points at `/__mock_asset/…`. `shoot.py` answers that path with 20 seconds of silent WAV.
- Before each page load, localStorage is cleared. The researcher id `demo`, a second gloss language (`es`) and update checks off are set. The last project path is set too, so the app reopens the project at start. The no-project shot leaves out the project path.

The harness selects the text and the segment through the app's own `src/store.ts`, which it imports in the page. It brings a panel to the front by sending a pointer event to its dock tab. Both work while the tabs are hidden or restyled.

The top bar's menus are the app's own (`src/menu.ts`), so an open menu could be shot; the harness does not take one yet.

## The data is synthetic

The project, the language («Tokari»: `nikani ŋai toɾuma`), the people, the devices, the keys and the waveform are all made up. The waveform comes from a formula. The concept list glosses are the Swadesh 100 list, from the Concepticon data the app already includes. Do not add real field data, recordings or people to the fixtures, because this repository is public.

When you add a backend command to `src/api.ts`, add a fixture in `fixtures.js` with the same shape. Otherwise the run lists the command as unknown.

If a run times out waiting for the timeline after you edited `src/store.ts`, restart the Vite server it reuses (port 1420). Vite's hot reload gives the edited module a new URL, so the harness's `import('/src/store.ts')` gets a second copy of the store that the page never renders.
