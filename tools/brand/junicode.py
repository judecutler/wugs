"""Cut Junicode 2 into the download page's two web fonts.

    git clone --depth 1 --branch v2.226 https://github.com/psb1558/Junicode-font
    pip install fonttools brotli
    python3 tools/brand/junicode.py Junicode-font/webfiles/JunicodeVF-Roman.woff2

Writes site/fonts/junicode-latin-wght.woff2 (what every page downloads) and
junicode-ipa-wght.woff2 (fetched only when a page shows a character in IPA's
range; its unicode-range in site/style.css must match IPA below). Both keep
only the weight axis. The IPA file also carries basic Latin, so a letter with
a combining mark (t̪, ə̃) is shaped in one font and the mark sits right.
"""
import sys
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parents[2]
LATIN = "U+0000-00FF,U+0131,U+0152-0153,U+2000-206F,U+20AC,U+2122"
IPA = "U+0100-02FF,U+0300-036F,U+1D00-1DFF,U+1E00-1EFF,U+2190-2193,U+2C60-2C7F,U+A700-A7FF"
FEATURES = ["kern", "liga", "mark", "mkmk", "ccmp", "locl"]


def cut(source: str, unicodes: str, out: str) -> None:
    font = TTFont(source)
    options = subset.Options()
    options.layout_features = FEATURES
    sub = subset.Subsetter(options)
    sub.populate(unicodes=subset.parse_unicodes(unicodes))
    sub.subset(font)
    # Pin every axis but weight at its default (subset first: the instancer
    # leaves gvar entries the subsetter then trips on).
    font = instantiateVariableFont(font, {a.axisTag: None for a in font["fvar"].axes if a.axisTag != "wght"})
    font.flavor = "woff2"
    font.save(ROOT / "site/fonts" / out)


def main(source: str) -> None:
    cut(source, LATIN, "junicode-latin-wght.woff2")
    cut(source, "U+0020-007E," + IPA, "junicode-ipa-wght.woff2")


if __name__ == "__main__":
    main(sys.argv[1])
