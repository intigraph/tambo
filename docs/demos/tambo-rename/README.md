# Tambo package and compatibility command

This terminal recording compares the retained `cargo metadata` baseline at
`47389353fe7f73e6668745a7607125a1427a0d6e` with source candidate
`3278c3402f55` (the implementation commit). The baseline declares the Caboodle
package and one executable. The candidate declares Tambo and both `tambo` and
`caboodle`, then runs each candidate's actual version command. Both entry points
use the same CLI implementation. This is a source fixture, not an installed
host, repository redirect, registry publication, or package-manager cutover.

The retained baseline is identified in the recording; it is not re-executed.
Reproduce the candidate portion with:

```sh
cargo build --release --locked
sh docs/demos/tambo-rename/run.sh
```

Download [output.log](output.log) and [timing.log](timing.log), then replay with
util-linux `scriptreplay`:

```sh
scriptreplay --log-out output.log --log-timing timing.log
```

Replay was checked locally. These are a terminal recording and timing file,
not an inline video. The small sanitized recording is retained in this demo
directory with the PR. The independent release fixture also checks matching
plan output, invalid-command refusal, a missing compatibility binary, and a
corrupted archive. Full self-update and installed migration acceptance follow
integration with the pending package-backend changes.
