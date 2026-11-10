"""Download every phone-transcription model in the bundled catalogue into
<out>/<id>/ and check each file against the SHA-256 its manifest pins.
Writes <out>/models.json: id, name, runtime and the [runtime] settings."""
import hashlib, json, os, sys, tomllib, urllib.request

catalog, out = sys.argv[1], sys.argv[2]
models = []
for name in sorted(os.listdir(catalog)):
    if not name.endswith(".toml"):
        continue
    m = tomllib.load(open(os.path.join(catalog, name), "rb"))
    if m["model"]["capability"] != "phone-transcription":
        continue
    dest = os.path.join(out, m["package"]["id"])
    os.makedirs(dest, exist_ok=True)
    for f in m["files"]:
        path = os.path.join(dest, f["path"])
        if not os.path.exists(path):
            urllib.request.urlretrieve(f["url"], path)
        digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
        if f.get("sha256") and f["sha256"] != digest:
            sys.exit(f"{f['path']} of {m['package']['id']} does not match its pinned SHA-256")
        print(m["package"]["id"], f["path"], os.path.getsize(path), digest)
    models.append({"id": m["package"]["id"], "name": m["package"]["name"], "runtime": m["model"]["runtime"], "params": m.get("runtime", {})})
json.dump(models, open(os.path.join(out, "models.json"), "w"), indent=1)
