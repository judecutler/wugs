"""Build latest.json for tauri-plugin-updater from a draft release's .sig assets.

Run by the release workflow once every installer leg has uploaded, so the
manifest is written once (parallel legs merging one file can drop a
platform). Fails unless Windows, both macOS architectures and Linux are
present, and unless the tag matches tauri.conf.json's version (the app
requires the signed version to match the announced one).

  python3 -I updater_manifest.py <owner/repo> <tag> <notes file> <tauri.conf.json>

Needs GITHUB_TOKEN with contents: write. Standard library only.
"""

import datetime
import json
import os
import sys
import urllib.parse
import urllib.request

API = "https://api.github.com"
UPLOADS = "https://uploads.github.com"

# Asset suffix -> the platform keys the updater looks up ({os}-{arch} and
# {os}-{arch}-{bundle}). The macOS build is universal, so it serves both.
KINDS = [
    ("-setup.exe", ["windows-x86_64", "windows-x86_64-nsis"]),
    (".app.tar.gz", ["darwin-aarch64", "darwin-x86_64", "darwin-aarch64-app", "darwin-x86_64-app"]),
    (".AppImage", ["linux-x86_64", "linux-x86_64-appimage"]),
    (".deb", ["linux-x86_64-deb"]),
]
REQUIRED = ["windows-x86_64", "darwin-aarch64", "darwin-x86_64", "linux-x86_64"]


def manifest(version, notes, pub_date, signatures, download_base):
    """`signatures` maps an asset name to the text of its .sig asset."""
    platforms = {}
    for name in sorted(signatures):
        for suffix, keys in KINDS:
            if name.endswith(suffix):
                url = f"{download_base}/{urllib.parse.quote(name)}"
                for key in keys:
                    if key in platforms:
                        raise SystemExit(f"two assets claim {key}: {platforms[key]['url']} and {url}")
                    platforms[key] = {"signature": signatures[name].strip(), "url": url}
    missing = [k for k in REQUIRED if k not in platforms]
    if missing:
        raise SystemExit(f"no signed updater asset for {', '.join(missing)}; found {sorted(signatures)}")
    return {"version": version, "notes": notes, "pub_date": pub_date, "platforms": platforms}


def request(method, url, token, data=None, accept="application/vnd.github+json", content_type=None):
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Authorization", f"Bearer {token}")
    req.add_header("Accept", accept)
    req.add_header("X-GitHub-Api-Version", "2022-11-28")
    if content_type:
        req.add_header("Content-Type", content_type)
    with urllib.request.urlopen(req, timeout=60) as resp:
        return resp.read()


def find_release(repo, tag, token):
    # Drafts are not served by /releases/tags/{tag}; list instead.
    page = 1
    while True:
        batch = json.loads(request("GET", f"{API}/repos/{repo}/releases?per_page=100&page={page}", token))
        for release in batch:
            if release["tag_name"] == tag:
                return release
        if len(batch) < 100:
            raise SystemExit(f"no release for tag {tag}")
        page += 1


def main():
    repo, tag, notes_path, conf_path = sys.argv[1:5]
    token = os.environ["GITHUB_TOKEN"]
    version = tag.removeprefix("v")
    with open(conf_path, encoding="utf-8") as f:
        conf_version = json.load(f)["version"]
    if conf_version != version:
        raise SystemExit(f"tag {tag} does not match tauri.conf.json version {conf_version}")
    with open(notes_path, encoding="utf-8") as f:
        notes = f.read().strip()
    release = find_release(repo, tag, token)
    assets = {a["name"]: a for a in release["assets"]}
    signatures = {}
    for name, asset in assets.items():
        if name.endswith(".sig") and name[: -len(".sig")] in assets:
            body = request("GET", f"{API}/repos/{repo}/releases/assets/{asset['id']}", token, accept="application/octet-stream")
            signatures[name[: -len(".sig")]] = body.decode("utf-8")
    if any(n.endswith(".deb") for n in assets) and not any(n.endswith(".deb") for n in signatures):
        # The updater on a .deb install looks for linux-x86_64-deb, then falls
        # back to the AppImage entry, which dpkg cannot install.
        print("::warning::the .deb has no updater signature; .deb installs will fail to update in place")
    pub_date = datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    doc = manifest(version, notes, pub_date, signatures, f"https://github.com/{repo}/releases/download/{urllib.parse.quote(tag)}")
    if "latest.json" in assets:
        request("DELETE", f"{API}/repos/{repo}/releases/assets/{assets['latest.json']['id']}", token)
    request(
        "POST",
        f"{UPLOADS}/repos/{repo}/releases/{release['id']}/assets?name=latest.json",
        token,
        data=json.dumps(doc, indent=2).encode("utf-8"),
        content_type="application/json",
    )
    print(json.dumps(doc, indent=2))


if __name__ == "__main__":
    main()
