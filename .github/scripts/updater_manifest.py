"""Build latest.json for tauri-plugin-updater from a folder of release files.

Run by the release workflow's `publish` job on the files the `build` jobs
made and the `sign` job signed, so the manifest is written once, from local
files, by a job that holds no signing key. Fails unless every updater
payload (Windows NSIS installer, macOS universal .app.tar.gz, Linux AppImage
and .deb) is present exactly once with a .sig, unless every signature was
made by the key whose public half is in tauri.conf.json, names its own file
and the tag's version (the app requires the signed version to match the
announced one), and unless the tag matches tauri.conf.json.

  python3 -I updater_manifest.py <tag> <notes file> <tauri.conf.json> <release dir> <owner/repo>

Writes <release dir>/latest.json and prints it. Standard library only.
"""

import base64
import datetime
import json
import os
import sys
import urllib.parse

# Payload suffix -> the platform keys the updater looks up ({os}-{arch}-{bundle}
# first, then {os}-{arch}). The macOS build is universal, so it serves both
# architectures. The plain linux-x86_64 key is the AppImage's; a .deb install
# asks for linux-x86_64-deb first and finds its own package, so it is never
# handed an AppImage.
KINDS = [
    ("-setup.exe", ["windows-x86_64", "windows-x86_64-nsis"]),
    (".app.tar.gz", ["darwin-aarch64", "darwin-x86_64", "darwin-aarch64-app", "darwin-x86_64-app"]),
    (".AppImage", ["linux-x86_64", "linux-x86_64-appimage"]),
    (".deb", ["linux-x86_64-deb"]),
]
# Installers that are not updater payloads: attached to the release, never signed.
NOT_PAYLOADS = (".dmg",)
MANIFEST = "latest.json"


def minisign_lines(blob, what):
    """The lines of a base64-wrapped minisign file (Tauri's .sig and pubkey form)."""
    try:
        return base64.b64decode(blob.strip(), validate=True).decode("utf-8").splitlines()
    except ValueError as e:
        raise SystemExit(f"{what} is not base64 minisign text: {e}")


def key_id(line, what):
    """The 8-byte key number after the 2-byte algorithm of a minisign key or signature line."""
    try:
        raw = base64.b64decode(line.strip(), validate=True)
    except ValueError as e:
        raise SystemExit(f"{what} has a malformed key line: {e}")
    if len(raw) < 10:
        raise SystemExit(f"{what} has a short key line")
    return raw[2:10]


def public_key_id(pubkey):
    lines = minisign_lines(pubkey, "the updater pubkey")
    if len(lines) < 2:
        raise SystemExit("the updater pubkey has no key line")
    return key_id(lines[1], "the updater pubkey")


def read_signature(signature, name):
    """(key id, trusted comment fields) of a .sig."""
    lines = minisign_lines(signature, f"{name}.sig")
    if len(lines) < 3 or not lines[2].startswith("trusted comment: "):
        raise SystemExit(f"{name}.sig has no trusted comment")
    fields = dict(f.split(":", 1) for f in lines[2][len("trusted comment: "):].split("\t") if ":" in f)
    return key_id(lines[1], f"{name}.sig"), fields


def manifest(version, notes, pub_date, names, signatures, download_base, pubkey):
    """`names`: every file in the release folder. `signatures`: payload name -> .sig text.
    `pubkey`: tauri.conf.json's plugins.updater.pubkey."""
    expected_key = public_key_id(pubkey)
    platforms = {}
    found = {suffix: [] for suffix, _ in KINDS}
    for name in sorted(names):
        if name == MANIFEST or name.endswith(".sig"):
            continue
        suffix = next((s for s, _ in KINDS if name.endswith(s)), None)
        if suffix is None:
            if not name.endswith(NOT_PAYLOADS):
                raise SystemExit(f"unexpected file in the release: {name}")
            if name in signatures:
                raise SystemExit(f"{name} is not an updater payload but has a .sig")
            continue
        found[suffix].append(name)
    for suffix, keys in KINDS:
        if len(found[suffix]) != 1:
            raise SystemExit(f"expected one *{suffix} updater payload, found {found[suffix]}")
        name = found[suffix][0]
        if name not in signatures:
            raise SystemExit(f"{name} has no .sig; refusing to announce an unsigned update")
        signer, comment = read_signature(signatures[name], name)
        if signer != expected_key:
            raise SystemExit(f"{name}.sig was made by key {signer.hex()}, not the app's key {expected_key.hex()}")
        if comment.get("file") != name:
            raise SystemExit(f"{name}.sig was made for {comment.get('file')!r}")
        if comment.get("version") != version:
            raise SystemExit(f"{name}.sig is bound to version {comment.get('version')!r}, not {version}")
        url = f"{download_base}/{urllib.parse.quote(name)}"
        for key in keys:
            platforms[key] = {"signature": signatures[name].strip(), "url": url}
    stray = sorted(set(signatures) - {n for names_ in found.values() for n in names_})
    if stray:
        raise SystemExit(f"signatures without a payload: {stray}")
    return {"version": version, "notes": notes, "pub_date": pub_date, "platforms": platforms}


def main():
    tag, notes_path, conf_path, folder, repo = sys.argv[1:6]
    version = tag.removeprefix("v")
    with open(conf_path, encoding="utf-8") as f:
        conf = json.load(f)
    conf_version = conf["version"]
    if conf_version != version:
        raise SystemExit(f"tag {tag} does not match tauri.conf.json version {conf_version}")
    with open(notes_path, encoding="utf-8") as f:
        notes = f.read().strip()
    names = os.listdir(folder)
    signatures = {}
    for name in names:
        if name.endswith(".sig"):
            with open(os.path.join(folder, name), encoding="utf-8") as f:
                signatures[name[: -len(".sig")]] = f.read()
    pub_date = datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    doc = manifest(version, notes, pub_date, names, signatures, f"https://github.com/{repo}/releases/download/{urllib.parse.quote(tag)}", conf["plugins"]["updater"]["pubkey"])
    text = json.dumps(doc, indent=2)
    with open(os.path.join(folder, MANIFEST), "w", encoding="utf-8") as f:
        f.write(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
