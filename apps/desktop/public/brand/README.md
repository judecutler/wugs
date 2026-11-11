# Brand assets

Drop the wugs logo files here; the app picks them up with no code change.

| File | Used for | Notes |
| --- | --- | --- |
| `logo.svg` | the top bar, left of the kit name | SVG, drawn on the light bar (`--panel`); shown at `--logo-height` (20 px), width follows the drawing. Until this file exists the top bar shows the kit name alone. |

App icons (the window, taskbar, installer and macOS dock) are generated from
one square image, at least 1024 × 1024 px, PNG or SVG:

    cd apps/desktop
    npx tauri icon path/to/icon.png

That rewrites `src-tauri/icons/` (`icon.ico`, `icon.icns`, `icon.png` and the
sizes Tauri needs); commit the result.

Colours, fonts and corner radii live in `src/theme/tokens.css`. Change a value
there and every panel, the timeline canvas and the recording meter follow.
