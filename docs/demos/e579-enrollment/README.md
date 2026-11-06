# e579: disabled own-UID candidates and bounded delivery

Target: offline source/fixtures, base `62b2727c14450ccce147e5360663a1e81ea856f2`
to source `0b41557`. The base tree has neither renderer nor bounded wrapper.
The recording shows the real candidate renderer binding `_stctl`, keeping its
launchd job disabled, rejecting UID501, then a disposable fake receiver exiting75
twice. The third wrapper invocation refuses and the child-call count stays2.
No native keys, accounts, launchd/sudo operations or network writes occur.

Replayable terminal [output](output.log) and [timing](timing.log):

```sh
scriptreplay --log-out output.log --log-timing timing.log
```

Repeat from the repository root with `python3 -B docs/demos/e579-enrollment/demo.py`.
This is a terminal recording, not an inline video. It proves candidate behavior
only. Native topology, enrollment/signing/isolation and readiness stay unproved;
the actual route sheet and operator admission remain required. Small sanitized
recording and source are retained in the existing docs/demos convention.
