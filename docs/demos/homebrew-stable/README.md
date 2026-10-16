# Homebrew stable update demonstration

Baseline: checksummed official `v0.3.4`, source
`e5cf4643315ca631f73be21f67b1182ea1a0df81`.
Candidate: clean source `a3ef5ee3faa0`, same package version and a different
reported source identity. This recording is a local fixture, not a Mac install.

```sh
scriptreplay --log-timing timing.log --log-out output.log
# Or exercise your own two real installer binaries:
python3 demo.py /path/to/baseline /path/to/candidate
```

The actual installer CLI receives a normal retrieval plan and disposable
Homebrew, member, and package receipts. Curl is stubbed to refuse any network
attempt; fixture executables are owned files in the temporary home.

With installed 0.28 and a stale local tap, the baseline reports ahead of the
embedded 0.27.3 pin and sends neither metadata refresh nor upgrade. The candidate
refreshes exactly once, discovers stable 0.29, upgrades exactly once and verifies
the new receipt, executable and positive/negative index controls. With the same
formula pinned, the candidate refreshes metadata but leaves 0.28 installed and
sends no upgrade or unpin. The recording asserts all four outcomes.

The regression suite separately covers newer installed versions, failed
refresh/metadata/upgrade, an unchanged receipt after a nominal upgrade, malformed
channel data and unrelated formula entries. Source checks are not evidence of
release publication, scheduled host adoption or a quiet acceptance window.
