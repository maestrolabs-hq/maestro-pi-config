# Agent orchestration and the steward pilot

Status: approved design, not yet implemented. This spec covers two related
changes: a model routing matrix for every delegated agent role, and a single
local clerical helper called the steward. It supersedes the older routing
policy notes kept outside this workspace.

## Part 1 — Model routing matrix

### Objectives

The matrix serves four explicit objectives, decided together:

1. **Quota resilience.** Every role carries a cross-provider fallback so a
   subscription usage limit degrades the lane to the other family instead of
   killing it.
2. **Cross-family review.** Review roles run on a different model family than
   the writer they review; each family catches the other's blind spots.
3. **Specialization by strength.** Deep reasoning goes to the strongest
   models, fast bounded work to the fastest, very long context to the model
   that carries it.
4. **Load spreading.** Work is distributed across two subscriptions plus a
   local runtime, so no single quota carries the whole estate.

### Core principle

Every role carries a cross-family pair: one primary and one fallback from the
other family. Mechanical, self-validating roles additionally carry a local
third tier on `llama.cpp/qwen38`. Judgment roles never do: a weak local model
in a review lane risks silent rubber-stamp approvals, and a dead review lane
is more honest than a wrong one.

### The matrix

| Role | Primary (thinking) | Fallback chain |
| --- | --- | --- |
| main session | `claude-bridge/claude-fable-5` (medium) | manual switch only |
| scout | `claude-bridge/claude-sonnet-5` (low) | `openai-codex/gpt-5.6-luna:low`, `llama.cpp/qwen38` |
| delegate | `claude-bridge/claude-sonnet-5` (medium) | `openai-codex/gpt-5.6-luna:medium`, `llama.cpp/qwen38` |
| spark (new custom agent) | `openai-codex/gpt-5.3-codex-spark` (medium) | `claude-bridge/claude-sonnet-5:medium`, `llama.cpp/qwen38` |
| worker | `openai-codex/gpt-5.6-luna` (medium) | `claude-bridge/claude-sonnet-5:medium` |
| researcher | `openai-codex/gpt-5.6-sol` (medium) | `claude-bridge/claude-fable-5:medium` |
| reviewer | cross-family by parent provider | see below |
| adversary (new custom agent) | `claude-bridge/claude-opus-5` (xhigh) | `openai-codex/gpt-5.6-sol:xhigh` |
| oracle | `openai-codex/gpt-5.6-sol` (xhigh) | `claude-bridge/claude-opus-5:high` |
| watchdog | `openai-codex/gpt-5.5:high` | none |

Additional decisions:

- **Main session.** `claude-bridge/claude-fable-5` at thinking `medium`,
  chosen for its 1M context: orchestration is the most context-hungry role in
  the loop, and it is the largest single token consumer, so placing it on the
  claude-bridge subscription leaves the openai-codex quota for workers. Set
  through `defaultProvider` and `defaultModel`. **No automatic fallback exists
  for the main session**: a quota failure there requires a manual model
  switch. The orchestrator's job is routing, synthesis, and judgment of
  continuity; deep reasoning is delegated to oracle and adversary, which is
  why `medium` suffices as its default.
- **Reviewer.** Routed by parent provider through
  `subagents.agentOverridesByProvider`. When the session runs on
  `openai-codex`, the reviewer is `claude-bridge/claude-opus-5:high` with
  `openai-codex/gpt-5.6-terra:high` as fallback. When the session runs on
  `claude-bridge`, the reviewer is `openai-codex/gpt-5.6-terra:high` with
  `claude-bridge/claude-opus-5:high` as fallback. The review family is always
  opposite the writing family, and the fallback preserves a strong reviewer
  rather than a cheap one.
- **Watchdog.** `openai-codex/gpt-5.5:high`, the complementary family to the
  main session, enabled for boundary reviews only: it reviews the aggregate
  worktree diff at safe end-of-turn boundaries, which in this estate means
  the aggregate result of delegated writers. Cadence reviews stay off until
  boundary reviews prove their value.
- **Thinking ceiling.** `subagents.maxThinking: "xhigh"`. Running
  `gpt-5.6-sol` at `max` stays a deliberate per-dispatch act reserved for the
  hardest consequential decisions, never a standing default.
- **Model scope.** None configured. The purely advisory mode this design
  assumed does not exist in the runtime, and the matrix needs no scope
  enforcement to work; revisit only if unapproved-model drift appears.
- **Long context.** `claude-bridge/claude-fable-5` (1M) as a per-dispatch
  override for whole-repository audits and giant diffs. Not a standing agent:
  the need is occasional and the override is one field at dispatch time.

### Configuration

The `subagents` block in the governed `config/pi/settings.json`:

```jsonc
"subagents": {
  "maxThinking": "xhigh",
  "agentOverrides": {
    "claude-code":        { "disabled": true },
    "claude-code-writer": { "disabled": true },
    "codex-exec":         { "disabled": true },
    "codex-exec-writer":  { "disabled": true },
    "cursor-agent":       { "disabled": true },
    "cursor-agent-writer": { "disabled": true },
    "scout": {
      "model": "claude-bridge/claude-sonnet-5",
      "thinking": "low",
      "fallbackModels": ["openai-codex/gpt-5.6-luna:low", "llama.cpp/qwen38"]
    },
    "delegate": {
      "model": "claude-bridge/claude-sonnet-5",
      "thinking": "medium",
      "fallbackModels": ["openai-codex/gpt-5.6-luna:medium", "llama.cpp/qwen38"]
    },
    "worker": {
      "model": "openai-codex/gpt-5.6-luna",
      "thinking": "medium",
      "fallbackModels": ["claude-bridge/claude-sonnet-5:medium"]
    },
    "researcher": {
      "model": "openai-codex/gpt-5.6-sol",
      "thinking": "medium",
      "fallbackModels": ["claude-bridge/claude-fable-5:medium"]
    },
    "oracle": {
      "model": "openai-codex/gpt-5.6-sol",
      "thinking": "xhigh",
      "fallbackModels": ["claude-bridge/claude-opus-5:high"]
    }
  },
  "agentOverridesByProvider": {
    "openai-codex": {
      "reviewer": {
        "model": "claude-bridge/claude-opus-5",
        "thinking": "high",
        "fallbackModels": ["openai-codex/gpt-5.6-terra:high"]
      }
    },
    "claude-bridge": {
      "reviewer": {
        "model": "openai-codex/gpt-5.6-terra",
        "thinking": "high",
        "fallbackModels": ["claude-bridge/claude-opus-5:high"]
      }
    }
  }
}
```

Two new custom agents, governed as sources in `config/pi/agents/` and
provisioned to the live agent directory.

`config/pi/agents/spark.md`:

```markdown
---
name: spark
description: Fast precise executor. Give it a fully dictated, bounded task —
  exact edits, exact commands, exact validation commands. Never exploration,
  never large reads, never long context. If the task needs reading to
  understand, it belongs to worker, not spark.
tools: read, grep, find, ls, bash, edit, write
model: openai-codex/gpt-5.3-codex-spark
thinking: medium
fallbackModels:
  - claude-bridge/claude-sonnet-5:medium
  - llama.cpp/qwen38
---

Execute exactly the dictated task. Run every validation command given. Report
exact outputs. If anything is ambiguous or requires exploration, stop and
report instead of improvising.
```

`config/pi/agents/adversary.md`:

```markdown
---
name: adversary
description: Adversarial reviewer. Attacks a change or design looking for what
  its author and its ordinary reviewer missed. Read-only; never edits.
tools: read, grep, find, ls
model: claude-bridge/claude-opus-5
thinking: xhigh
fallbackModels:
  - openai-codex/gpt-5.6-sol:xhigh
---

Review the supplied change or design adversarially. Hunt for what the author
and the ordinary review missed: hidden coupling, silent scope changes, wrong
assumptions, evaded gates, false claims of verification. Report findings with
exact file and line references. Do not edit anything.
```

### Documented limits

These are runtime facts, recorded so nobody designs against behavior that does
not exist:

- Fallback fires only on retryable provider failures — quota and usage
  limits, rate limits, authentication errors, overload, 5xx responses, and
  empty responses — and only before any tool call has run in the child turn.
- There is **no difficulty-based escalation** in the runtime. The pairs
  deliver degradation on failure, not promotion on hardness. Promoting a task
  to a stronger model remains an orchestrator decision at re-dispatch.
- Run-deadline expiry and tool failures never trigger fallback.
- Behavior is as of pi-subagents 0.64.0: quota errors became fallback-eligible
  in 0.51.0, and `agentOverrides` reached custom agents in 0.63.0. Older
  policy notes describing these as open bugs are obsolete.

### Reconciliation note

The governed `config/pi/settings.json` currently lags the live settings: it
is missing `defaultTools`, the six disabled external CLI agents, and part of
the package list. The implementation change reconciles the governed copy with
the live state explicitly — the matrix must not land on top of a silently
stale baseline.

## Part 2 — Steward pilot

### Scope

One clerical helper, not a fleet. The estate already covers durable-fact
capture, output compression, and boundary review through existing tooling.
The one gap worth a pilot is round-by-round task-state upkeep, which today
burns orchestrator turns and context on clerical work.

### Design

- **A resident model on the router.** The estate model router — the
  `maestro-llamacpp` repository, a Rust router exposing one dedicated
  endpoint per model — serves a small instruction model (Qwen3 0.6B class,
  quantized, CPU acceptable) declared **resident**: always loaded, never
  evicted by on-demand swaps. The steward calls that model's dedicated
  endpoint, for example `http://127.0.0.1:8080/models/qwen3-06b/v1`.
  Residency provides the isolation a separate process was previously
  specified for: a steward call never triggers a model swap and never
  competes with the heavy models. Process and unit governance for model
  serving lives in the `maestro-llamacpp` repository; the router design
  itself lives in the `maestro-llamacpp` founding specification.
- **A Pi extension.** `steward.ts`, governed in this repository. On
  `agent_settled` it sends the round summary plus the current task state to
  the resident model's dedicated endpoint, using `llama.cpp` constrained
  decoding with a strict JSON schema, so the output is structurally valid by
  construction. The operation vocabulary is closed: `add`, `complete`,
  `block`, `note`, `noop`.
- **Deterministic validation between model and disk.** The extension checks
  every proposed operation and writes exactly one state file. The model never
  touches tools, the shell, or arbitrary paths. Semantic mistakes remain
  possible; the validation layer and the operator verdicts are the guard.
- **State injection.** A compact summary of the state file is injected at
  `before_agent_start` of the next round, so the orchestrator reads current
  task state without maintaining it.
- **A correction journal.** Every proposed operation is recorded with the
  operator verdict — accepted, corrected, or rejected. This journal is the
  free evaluation set for any future fine-tune decision.

### Non-goals

- No training now. Off-the-shelf model, strict schema, few-shot examples if
  needed. A fine-tune becomes worth discussing only if the journal shows
  systematic errors after real use.
- No additional helpers. Router, librarian, curator, janitor and similar
  ideas wait until the steward proves or disproves the pattern.
- No write path beyond the single state file.

### Future

The steward's state file is a candidate for absorption by the estate's
durable ledger once the MCP interface exists. The pilot is built knowing this
is interim state, not a parallel system of record.
