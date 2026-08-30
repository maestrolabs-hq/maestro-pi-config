# maestro-pi-config

Single home for everything that modifies pi's behavior.

Nothing that changes how pi runs lives anywhere else: not in scattered dotfiles,
not hand-edited on one machine. It is captured here, versioned, and deployable
to any machine.

## Scope

| Area | What it covers |
| --- | --- |
| Config capture | pi settings, MCP servers, skills, packages, provider setup |
| Deploy | apply this configuration to a fresh machine |
| Restore | roll a machine back to a known-good snapshot |
| Extensions | TypeScript extensions that change pi's runtime behavior |

## Layout

```text
config/      captured pi configuration
extensions/  pi runtime extensions (TypeScript)
docs/        design notes and runbooks
```

## First extension: mempalace hooks

Pi exposes lifecycle events that map cleanly onto mempalace's hook model:

| mempalace hook | pi event |
| --- | --- |
| `session-start` | `session_start` |
| `precompact` | `session_before_compact` |
| `stop` | `agent_end` |
| `session-end` | `session_shutdown` |

`mempalace hook run` only accepts `--harness claude-code|codex`, so the extension
calls mempalace's MCP tools directly rather than impersonating another harness's
stdin format.

## Status

Nothing implemented yet.
