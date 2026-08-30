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
just bootstrap                 # the whole plan, changes nothing
just bootstrap --auto-approve  # installs, then converges configuration
```

`provision`, `plan` and `apply` can be run on their own.

Configuration follows `terraform plan` / `terraform apply`. A plan reports only
what would actually change, so a machine already in step reads `No changes`
rather than a list of files it would rewrite identically.

`just plan` saves to `plan.out`, and `just apply` carries out that file. The
saved plan records the digest each source had in the repository and each target
had on the machine; applying re-reads both and refuses if either moved. So an
apply cannot quietly do something other than what was reviewed, and it checks
every action before writing any -- a stale plan leaves the machine untouched.

`just apply-now --auto-approve` re-plans and acts in one step, for when a saved
plan is more ceremony than the change deserves.

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

### The gauntlet-loop skill is an adaptation

Upstream is written for Claude Code and uses two primitives pi does not have:
the `/loop` slash command and `ultracode`. The captured copy replaces both, so
it is not the same file the package ships.

The two do not collide. `pi install` clones the package to the agent's `git/`
directory, and pi does not read skills from a package's `.claude/skills` path;
the copy pi actually loads is the one restored into the agent's `skills/`
directory. Provisioning runs before restoring, so ours is written last either
way.

The risk is not collision but erosion: re-capturing from a pristine clone would
silently reinstate instructions pi cannot follow, and the symptom would be a
model quietly ignoring a line rather than an error. `tests/skills.rs` fails if
`ultracode` or `/loop` appear outside the note explaining they were replaced,
or if any of the three prompt templates loses its replacement line.

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
`<home>/.local/bin/gh` and three other absolute binaries; those are now
resolved from `PATH`. Graphify's graph file could not be templated because the
adapter does not interpolate `args`, so its entry uses `cwd: ~/.graphify` — which
*is* expanded — with a relative filename.

`bin/` is templated like the tool configs: the scripts store `${HOME}` and a
restore expands it, so the same script works for any user on any machine.

### Local models are not restored

The scripts under `bin/` start llama.cpp against specific GGUF files. Those
weights are roughly 207 GB and are not in this repository -- nothing here
fetches them either.

A restored machine therefore has working scripts pointing at files it does not
have. Starting the router will fail, and pi will fail to reach a `llama.cpp`
provider, until the weights are present under `${HOME}/models/` at the paths the
scripts name. `config/pi/models-store.json` restores the *catalogue* of nine
local models; the catalogue is not the weights.

Two paths out: fetch the weights to the same layout, or edit the scripts and the
catalogue to point at what that machine actually has. Either is deliberate work,
not something a restore can do for you.
