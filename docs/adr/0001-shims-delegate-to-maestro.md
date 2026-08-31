# ADR 0001: Pi shims delegate to Maestro

- Status: Accepted
- Date: 2026-08-30

## Context

Pi emits lifecycle events and loads only `.ts` and `.js` extensions — its
resource loader matches `/\.(ts|js)$/`, so the code that subscribes to those
events must be TypeScript. That constraint says nothing about where the work
behind an event should happen.

An earlier extension put the work in the shim: it spawned a memory tool
directly, detached the child, and returned. Nothing observed whether the write
landed. When the consumer was unavailable the session material was lost with no
record, because the only copy was in flight to a process nobody was watching.

Maestro is the master orchestrator for agent work. Memory capture is one of
several concerns it owns; governance, monitoring and outward bridges are
others.

## Decision

A shim holds no policy. It translates a pi lifecycle event into a versioned
JSON envelope, writes it to `maestro` on stdin, and waits for an acknowledgement
before resolving.

Maestro owns everything behind that boundary: it writes the envelope to a
durable queue, acknowledges, and only then delivers to a consumer. Maestro
never learns which pi event produced an envelope.

Consumers are reached over MCP rather than their command-line interfaces.
Every consumer this was built against speaks MCP, so one client
abstraction serves all of them, and adding another is configuration
rather than code.

The shim waits for the acknowledgement under a hard timeout and never throws.
Capturing memory must not be able to fail a session or hold it open.

## Boundaries

This repository holds pi configuration, pi settings, and shim scripts. It does
not hold orchestration logic, queue behaviour, delivery policy, or consumer
knowledge. Those live in Maestro core.

Which pi event maps to which envelope is pi knowledge and stays here.

## Consequences and risks

A capture survives a consumer being unavailable, which the previous design
could not offer.

Each event now costs a process spawn and an acknowledgement round trip instead
of a detached spawn. Three write events per session makes that cost
irrelevant; the hard timeout bounds the worst case.

Two repositories change together when the envelope changes. The envelope is
versioned specifically to make that a compatible change rather than a
lockstep one.

Maestro becomes a hard dependency of the shim. If `maestro` is absent the shim
must degrade to doing nothing rather than erroring, which means an absent
binary is silent — an operational risk to surface through `maestro memory
status` rather than through the session.

## Non-goals

This decision does not specify the envelope's fields, the queue's schema, or
the retry schedule.
