# Cloud bootstrap: Seeds instead of Beads

Run `python3 docs/demos/cloud-seeds/demo.py` from this checkout. The same
network-free private fixture has the other prerequisites already installed.
The base setup script at `76f63a2e` attempts the legacy npm Beads install; the
candidate requests the Seeds release installer. The installed-Seeds control
skips that download. No real package manager, tracker store, or service is used.

`output.txt` and `timing.txt` are the actual terminal recording, retained with
this source. Download both and run `scriptreplay --timing timing.txt --typescript
output.txt`; terminal replay is required, not a browser video embed. The demo
imports the integration fixture used by the bootstrap test suite. It proves
bootstrap routing and reporting, not published release integrity or a live
cloud-environment install.
