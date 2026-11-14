#!/usr/bin/env python3
"""# arming: CI/offline fixture controls; no release or native install."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import release_candidate as c


def fixture():
    return {"schema": "release-plan/v1", "tool": "tambo", "repository": "intigraph/tambo",
            "version": "1.2.0", "source": {"commit": "a" * 40, "tree_sha256": "b" * 64,
            "lockfiles_sha256": "c" * 64}, "build": {"run_id": "123", "attempt": 1,
            "workflow_sha256": "d" * 64}, "targets": [{"os": "linux", "architecture": "x86_64",
            "artifacts": [{"name": "tambo-1.2.0.tar.gz", "kind": "archive"},
                          {"name": "tambo-1.2.0.crate", "kind": "crate"}]}],
            "toolchain": {"rust": "e" * 64}, "dependencies": {"lock": "f" * 64},
            "compatibility_sha256": "1" * 64, "breaking_records": [],
            "previous_stable_sha256": "2" * 64, "restore_plan_sha256": "3" * 64,
            "capabilities": {name: ["v1"] for name in
                             ("store_read", "store_write", "pendant_read", "pendant_write", "cli", "config")},
            "feature_profile": "default", "experimental_features": [],
            "required_receipts": ["required", "extended", "upgrade", "compatibility"],
            "verification_policy_sha256": "4" * 64}


class Candidate(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="candidate-controls-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.frozen = c.freeze(fixture())
        self.inventory = {"schema": "candidate-build/v1", "plan_id": self.frozen["plan_id"],
                          "source": self.frozen["plan"]["source"],
                          "build": self.frozen["plan"]["build"], "artifacts": [],
                          "receipts": {name: "5" * 64 for name in fixture()["required_receipts"]}}
        for name, kind in (("tambo-1.2.0.tar.gz", "archive"), ("tambo-1.2.0.crate", "crate")):
            data = ("fixture bytes " + name).encode()
            (self.root / name).write_bytes(data)
            self.inventory["artifacts"].append({"name": name, "kind": kind,
                "os": "linux", "architecture": "x86_64", "embedded_version": "1.2.0",
                "url": "https://github.com/intigraph/tambo/releases/download/candidate-123/" + name,
                "size": len(data), "sha256": c.digest(data)})

    def test_canonical_and_domain_binding(self):
        plan = fixture()
        self.assertEqual(c.freeze(plan), c.freeze(dict(reversed(list(plan.items())))))
        altered = copy.deepcopy(plan)
        altered["build"]["attempt"] = 2
        self.assertNotEqual(c.freeze(plan)["plan_id"], c.freeze(altered)["plan_id"])

    def test_freeze_requires_final_version_and_complete_inputs(self):
        for version in ("1.2.0-rc.1", "1.2.0-nightly", "01.2.0", "v1.2.0"):
            value = fixture()
            value["version"] = version
            with self.assertRaises(ValueError):
                c.freeze(value)
        for name in fixture():
            value = fixture()
            del value[name]
            with self.assertRaises(ValueError):
                c.freeze(value)

    def test_modified_frozen_plan_refuses(self):
        self.frozen["plan"]["version"] = "1.2.1"
        with self.assertRaises(ValueError):
            c.seal(self.frozen, self.inventory, self.root)

    def test_missing_target_package_receipt_and_duplicate_refuse(self):
        for change in ("artifact", "receipt", "duplicate"):
            value = copy.deepcopy(self.inventory)
            if change == "artifact":
                value["artifacts"].pop()
            elif change == "receipt":
                value["receipts"].pop("extended")
            else:
                value["artifacts"].append(value["artifacts"][0])
            with self.assertRaises(ValueError):
                c.seal(self.frozen, value, self.root)

    def test_version_source_checksum_and_build_mismatch_refuse(self):
        for change in ("embedded_version", "sha256", "size", "source", "build"):
            value = copy.deepcopy(self.inventory)
            if change == "source":
                value["source"]["commit"] = "e" * 40
            elif change == "build":
                value["build"]["attempt"] += 1
            else:
                value["artifacts"][0][change] = {"size": 1, "sha256": "0" * 64,
                                                 "embedded_version": "1.2.1"}[change]
            with self.assertRaises(ValueError):
                c.seal(self.frozen, value, self.root)

    def test_create_exclusive_and_duplicate_json(self):
        output = self.root / "frozen.json"
        c.write_once(output, self.frozen)
        original = output.read_bytes()
        with self.assertRaises(FileExistsError):
            c.write_once(output, self.frozen)
        self.assertEqual(output.read_bytes(), original)
        output.write_text('{"schema":1,"schema":2}')
        with self.assertRaises(ValueError):
            c.read(output)

    def test_symlink_refuses(self):
        artifact = self.root / self.inventory["artifacts"][0]["name"]
        data = artifact.read_bytes()
        artifact.unlink()
        other = self.root / "other"
        other.write_bytes(data)
        artifact.symlink_to(other)
        with self.assertRaises(ValueError):
            c.seal(self.frozen, self.inventory, self.root)

    def test_real_detached_signature_and_same_bytes_promotion_inputs(self):
        # Ephemeral TEST keys only. Production adapter never creates keys/signs.
        key, public, signature = (self.root / n for n in ("test.key", "test.pub", "test.sig"))
        subprocess.run(["openssl", "genpkey", "-algorithm", "ED25519", "-out", str(key)], check=True)
        subprocess.run(["openssl", "pkey", "-in", str(key), "-pubout", "-out", str(public)], check=True)
        candidate = c.seal(self.frozen, self.inventory, self.root)
        manifest = self.root / "candidate.json"
        c.write_once(manifest, candidate)
        subprocess.run(["openssl", "pkeyutl", "-sign", "-inkey", str(key), "-rawin", "-in",
                        str(manifest), "-out", str(signature)], check=True)
        identity = c.digest(public.read_bytes())
        self.assertEqual(c.verify(candidate, self.root, signature, public, identity), candidate["candidate_id"])
        before = {a["name"]: (self.root / a["name"]).read_bytes() for a in self.inventory["artifacts"]}
        promotion = c.promotion_inputs(candidate)
        self.assertEqual(promotion["authorization"], "NOT_ADMITTED")
        self.assertEqual(promotion["artifacts"], self.inventory["artifacts"])
        self.assertEqual(before, {name: (self.root / name).read_bytes() for name in before})
        with self.assertRaises(ValueError):
            c.verify(candidate, self.root, signature, public, "0" * 64)
        signature.write_bytes(b"bad signature")
        with self.assertRaises(ValueError):
            c.verify(candidate, self.root, signature, public, identity)

    def test_changed_artifact_after_seal_refuses(self):
        candidate = c.seal(self.frozen, self.inventory, self.root)
        (self.root / self.inventory["artifacts"][0]["name"]).write_bytes(b"changed")
        with self.assertRaises(ValueError):
            c.verify(candidate, self.root, self.root / "absent.sig", self.root / "absent.pub", "0" * 64)

    def test_tambo_source_adapter_clean_committed_version_and_dirty_refusal(self):
        repo = self.root / "source"
        repo.mkdir()
        def git(*args):
            return subprocess.check_output(["git", "-C", str(repo), *args], stderr=subprocess.DEVNULL)
        git("init", "-q")
        (repo / "Cargo.toml").write_text('[package]\nname = "tambo"\nversion = "1.2.0"\n')
        (repo / "Cargo.lock").write_text("fixture lock\n")
        git("add", ".")
        git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
            "-c", "commit.gpgsign=false", "commit", "-qm", "fixture final version")
        version, source = c.source_inputs(repo)
        self.assertEqual(version, "1.2.0")
        self.assertEqual(source["commit"], git("rev-parse", "HEAD").decode().strip())
        self.assertEqual(source["lockfiles_sha256"], c.digest(b"fixture lock\n"))
        (repo / "Cargo.toml").write_text('[package]\nname = "tambo"\nversion = "1.2.1"\n')
        with self.assertRaises(ValueError):
            c.source_inputs(repo)


if __name__ == "__main__":
    unittest.main()
