# Successor runtime input demonstration

The unchanged packet on base `7fd277747df3ea63a50c485c7b1f566654eb6b0d`
selects Seeds 0.13.0 and ST `bd499fbc`. The successor selects published Seeds
0.14.0 and ST `dd11e1ce`, with manifest
`33de07c81561a45b3fd49a8f88160b9761300732cc99c16b21670d5c026acb9a`.
Watch the selected input pins change, then the actual runtime inventory verify.
The PR identifies the exact candidate head.

[Recording](output.log) and [timing](timing.log) are retained in Git alongside
this small reproducer. Download both for terminal playback:

```sh
scriptreplay --log-out docs/demos/runtime-ca-successor/output.log --log-timing docs/demos/runtime-ca-successor/timing.log
python3 docs/demos/runtime-ca-successor/demo.py --directory macos-arm64-job-runtime
```

Build the runtime first with the pinned-input recipe in the
[operator packet](../../design/macos-controller-runtime-packet.md). This is an
unprivileged source/configuration demonstration, recorded on Linux. It never
executes the bundled Mac interpreter/client, creates accounts or keys, contacts
a production endpoint, installs files, or proves native job-identity readiness.
