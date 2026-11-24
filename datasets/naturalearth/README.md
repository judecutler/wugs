# Natural Earth land outline

`land-110m.path` is the land of the world at 1:110 million scale, drawn as one SVG path in plain longitude and latitude: x is the longitude plus 180 (0 to 360) and y is 90 minus the latitude (0 to 180). The wugs desktop app embeds it as the background of the Typology panel's maps, where each language is a dot at its Glottolog coordinates.

## Source and licence

[Natural Earth](https://www.naturalearthdata.com/) is in the public domain. No permission is needed to use it, and no attribution is required; we credit it gladly: "Made with Natural Earth."

Built from `geojson/ne_110m_land.geojson` in [nvkelso/natural-earth-vector](https://github.com/nvkelso/natural-earth-vector) at commit `ca96624a56bd078437bca8184e78163e5039ad19` by:

```
python3 -I tools/data/naturalearth.py ne_110m_land.geojson datasets/naturalearth/land-110m.path
```
