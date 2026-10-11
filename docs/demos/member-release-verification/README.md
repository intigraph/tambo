# Member release verification demo

[Terminal recording](demo.cast): the existing CLI refuses the standalone command;
the candidate exposes it and refuses an unknown member. This is a command-surface
demo, not a live publication or installation acceptance trace.

The companion `caboodle-verify` job runs `caboodle verify-member seeds` against
the candidate checkout. The offline fixture controls prove a complete functional
roundtrip and reject wrong identity, wrong version, missing marker and tampered
archive. The command stages temporary release bytes and leaves managed programs
absent. Published-release proof and full CI are separate required gates.
