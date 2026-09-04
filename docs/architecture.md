# Architecture *(superseded, never built)*

> This design is superseded — see
> [ADR 0001](./adr/0001-shims-delegate-to-maestro.md). The planned
> `maestro memory` surface it targets was dropped from maestro-core, so the
> shim and the queue below have no target to talk to and will not be built.
> Nothing in this document is implemented; it is kept as a historical record
> of the design, not as a plan the shim will follow.

What this repository owns, and where it stops.

## Position

```text
pi session
   │  lifecycle event
   ▼
shim (TypeScript, this repo)
   │  envelope on stdin, acknowledgement on stdout
   ▼
maestro (Rust, maestro-core)
   │  durable queue → delivery
   ▼
memory sink   graph sink   ...            (over MCP)
```

The shim is the only part of this picture that lives here. Everything below the
boundary is Maestro core's, including the queue, delivery, retries and all
consumer knowledge.

## Event mapping

Pi knowledge, so it lives here. Four events, three of which write.

| Pi event | Direction | Purpose |
| --- | --- | --- |
| `session_start` | read | warm recall for this project |
| `before_agent_start` | read | inject recalled context, once per session |
| `agent_settled` | write | capture material new since the watermark |
| `session_before_compact` | write | capture while context is still intact |
| `session_shutdown` | write | final capture |

`agent_settled` rather than `agent_end`: pi may auto-retry or auto-compact after
`agent_end`, so `agent_end` does not mean the run is finished. `agent_settled`
does.

`session_start` cannot inject. It fires before a prompt exists, so recall is two
events — warm at `session_start`, inject at the first `before_agent_start` of
the session and not on later turns, which would re-spend tokens every prompt.

## Shim contract

A shim:

- writes one versioned JSON envelope per event to `maestro` on stdin
- waits for the acknowledgement, under a hard timeout
- resolves on every path, including failure, and never throws
- does nothing when `maestro` is absent
- skips entirely in subagent children, so material is captured once

A shim does not decide what to capture, where it is filed, when it is delivered,
or what happens when delivery fails.

## Capture scope

A capture carries only material new since that session's watermark. Three
captures per session sending the full transcript each time would make the last
one re-send the whole session and push the cost of noticing onto the consumer.

## Recall scope

Recall is bounded, because it is injected into the system prompt and competes
with the session's working context. It is bounded to the current project's scope,
so one project's material never lands in another project's session.

## Failure posture

Capturing memory was never meant to be allowed to fail a pi session, hold one
open, or surface an error in it. This design assumed operational visibility
would belong to a `maestro memory status` surface rather than the session
itself. That surface will not be built (see ADR 0001), so no such visibility
exists, and no replacement mechanism is proposed here.
