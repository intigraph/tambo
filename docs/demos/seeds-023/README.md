# Seeds 0.2.3 reviewed release pin

The [terminal recording](output.txt) and [timing file](timing.txt) read the same
member-manifest fields on reviewed base `5ca4a12f42a0848480d9f9f72db241ab32b1e0ca`
and the candidate: Seeds **0.2.2 → 0.2.3**, with the same four target names and
new archive hashes. The PR caption binds the exact demonstrated candidate head.

Reproduce from a clean candidate checkout:

```sh
bash docs/demos/seeds-023/run.sh
```

After downloading the output and timing files, replay with:

```sh
scriptreplay --log-out output.txt --log-timing timing.txt
```

This is a configuration recording, not an installed-host demonstration.
The official bump-member command independently downloaded and hashed all four
published archives, and native identity/version probes ran with a private HOME
and cleared environment. Target set, profiles, identity and functional verifier
remain the same. Installation requires the reviewed normal updater and separate
runtime acceptance. The small recording remains in this directory; playback
requires a terminal replay tool.
