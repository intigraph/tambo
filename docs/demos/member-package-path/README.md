# Package member version read-back

Run the same real CLI probe with an official before binary and the candidate:

```sh
python3 docs/demos/member-package-path/demo.py /path/to/official-tambo before
python3 docs/demos/member-package-path/demo.py /path/to/candidate-tambo after
```

The disposable fixture puts an old Seeds executable in Cargo and the current
package executable on PATH. Before: a valid package receipt is rejected because
the managed copy is read. After: PATH read-back succeeds. A fixture without the
managed copy remains successful; a stale PATH executable is still rejected.
These are local fixtures; they do not prove installation or natural delivery.
