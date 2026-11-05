# Current cloud pendant command

Run `python3 docs/demos/current-cloud-pendants/run.py` from a checkout.
The real optional cloud setup block calls the current executable with the
repository and authority paths intact, including spaces. Only a synthetic
current executable is available. The fixture records arguments; it does not
run an installer, download a pack, or contact a graph service.

Replay `output.txt` with its `timing.txt` using `scriptreplay --log-timing timing.txt --log-out output.txt`. The recording preserves terminal CRLF bytes.
The full setup script's other installer lanes are deliberately outside this demo.
