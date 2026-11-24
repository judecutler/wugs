"""Build datasets/naturalearth/land-110m.path from Natural Earth's 1:110m land.

Usage: python3 -I tools/data/naturalearth.py <natural-earth-vector>/geojson/ne_110m_land.geojson datasets/naturalearth/land-110m.path

Writes the land as one SVG path in plain longitude/latitude (equirectangular):
x = longitude + 180 (0 to 360), y = 90 - latitude (0 to 180), one decimal, so
a map draws it in a 360 x 180 box and places a language at the same x and y.
Natural Earth is in the public domain (see the README beside the output).
"""
import json
import sys


def ring(points):
    out = []
    last = None
    for lon, lat in points:
        p = (round(lon + 180, 1), round(90 - lat, 1))
        if p != last:
            out.append(p)
            last = p
    if len(out) < 3:
        return ""
    head = f"M{out[0][0]:g} {out[0][1]:g}"
    return head + "".join(f"L{x:g} {y:g}" for x, y in out[1:]) + "Z"


def main(src, out):
    data = json.load(open(src, encoding="utf-8"))
    parts = []
    for feature in data["features"]:
        geom = feature["geometry"]
        polys = geom["coordinates"] if geom["type"] == "MultiPolygon" else [geom["coordinates"]]
        for poly in polys:
            for r in poly:
                parts.append(ring(r))
    path = "".join(p for p in parts if p)
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write(path + "\n")
    print(f"{len(path)} characters written to {out}")


main(sys.argv[1], sys.argv[2])
