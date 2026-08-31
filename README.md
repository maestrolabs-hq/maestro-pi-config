<div align="center">

# Maestro-Pi-Config

**A machine, rebuildable from a repository**

Every post-install change to a pi installation, captured so it can be restored anywhere.

  <a href="https://github.com/maestrolabs-hq/maestro-pi-config/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/maestrolabs-hq/maestro-pi-config/ci.yml?branch=main&style=for-the-badge&label=CI&labelColor=1c1c1c&color=2ea043"></a>
  <a href="https://github.com/maestrolabs-hq/maestro-pi-config/actions/workflows/heavy.yml"><img alt="Heavy" src="https://img.shields.io/github/actions/workflow/status/maestrolabs-hq/maestro-pi-config/heavy.yml?branch=main&style=for-the-badge&label=Heavy&labelColor=1c1c1c&color=8957e5"></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/maestrolabs-hq/maestro-pi-config"><img alt="OpenSSF Scorecard" src="https://img.shields.io/ossf-scorecard/github.com/maestrolabs-hq/maestro-pi-config?style=for-the-badge&label=Scorecard&labelColor=1c1c1c"></a>
  <a href="https://github.com/maestrolabs-hq/maestro-pi-config/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/badge/License-MIT-1c1c1c?style=for-the-badge&labelColor=1c1c1c&color=0969da"></a>

  <img alt="Rust" src="https://img.shields.io/badge/Rust-1.98-CE422B?style=flat-square&logo=rust&logoColor=white">

</div>

---

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
