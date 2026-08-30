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

## First extension: the memory shim

Pi loads only `.ts` and `.js`, so a shim is the sole way into its process. The
shim holds no policy: it turns a pi lifecycle event into an envelope, hands it to
`maestro`, and waits for the acknowledgement. Everything behind that boundary —
the ledger, delivery, retries, and any knowledge of MemPalace — belongs to
Maestro. See [ADR-0001](./docs/adr/0001-shims-delegate-to-maestro.md).

The four events, and why each:

| Pi event | Direction | Why this one |
| --- | --- | --- |
| `session_start` | read | warm recall for this project |
| `before_agent_start` | read | inject recalled context, once per session |
| `agent_settled` | write | not `agent_end`: pi may auto-retry or auto-compact after that, so `agent_end` does not mean finished |
| `session_before_compact` | write | capture while the context is still intact |
| `session_shutdown` | write | final capture |

The full mapping, the shim's contract and the failure posture are in
[docs/architecture.md](./docs/architecture.md).

## Status

Nothing implemented yet.
