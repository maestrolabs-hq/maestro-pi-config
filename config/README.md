# Captured configuration

Everything done to this machine after installing pi. Restoring it elsewhere
should reproduce the same behaviour, minus credentials.

## What is here

| Path | Restores to | Holds |
| --- | --- | --- |
| `pi/settings.json` | `<pi agent dir>/settings.json` | packages, provider, default model, theme |
| `pi/claude-bridge.json` | `<pi agent dir>/claude-bridge.json` | bridge provider |
| `pi/models-store.json` | `<pi agent dir>/models-store.json` | model catalogue: 7 openai-codex, 9 llama.cpp |
| `pi/skills/gauntlet-loop/` | `<pi agent dir>/skills/` | the skill, adapted off Claude Code primitives |
| `mcp/mcp.json` | `<user config dir>/mcp/mcp.json` | seven MCP servers |
| `tools/mempalace/config.json` | `~/.mempalace/` | wings, rooms, palace location |
| `tools/codegraphcontext/config.yaml` | `~/.codegraphcontext/` | context mode |
| `tools/codegraphcontext/env.template` | `~/.codegraphcontext/.env` | backend settings; `${HOME}` expands on restore |
| `bin/` | a directory on `PATH` | llama router, qwen, STT and TTS start/stop scripts |
| `shell/cargo-env.sh` | appended to shell rc | puts our runtime on `PATH` |
| `INVENTORY.md` | — | versions and where each binary came from |

## Restoring on a fresh machine

Configuration is worthless if what it names is not installed. `settings.json`
declares packages; `mcp.json` names binaries. Neither is any use on a machine
that has none of them, so provisioning comes first.

```text
just bootstrap        # prints both plans, changes nothing
just bootstrap-apply  # installs, then writes
```

`config/provision.txt` is the manifest: pinned versions, one step per line.
Moving a version there is a deliberate edit.

Every git package is pinned to a ref, so a restore gets the same code rather
than whatever `main` happened to be that day. Superpowers pins to the tag
`v6.3.0`; the other two have no tags at their installed commit, so they pin to
a SHA. The cost is that `pi update` no longer moves them: raising a pin is now
an edit here, which is the point.

The npm packages are deliberately left unpinned. They resolve by semver from
published releases rather than a moving branch, and pinning them would trade a
real benefit -- `pi update` keeping them current -- for reproducibility they
mostly already have.

Four things it cannot install for you, because they are its prerequisites:
rustup, Node 24, uv, and pi. Two more are fetched by hand, because guessing a
release asset name silently installs the wrong thing: `gh` and
`github-mcp-server`.

The last step is always re-authenticating. Credentials are not here.

## What is deliberately absent

Credentials never leave the machine they were issued on. `auth.json` and the
GitHub token are not here and must not be added. A restore therefore ends with
re-authenticating, not with everything already working.

Also absent: `trust.json` (per-machine, absolute paths), the 616 MB of
reinstallable packages under `npm/`, `git/` and `bin/`, session history, MCP
caches, and ~207 GB of model weights. All of it is either regenerable or
machine-specific.

## Paths

Nothing here contains an absolute path, per maestro-core ADR-0001.

Two places needed work to make that true. `mcp.json` referenced
`/home/franc/.local/bin/gh` and three other absolute binaries; those are now
resolved from `PATH`. Graphify's graph file could not be templated because the
adapter does not interpolate `args`, so its entry uses `cwd: ~/.graphify` — which
*is* expanded — with a relative filename.

The exception is `bin/`: those scripts point at model files by absolute path.
They are captured as they are, and a restore on a machine that stores models
elsewhere must edit them.
