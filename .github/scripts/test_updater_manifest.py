"""Tests for updater_manifest.py: python3 -I -m unittest discover -s .github/scripts"""

import base64
import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(HERE, "updater_manifest.py")
FILES = ["wugs_1.2.3_x64-setup.exe", "wugs.app.tar.gz", "wugs_1.2.3_amd64.AppImage", "wugs_1.2.3_amd64.deb"]


KEY = bytes(range(1, 9))
OTHER_KEY = bytes(8)


def b64(raw):
    return base64.b64encode(raw).decode()


def pubkey(key=KEY):
    text = f"untrusted comment: minisign public key: X\n{b64(b'Ed' + key + bytes(32))}\n"
    return b64(text.encode())


def sig(name, version="1.2.3", key=KEY):
    text = (
        "untrusted comment: signature from tauri secret key\n"
        f"{b64(b'ED' + key + bytes(64))}\n"
        f"trusted comment: timestamp:1\tfile:{name}\tversion:{version}\n"
        f"{b64(bytes(64))}\n"
    )
    return b64(text.encode())


class Manifest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = os.path.join(self.tmp.name, "release")
        os.mkdir(self.dir)
        for name in FILES + ["wugs_1.2.3_universal.dmg"]:
            self.write(name, "x")
        for name in FILES:
            self.write(name + ".sig", sig(name))
        self.notes = os.path.join(self.tmp.name, "notes.md")
        with open(self.notes, "w") as f:
            f.write("wugs 1.2.3\n\nWhat changed.\n")
        self.conf = os.path.join(self.tmp.name, "tauri.conf.json")
        with open(self.conf, "w") as f:
            json.dump({"version": "1.2.3", "plugins": {"updater": {"pubkey": pubkey()}}}, f)

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, name, text):
        with open(os.path.join(self.dir, name), "w") as f:
            f.write(text)

    def run_script(self, tag="v1.2.3"):
        return subprocess.run([sys.executable, "-I", SCRIPT, tag, self.notes, self.conf, self.dir, "owner/repo"], capture_output=True, text=True)

    def test_writes_every_platform_key(self):
        r = self.run_script()
        self.assertEqual(r.returncode, 0, r.stderr)
        with open(os.path.join(self.dir, "latest.json")) as f:
            doc = json.load(f)
        self.assertEqual(doc["version"], "1.2.3")
        self.assertEqual(doc["notes"], "wugs 1.2.3\n\nWhat changed.")
        base = "https://github.com/owner/repo/releases/download/v1.2.3/"
        p = doc["platforms"]
        self.assertEqual(p["windows-x86_64"]["url"], base + "wugs_1.2.3_x64-setup.exe")
        self.assertEqual(p["windows-x86_64-nsis"], p["windows-x86_64"])
        for key in ["darwin-aarch64", "darwin-x86_64", "darwin-aarch64-app", "darwin-x86_64-app"]:
            self.assertEqual(p[key]["url"], base + "wugs.app.tar.gz")
        self.assertEqual(p["linux-x86_64"]["url"], base + "wugs_1.2.3_amd64.AppImage")
        self.assertEqual(p["linux-x86_64-appimage"], p["linux-x86_64"])
        # A .deb install looks up linux-x86_64-deb first and gets its own package.
        self.assertEqual(p["linux-x86_64-deb"]["url"], base + "wugs_1.2.3_amd64.deb")
        self.assertEqual(p["linux-x86_64-deb"]["signature"], sig("wugs_1.2.3_amd64.deb"))
        self.assertFalse(any(v["url"].endswith(".dmg") for v in p.values()))

    def assertRefused(self, needle, tag="v1.2.3"):
        r = self.run_script(tag)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn(needle, r.stderr)
        self.assertFalse(os.path.exists(os.path.join(self.dir, "latest.json")))

    def test_a_deb_without_a_signature_is_an_error(self):
        os.remove(os.path.join(self.dir, "wugs_1.2.3_amd64.deb.sig"))
        self.assertRefused("wugs_1.2.3_amd64.deb has no .sig")

    def test_every_payload_is_required(self):
        os.remove(os.path.join(self.dir, "wugs.app.tar.gz"))
        os.remove(os.path.join(self.dir, "wugs.app.tar.gz.sig"))
        self.assertRefused("expected one *.app.tar.gz")

    def test_two_payloads_of_one_kind_are_refused(self):
        self.write("other.AppImage", "x")
        self.write("other.AppImage.sig", sig("other.AppImage"))
        self.assertRefused("expected one *.AppImage")

    def test_a_signature_for_another_version_is_refused(self):
        self.write("wugs_1.2.3_x64-setup.exe.sig", sig("wugs_1.2.3_x64-setup.exe", "1.2.2"))
        self.assertRefused("bound to version '1.2.2'")

    def test_a_signature_by_another_key_is_refused(self):
        self.write("wugs.app.tar.gz.sig", sig("wugs.app.tar.gz", key=OTHER_KEY))
        self.assertRefused("not the app's key")

    def test_a_signature_for_another_file_is_refused(self):
        self.write("wugs_1.2.3_amd64.AppImage.sig", sig("wugs_1.2.3_amd64.deb"))
        self.assertRefused("was made for")

    def test_the_tag_must_match_the_config(self):
        self.assertRefused("does not match tauri.conf.json", tag="v1.2.4")

    def test_unexpected_files_are_refused(self):
        self.write("wugs.msi", "x")
        self.assertRefused("unexpected file")


if __name__ == "__main__":
    unittest.main()
