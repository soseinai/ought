[private]
default:
    @just --list

# Install UI dependencies (only if node_modules is missing)
_npm-install:
    cd crates/ought-server/ui && [ -d node_modules ] || npm ci


# ============================================================
# typescript
# ============================================================

# Build the Svelte UI (output goes to crates/ought-server/dist/)
[group: 'typescript']
build-ui: _npm-install
    cd crates/ought-server/ui && npm run build

# Run UI tests (placeholder — replace with a real runner like vitest)
[group: 'typescript']
test-ui:
    @echo "test-ui: ok (placeholder, no UI tests yet)"

# Lint / type-check the Svelte UI
[group: 'typescript']
lint-ui: _npm-install
    cd crates/ought-server/ui && npm run check


# ============================================================
# rust
# ============================================================

# Build the Rust workspace. Pass `release` for an optimized build.
[group: 'rust']
build-rust profile="debug":
    cargo build {{ if profile == "release" { "--release" } else { "" } }}

# Run Rust tests (UI must be built first so rust-embed can find dist/)
[group: 'rust']
test-rust: build-ui
    cargo test

# Lint the Rust workspace
[group: 'rust']
lint-rust: build-ui
    cargo clippy --all-targets


# ============================================================
# all
# ============================================================

# Build everything (UI + Rust). Pass `release` for an optimized build.
[group: 'all']
build profile="debug": build-ui (build-rust profile)

# Build a release binary and install `ought` to ~/.local/bin
[group: 'all']
install: (build "release")
    mkdir -p ~/.local/bin
    install -m 755 target/release/ought ~/.local/bin/ought
    @echo "installed: ~/.local/bin/ought"

# Run all tests (UI + Rust)
[group: 'all']
test: test-ui test-rust

# Lint everything (UI + Rust)
[group: 'all']
lint: lint-ui lint-rust

# Run the same checks CI runs
[group: 'all']
ci: test lint

# Publish all workspace crates to crates.io (extra args forwarded to cargo)
[group: 'all']
publish-crates *args: build-ui
    cargo publish --workspace --allow-dirty {{args}}

# Remove all build artifacts
[group: 'all']
clean:
    cargo clean
    rm -rf crates/ought-server/dist
    rm -rf crates/ought-server/ui/node_modules
