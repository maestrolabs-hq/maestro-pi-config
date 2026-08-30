# Optional convenience task runner (https://just.systems).
# Every command below works standalone -- just is never required.

# Our runtime, ahead of whatever the ambient PATH carries. Derived, never
# hardcoded: `home_directory()` resolves on Windows, macOS and Linux alike,
# and the separator follows the OS rather than assuming Unix. See
# maestro-core ADR-0001.
path_sep := if os_family() == "windows" { ";" } else { ":" }
export PATH := home_directory() / ".cargo" / "bin" + path_sep + home_directory() / ".local" / "bin" + path_sep + env('PATH')

# Install what this repository needs. Idempotent.
install:
    rustup toolchain install --profile minimal 1.98.0
    rustup component add clippy rustfmt
    cargo binstall -y prek cargo-deny cargo-machete
    npm ci --no-audit --no-fund || npm install --no-audit --no-fund

# Wire the local hooks. Both types come from default_install_hook_types.
setup:
    prek install --install-hooks

# Run the quality gates. CI runs these same commands, not equivalents.
#
# `tsc --noEmit` and `node --test` are configured but absent here until the
# first shim lands: tsc exits TS18003 with no inputs, and a gate that fails
# for want of input catches nothing. Biome still covers the JSON.
check:
    cargo fmt --all --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test
    cargo machete
    cargo deny check
    npx biome format .
    npx biome lint .

# Format in place. `check` only verifies.
fmt:
    cargo fmt --all
    npx biome format --write .

# A fresh machine, end to end: install what is needed, then write config.
# Prints a plan and changes nothing; pass --apply to act.
#
#   just bootstrap            what would happen
#   just bootstrap --apply    make it happen
bootstrap *FLAGS:
    cargo run --quiet -- provision {{FLAGS}}
    cargo run --quiet -- restore {{FLAGS}}

# What has to be installed before configuration means anything.
provision *FLAGS:
    cargo run --quiet -- provision {{FLAGS}}

# Repository -> machine.
restore *FLAGS:
    cargo run --quiet -- restore {{FLAGS}}

# Is this machine's configuration still what the repository holds?
status:
    cargo run --quiet -- status

# Machine -> repository. Drift becomes a git diff; review it before committing.
sync:
    cargo run --quiet -- sync

# Prove the gates do not depend on the ambient PATH.
doctor:
    @echo "just    $(command -v just)"
    @echo "cargo   $(command -v cargo)"
    @echo "prek    $(command -v prek)"
    @echo "rustc   $(rustc --version)"
