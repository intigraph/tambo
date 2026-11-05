# Compatibility

The `caboodle` command remains an alias for `tambo`. Existing
`caboodle-plan.toml`, `caboodle-intent.toml`, `.caboodle` state, `CABOODLE_*`
environment variables, Rust crate imports and published vocabulary IRIs remain
valid. These are compatibility contracts; renaming the product does not move
installed state or rewrite stored graph identities.

The release installer prefers a `tambo-` archive. Only an HTTP 404 selects a
legacy `caboodle-` archive; checksum and transport failures never select a
fallback. Existing package-manager identifiers remain valid until their own
managed migration.
