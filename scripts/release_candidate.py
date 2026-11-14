#!/usr/bin/env python3
"""Offline immutable candidate inputs; no download, build, signing or promotion.

The existing producer freezes a final-version plan BEFORE building, signs the
sealed manifest with its existing publisher, and consumers verify pinned bytes.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

DOMAIN = b"quipu-stack/release-candidate/v1\0"
PLAN_DOMAIN = b"quipu-stack/release-plan/v1\0"
HEX = r"[0-9a-f]{64}"


def require(value, message):
    if not value:
        raise ValueError(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"),
                      ensure_ascii=True, allow_nan=False).encode() + b"\n"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def read(path):
    return json.loads(Path(path).read_text(), object_pairs_hook=unique,
                      parse_constant=lambda _: require(False, "nonfinite JSON"))


def fields(value, names):
    require(isinstance(value, dict) and set(value) == set(names.split()),
            "unknown or missing fields")


def sha(value):
    require(isinstance(value, str) and re.fullmatch(HEX, value), "invalid SHA256")


def token(value):
    require(isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", value),
            "invalid identifier")


def plan(value):
    fields(value, "schema tool repository version source build targets toolchain dependencies "
           "compatibility_sha256 breaking_records previous_stable_sha256 restore_plan_sha256 "
           "capabilities feature_profile experimental_features required_receipts verification_policy_sha256")
    require(value["schema"] == "release-plan/v1", "unknown plan schema")
    token(value["tool"])
    require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", value["repository"]),
            "invalid repository")
    require(re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", value["version"]),
            "candidate requires final per-tool version, never rc/nightly rewriting")
    fields(value["source"], "commit tree_sha256 lockfiles_sha256")
    require(re.fullmatch(r"[0-9a-f]{40}", value["source"]["commit"]), "full source commit required")
    sha(value["source"]["tree_sha256"])
    sha(value["source"]["lockfiles_sha256"])
    fields(value["build"], "run_id attempt workflow_sha256")
    require(re.fullmatch(r"[1-9][0-9]*", value["build"]["run_id"]), "immutable build run required")
    require(type(value["build"]["attempt"]) is int and value["build"]["attempt"] > 0,
            "build attempt required; retries create new identities")
    sha(value["build"]["workflow_sha256"])
    for key in ("toolchain", "dependencies"):
        require(isinstance(value[key], dict) and value[key], "empty dependency/toolchain inventory")
        for name, identity in value[key].items():
            token(name)
            sha(identity)
    for key in ("compatibility_sha256", "previous_stable_sha256", "restore_plan_sha256",
                "verification_policy_sha256"):
        sha(value[key])
    for key in ("breaking_records", "experimental_features", "required_receipts"):
        require(isinstance(value[key], list) and len(value[key]) == len(set(value[key])),
                "invalid or duplicate evidence inventory")
        for name in value[key]:
            token(name)
    require(value["required_receipts"], "test receipt inventory required")
    token(value["feature_profile"])
    fields(value["capabilities"], "store_read store_write pendant_read pendant_write cli config")
    for versions in value["capabilities"].values():
        require(isinstance(versions, list) and versions and len(versions) == len(set(versions)),
                "explicit supported capability versions required")
        for version in versions:
            token(version)
    require(isinstance(value["targets"], list) and value["targets"], "empty advertised target set")
    names = set()
    for target in value["targets"]:
        fields(target, "os architecture artifacts")
        token(target["os"])
        token(target["architecture"])
        identity = (target["os"], target["architecture"])
        require(identity not in names, "duplicate target")
        names.add(identity)
        require(isinstance(target["artifacts"], list) and target["artifacts"], "missing distributions")
        seen = set()
        for artifact in target["artifacts"]:
            fields(artifact, "name kind")
            token(artifact["name"])
            require(artifact["name"] not in seen, "duplicate distribution")
            seen.add(artifact["name"])
            require(artifact["kind"] in ("archive", "wheel", "crate", "vocabulary", "library", "pendant"),
                    "unsupported distribution adapter")
    return value


def freeze(value):
    value = plan(value)
    return {"schema": "frozen-release-plan/v1", "plan": value,
            "plan_id": digest(PLAN_DOMAIN + canonical(value))}


def frozen(value):
    fields(value, "schema plan plan_id")
    require(value == freeze(value["plan"]), "frozen plan identity mismatch")
    return value["plan"]


def source_inputs(checkout):
    """Inspect committed Tambo metadata; never patch versions or build."""
    def git(*args):
        return subprocess.check_output(["git", "-C", str(checkout), *args])
    require(not git("status", "--porcelain"), "freeze requires a clean checkout")
    manifest = git("show", "HEAD:Cargo.toml").decode()
    require("[package]\n" in manifest, "Tambo package metadata required")
    package = manifest.split("[package]\n", 1)[1].split("\n[", 1)[0]
    require(re.search(r'^name = "tambo"$', package, re.M), "not the Tambo adapter")
    version = re.search(r'^version = "([^"]+)"$', package, re.M)
    require(version is not None, "committed explicit package version required")
    return version[1], {"commit": git("rev-parse", "HEAD").decode().strip(),
                        "tree_sha256": digest(git("ls-tree", "-rz", "--full-tree", "HEAD")),
                        "lockfiles_sha256": digest(git("show", "HEAD:Cargo.lock"))}


def expected(value):
    return {(t["os"], t["architecture"], a["name"]): a["kind"]
            for t in value["targets"] for a in t["artifacts"]}


def local_file(root, name):
    token(name)
    path = root / name
    require(not path.is_symlink() and path.is_file(), "missing or symlink artifact")
    return path


def seal(freeze_record, inventory, root):
    value = frozen(freeze_record)
    fields(inventory, "schema plan_id source build artifacts receipts")
    require(inventory["schema"] == "candidate-build/v1", "unknown build record")
    require(inventory["plan_id"] == freeze_record["plan_id"], "build did not bind frozen plan")
    require(inventory["source"] == value["source"] and inventory["build"] == value["build"],
            "source or build differs from frozen inputs")
    require(set(inventory["receipts"]) == set(value["required_receipts"]), "missing/extra test receipts")
    for receipt in inventory["receipts"].values():
        sha(receipt)
    wanted, found = expected(value), {}
    for artifact in inventory["artifacts"]:
        fields(artifact, "os architecture name kind url size sha256 embedded_version")
        key = (artifact["os"], artifact["architecture"], artifact["name"])
        require(key in wanted and key not in found, "unexpected/duplicate distribution")
        require(artifact["kind"] == wanted[key], "distribution kind differs")
        require(artifact["embedded_version"] == value["version"], "version patched or mismatched")
        # URLs are immutable publisher locators, never fetched by this offline adapter.
        require(isinstance(artifact["url"], str) and artifact["url"].startswith("https://")
                and not any(c in artifact["url"] for c in ("@", "?", "#", "\n", "\r")),
                "public credential-free HTTPS locator required")
        sha(artifact["sha256"])
        data = local_file(root, artifact["name"]).read_bytes()
        require(type(artifact["size"]) is int and artifact["size"] == len(data) and len(data) > 0,
                "artifact size mismatch")
        require(digest(data) == artifact["sha256"], "artifact checksum mismatch")
        found[key] = artifact
    require(set(found) == set(wanted), "missing advertised target/package")
    payload = {"schema": "release-candidate/v1", "experimental": True,
               "frozen": freeze_record, "build": inventory}
    return {"candidate_id": digest(DOMAIN + canonical(payload)), "payload": payload}


def verify(candidate, root, signature, public_key, key_sha256):
    fields(candidate, "candidate_id payload")
    payload = candidate["payload"]
    fields(payload, "schema experimental frozen build")
    require(payload["schema"] == "release-candidate/v1" and payload["experimental"] is True,
            "candidate must retain experimental channel")
    require(candidate == seal(payload["frozen"], payload["build"], root), "candidate identity differs")
    sha(key_sha256)
    require(digest(public_key.read_bytes()) == key_sha256, "untrusted signing identity")
    key_info = subprocess.run(["openssl", "pkey", "-pubin", "-in", str(public_key),
                               "-text_pub", "-noout"], capture_output=True, check=False)
    require(key_info.returncode == 0 and key_info.stdout.startswith(b"ED25519 Public-Key:"),
            "only reviewed Ed25519 detached provenance is supported")
    # Existing publisher supplies the detached signature. No key minting/signing,
    # downloaded verifier, arbitrary command, or trust in a verified=true field.
    with tempfile.TemporaryDirectory(prefix="candidate-verify-") as directory:
        message = Path(directory) / "candidate.json"
        message.write_bytes(canonical(candidate))
        result = subprocess.run(["openssl", "pkeyutl", "-verify", "-pubin", "-inkey",
                                 str(public_key), "-rawin", "-in", str(message),
                                 "-sigfile", str(signature)], capture_output=True, check=False)
        require(result.returncode == 0, "detached signature verification refused")
    return candidate["candidate_id"]


def write_once(path, value):
    # Exclusive creation: no clobber, including identical replay or symlinks.
    with path.open("xb") as output:
        output.write(canonical(value))


def promotion_inputs(candidate):
    """Only call after verify; inputs are not a promotion authorization receipt."""
    value = candidate["payload"]["frozen"]["plan"]
    return {"schema": "same-byte-promotion-inputs/v1", "candidate_id": candidate["candidate_id"],
            "version": value["version"], "authorization": "NOT_ADMITTED",
            "artifacts": candidate["payload"]["build"]["artifacts"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    p = commands.add_parser("freeze")
    p.add_argument("plan", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("--checkout", required=True, type=Path)
    p = commands.add_parser("seal")
    p.add_argument("frozen", type=Path)
    p.add_argument("inventory", type=Path)
    p.add_argument("artifacts", type=Path)
    p.add_argument("output", type=Path)
    p = commands.add_parser("verify")
    p.add_argument("candidate", type=Path)
    p.add_argument("artifacts", type=Path)
    p.add_argument("signature", type=Path)
    p.add_argument("public_key", type=Path)
    p.add_argument("key_sha256")
    p.add_argument("--promotion-inputs", type=Path,
                   help="write inert same-byte inputs; does not approve publication")
    args = parser.parse_args()
    try:
        if args.command == "freeze":
            value = read(args.plan)
            version, source = source_inputs(args.checkout)
            require(value["version"] == version and value["source"] == source,
                    "plan differs from committed Tambo source/version")
            write_once(args.output, freeze(value))
        elif args.command == "seal":
            write_once(args.output, seal(read(args.frozen), read(args.inventory), args.artifacts))
        else:
            candidate = read(args.candidate)
            print(verify(candidate, args.artifacts, args.signature,
                         args.public_key, args.key_sha256))
            if args.promotion_inputs:
                write_once(args.promotion_inputs, promotion_inputs(candidate))
    except (ValueError, OSError, TypeError, KeyError) as error:
        parser.exit(1, "candidate REFUSED: " + str(error) + "\n")


if __name__ == "__main__":
    main()
