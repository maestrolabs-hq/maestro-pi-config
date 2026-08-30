# Architecture

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
MemPalace / CodeGraphContext / Graphify   (over MCP)
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
with the session's working context. It is scoped to the current project's wing,
so one project's material never lands in another project's session.

## Failure posture

Capturing memory is never allowed to fail a pi session, hold one open, or
surface an error in it. Every failure resolves quietly and becomes visible
through Maestro's own status surface instead.

The cost of that posture: an absent or broken `maestro` is silent from inside
pi. Operational visibility belongs to `maestro memory status`, not to the
session.
