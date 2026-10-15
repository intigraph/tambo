# Stack overview: lead prose stays under the drift guard

This output-only terminal recording runs the same proposed overview through
the parser before and after this change. The old parser refuses prose before
the table; the new parser accepts the whole overview. Changing only the lead
wording still fails the exact-copy check. No live service or deployment is used.

- Before: `4911abb3d31de960753a56107ace798523135ece`.
- Demonstrated source: `ff61939` (later changes add only this recording).
- Input: the proposed `docs/stack-table.md`, copied to both `overview.md` and
  `README.md` in an isolated directory.
- Invocation on each revision: `python3 check-stack-table.py --canonical overview.md README.md`.
- Negative control: replace `governed, bitemporal` only in the README copy.

[Readable output / recording](output.txt) and [timing](timing.txt) are retained
in Git with this caption; later wording changes need a new recording. These
small text artifacts use repository retention and contain no private paths.
Download both, then replay with util-linux:

```sh
scriptreplay --log-out output.txt --log-timing timing.txt
```

Playback was checked with `scriptreplay`; this is a terminal replay, not a video
or a claim that Markdown rendering or a public deployment was tested. The
candidate Caboodle book was also built with `mdbook build docs/book`.
