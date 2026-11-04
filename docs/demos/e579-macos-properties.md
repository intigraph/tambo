# macOS account-property and umask regression

This fixture recording compares base `8755317372950f071508d31c92ae3107a1d9e9d2` with the candidate packet script (SHA256 `ee2e3748882fe28f02e611fce73db4632b27ef0b0dec009e225d671d969ee523`). It feeds the real macOS 15 property spelling `dsAttrTypeNative:IsHidden: 1` to the reader, then supplies duplicate bare/namespaced aliases and creates a temporary directory under umask 077.

The base misparses the property, accepts duplicate aliases, and creates mode 0700. The candidate reads IsHidden=1, refuses duplicate aliases, and creates mode 0755. Native command responses and root ownership validation are mocked; directory permissions are real. This is source behavior, not a macOS recovery or installation receipt.

[Transcript](e579-macos-properties.txt), [timing](e579-macos-properties.timing), [reproducer](e579-macos-properties.py). Small output/timing files are retained with the source in the existing demos directory. Replay with:

```sh
scriptreplay --log-out docs/demos/e579-macos-properties.txt --log-timing docs/demos/e579-macos-properties.timing
```

Repeat the fixture with `python3 docs/demos/e579-macos-properties.py` from a full source checkout. Playback was checked with scriptreplay; no inline video player is promised.
