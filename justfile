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
    cargo binstall -y prek cargo-deny cargo-machete similarity-rs

# Wire the local hooks. Both types come from default_install_hook_types.
setup:
    prek install --install-hooks

# Run the quality gates. CI runs these same commands, not equivalents.
check:
    cargo fmt --all --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test
    cargo machete
    cargo deny check

# Format in place. `check` only verifies.
fmt:
    cargo fmt --all

# What a restore would change on this machine. Reads only, and saves the plan
# so an apply carries out exactly what was reviewed.
plan:
    cargo run --quiet -- plan --out plan.out

# Carry out the saved plan. Refuses if the repository or the machine moved
# since it was written, so an apply can never do something unreviewed.
apply:
    cargo run --quiet -- apply plan.out

# Re-plan and act in one step, without a saved plan to check against.
apply-now *FLAGS:
    cargo run --quiet -- apply {{FLAGS}}

# What has to be installed before configuration means anything.
provision *FLAGS:
    cargo run --quiet -- provision {{FLAGS}}

# A fresh machine, end to end: install, then converge configuration.
#
#   just bootstrap                 the whole plan, changes nothing
#   just bootstrap --auto-approve  carries it out
bootstrap *FLAGS:
    cargo run --quiet -- provision {{FLAGS}}
    cargo run --quiet -- apply {{FLAGS}}

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
