#!/usr/bin/env python3
"""Packaging regression tests using tiny local fixtures; no Rust build or GPU."""

import hashlib
import json
import os
from pathlib import Path
import platform
import runpy
import subprocess
import tempfile
import unittest
from unittest.mock import patch

DIST = runpy.run_path(str(Path(__file__).with_name("dist")))
PACKAGE = DIST["package"]
VERIFY = DIST["verify_distribution"]


class DistributionTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="bestiary-dist-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.dist = self.root / "dist"
        self.suffix = ".exe" if platform.system() == "Windows" else ""
        (self.root / "target" / "release").mkdir(parents=True)
        (self.root / "config").mkdir()
        (self.root / "config" / "world.ron").write_text("(fixture: true)\n", encoding="utf-8")
        (self.root / "config" / "viewer.ron").write_text("(cosmetic: true)\n", encoding="utf-8")
        for name in DIST["DOCUMENTS"]:
            (self.root / name).write_text(f"fixture {name}\n", encoding="utf-8")
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n', encoding="utf-8")
        (self.root / ".gitignore").write_text("/target/\n/dist/\n", encoding="utf-8")
        for name in DIST["BINARIES"]:
            path = self.root / "target" / "release" / (name + self.suffix)
            path.write_bytes(b"#!/bin/sh\nexit 0\n")
            path.chmod(0o755)
        self.git("init", "-q")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "user.name", "Packaging fixture")
        self.git("add", ".")
        self.git("commit", "-qm", "fixture source")

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.root), *args], text=True).strip()

    def test_make_dist_produces_three_host_binaries_config_docs_checksums_and_unreleased_provenance(self):
        manifest = PACKAGE(self.root)
        self.assertEqual(manifest, VERIFY(self.dist))
        self.assertEqual(manifest["source"]["commit"], self.git("rev-parse", "HEAD"))
        self.assertFalse(manifest["source"]["dirty"])
        self.assertEqual(manifest["release_status"], "unreleased")
        self.assertIs(manifest["released"], False)
        self.assertEqual(manifest["version"], "0.1.0")
        self.assertEqual(manifest["platform"]["system"], platform.system())
        self.assertEqual(set(manifest["binaries"]), {name + self.suffix for name in DIST["BINARIES"]})
        for name, record in manifest["binaries"].items():
            actual = hashlib.sha256((self.dist / name).read_bytes()).hexdigest()
            self.assertEqual(record["sha256"], actual)
        for name in DIST["DOCUMENTS"]:
            self.assertEqual((self.dist / name).read_bytes(), (self.root / name).read_bytes())
        self.assertEqual((self.dist / "config" / "world.ron").read_bytes(), (self.root / "config" / "world.ron").read_bytes())
        self.assertIn("  manifest.json\n", (self.dist / "SHA256SUMS").read_text())
        self.assertEqual(self.git("tag", "--list"), "")

    def test_make_dist_dirty_source_is_explicit_and_unknown_files_survive_regeneration(self):
        PACKAGE(self.root)
        (self.dist / "personal-notes.txt").write_text("keep me\n", encoding="utf-8")
        (self.root / "RUNBOOK.md").write_text("revised instructions\n", encoding="utf-8")
        manifest = PACKAGE(self.root)
        self.assertTrue(manifest["source"]["dirty"])
        self.assertTrue(any("RUNBOOK.md" in entry for entry in manifest["source"]["status"]))
        self.assertIn(" M RUNBOOK.md", manifest["source"]["status"])
        self.assertEqual((self.dist / "personal-notes.txt").read_text(), "keep me\n")
        self.assertEqual((self.dist / "RUNBOOK.md").read_text(), "revised instructions\n")
        self.assertNotIn("personal-notes.txt", manifest["files"])
        VERIFY(self.dist)

    def test_make_dist_missing_binary_or_document_preserves_existing_package(self):
        original = PACKAGE(self.root)
        for name in ["RUNBOOK.md", "target/release/" + "bestiary" + self.suffix]:
            path = self.root / name
            saved = path.read_bytes()
            mode = path.stat().st_mode
            path.unlink()
            with self.assertRaisesRegex(ValueError, "distribution input"):
                PACKAGE(self.root)
            self.assertEqual(VERIFY(self.dist), original)
            path.write_bytes(saved)
            path.chmod(mode)
        self.assertFalse(list(self.root.glob(".bestiary-dist-*")))

    def test_make_dist_integrity_check_detects_tampered_binary_manifest_and_checksums(self):
        PACKAGE(self.root)
        for name in ["bestiary" + self.suffix, "manifest.json", "SHA256SUMS"]:
            path = self.dist / name
            original = path.read_bytes()
            if name == "manifest.json":
                metadata = json.loads(original)
                metadata["version"] = "unexpected"
                path.write_text(json.dumps(metadata), encoding="utf-8")
            else:
                path.write_bytes(original + b"tampered\n")
            with self.assertRaises(ValueError):
                VERIFY(self.dist)
            path.write_bytes(original)
            VERIFY(self.dist)

    def test_make_dist_refuses_to_discard_edited_shipped_config(self):
        PACKAGE(self.root)
        path = self.dist / "config" / "world.ron"
        path.write_text("my tuned config\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "integrity check failed"):
            PACKAGE(self.root)
        self.assertEqual(path.read_text(), "my tuned config\n")

    def test_make_dist_refuses_unowned_collisions_and_symlink_destinations(self):
        self.dist.mkdir()
        (self.dist / "README.md").write_text("user file\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unowned dist file"):
            PACKAGE(self.root)
        self.assertEqual((self.dist / "README.md").read_text(), "user file\n")
        if os.name == "nt":
            return  # Symlink creation can require administrator/developer mode.
        (self.dist / "README.md").unlink()
        outside = self.root / "outside"
        outside.mkdir()
        (self.dist / "config").symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            PACKAGE(self.root)
        self.assertFalse(list(outside.iterdir()))

    def test_make_dist_rolls_back_directory_swap_when_install_fails(self):
        original = PACKAGE(self.root)
        (self.root / "README.md").write_text("new source\n", encoding="utf-8")
        real_replace = os.replace

        def fail_install(source, destination):
            if Path(source).name.startswith(".bestiary-dist-stage-"):
                raise OSError("injected install failure")
            return real_replace(source, destination)

        with patch("os.replace", side_effect=fail_install):
            with self.assertRaisesRegex(OSError, "injected install failure"):
                PACKAGE(self.root)
        self.assertEqual(VERIFY(self.dist), original)
        self.assertFalse(list(self.root.glob(".bestiary-dist-*")))

    def test_make_dist_rejects_untrusted_manifest_paths_without_touching_outside_files(self):
        PACKAGE(self.root)
        outside = self.root / "outside.txt"
        outside.write_text("preserve\n", encoding="utf-8")
        manifest = json.loads((self.dist / "manifest.json").read_text())
        manifest["files"]["../outside.txt"] = hashlib.sha256(outside.read_bytes()).hexdigest()
        (self.dist / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unsafe or unowned"):
            PACKAGE(self.root)
        self.assertEqual(outside.read_text(), "preserve\n")

    def test_make_dist_integrity_requires_hashed_binaries_and_well_formed_metadata(self):
        PACKAGE(self.root)
        path = self.dist / "manifest.json"
        original = path.read_text()
        manifest = json.loads(original)
        name = "bestiary" + self.suffix
        del manifest["files"][name]
        manifest["binaries"][name]["sha256"] = None
        path.write_text(json.dumps(manifest), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "all three host executables"):
            VERIFY(self.dist)
        for malformed in [[], {"source": []}, {"platform": None}]:
            path.write_text(json.dumps(malformed), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "manifest"):
                VERIFY(self.dist)
        path.write_text(original, encoding="utf-8")
        VERIFY(self.dist)


if __name__ == "__main__":
    unittest.main(verbosity=2)
