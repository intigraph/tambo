# Reviewed Quipu pair: metadata-only CLI demonstration

Trigger: `caboodle update-release --tool quipu --check` against the same
synthetic installed pair (both 0.11.1) and exact 0.12.0 publication metadata.
The installed baseline refuses Quipu at tool selection; the candidate admits
metadata inspection and reports that checksum/functional proof has not run.
The fixture refuses unexpected HTTP requests and asserts no artifact download,
backup, state write or replacement.

Repeat with two absolute executable paths:

```sh
BEFORE_BINARY=/path/to/baseline/caboodle AFTER_BINARY=/path/to/candidate/caboodle \
  sh docs/demos/quipu-pair/run.sh
```

The recorded baseline is Caboodle 0.2.14 (`02c7f76bc05d`), an installed binary.
The candidate is source revision `0addc618c8fa`, based on `2b9635e`; later
fixture-intent/recording changes and later durability corrections preserve this
metadata-only behavior. The recording does not demonstrate the directory
barrier corrections; those have separate ordered fault controls.
This is synthetic metadata inspection, not a production update or archive
execution. The separate isolated library controls exercise paired replacement,
failed proof and interruption recovery with fixture artifacts. Server startup
and host delivery remain unproven.

`output.log` and `timing.log` are an output-only terminal recording. Download
both and replay with util-linux `scriptreplay --log-out output.log --log-timing
timing.log`; `output.log` also provides a readable transcript. Replay does not
require the test fixture. These small sanitized artifacts are retained in this
directory with the code for review; no external artifact host is provisioned.
