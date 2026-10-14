#!/usr/bin/env python3
"""Native text-share integrity and safe rebuild controls; requires Quipu."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
NAMES = {"stack-map": 54, "stack-operations": 37}
MEMBERS = {"manifest.json", "manifest.ttl", "export.nt", "shapes.ttl"}
QUIPU = shutil.which(os.environ.get("QUIPU", "quipu"))


class TextPendants(unittest.TestCase):
    def setUp(self):
        self.assertIsNotNone(QUIPU, "native Quipu is required; tests do not skip")
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name)

    def native(self, *args):
        return subprocess.run([QUIPU, *map(str, args)], capture_output=True, text=True)

    def bundle(self, name):
        return ROOT / "packs" / name / "pendant"

    def test_checked_in_bundles_are_text_and_native_verified(self):
        for name, count in NAMES.items():
            with self.subTest(name=name):
                bundle = self.bundle(name)
                self.assertEqual({p.name for p in bundle.iterdir()}, MEMBERS)
                for path in bundle.iterdir():
                    self.assertFalse(path.read_bytes().startswith(b"SQLite format 3\0"))
                    path.read_text(encoding="utf-8")
                manifest = json.loads((bundle / "manifest.json").read_text())
                self.assertEqual(manifest["pack_dir"], "pendant")
                self.assertEqual(manifest["scope"]["value"], f"https://caboodle.dev/graph/{name}")
                result = self.native("import", bundle, "--db", self.work / f"{name}.db")
                self.assertEqual(result.returncode, 0, result.stderr)
                receipt = json.loads(result.stdout)
                self.assertEqual(receipt["graph_hash"], manifest["graph_hash"])
                self.assertEqual(receipt["share_id"], manifest["share_id"])
                self.assertEqual(receipt["attestation"]["tier"], "transport")
                self.assertEqual(sum(receipt["triples"].values()), count)
                if name == "stack-map":
                    self.assertEqual(receipt["outcome"], "quarantined")
                    self.assertIn("off_vocabulary", receipt["promotion"]["blockers"])
                else:
                    self.assertEqual(receipt["outcome"], "staged")

    def test_tampered_graph_and_manifest_are_refused(self):
        for field in ("graph", "manifest"):
            with self.subTest(field=field):
                bundle = self.work / field
                shutil.copytree(self.bundle("stack-map"), bundle)
                if field == "graph":
                    with (bundle / "export.nt").open("a") as out:
                        out.write('<urn:tampered> <urn:value> "changed" .\n')
                else:
                    path = bundle / "manifest.json"
                    manifest = json.loads(path.read_text())
                    manifest["graph_hash"] = "sha256:" + "0" * 64
                    path.write_text(json.dumps(manifest))
                result = self.native("import", bundle, "--db", self.work / f"{field}.db")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("mismatch", result.stderr)

    def test_known_local_vocabulary_can_stage_without_automatic_promotion(self):
        # This explicit fixture models reviewed receiver configuration. It is
        # not embedded in the share or installed in any actual consumer.
        shape = self.work / "known.ttl"
        shape.write_text('''@prefix sh: <http://www.w3.org/ns/shacl#> .
<urn:fixture:Component> a sh:NodeShape ;
 sh:targetClass <https://caboodle.dev/ns#StackComponent> .
''')
        db = self.work / "known.db"
        loaded = self.native("shapes", "load", "fixture", shape, "--db", db)
        self.assertEqual(loaded.returncode, 0, loaded.stderr)
        result = self.native("import", self.bundle("stack-map"), "--db", db)
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads(result.stdout)
        self.assertEqual(receipt["outcome"], "staged")
        self.assertEqual(receipt["triples"], {"accepted": 54, "quarantined": 0})

    def test_rebuild_preserves_semantic_graphs(self):
        output = self.work / "rebuilt"
        result = self.build(output)
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in NAMES:
            before = json.loads((self.bundle(name) / "manifest.json").read_text())
            after = json.loads((output / name / "pendant/manifest.json").read_text())
            self.assertEqual(before["graph_hash"], after["graph_hash"])
            self.assertEqual((self.bundle(name) / "export.nt").read_bytes(),
                             (output / name / "pendant/export.nt").read_bytes())
        self.assertEqual(list(output.glob(".stack-pendants.*")), [])

    def build(self, output, binary=QUIPU):
        env = dict(os.environ, QUIPU=binary, PACKS_OUTPUT_DIR=str(output))
        return subprocess.run(["sh", str(ROOT / "scripts/build-stack-packs.sh")],
                              env=env, capture_output=True, text=True)

    def test_failed_native_verification_preserves_existing_output(self):
        output = self.work / "existing"
        for name in NAMES:
            shutil.copytree(self.bundle(name), output / name / "pendant")
        def hashes():
            return {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest()
                    for p in output.rglob("*") if p.is_file()}
        before = hashes()
        wrapper = self.work / "refuse-import"
        wrapper.write_text('#!/bin/sh\nif [ "$1" = import ]; then exit 9; fi\n'
                           'exec "$REAL_QUIPU" "$@"\n')
        wrapper.chmod(0o755)
        env = dict(os.environ, QUIPU=str(wrapper), REAL_QUIPU=QUIPU,
                   PACKS_OUTPUT_DIR=str(output))
        result = subprocess.run(["sh", str(ROOT / "scripts/build-stack-packs.sh")],
                                env=env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(hashes(), before)
        self.assertEqual(list(output.glob(".stack-pendants.*")), [])

    def test_legacy_names_are_directory_aliases(self):
        for name in NAMES:
            for extension in ("pendant.db", "qpack.db"):
                path = ROOT / "packs" / f"{name}.{extension}"
                self.assertTrue(path.is_symlink())
                self.assertEqual(path.resolve(), self.bundle(name).resolve())
                result = self.native("import", path, "--db", self.work / f"{name}-{extension}.db")
                self.assertEqual(result.returncode, 0, result.stderr)
                receipt = json.loads(result.stdout)
                manifest = json.loads((self.bundle(name) / "manifest.json").read_text())
                self.assertEqual(receipt["share_id"], manifest["share_id"])


if __name__ == "__main__":
    unittest.main()
