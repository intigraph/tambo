# Codex turn ingestion without payload argv

One synthetic Stop event carries a 262144-byte assistant message. The base
bundle's actual notify argv fails with Linux `E2BIG` before its consumer starts.
The candidate bundle's actual Stop command reads the same event on stdin and
creates exactly one `codex/agent_turn` row with the fixture session. A non-Stop
negative control is refused and leaves the count at one.

- Base: `e076a8309a07b29d317429df3a91083adb1e98fe`.
- Candidate product: `5b63848bd7068faebfa1de02247d9495326aadcf`.
- Consumer: installed `dp 0.12.2 (0c9e18b)`; database is disposable and private.
- Target: source bundle transport, not a registered or running crew hook.

Run `python3 docs/demos/codex-stop-stdin.py` on Linux with `dp >= 0.10.0`.
The script reads both actual bundle configurations and adds only a global
`--db` option to isolate the candidate command's storage.

The small output and timing files live beside this caption as retained review
artifacts. Download both to replay with util-linux:

```sh
scriptreplay --log-out codex-stop-stdin.output --log-timing codex-stop-stdin.timing
```

[Recorded output](codex-stop-stdin.output) is also a readable transcript;
[timing](codex-stop-stdin.timing) and [reproducer](codex-stop-stdin.py) are retained.
Playback was checked locally. No inline player is claimed. Registration,
provisioning, session refresh and a naturally occurring long-turn ingestion
remain separate installed-acceptance gates.
