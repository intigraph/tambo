# Skill pendants and profile staging

On Unix, Tambo can publish a local skill as a native pendant and materialize a
complete pinned profile into an owned staging tree. The authoring source stays
in its owning repository. These commands do not register a shared catalogue or
change Claude/Codex settings.

```sh
tambo skills publish --help
tambo skills sync --help
```

`publish` accepts one source directory, explicit release metadata and reviewed
render shapes. It produces a native share archive, catalogue Turtle and a
`release.json` completion marker in a new directory. Public output must pass
native outward identifier policy; internal shares retain their native stamp.
A verified but quarantined native import remains quarantined.

`sync` accepts explicit profile/catalogue JSON, a map of native share IDs to
local archives, and a separate native shape-authority archive. The pinned
selector refuses incomplete, missing, duplicate, wrong-version or unsupported
selections. Input files are data, not authenticated graph authority. No
repository or render-specification IRI is fetched as a URL.

Native composition verifies the selected frozen shares. Native rendering then
publishes the whole `claude/<name>/` and `codex/<name>/` profile tree, retaining
raw markdown, binary bytes and executable modes. The renderer owns NOOP
checking, atomic publication and retirement of previously owned files; unknown
destination files refuse. Use a dedicated owned profile directory, never a
shared harness root.

The generated `profile.json` records selected pins, source transaction and
shape-authority share identity. This staging receipt does not prove that a
session loaded those skills. Live registration, scheduled propagation and
loaded-version history require the settings owner's separate reviewed route.

See the [source contract and fixture commands](https://github.com/intigraph/tambo/blob/main/docs/design/skill-profile-selection.md)
for metadata fields, budgets and the two-public-plus-one-private integration
control. That control uses disposable databases and explicit test binaries;
it does not install a tool or touch an existing harness.
