# Built-in tool pins

`pins.toml` holds the reviewed release of each of Tambo's five built-in tools:
quipu, bobbin, yupana, desire-path and camayoc. Tambo embeds it at build time,
and `src/builtin_pins.rs` validates it. Unlike a stack member, a built-in is
installed by its own adapter in `src/adapter.rs`, so it cannot be a
`members/*.toml` file (a member may not shadow a built-in name). Its pin is
data all the same, so the unattended bump job can move it (aegis-mecge2.10).

To move a pin to a new release:

    tambo bump-builtin <tool>                       # latest stable release
    tambo bump-builtin <tool> --tag <t>             # a specific release
    tambo bump-builtin <tool> --probe-wrapper <exe> # run the new release confined

The bump reads the release and refuses a downgrade. It then:

- re-derives every recorded `[sha256]` digest by downloading the asset and
  requiring its bytes to hash to the release's published sums line;
- resolves `revision` from the release tag's commit, for tools Tambo builds or
  installs from source (desire-path, camayoc);
- for a tool with a `program` (quipu, bobbin, yupana), unpacks the host
  target's verified asset and requires the program's `--version` to report the
  new version, through `--probe-wrapper` when given.

It rewrites only that tool's `version`, `revision` and `[sha256]` values, then
reads the file back. Review the diff and open a PR; the new pin reaches users
with the next Tambo release.

Tests read the pins from this file instead of repeating them, so a bump PR
stays green without test edits.
