# Vision compatibility-inventory correction

Main's compatibility guard rejects the vision file because its ledger contains
one hash for a line already removed by the reviewed vision update. The candidate
carries the existing one-entry correction; no new allowance is added.

Base: `3c6688ff6b3b117185a5bafc8dd2776f80be663f`. Candidate: this correction.
The driver uses disposable fixtures containing the actual vision and its ledger
entry, verifies every other ledger entry is unchanged, and invokes the real
checker. Before exits 1, after exits 0, and a new-line negative control exits 1.
The real vision text is unchanged. Full repository checks run separately.

Run `just --command python3 docs/demos/vision-inventory/demo.py`.
[Actual output](output.txt) and [timing](timing.txt) can be replayed with
`scriptreplay --log-out output.txt --log-timing timing.txt`.
This records local fixture behavior; hosted acceptance remains separate.
