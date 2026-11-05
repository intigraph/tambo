# Endpoint trust decision CLI

Before (installed Caboodle 0.3.2): `packs trust check-endpoint` exits 2 because
there is no endpoint decision command. After (candidate Tambo 0.3.7): the same
command delegates to existing `TrustConfig::decide_endpoint`: missing foreign
consent exits 3, an exact repository/endpoint allow exits 0, and revocation exits
3 again. The check does not write policy, contact an endpoint, or add federation.
No installed/runtime change is claimed.

Reproduce: `python3 docs/demos/endpoint-trust/demo.py /path/to/candidate [/path/to/baseline]`.
The script uses a temporary policy and synthetic public URLs. The terminal
recording (command-header paths redacted) is `output.gz` (gzip compressed); readable output is `transcript.txt`.
Replay after decompression with `scriptreplay --log-out output --log-timing timing.txt`.
