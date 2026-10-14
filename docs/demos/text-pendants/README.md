# Text pendant conversion

The recording compares the original checked-in SQLite artifacts at
`8cec4fd3e07c1b3da98563879c1ecbb6a6ecdff5` with the candidate text directories.
Watch the two 221,184-byte SQLite payloads become four inspectable UTF-8 members
per pendant, preserving 54 and 37 graph triples. The old filename aliases point
to those directories, not to SQLite files.

Download [output.log](output.log) and [timing.log](timing.log), then replay with:

```bash
scriptreplay --log-out output.log --log-timing timing.log
```

Reproduce with `python3 scripts/probe-text-pendants.py`. Both targets are source
fixtures, not an installed service. Native tests separately check hashes, exact
graph preservation, tamper refusal, unknown-vocabulary quarantine, explicitly
configured receiver staging and failed-build preservation. No promotion,
certification, automatic loading or deployment is demonstrated.
