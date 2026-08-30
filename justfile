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
    npm ci --no-audit --no-fund || npm install --no-audit --no-fund
    cargo binstall -y prek

# Wire the local hooks. Both types come from default_install_hook_types.
setup:
    prek install --install-hooks

# Run the quality gates. CI runs these same commands, not equivalents.
# `tsc --noEmit` and `node --test` are deliberately absent until the first
# TypeScript lands: tsc exits TS18003 with no inputs, and a gate that fails
# for want of input catches nothing. Restore both with the first .ts file --
# the config for them is already here and correct.
check:
    npx biome format .
    npx biome lint .

# Format in place. `check` only verifies.
fmt:
    npx biome format --write .

# Prove the gates do not depend on the ambient PATH.
doctor:
    @echo "just    $(command -v just)"
    @echo "node    $(command -v node) $(node --version)"
    @echo "npm     $(command -v npm)"
    @echo "prek    $(command -v prek)"
