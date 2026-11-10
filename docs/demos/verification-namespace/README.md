# Explicit namespace for the verification write

An unconfigured local Quipu store rejects the verifier's episode because it
mints named entities. The new invocation supplies the probe's own reserved
example namespace; the exact marker is then readable from that same store.

The recording exercises that command boundary using Quipu 0.17.0. The first
invocation matches Tambo base `72be5afd5dc9a985661da7561c17956f343d6afd`;
the second matches the changed `verify_core` invocation (adapter source blob
`ef4c41bdf2c7b3de8ca5734be4af2aac459a5fef`). Both use the same episode and
disposable local database. No server or model is used. This is a command-level
reproducer, not a recording of a production installation.

Run from the repository root with a Quipu 0.16+ CLI on PATH:

```sh
python3 docs/demos/verification-namespace/demo.py
```

The small sanitized terminal recording is retained here with the source.
Download [output.log](output.log) and [timing.log](timing.log), then replay:

```sh
scriptreplay --log-out output.log --log-timing timing.log
```

The output log is also the readable transcript. Inline video playback is not
provided. The broader installer round trip is checked separately in an isolated
environment; this recording establishes only the namespace/write/read boundary.
