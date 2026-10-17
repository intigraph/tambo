"""Run with candidate binary and optional pre-change binary; no network or live policy."""
import pathlib
import subprocess
import sys
import tempfile

candidate = str(pathlib.Path(sys.argv[1]).resolve())
baseline = str(pathlib.Path(sys.argv[2]).resolve()) if len(sys.argv) > 2 else None
with tempfile.TemporaryDirectory() as scratch:
    policy = pathlib.Path(scratch) / "trust.toml"
    repo = "https://github.com/foreign/project.git"
    endpoint = "https://example.org/query"

    def check(binary, expected):
        result = subprocess.run(
            [binary, "packs", "trust", "--file", str(policy), "check-endpoint", repo, endpoint],
            text=True, capture_output=True, check=False,
        )
        print((result.stdout + result.stderr).strip(), flush=True)
        print(f"exit={result.returncode}", flush=True)
        assert result.returncode == expected

    if baseline:
        print("BEFORE: endpoint policy decision has no CLI", flush=True)
        check(baseline, 2)
    print("AFTER: foreign endpoint without consent", flush=True)
    check(candidate, 3)
    assert not policy.exists()
    policy.write_text('[[allow_endpoint]]\nrepo = "github.com/foreign/project"\nendpoint = "https://example.org/query"\n')
    before = policy.read_bytes()
    print("AFTER: exact endpoint consent", flush=True)
    check(candidate, 0)
    assert policy.read_bytes() == before
    policy.unlink()
    print("AFTER: consent revoked", flush=True)
    check(candidate, 3)
