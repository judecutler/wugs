# The wugs

The owner's own wug drawings, made in Procreate, one PNG per layer in
`layers/`, numbered bottom to top as they stack (`NN-<part>.png`). Every
layer was exported over a white background, so the white fills (paper,
monocle and glasses lenses, the helmet visor) are rebuilt from the inside of
their outlines when traced; the see-through parts are drawn at 35% opacity,
as set in Procreate.

`python3 tools/brand/trace_wugs.py` (needs `rsvg-convert` and `pip install
potracer pillow scipy numpy`; the script's header records the versions used)
traces them into:

- `apps/desktop/src/theme/wug-parts.json`: every part as a vector path, which
  `src/theme/wug.ts` stacks into a wug with any body colour and accessories;
- `apps/desktop/public/brand/logo.svg`: the plain wug in brand blue `#00C1E3`;
- `icon.svg` and `icon.png` here: the app icon source. Make the platform
  icons from it with `cd apps/desktop && npx tauri icon ../../design/wugs/icon.png -o src-tauri/icons`,
  then keep only `icon.png`, `icon.ico` and `icon.icns`, and run
  `python3 tools/brand/windows_icon.py` last: it replaces `icon.ico` with the
  taskbar-sized Windows icon (the wug alone, lines thickened for 16–48 px).

To add an accessory: draw its line and fill as separate layers on the same
512×512 canvas, hide the background layer, export, add the files here with a
number that puts them in the right place in the stack, rerun the script, and
list the parts under a new entry in `ACCESSORIES` in `src/theme/wug.ts`.

## The wordmark

The full logo on the brand sheet (WUG_1.pdf, page 1) sets "WUG" in FreeSerif
outlines with the wug as the S. `tools/brand/wordmark.py` lifts those outlines
from the sheet and adds the traced wug, writing `site/img/wordmark.svg`
(instructions in the script).
