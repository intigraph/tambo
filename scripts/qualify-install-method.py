#!/usr/bin/env python3
"""Exercise Quipu package ownership on fresh GitHub-hosted runners only.

No credentials, remote Quipu, live-machine installation or provisioning path.
Reports distinguish a package-manager upgrade command from a version change.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time
import urllib.request
import uuid

KEY_URL = "https://scbrown.github.io/apt/key.gpg"
FINGERPRINT = "AB5625C5612F643BF76464DF0DC99924994EC959"
KEYRING = "/etc/apt/keyrings/scbrown-stack.gpg"
SOURCE = "/etc/apt/sources.list.d/scbrown-stack.list"
IMAGE = re.compile(r"ghcr\.io/intigraph/quipu(?::[0-9][A-Za-z0-9._-]{0,63}|@sha256:[0-9a-f]{64})")


class Failure(RuntimeError):
    pass


def require_hosted(method, output, env):
    expected = "macOS" if method == "brew" else "Linux"
    if (env.get("GITHUB_ACTIONS") != "true" or env.get("INSTALL_METHOD_RUNNER") != "github-hosted"
            or env.get("RUNNER_OS") != expected
            or env.get("GITHUB_REPOSITORY") not in ("intigraph/tambo", "scbrown/tambo")):
        raise Failure("requires the admitted fresh GitHub-hosted runner; local execution refused")
    base = Path(env["RUNNER_TEMP"]).resolve()
    path = output.resolve()
    if path == base or not path.is_relative_to(base) or path.exists():
        raise Failure("report directory must be new and below RUNNER_TEMP")


def image_reference(value):
    if not IMAGE.fullmatch(value):
        raise Failure("select a versioned or digest-pinned public Quipu image")
    return value


def primary_fingerprints(text):
    result, primary = [], False
    for line in text.splitlines():
        columns = line.split(":")
        if columns[0] == "pub":
            primary = True
        elif columns[0] == "fpr" and primary:
            if len(columns) <= 9:
                raise Failure("malformed primary fingerprint")
            result.append(columns[9])
            primary = False
    return result


def mcp_responses(text):
    replies = {}
    for line in text.splitlines():
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict) and type(value.get("id")) is int and value["id"] in (1, 2):
            if value["id"] in replies or "error" in value or not isinstance(value.get("result"), dict):
                raise Failure("MCP response is duplicated or refused")
            replies[value["id"]] = value["result"]
    if (set(replies) != {1, 2} or not isinstance(replies[1].get("protocolVersion"), str)
            or not replies[1]["protocolVersion"] or not isinstance(replies[2].get("tools"), list)
            or not replies[2]["tools"]):
        raise Failure("MCP initialization/tool-list response is incomplete")
    return replies


def versions(run, commands):
    raw = [run.command([str(path), "--version"]).stdout for path in commands]
    parsed = []
    for text in raw:
        match = re.search(r"(?im)^quipu(?:-server)?\s+v?(\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?)\b", text)
        if not match:
            raise Failure("an installed command returned no Quipu version")
        parsed.append(match.group(1))
    if not parsed or len(set(parsed)) != 1:
        raise Failure("installed companion versions disagree")
    return raw, parsed[0]


class Run:
    def __init__(self, method, output):
        self.root = output
        output.mkdir(parents=True)
        self.report = dict(schema=1, method=method, status="RUNNING",
                           checkout=os.environ["GITHUB_SHA"], run_id=os.environ["GITHUB_RUN_ID"],
                           runner_image=os.environ.get("ImageVersion", "UNKNOWN"),
                           commands=[], upgrade_version_changed=False,
                           full_tambo="UNVERIFIED: canonical credential not admitted")
        self.cleanup = []
        self.data = output / "project/.bobbin/quipu/quipu.db"
        self.data.parent.mkdir(parents=True)
        self.data.write_bytes(b"qualification-owned-data-marker\n")
        self.marker = hashlib.sha256(self.data.read_bytes()).hexdigest()

    def command(self, argv, *, allowed=(0,), timeout=600, input=None, env=None):
        print("+ " + shlex.join(argv), flush=True)
        started = time.monotonic()
        index = len(self.report["commands"])
        try:
            result = subprocess.run(argv, input=input, env=env, capture_output=True,
                                    text=True, timeout=timeout, check=False)
        except subprocess.TimeoutExpired as error:
            def text(value):
                return value.decode(errors="replace") if isinstance(value, bytes) else value or ""
            (self.root / f"{index:02d}.stdout.log").write_text(text(error.stdout))
            (self.root / f"{index:02d}.stderr.log").write_text(text(error.stderr))
            self.report["commands"].append(dict(argv=argv, exit=None, timed_out=True,
                                               seconds=time.monotonic() - started))
            raise Failure(f"command {index} timed out; inspect its retained output") from error
        (self.root / f"{index:02d}.stdout.log").write_text(result.stdout)
        (self.root / f"{index:02d}.stderr.log").write_text(result.stderr)
        self.report["commands"].append(dict(argv=argv, exit=result.returncode,
                                           seconds=time.monotonic() - started))
        print(f"exit={result.returncode}", flush=True)
        if result.returncode not in allowed:
            raise Failure(f"command {index} failed; inspect its retained output")
        return result

    def preserved(self):
        if not self.data.is_file() or hashlib.sha256(self.data.read_bytes()).hexdigest() != self.marker:
            raise Failure("package removal changed the independent project data marker")
        self.report["data_preserved_by_package_removal"] = True
        self.data.unlink()
        if self.data.exists():
            raise Failure("explicit owned data removal failed")
        self.report["explicit_owned_data_removal"] = True

    def finish(self, error=None):
        failures = []
        for cleanup in reversed(self.cleanup):
            try:
                cleanup()
            except (Failure, OSError, subprocess.SubprocessError) as exc:
                failures.append(type(exc).__name__)
        self.report["cleanup_errors"] = failures
        self.report["status"] = "FAIL" if error or failures else "PASS"
        if error:
            self.report["error"] = type(error).__name__ + ": " + str(error)
        (self.root / "result.json").write_text(json.dumps(self.report, indent=2) + "\n")
        print(json.dumps(self.report), flush=True)
        return 1 if error or failures else 0


def brew(run, *, trust_formula=False):
    if not trust_formula:
        raise Failure("Homebrew requires explicit --trust-brew-formula admission for scbrown/tap/quipu")
    run.report["formula_trust_admitted"] = "scbrown/tap/quipu"
    formulas = run.command(["brew", "list", "--formula"]).stdout.splitlines()
    prefix = Path(run.command(["brew", "--prefix"]).stdout.strip())
    names = [prefix / "bin" / name for name in ("quipu", "qp", "quipu-server")]
    if "quipu" in formulas or any(os.path.lexists(path) for path in names):
        raise Failure("Homebrew Quipu is already present; this is not a clean install")
    run.report["baseline_absent"] = True
    def remove():
        if "quipu" in run.command(["brew", "list", "--formula"]).stdout.splitlines():
            # Cleanup uses the installed formula receipt; after uninstall,
            # Homebrew can instead reload a now-untrusted tap formula.
            # Keep a refusal fatal, while still removing our owned install.
            try:
                run.command(["brew", "cleanup", "quipu"])
            finally:
                run.command(["brew", "uninstall", "quipu"])
        if "quipu" in run.command(["brew", "list", "--formula"]).stdout.splitlines():
            raise Failure("formula remains after removal")
        if any(os.path.lexists(path) for path in names):
            raise Failure("a Homebrew-owned command or alias remains")
        run.report["package_and_alias_removed"] = True
    def untrust():
        run.command(["brew", "untrust", "--formula", "scbrown/tap/quipu"])
        run.report["formula_trust_removed"] = True
    run.cleanup.append(untrust)
    run.cleanup.append(remove)
    run.command(["brew", "tap", "scbrown/tap"])
    run.command(["brew", "trust", "--formula", "scbrown/tap/quipu"])
    tap = run.command(["brew", "--repository", "scbrown/tap"]).stdout.strip()
    run.report["tap_commit_before"] = run.command(["git", "-C", tap, "rev-parse", "HEAD"]).stdout.strip()
    run.command(["brew", "install", "scbrown/tap/quipu"])
    before, version_before = versions(run, names)
    if (not names[1].is_symlink() or Path(os.readlink(names[1])).is_absolute()
            or names[1].resolve() != names[0].resolve()):
        raise Failure("qp must be the formula's relative alias")
    run.command(["brew", "test", "scbrown/tap/quipu"])
    run.command(["brew", "list", "--verbose", "quipu"])
    run.command(["brew", "update"])
    run.command(["brew", "upgrade", "quipu"])
    after, version_after = versions(run, names)
    run.command(["brew", "test", "scbrown/tap/quipu"])
    run.report.update(versions_before=before, versions_after=after,
                      version_before=version_before, version_after=version_after,
                      upgrade_command_executed=True, upgrade_version_changed=version_before != version_after)
    remove()
    run.cleanup.pop()  # removal completed
    run.cleanup.pop()  # attempt untrust once; a refusal must not be retried
    untrust()
    run.preserved()


def apt(run):
    baseline = run.command(["dpkg-query", "-W", "-f=${db:Status-Status}", "quipu"], allowed=(0, 1))
    if baseline.returncode == 0 or Path(KEYRING).exists() or Path(SOURCE).exists():
        raise Failure("Quipu package/repository is already present; clean install required")
    run.report["baseline_absent"] = True
    def remove():
        found = run.command(["dpkg-query", "-W", "-f=${db:Status-Status}", "quipu"], allowed=(0, 1))
        if found.returncode == 0:
            run.command(["sudo", "-n", "apt-get", "purge", "--yes", "quipu"])
        after = run.command(["dpkg-query", "-W", "-f=${db:Status-Status}", "quipu"], allowed=(0, 1))
        if after.returncode != 1:
            raise Failure("package remains after purge")
        for path in run.report.get("package_commands", []):
            if os.path.lexists(path):
                raise Failure("package-owned command remains after purge")
        run.command(["sudo", "-n", "rm", "-f", "--", SOURCE, KEYRING])
        run.command(["sudo", "-n", "apt-get", "update"])
        run.report["package_and_alias_removed"] = True
    run.cleanup.append(remove)
    key = run.root / "key.gpg"
    with urllib.request.urlopen(KEY_URL, timeout=30) as response:
        data = response.read(1024 * 1024 + 1)
    if len(data) > 1024 * 1024:
        raise Failure("repository key exceeds byte budget")
    key.write_bytes(data)
    gpg_home = run.root / "gnupg"; gpg_home.mkdir(mode=0o700)
    keys = run.command(["gpg", "--homedir", str(gpg_home), "--batch", "--show-keys",
                        "--with-colons", str(key)]).stdout
    if primary_fingerprints(keys) != [FINGERPRINT]:
        raise Failure("repository primary signing key fingerprint differs")
    run.report["repository_primary_fingerprint"] = FINGERPRINT
    run.command(["sudo", "-n", "install", "-D", "-m", "0644", str(key), KEYRING])
    entry = run.root / "source.list"
    entry.write_text(f"deb [signed-by={KEYRING}] https://scbrown.github.io/apt stable main\n")
    run.command(["sudo", "-n", "install", "-m", "0644", str(entry), SOURCE])
    run.command(["sudo", "-n", "apt-get", "update"])
    run.command(["sudo", "-n", "env", "DEBIAN_FRONTEND=noninteractive", "apt-get", "install", "--yes", "quipu"])
    files = run.command(["dpkg-query", "-L", "quipu"]).stdout.splitlines()
    commands = [path for path in files if Path(path).name in ("quipu", "qp", "quipu-server")
                and os.path.isfile(path) and os.access(path, os.X_OK)]
    if not {"quipu", "quipu-server"} <= {Path(path).name for path in commands}:
        raise Failure("package does not provide both executable companions")
    run.report["package_commands"] = commands
    run.report["package_version"] = run.command(["dpkg-query", "-W", "-f=${Version}", "quipu"]).stdout
    before, version_before = versions(run, commands)
    run.command(["sudo", "-n", "apt-get", "update"])
    run.command(["sudo", "-n", "env", "DEBIAN_FRONTEND=noninteractive", "apt-get", "install",
                 "--yes", "--only-upgrade", "quipu"])
    after, version_after = versions(run, commands)
    run.report.update(versions_before=before, versions_after=after,
                      version_before=version_before, version_after=version_after,
                      upgrade_command_executed=True, upgrade_version_changed=version_before != version_after)
    remove()
    run.cleanup.clear()
    run.preserved()


def container(run, image):
    image_reference(image)
    run.command(["docker", "version"])
    existed = run.command(["docker", "image", "inspect", image], allowed=(0, 1))
    if existed.returncode == 0:
        raise Failure("selected image is already present; clean pull required")
    run.command(["docker", "pull", image])
    run.cleanup.append(lambda: run.command(["docker", "image", "rm", image]))
    info = json.loads(run.command(["docker", "image", "inspect", image]).stdout)
    digests = info[0].get("RepoDigests", [])
    pinned = next((value for value in digests if re.fullmatch(r"ghcr.io/intigraph/quipu@sha256:[0-9a-f]{64}", value)), None)
    if pinned is None:
        raise Failure("pulled image has no matching immutable registry digest")
    run.report.update(requested_image=image, actual_image=pinned)
    for binary in ("quipu", "quipu-server"):
        run.command(["docker", "run", "--rm", "--entrypoint", f"/usr/local/bin/{binary}", pinned, "--version"])
    volume = "quipu-method-" + uuid.uuid4().hex
    run.command(["docker", "volume", "create", "--label", "install-method-qualification=true", volume])
    run.cleanup.append(lambda: run.command(["docker", "volume", "rm", volume]))
    probe = run.root / "probe.ttl"
    marker = "qualification-" + uuid.uuid4().hex
    probe.write_text(f'<urn:method:subject> <urn:method:predicate> "{marker}" .\n')
    # The image runs UID10001: only this public fixture file is made readable.
    run.root.chmod(0o755); probe.chmod(0o644)
    def command(args):
        return ["docker", "run", "--rm", "--volume", volume + ":/data",
                "--volume", str(probe) + ":/probe.ttl:ro", "--entrypoint", "/usr/local/bin/quipu",
                pinned, *args, "--db", "/data/quipu.db"]
    run.command(command(["knot", "/probe.ttl"]))
    result = run.command(command(["read", "SELECT ?value WHERE { <urn:method:subject> <urn:method:predicate> ?value }"]))
    if marker not in result.stdout:
        raise Failure("a replacement container could not read the retained volume marker")
    run.command(["docker", "volume", "inspect", volume])
    messages = [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2024-11-05", "capabilities": {},
            "clientInfo": {"name": "install-method-qualification", "version": "1"}}},
        {"jsonrpc": "2.0", "method": "notifications/initialized"},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}},
    ]
    mcp_id = run.command(["docker", "create", "--interactive", "--volume", volume + ":/data", pinned]).stdout.strip()
    if not re.fullmatch(r"[0-9a-f]{64}", mcp_id):
        raise Failure("MCP container identity is unreadable")
    # The exact owned CID is retained before starting; even a timeout may leave
    # a server running after Docker's client exits, so cleanup must name it.
    run.cleanup.append(lambda: run.command(["docker", "rm", "--force", mcp_id]))
    stream = run.command(["docker", "start", "--attach", "--interactive", mcp_id],
                         input="".join(json.dumps(row) + "\n" for row in messages), timeout=120)
    mcp_responses(stream.stdout)
    state = json.loads(run.command(["docker", "inspect", "--format", "{{json .State}}", mcp_id]).stdout)
    if state.get("Running") is not False or state.get("ExitCode") != 0:
        raise Failure("MCP server did not terminate successfully")
    run.command(["docker", "rm", mcp_id]); run.cleanup.pop()
    run.report["mcp_initialize_and_tools_list"] = True
    # Back up only the fixture database; never export a generated signing key.
    backup = run.root / "quipu-backup.db"
    created = run.command(["docker", "create", "--volume", volume + ":/data", pinned]).stdout.strip()
    if not re.fullmatch(r"[0-9a-f]{64}", created):
        raise Failure("container identity is unreadable")
    run.cleanup.append(lambda: run.command(["docker", "rm", created]))
    run.command(["docker", "cp", created + ":/data/quipu.db", str(backup)])
    run.command(["docker", "rm", created]); run.cleanup.pop()
    source_hash = run.command(["docker", "run", "--rm", "--volume", volume + ":/data:ro",
                              "--entrypoint", "/usr/bin/sha256sum", pinned, "/data/quipu.db"]).stdout.split()[0]
    backup_hash = hashlib.sha256(backup.read_bytes()).hexdigest()
    if backup_hash != source_hash:
        raise Failure("owned database backup did not match retained volume bytes")
    run.report.update(volume_survived_container_removal=True, backup_sha256=backup_hash,
                      upgrade_command_executed=True, upgrade_version_changed=False,
                      upgrade_scope="recreation with the same verified digest; no version transition claimed")
    run.command(["docker", "volume", "rm", volume]); run.cleanup.pop()
    if run.command(["docker", "volume", "inspect", volume], allowed=(0, 1)).returncode != 1:
        raise Failure("owned volume remains after explicit data removal")
    run.command(["docker", "image", "rm", image]); run.cleanup.pop()
    if run.command(["docker", "image", "inspect", image], allowed=(0, 1)).returncode != 1:
        raise Failure("selected image reference remains after removal")
    run.report["package_and_alias_removed"] = True
    run.preserved()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("method", choices=("brew", "apt", "container"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--image", default="ghcr.io/intigraph/quipu:0.8.1")
    parser.add_argument("--trust-brew-formula", action="store_true",
                        help="explicitly trust only scbrown/tap/quipu on the disposable runner")
    args = parser.parse_args(argv)
    try:
        require_hosted(args.method, args.output, os.environ)
    except (Failure, KeyError) as error:
        print(str(error), file=sys.stderr)
        return 2
    run = Run(args.method, args.output)
    error = None
    try:
        {"brew": lambda r: brew(r, trust_formula=args.trust_brew_formula), "apt": apt, "container": lambda r: container(r, args.image)}[args.method](run)
    except (Failure, OSError, ValueError, subprocess.SubprocessError) as caught:
        error = caught
    return run.finish(error)


if __name__ == "__main__":
    raise SystemExit(main())
