# Local file-link demonstration

Source behavior: c5705d4; baseline: 2b9635e21f3da1a5ccf4d12afc23db84a9d8bda5.
This is a disposable source fixture, not an installed service.

The recording checks the same Markdown link three times. The ordinary GitHub
HTML request returned HTTP 503 (exit 2); the candidate remapped that destination
to the checkout and passed 1/1; removing the local target failed 1/1 (exit 2).
The live before result is an availability sample, not a deterministic fixture.
The candidate-only offline flag isolates this demonstration; production remains online.

Run from the repository root with LYCHEE pointing to the workflow-pinned
lychee 0.24.2 binary:

    sh docs/demos/local-doc-links/run.sh

[Recording output](output.log) and [timing](timing.log) are retained in Git.
Download both and replay:

    scriptreplay --log-out output.log --log-timing timing.log

The readable output also serves as a transcript.

Only this repository's blob/main/ prefix is remapped. Its file and fragment
checks use the candidate checkout: a target valid only on the PR branch can pass.
Other owners, repositories, branches, and tree links retain their original URLs.
