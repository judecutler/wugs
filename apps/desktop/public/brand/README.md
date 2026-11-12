# Brand assets

Drop the wugs logo files here; the app picks them up with no code change.

| File | Used for | Notes |
| --- | --- | --- |
| `logo.svg` | the top bar, left of the kit name | the plain wug in brand blue, written by `tools/brand/trace_wugs.py` from the drawings in `design/wugs/`; shown at `--logo-height` (28 px), width follows the drawing. Without this file the top bar shows the kit name alone. |

App icons (the window, taskbar, installer and macOS dock) are generated from
`design/wugs/icon.png` (1024 × 1024, written by the same script):

    cd apps/desktop
    npx tauri icon ../../design/wugs/icon.png -o src-tauri/icons

Keep `icon.ico`, `icon.icns` and `icon.png` (the files `tauri.conf.json` names)
and delete the other sizes it writes; commit the result.

Colours, fonts and corner radii live in `src/theme/tokens.css`. Change a value
there and every panel, the timeline canvas and the recording meter follow.

Wug characters (each kit's wug, accessories, recolouring) are drawn from
vector parts in `src/theme/wug-parts.json` by `src/theme/wug.ts`; see
`design/wugs/README.md`.
