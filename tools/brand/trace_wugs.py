"""Trace the wug layer drawings into the vector parts the app draws.

Usage (from the repo root):
    python3 -I tools/brand/trace_wugs.py

Reads design/wugs/layers/NN-<part>.png (Procreate layers, numbered bottom to
top, each exported over a white background) and writes:
    apps/desktop/src/theme/wug-parts.json   every part's path, in stacking order
    apps/desktop/public/brand/logo.svg      the plain wug for the top bar
    design/wugs/icon.svg, icon.png           the app icon source (npx tauri icon)

Needs rsvg-convert (librsvg) and: pip install potracer==0.0.4 pillow scipy numpy
(the committed output was made with numpy 2.5.3, scipy 1.18.1, Pillow 12.3.0;
other versions can move paths by a fraction of a pixel). `potracer` installs
as the module `potrace`, as does the unrelated `pypotrace`: install only one.
"""
import json
import re
import subprocess
from pathlib import Path

import numpy as np
import potrace
from PIL import Image
from scipy import ndimage

ROOT = Path(__file__).resolve().parents[2]
LAYERS = ROOT / "design" / "wugs" / "layers"
SIZE = 512
SCALE = 4  # trace at 2048 px for smooth curves, write paths in 512 units

# Fills drawn in white vanish into the export's white background; they are
# rebuilt from the inside of their own outline.
DERIVED_FROM = {
    "paper-shape": "paper-line",
    "monocle-shape": "monocle-line",
    "glasses-shape": "glasses-line",
    "spacesuit-mask-shape": "spacesuit-line",
}
# Layer opacity (Procreate keeps it out of the PNG export): the owner set the
# see-through parts at 35%.
OPACITY = {"monocle-shape": 0.35, "glasses-shape": 0.35, "spacesuit-mask-shape": 0.35}
BRAND_BLUE = "#00c1e3"
CREAM = "#fff8de"


def read(name):
    return np.asarray(Image.open(name).convert("RGB")).astype(np.float32)


def coverage(rgb):
    """0..1 ink: distance from white, scaled by the layer's own strongest ink."""
    dist = 255.0 - rgb.min(axis=2)
    strong = dist[dist > 60]
    full = np.percentile(strong, 90) if strong.size else 255.0
    return np.clip(dist / max(full, 1.0), 0.0, 1.0)


def ink_colour(rgb):
    dist = 255.0 - rgb.min(axis=2)
    if not (dist > 60).any():
        return "#ffffff"
    core = rgb[dist >= np.percentile(dist[dist > 60], 50)]
    r, g, b = np.median(core, axis=0).astype(int)
    return f"#{r:02x}{g:02x}{b:02x}"


def to_bitmap(cov):
    big = Image.fromarray((cov * 255).astype(np.uint8)).resize((SIZE * SCALE,) * 2, Image.BICUBIC)
    big = ndimage.gaussian_filter(np.asarray(big).astype(np.float32) / 255, sigma=SCALE * 0.6)
    return big > 0.5


def trace(bitmap):
    # potracer fills the dark pixels, so ink goes in as False.
    path = potrace.Bitmap(~bitmap).trace(turdsize=SCALE * SCALE * 6, alphamax=1.25, opticurve=True, opttolerance=0.4)
    f = lambda p: f"{p.x / SCALE:.1f} {p.y / SCALE:.1f}"
    out = []
    for curve in path:
        out.append(f"M{f(curve.start_point)}")
        for seg in curve.segments:
            out.append(f"L{f(seg.c)}L{f(seg.end_point)}" if seg.is_corner else f"C{f(seg.c1)} {f(seg.c2)} {f(seg.end_point)}")
        out.append("Z")
    return "".join(out)


def derived_fill(line_cov, cut=None):
    """The inside of a closed outline, reaching under the stroke."""
    inside = ndimage.binary_fill_holes(line_cov > 0.5)
    fill = ndimage.binary_erosion(inside, iterations=3)
    if cut is not None:
        fill &= ~cut
    return fill.astype(np.float32)


def own_line(shape):
    return re.sub(r"(-base|-mask)?-shape$", "-line", shape)


def tuck_under(fill_cov, line_cov):
    """Grow a hand-painted fill under its outline, so no gap shows between
    them, but never past the outline's outer edge."""
    if line_cov is None:
        return fill_cov
    filled = fill_cov > 0.5
    reach = ndimage.binary_fill_holes(line_cov > 0.5)
    grown = ndimage.binary_dilation(filled, iterations=5) & reach
    return np.maximum(fill_cov, grown.astype(np.float32))


def svg(paths, background=None):
    bg = f'<rect width="{SIZE}" height="{SIZE}" fill="{background}"/>' if background else ""
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}">{bg}{paths}</svg>\n'


def path_el(part, fill=None):
    op = f' fill-opacity="{part["opacity"]}"' if part["opacity"] != 1 else ""
    return f'<path fill="{fill or part["fill"]}"{op} d="{part["d"]}"/>'


def main():
    files = sorted(LAYERS.glob("*.png"), key=lambda f: int(f.stem.split("-", 1)[0]))
    names = {re.sub(r"^\d+-", "", f.stem): f for f in files}
    cov = {n: coverage(read(f)) for n, f in names.items()}
    parts = []
    for name, f in names.items():
        kind = "line" if name.endswith("-line") else "shape"
        if name in DERIVED_FROM:
            cut = cov["spacesuit-base-shape"] > 0.3 if name == "spacesuit-mask-shape" else None
            c, fill = derived_fill(cov[DERIVED_FROM[name]], cut), "#ffffff"
        else:
            c = cov[name]
            fill = "#000000" if kind == "line" else ink_colour(read(f))
            if kind == "shape":
                c = tuck_under(c, cov.get(own_line(name)))
        d = trace(to_bitmap(c))
        assert d, f"{name} traced to nothing"
        parts.append({"id": name, "kind": kind, "fill": fill, "opacity": OPACITY.get(name, 1), "d": d})

    theme = ROOT / "apps" / "desktop" / "src" / "theme"
    (theme / "wug-parts.json").write_text(json.dumps({"size": SIZE, "parts": parts}, separators=(",", ":")) + "\n")
    by = {p["id"]: p for p in parts}
    plain = path_el(by["body-shape"], BRAND_BLUE) + "".join(path_el(by[i]) for i in ("body-line", "eye-shape"))
    ys, xs = np.nonzero((cov["body-line"] > 0.3) | (cov["body-shape"] > 0.3))
    pad = 6
    box = f"{xs.min() - pad} {ys.min() - pad} {xs.max() - xs.min() + 2 * pad} {ys.max() - ys.min() + 2 * pad}"
    (ROOT / "apps" / "desktop" / "public" / "brand" / "logo.svg").write_text(
        svg(plain).replace(f'viewBox="0 0 {SIZE} {SIZE}"', f'viewBox="{box}"'))
    # The icon: the wug on a cream tile with a thick black rim, as drawn in the brand sheet.
    tile = (f'<rect x="20" y="20" width="472" height="472" rx="104" fill="{CREAM}" stroke="#000" stroke-width="28"/>'
            f'<g transform="translate(24 -16) scale(0.92)">{plain}</g>')
    icon = ROOT / "design" / "wugs" / "icon.svg"
    icon.write_text(svg(tile))
    subprocess.run(["rsvg-convert", "-w", "1024", "-h", "1024", str(icon), "-o", str(icon.with_suffix(".png"))], check=True)


if __name__ == "__main__":
    main()
