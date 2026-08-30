# Context

Glossary for maestro-pi-config. Terms only — no implementation detail.

## Maestro

A framework agents run under. It governs agent behaviour: refusing destructive
commands, running sanity checks, monitoring, observability, and carrying
messages outward. It is not specific to pi, and pi is not specific to it.

## Maestro core

The engine of the framework, and the master orchestrator. Owns the `maestro`
CLI and all logic. Every child project consumes it; it consumes none of them.
Knows nothing about pi.

## Maestro hook

A governance decision point: a place where Maestro inspects what an agent is
about to do and may refuse, warn, record, or forward it. Distinct from a pi
lifecycle event, which is only a notification that something happened in a pi
session. The two words "hook" refer to different things and must not be
conflated.

## Child project

Any project that consumes Maestro core. This repository is one.

## Pi modification

Any change to how pi behaves: settings, MCP server registrations, skills,
packages, provider setup, and runtime extensions. Every pi modification is
recorded in this repository, so a machine can be restored from it.

## Shim

The TypeScript extension pi loads. Pi loads only `.ts` and `.js`, so the shim
is the sole entry point into pi's process. A shim holds no policy and makes no
decisions: it translates a pi lifecycle event into an envelope, hands it to
Maestro, and waits for the acknowledgement.

## Pi lifecycle event

A notification pi emits as a session proceeds: a session starting, an agent run
settling, a compaction approaching, a session tearing down. Knowledge of which
event means what is pi knowledge, and stays in this repository.

## Envelope

The versioned JSON message a shim emits and Maestro accepts. The boundary
between this repository and Maestro core. Maestro never learns which pi event
produced an envelope.

## Capture

Writing session material into the durable queue. A capture is complete once it
is durable, before any consumer has seen it.

## Durable queue

Maestro's record of captures that have been accepted but not yet delivered. It
is what makes a capture survive a consumer being unavailable, and it is why
Maestro sits between a shim and a consumer rather than the shim calling the
consumer directly.

## Watermark

The point in a session's material up to which capture has already happened. A
capture carries only what is new since the watermark.

## Delivery

Handing a captured item to a downstream consumer such as MemPalace. Delivery
happens after acknowledgement and may fail without losing the capture.

## Dead letter

A capture that has exhausted its delivery attempts. It is parked and remains
visible and re-drainable. A capture is never dropped.

## Recall

Reading prior material back at the start of a session. The inverse of capture.
Bounded, because it competes with the session's working context.

## Wing / Room

How MemPalace files material. A wing is the top-level division and corresponds
to one project; a room subdivides a wing by kind of material. Recall is scoped
to the current project's wing by default.
