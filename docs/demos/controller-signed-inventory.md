# Signed macOS account inventory

Stock macOS account records include `nobody -2` and `nogroup -1`. The same mocked `dscl` output is refused on base `7fd2777` and parsed with its signed IDs intact on the candidate (packet script SHA256 `adaa908c7b98589bc38db80b06d3d1346fd465a13ae631a9e01591a2be171bf0`). Duplicate names, malformed IDs and empty inventory remain refused.

[Recording transcript](controller-signed-inventory.txt), [timing](controller-signed-inventory.timing), [reproducer](controller-signed-inventory.py). These small source-only fixtures follow the existing demos directory retention convention. All native commands are mocked; this recording does not prove native setup or installation. Playback was checked with:

```sh
scriptreplay --log-out docs/demos/controller-signed-inventory.txt --log-timing docs/demos/controller-signed-inventory.timing
```

Repeat from a full source checkout with `python3 -B docs/demos/controller-signed-inventory.py`. Playback requires downloading the transcript and timing pair; no inline video player is assumed.
