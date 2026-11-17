#!/usr/bin/env python3
"""Whole-bundle transaction tests using private fixtures and real Linux renames."""
import fcntl
import json
import os
from pathlib import Path
import platform
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from hybrid_payload import MANIFEST, archive_bundle, expected_files, release_metadata, sha, verify
from hybrid_linux_install import ACTIVE, BACKUPS, EXECUTABLES, LEASE, install, rollback

TARGET = f"linux-{platform.machine()}"


@unittest.skipUnless(platform.system() == "Linux" and os.geteuid() != 0,
                     "Linux ordinary-user fixture required")
class WholeBundleTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=Path.home())
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        self.root = self.home / "managed"
        self.baseline = self.make_bundle("1")
        self.candidate = self.make_bundle("2")

    def make_bundle(self, token):
        output = self.home / f"build-{token}"
        root = output / ACTIVE
        root.mkdir(parents=True)
        version, ipc = release_metadata()
        files = {}
        for relative in expected_files(TARGET):
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((relative + token).encode())
            path.chmod(0o755 if relative in EXECUTABLES else 0o644)
            files[relative] = sha(path)
        (root / MANIFEST).write_text(json.dumps({
            "schema": 1, "version": version, "ipc": ipc, "target": TARGET,
            "revision": token * 40, "files": files,
        }), encoding="utf-8")
        (root / MANIFEST).chmod(0o644)
        package = archive_bundle(root, TARGET, output)
        return package, sha(package)

    def switch(self, package):
        return install(*package, self.root, TARGET)

    def active(self):
        return verify(self.root / ACTIVE, TARGET)["revision"]

    def test_verified_activation_real_exchange_and_rollback(self):
        self.assertEqual(self.switch(self.baseline)["status"], "installed")
        self.assertEqual(self.switch(self.baseline)["status"], "already-installed")
        result = self.switch(self.candidate)
        self.assertEqual(result["status"], "upgraded")
        self.assertEqual(self.active(), "2" * 40)
        restored = rollback(self.root, result["rollback_slot"], TARGET)
        self.assertEqual(restored["revision"], "1" * 40)
        self.assertEqual(self.active(), "1" * 40)

    def test_corrupt_archive_busy_lease_and_interruption_preserve_active_bytes(self):
        self.switch(self.baseline)
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            self.switch((self.candidate[0], "0" * 64))
        with (self.root / LEASE).open("rb") as lease:
            fcntl.flock(lease, fcntl.LOCK_SH)
            with self.assertRaisesRegex(ValueError, "in use"):
                self.switch(self.candidate)
        with patch("hybrid_linux_install.exchange", side_effect=OSError("injected pre-exchange failure")):
            with self.assertRaisesRegex(OSError, "injected"):
                self.switch(self.candidate)
        self.assertEqual(self.active(), "1" * 40)
        [staged] = list((self.root / BACKUPS).iterdir())
        self.assertEqual(verify(staged, TARGET)["revision"], "2" * 40)

    def test_incomplete_input_and_invalid_rollback_preserve_active_bytes(self):
        self.switch(self.baseline)
        broken = self.home / "unexpected.tar.gz"
        with tarfile.open(broken, "w:gz") as archive:
            archive.add(self.baseline[0], arcname="../unexpected")
        with self.assertRaisesRegex(ValueError, "unexpected archive member"):
            self.switch((broken, sha(broken)))
        with self.assertRaisesRegex(ValueError, "exact managed rollback slot"):
            rollback(self.root, "../foreign", TARGET)
        self.assertEqual(self.active(), "1" * 40)


if __name__ == "__main__":
    unittest.main()
