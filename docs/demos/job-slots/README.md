# Explicit account-plan fixture

The same read-only `job-slots` call against base `77efa13` and implementation
`415ef70` shows the missing base command, then the exact bounded account plan.
Changing the manifest without changing its reservation digest refuses. The
fixture contains account metadata only and never invokes `--apply`, creates an
account, enrolls a key, installs a helper or enables readiness.

Build the base and head binaries through the normal guarded source build and
place copies at `demo-bin/tambo-base` and `demo-bin/tambo-head`. Then run
`python3 -I docs/demos/job-slots/run.py`. These are inert fixture inputs, not a
complete Shantytown launch capability or installed native isolation proof.

The retained terminal recording is [output.txt](output.txt) with
[timing.txt](timing.txt). Download both and replay with
`scriptreplay --log-out output.txt --log-timing timing.txt` (util-linux).
Playback was checked from the retained files. These small sanitized text
artifacts remain in this docs directory with the reproducer.
