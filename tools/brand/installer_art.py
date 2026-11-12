"""Installer artwork from the brand wug: the Windows installer's header and
sidebar (NSIS wants 24-bit BMPs, 150x57 and 164x314) and the macOS disk
image background (660x400). Run from the repo root after trace_wugs.py:

    python3 tools/brand/installer_art.py

Needs rsvg-convert and Pillow.
"""
import io
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
LOGO = ROOT / "apps" / "desktop" / "public" / "brand" / "logo.svg"
OUT = ROOT / "apps" / "desktop" / "src-tauri" / "installer"
CREAM = (0xFF, 0xF8, 0xDE)
INK = (0x1B, 0x1A, 0x17)


def wug(height):
    png = subprocess.run(["rsvg-convert", "-h", str(height), str(LOGO)], check=True, capture_output=True).stdout
    return Image.open(io.BytesIO(png)).convert("RGBA")


def canvas(w, h):
    return Image.new("RGBA", (w, h), CREAM + (255,))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    # Header: the wug at the right edge, as NSIS draws its title on the left.
    head = canvas(150, 57)
    w = wug(45)
    head.alpha_composite(w, (150 - w.width - 8, 6))
    head.convert("RGB").save(OUT / "nsis-header.bmp")
    # Sidebar: a big wug walking in from the bottom, a rule of ink at the edge.
    side = canvas(164, 314)
    w = wug(150)
    side.alpha_composite(w, ((164 - w.width) // 2, 314 - w.height - 36))
    ImageDraw.Draw(side).rectangle([160, 0, 163, 314], fill=INK)
    side.convert("RGB").save(OUT / "nsis-sidebar.bmp")
    # Disk image: the wug on the left, room for the app and Applications icons.
    dmg = canvas(660, 400)
    w = wug(150)
    dmg.alpha_composite(w, (40, 400 - w.height - 40))
    dmg.convert("RGB").save(OUT / "dmg-background.png")


if __name__ == "__main__":
    main()
