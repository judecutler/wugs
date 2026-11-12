"""Background patterns: one repeating SVG tile per motif, drawn in black on
transparent and used as a CSS mask, so the app colours them from a token.
Original geometric tiles in the manner of each tradition, not copies of
any one work. Run from the repo root:

    python3 tools/brand/patterns.py
"""
import math
from pathlib import Path

OUT = Path(__file__).resolve().parents[2] / "apps" / "desktop" / "public" / "patterns"


def svg(w, h, body, sw=1.4):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w:g}" height="{h:g}" viewBox="0 0 {w:g} {h:g}">'
            f'<g fill="none" stroke="#000" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round">{body}</g></svg>\n')


def pts(ps):
    return " ".join(f"{x:.2f},{y:.2f}" for x, y in ps)


def girih():
    """Eight-pointed stars (khatam: two squares turned 45 degrees) on a
    square grid, joined corner to corner by strapwork: the star-and-cross
    field of Persian tilework."""
    s = 72
    r = 16
    body = []
    def square(cx, cy, turn):
        return [(cx + r * math.cos(turn + k * math.pi / 2), cy + r * math.sin(turn + k * math.pi / 2)) for k in range(4)]
    for cx, cy in [(0, 0), (s, 0), (0, s), (s, s), (s / 2, s / 2)]:
        body.append(f'<polygon points="{pts(square(cx, cy, 0))}"/>')
        body.append(f'<polygon points="{pts(square(cx, cy, math.pi / 4))}"/>')
    # Strapwork: each star point reaches toward its neighbours.
    for cx, cy in [(0, s / 2), (s, s / 2), (s / 2, 0), (s / 2, s)]:
        body.append(f'<polygon points="{pts([(cx - 7, cy), (cx, cy - 7), (cx + 7, cy), (cx, cy + 7)])}"/>')
    for ax, ay, bx, by in [(r, 0, s / 2 - 7, 0), (0, r, 0, s / 2 - 7)]:
        pass
    for (x1, y1, x2, y2) in [(r, 0, s / 2 - 7, 0), (s / 2 + 7, 0, s - r, 0), (0, r, 0, s / 2 - 7), (0, s / 2 + 7, 0, s - r),
                             (r, s, s / 2 - 7, s), (s / 2 + 7, s, s - r, s), (s, r, s, s / 2 - 7), (s, s / 2 + 7, s, s - r)]:
        body.append(f'<line x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}"/>')
    return svg(s, s, "".join(body), 1.2)


def shippo():
    """Shippo (seven treasures): circles overlapping on a grid, so every
    meeting makes a four-petalled flower, a wagara of good fortune."""
    s = 40
    r = s / 2
    body = [f'<circle cx="{x:g}" cy="{y:g}" r="{r:g}"/>' for x, y in [(0, 0), (s, 0), (0, s), (s, s), (r, r)]]
    return svg(s, s, "".join(body), 1.1)


def asanoha():
    """Asanoha: the hemp-leaf star on a triangular lattice."""
    a = 52
    hgt = a * math.sqrt(3)
    body = []
    # Two rows of equilateral triangles fill a tile of width a, height a*sqrt(3).
    tris = []
    for row, y0 in enumerate([0, hgt / 2]):
        for i in range(-1, 3):
            x0 = i * a / 2
            if (i + row) % 2 == 0:
                tris.append([(x0 - a / 2, y0 + hgt / 2), (x0 + a / 2, y0 + hgt / 2), (x0, y0)])
            else:
                tris.append([(x0 - a / 2, y0), (x0 + a / 2, y0), (x0, y0 + hgt / 2)])
    for t in tris:
        cx = sum(p[0] for p in t) / 3
        cy = sum(p[1] for p in t) / 3
        body.append(f'<polygon points="{pts(t)}"/>')
        for x, y in t:
            body.append(f'<line x1="{x:.2f}" y1="{y:.2f}" x2="{cx:.2f}" y2="{cy:.2f}"/>')
    return svg(a, hgt, "".join(body), 0.9)


def eslimi():
    """Eslimi: a scrolling vine whose curls end in palmettes and buds."""
    w, h = 120, 60
    body = [
        # the vine: one S-wave across the tile, continuous at the edges
        f'<path d="M0,30 C20,8 40,8 60,30 S100,52 120,30"/>',
        # curls leaving the vine
        '<path d="M30,14 C34,4 46,6 44,14 C43,19 36,19 36,15"/>',
        '<path d="M90,46 C86,56 74,54 76,46 C77,41 84,41 84,45"/>',
        # palmettes: a fan of five lobes on a small cup
        '<path d="M60,30 C56,22 52,20 50,14 M60,30 C58,20 58,16 60,9 M60,30 C62,20 62,16 64,10 M60,30 C64,22 68,20 70,14"/>',
        '<path d="M53,31 C56,35 64,35 67,31"/>',
        '<path d="M0,30 C-4,38 -8,40 -10,46 M120,30 C116,38 112,40 110,46 M120,30 C124,38 128,40 130,46"/>',
        # buds
        '<path d="M12,22 C8,16 12,12 16,16 C17,18 15,21 12,22 Z"/>',
        '<path d="M108,38 C112,44 108,48 104,44 C103,42 105,39 108,38 Z"/>',
    ]
    return svg(w, h, "".join(body), 1.2)


def petroglyph():
    """Marks after rock art: spirals, ringed dots, zigzags and a small
    four-legged animal, pecked in loose rows."""
    w = h = 150
    def spiral(cx, cy, turns=2.4, r=11):
        p = []
        for i in range(0, 60):
            t = i / 59 * turns * 2 * math.pi
            rr = r * i / 59
            p.append((cx + rr * math.cos(t), cy + rr * math.sin(t)))
        return f'<polyline points="{pts(p)}"/>'
    body = [
        spiral(30, 34),
        '<circle cx="104" cy="28" r="9"/><circle cx="104" cy="28" r="3.5"/><circle cx="104" cy="28" r="0.6"/>',
        '<polyline points="56,70 64,62 72,70 80,62 88,70 96,62"/>',
        '<circle cx="18" cy="108" r="1.6"/><circle cx="26" cy="104" r="1.6"/><circle cx="34" cy="108" r="1.6"/>',
        spiral(118, 112, 2.0, 9),
        # an animal: body, legs, head and horns
        '<path d="M54,120 L82,120 M58,120 L56,132 M64,120 L64,132 M74,120 L74,132 M80,120 L82,132 M82,120 L90,112 M90,112 L86,104 M90,112 L96,106 M54,120 L48,116"/>',
        '<path d="M126,64 a6,6 0 1,0 0.1,0 M126,58 L126,52 M132,64 L138,64 M126,70 L126,76 M120,64 L114,64"/>',
    ]
    return svg(w, h, "".join(body), 1.6)


def khatam():
    """Small eight-pointed stars on a tight grid: the girih's star alone, a
    finer texture for emphasis."""
    s, r = 26, 7
    def square(cx, cy, turn):
        return [(cx + r * math.cos(turn + k * math.pi / 2), cy + r * math.sin(turn + k * math.pi / 2)) for k in range(4)]
    body = []
    for cx, cy in [(s / 2, s / 2)]:
        body.append(f'<polygon points="{pts(square(cx, cy, 0))}"/><polygon points="{pts(square(cx, cy, math.pi / 4))}"/>')
    return svg(s, s, "".join(body), 1.0)


def palmette():
    """A row of small palmette fans on a running line: the eslimi's flower
    alone, for emphasis."""
    w, h = 40, 28
    body = ['<path d="M0,24 L40,24"/>',
            '<path d="M20,24 C16,18 12,16 11,10 M20,24 C18,16 18,12 20,6 M20,24 C22,16 22,12 24,7 M20,24 C24,18 28,16 29,10"/>',
            '<path d="M15,25 C17,28 23,28 25,25"/>']
    return svg(w, h, "".join(body), 1.0)


def kikko():
    """Kikko: the tortoise-shell hexagon wagara."""
    a = 14
    w, h = a * 3, a * math.sqrt(3)
    def hexagon(cx, cy):
        return [(cx + a * math.cos(k * math.pi / 3), cy + a * math.sin(k * math.pi / 3)) for k in range(6)]
    body = [f'<polygon points="{pts(hexagon(x, y))}"/>' for x, y in [(0, 0), (w, 0), (0, h), (w, h), (w / 2, h / 2)]]
    return svg(w, h, "".join(body), 1.0)


def seigaiha():
    """Seigaiha: rows of overlapping wave fans. Each fan covers the ones
    behind it, drawn as clipped arcs so the tile stays line-only."""
    w, h = 32, 16
    r = w / 2
    body = []
    rows = [(0, 0), (w, 0), (w / 2, h / 2), (0, h), (w, h)]
    for i, (cx, cy) in enumerate(rows):
        later = rows[i + 1:]
        clip = "".join(f'<circle cx="{x:g}" cy="{y:g}" r="{r:g}"/>' for x, y in later)
        body.append(f'<clipPath id="c{i}"><path d="M-40,-40H80V60H-40Z{"".join(f"M{x - r:g},{y:g}a{r:g},{r:g} 0 1,0 {2 * r:g},0a{r:g},{r:g} 0 1,0 {-2 * r:g},0Z" for x, y in later)}" clip-rule="evenodd"/></clipPath>')
        arcs = "".join(f'<path d="M{cx - r * k:.2f},{cy:.2f} A{r * k:.2f},{r * k:.2f} 0 0 1 {cx + r * k:.2f},{cy:.2f}"/>' for k in (1.0, 0.7, 0.4))
        body.append(f'<g clip-path="url(#c{i})">{arcs}</g>')
    return svg(w, h, "".join(body), 0.9)


def pecked():
    """Pecked dots, the ground of rock art: an even scatter."""
    w = h = 36
    dots = [(6, 8), (20, 4), (30, 14), (12, 22), (26, 28), (4, 32), (18, 16)]
    return svg(w, h, "".join(f'<circle cx="{x}" cy="{y}" r="1.1"/>' for x, y in dots), 1.0)


def zigzag():
    """Zigzags in rows, the commonest rock-art mark, for emphasis."""
    w, h = 24, 14
    return svg(w, h, '<polyline points="0,10 6,4 12,10 18,4 24,10"/>', 1.1)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for name, f in [("girih", girih), ("shippo", shippo), ("asanoha", asanoha), ("eslimi", eslimi), ("petroglyph", petroglyph),
                    ("khatam", khatam), ("palmette", palmette), ("kikko", kikko), ("seigaiha", seigaiha), ("pecked", pecked), ("zigzag", zigzag)]:
        (OUT / f"{name}.svg").write_text(f())


if __name__ == "__main__":
    main()
