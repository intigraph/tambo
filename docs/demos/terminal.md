# Review a local installation plan, then verify it

The 21-second `terminal/demo.svg` records a real interview, review of its actual
plan, and native verification in a private HOME. `terminal/transcript.txt` is the
static alternative. No user's installation is changed. The recording explicitly
says hook registration is **not verified**: the local tool proof does not claim
crew configuration or production readiness. Setup completed before the recording;
the animation does not claim to show an installation completing in 21 seconds.

Use Python 3.11+ and the checksummed Tambo v0.6.1 Linux release (or the matching
release for your supported platform). Download the archive and its sibling
`.sha256` from the public release, verify the checksum before extracting, and
keep the extracted executable outside your system PATH.
The Linux archive SHA256
for v0.6.1 is
`da9edd244411e9cd34b2e4bc7a4db3219e1b05832289f95f5d1a3a3aa9777134`.
The capture manifest
records its executable SHA and `tambo --version` result. The source recipe and
release pins are public; no credentials are used.

From the repository root, supply a **new, absent private directory**:

```sh
python3 docs/demos/terminal/prepare.py \
  --tambo /tmp/tambo-release/tambo --workspace /tmp/tambo-demo
python3 docs/demos/terminal-harness/render.py \
  docs/demos/terminal/demo.tape.json --workspace /tmp/tambo-demo \
  --tool tambo=/tmp/tambo-demo/.demo-home/.cargo/bin/tambo \
  --tool python3="$(command -v python3)" --output docs/demos/terminal
```

Preparation copies the real Tambo executable, runs the same interview, then
`doctor`, `apply` and `verify` with all paths rooted in the private HOME. It
stops at the first failure and preserves logs/state rather than resetting them.
It downloads Tambo's embedded checksummed tool releases and local vocabulary;
allow network and disk for that setup. This is the only installation step.
Preparation turns `fixture/answers.json` into `answers.txt`, including the final
Enter that selects the built-in question. The recorded interview reads that file.
`review-plan.py` reads the complete generated TOML and prints its selected tools,
purpose and question; review the full file before applying it elsewhere.

The shared recorder is documented in `terminal-harness.md`. The manifest and
plain transcript retain all verification qualifications. These are assets only;
README integration and any real installation require their own review.
