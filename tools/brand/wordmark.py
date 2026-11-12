"""Build the wugs wordmark (the full logo) for the download page.

The owner's brand sheet (WUG_1.pdf, page 1, not in the repo) sets "WUG" in
FreeSerif outlines with the wug standing in for the S. This lifts the three
letter outlines from that page and places the traced wug (logo.svg) where the
sheet puts it, so the letters stay vector and the wug is the one the app uses.

    pdftocairo -svg -f 1 -l 1 WUG_1.pdf page1.svg
    python3 tools/brand/wordmark.py page1.svg

Writes site/img/wordmark.svg. FreeSerif is GPLv3 with the font exception;
the outlines are part of a logo, not a font.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# Where the sheet puts things, in page points: the letters' group offset
# (`source-5` in pdftocairo's output) and the wug's measured box.
LETTERS_AT = (54, 142)
WUG_BOX = (1036, 287, 263, 281)
VIEW = "58 282 1246 292"


def main(page_svg: str) -> None:
    page = Path(page_svg).read_text()
    letters = []
    for gid, x, y in re.findall(r'<use xlink:href="#(glyph-0-\d)" x="([\d.]+)" y="([\d.]+)"/>', page):
        path = re.search(rf'<g id="{gid}">(.*?)</g>', page, re.S).group(1).strip()
        letters.append(f'<g transform="translate({LETTERS_AT[0] + float(x):.2f} {LETTERS_AT[1] + float(y):.2f})">{path}</g>')
    assert len(letters) == 3, f"expected W, U, G; found {len(letters)} glyphs"
    logo = (ROOT / "apps/desktop/public/brand/logo.svg").read_text()
    wug = re.search(r"<svg[^>]*>(.*)</svg>", logo, re.S).group(1)
    wug_view = re.search(r'viewBox="([^"]+)"', logo).group(1)
    x, y, w, h = WUG_BOX
    out = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{VIEW}" role="img" aria-label="wugs">\n'
        f'<g fill="#000">{"".join(letters)}</g>\n'
        f'<svg x="{x}" y="{y}" width="{w}" height="{h}" viewBox="{wug_view}">{wug}</svg>\n'
        "</svg>\n"
    )
    (ROOT / "site/img/wordmark.svg").write_text(out)


if __name__ == "__main__":
    main(sys.argv[1])
