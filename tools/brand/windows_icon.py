"""Build the Windows icon (taskbar, title bar, Start menu, installer).

The tiled icon (design/wugs/icon.svg) leaves only a few pixels of wug at
16-32 px and its cream tile disappears on a light taskbar, so Windows gets the
wug alone, filling the square, with its lines thickened for small sizes
(owner, 2026-11-12: "you can't see the logo in the Windows taskbar").
macOS and Linux keep the tile.

    python3 tools/brand/windows_icon.py

Needs rsvg-convert and Pillow. Writes design/wugs/icon-windows.svg and
apps/desktop/src-tauri/icons/icon.ico.
"""
import io
import re
import subprocess
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SIZES = (16, 20, 24, 32, 40, 48, 64, 256)


def main() -> None:
    logo = (ROOT / "apps/desktop/public/brand/logo.svg").read_text()
    x, y, w, h = map(float, re.search(r'viewBox="([^"]+)"', logo).group(1).split())
    inner = re.search(r"<svg[^>]*>(.*)</svg>", logo, re.S).group(1)
    # Heavier lines: the drawn line shapes get an outline of their own colour.
    inner = re.sub(r'<path fill="#000(?:000)?"', '<path fill="#000" stroke="#000" stroke-width="14" stroke-linejoin="round"', inner)
    side = max(w, h) * 1.08
    cx, cy = x + w / 2, y + h / 2
    svg = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{cx - side / 2:.1f} {cy - side / 2:.1f} {side:.1f} {side:.1f}">{inner}</svg>\n'
    src = ROOT / "design/wugs/icon-windows.svg"
    src.write_text(svg)
    frames = []
    for px in SIZES:
        png = subprocess.run(["rsvg-convert", "-w", str(px), "-h", str(px), str(src)], check=True, capture_output=True).stdout
        frames.append(Image.open(io.BytesIO(png)).convert("RGBA"))
    big = frames[-1]
    big.save(ROOT / "apps/desktop/src-tauri/icons/icon.ico", format="ICO", sizes=[(f.width, f.height) for f in frames], append_images=frames[:-1])


if __name__ == "__main__":
    main()
