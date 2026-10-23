# Seeds identity compatibility

Source-fixture transcript; this is not a recording or a Mac delivery receipt.
The test invokes the real identity probe against private executable fixtures,
using the embedded Seeds member declaration. No terminal recording was made.

The primary identity remains required. A program may additionally declare up to
eight unique literal `identity_alternatives`; every literal independently passes
the existing name/version-only refusal. Missing alternatives preserve the prior
manifest contract. For Seeds the reviewed markers are:

```toml
identity_contains = "seeds is the beads-compatible tracker"
identity_alternatives = ["seeds is the work-item tracker for the quipu stack"]
```

These are literal markers, not regular expressions or version-only identities.
The probe still rejects a foreign find-and-replace `sd`, a bare `sd 1.0.0`, and
unrelated Seeds text. Functional member verification remains separate.

Reproduce:

```sh
cargo test --lib seeds_identity_accepts_both_editions_and_refuses_foreign_sd
cargo test --lib identity_alternatives_are_explicit_strong_unique_and_bounded
```

Applying only the new identity regression to base
`c66eb462c038aeb05a8da1b9979524b45bf34adc` produced:

```text
assertion `left == right` failed: seeds is the work-item tracker for the quipu stack
  left: false
 right: true
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 153 filtered out
```

After the source change, a copied library-test executable run serially produced:

```text
test result: ok. 149 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 3.26s
```

The six existing ignored controls were not exercised. The initial parallel run
also hit a separate transient `Text file busy` in an alias-layout test; that
failure is retained and the same copied binary passed serially. This establishes
source-fixture behavior, not release publication or installed convergence.

The change adds a public Rust `Program` field as well as optional manifest data;
it is a source API extension, and release review must account for struct-literal
callers. No installed member manifest is edited by these commands.
