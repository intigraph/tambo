# Bootstrap compatibility correction

The default release installer must work before the repository rename and while
pinning historical Caboodle releases. This recording runs the same owned release
fixture through the actual installer scripts at:

- Before: `a696fccd13157db1943bbb4c99ed76d8cf84a93f`.
- After: `47d3ee0bf6ececc0150968295fee7d1a221a7baa`.

The fixture publishes latest-version metadata only under the compatibility
repository URL and a checksummed Caboodle-only `v0.2.23` archive. Before exits 1
and installs nothing. After exits 0 and installs the executable Caboodle fixture.
The test harness substitutes fixture responses for curl; this is a source-level
bootstrap demonstration, not a host install or public release.

From a clone containing both commit objects:

```sh
python3 docs/demos/bootstrap-compat/run.py
scriptreplay --timing docs/demos/bootstrap-compat/timing.txt docs/demos/bootstrap-compat/output.txt
```

[Recording transcript](output.txt) and [timing](timing.txt) are small terminal
recording artifacts retained with this source. The separate CI fixture suite
also covers pinned legacy installs, new dual-command archives, incomplete
archives, checksum failures, HTTP 500 and transport failures. Only a missing
Tambo archive (HTTP 404) permits the legacy archive fallback. The 0.3.0 release
and repository rename remain separate steps.
