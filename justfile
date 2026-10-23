# Tambo — the stack installer

default:
    @just --list

# Report what would block an install on this host; changes nothing
doctor plan="caboodle-plan.toml":
    cargo run -- doctor --plan "{{plan}}"

# Install the stack per a reviewed plan file
install plan="caboodle-plan.toml":
    cargo run -- install --plan "{{plan}}"

# Verify every installed tool: version read-back and controlled functional round-trip
verify plan="caboodle-plan.toml":
    cargo run -- verify --plan "{{plan}}"

check:
    python3 scripts/check-product-name.py
    python3 scripts/test-product-name.py
    sh -n scripts/install.sh scripts/check-release-assets.sh scripts/test-release-install.sh scripts/check-crate-version.sh scripts/setup-environment.sh scripts/build-stack-packs.sh
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test --all-features
    scripts/test-release-install.sh
    python3 scripts/test-release-repository.py
    python3 scripts/test-install-bootstrap.py
    python3 scripts/test-crate-version.py
    python3 scripts/test-stack-pendants.py
    python3 tests/test_skill_render_source.py

# Build the mdBook (requires mdbook)
book:
    mdbook build docs/book

# Serve the book locally with live reload
book-serve:
    mdbook serve docs/book --open

# Check README and rendered book links (requires mdbook and lychee)
docs-check:
    scripts/check-docs.sh
