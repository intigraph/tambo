# Controller groups on macOS

With the same macOS-shaped native-command replies, the base refuses a dedicated
controller because `id -G` includes inherited groups. The candidate accepts that
implicit baseline while still refusing staff membership and explicit short-name
or UUID grants, including grants into an otherwise implicit group.

The recording compares base `7d1a175499cd8968a0ca185c17cd6b92fa9e67b5` with
implementation `77217bc55d8a046dcbcaea09e4d8fc2743b2011c`. The later demo commit
adds only these artifacts and their text-handling rules. This is an offline
source reproducer: all native account commands, identity properties and private
home checks are mocked. It does not prove a native Mac receipt transition.
The login name is sanitized; numeric group observations are retained.

- [Recorded output and readable transcript](output.log)
- [Original timing](timing.log)
- [Repeatable reproducer](demo.py)

From the repository root:

```sh
python3 docs/demos/controller-implicit-groups/demo.py
scriptreplay --log-out docs/demos/controller-implicit-groups/output.log \
  --log-timing docs/demos/controller-implicit-groups/timing.log
```

Playback requires the two downloaded logs and util-linux `scriptreplay`;
inline playback is not assumed. The small recording stays with this change in
Git. No accounts, sudo, host files or prepared receipts are changed. Native
operator verification after merge remains separate.
