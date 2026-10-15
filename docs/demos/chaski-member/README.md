# Chaski member configuration proof

The [output recording](output.txt) and [timing](timing.txt) run the same manifest
census on base `47389353fe7f` and this candidate: Chaski 0 -> 1, with Shuttle 1
as the positive control. They show the three programs and the reader's required
firing, UNKNOWN-preserved and resolved result. This is a source configuration
recording; it does not demonstrate a fleet installation.

Reproduce from the repository root with `bash docs/demos/chaski-member/run.sh`.
Replay downloaded files with `scriptreplay --log-out output.txt --log-timing timing.txt`.
The small output/timing pair is retained here, following the existing demo
convention. It requires a terminal replay tool rather than an inline player.

For the actual published-wheel installation and functional negative control,
run `cargo test --locked --test chaski_member_live -- --ignored`. That test
installs all three programs under a temporary HOME, verifies the separate
reader, then requires a no-op writer to fail. The member workflow runs it on
Linux and macOS with Python 3.12. CI does not prove delivery to a user's Mac.
