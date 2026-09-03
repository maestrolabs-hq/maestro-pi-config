# Agent orchestration implementation plan

> **For agentic workers:** Use `superpowers:subagent-driven-development` or
> `superpowers:executing-plans` to run this plan task by task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put the Part 1 routing matrix from
`docs/superpowers/specs/2026-09-03-agent-orchestration-and-steward.md` into the
governed configuration, provision the two new agent sources to the live agent
directory, enable the boundary watchdog, and prove the result on the running
machine.

**Architecture:** Governed files under `config/` reach live destinations
through `src/manifest.rs`, which pairs a repository path with a machine path
derived from the environment. `plan` reports differences, `apply` writes them.
`Kind::Dir` entries mirror every file under a repository directory into the
live directory, which is how `config/pi/skills` reaches
`<pi agent dir>/skills`. The new agent sources need the same treatment: pi
discovers user-scope agents at `<pi agent dir>/agents/**/*.md`, and no manifest
entry covers that path today.

**Spec:** `docs/superpowers/specs/2026-09-03-agent-orchestration-and-steward.md`
(Part 1). Part 2, the steward pilot, is out of scope here and depends on the
`maestro-llamacpp` router.

## Global constraints

- The spec is the source of truth. Where this plan departs from it, the
  departure is named in the task and carries its evidence.
- One concern per commit. Reconciliation lands before the matrix, so the
  matrix diff shows only the matrix.
- Nothing is silently dropped. When governed and live configuration differ,
  every difference is resolved deliberately and recorded.
- Failing test first where the repository's test style supports it.
- No gate is weakened. A blocked check is reported, not bypassed.
- English only in tracked prose. `config/` is exempt from the accent gate by
  design, being captured third-party configuration.
- Conventional commits.
- Paths are derived from the environment; no tracked file names a machine.
- `just check` passes before every commit.

## Verified runtime facts

Recorded here so no task designs against behaviour that does not exist. All
evidence gathered from the installed `pi-subagents` 0.64.0 and
`pi-claude-bridge` 0.7.0.

| Key | Verdict | Evidence |
| --- | --- | --- |
| `subagents.agentOverrides.<name>` | As specified | `docs/models.md:9`, parsed at `src/agents/agents.ts:1187` |
| `subagents.agentOverridesByProvider.<provider>.<name>` | As specified | `docs/models.md:11`, parsed at `src/agents/agents.ts:1212-1222` |
| `subagents.maxThinking` | As specified | `docs/models.md:136-147`; levels `off, minimal, low, medium, high, xhigh, max` |
| frontmatter `fallbackModels` | As specified | `docs/agents.md:311`; block-list form at `docs/agents.md:288-294` |
| `subagents.watchdog.main.model` | Requires explicit thinking | `docs/watchdog.md:106` |

Two facts that shape the tasks:

- **The claude-bridge provider is registered at runtime**, not in
  `models-store.json`. `pi-claude-bridge` calls
  `pi.registerProvider("claude-bridge", ...)` at `src/index.ts:2064`, and its
  model ids come from `src/models.ts:5`. `claude-fable-5`, `claude-opus-5` and
  `claude-sonnet-5` all exist and map to 1M-context CLI variants
  (`src/models.ts:44-62`). Every claude-bridge entry in the matrix therefore
  depends on that package staying installed and authenticated. If it is
  removed, those models stop resolving and the fallback chains lose their
  cross-family half.
- **`<pi agent dir>/agents/` does not exist on this machine.** Nothing has
  written a user-scope agent before, which is why no manifest entry covers it.

---

## Task 1 — Validate the configuration schema before writing any configuration

The spec was written from design discussion, not from the parser. Confirm each
key against the implementation before any settings change is made.

One key has already been resolved this way. The spec originally asked for
`subagents.modelScope` in a purely advisory `warn` mode; the parser accepts
no such shape, and `enforce: true` turns an unapproved explicit per-run model
into a hard error rather than a warning. Rather than adopt a stricter guard
rail than the one designed, the user dropped `modelScope` from the design
entirely: nothing in the matrix depends on it, and scope enforcement can be
revisited if unapproved-model drift actually appears. Neither the spec nor
this plan configures it.

### Steps

- [ ] Confirm the package version under test.

```sh
node -e "console.log(require(process.env.HOME + '/.pi/agent/npm/node_modules/pi-subagents/package.json').version)"
```

Expected: `0.64.0`.

- [ ] Confirm every model id in the matrix resolves.

```sh
python3 - <<'PY'
import json, os
store = json.load(open(os.path.expanduser("~/.pi/agent/models-store.json")))
for provider, value in store.items():
    ids = [m.get("id") for m in value.get("models", [])]
    if ids:
        print(provider + ": " + ", ".join(ids))
PY
grep -n "MODEL_IDS_IN_ORDER" "$HOME/.pi/agent/npm/node_modules/pi-claude-bridge/src/models.ts"
```

Expected: `openai-codex` exposes `gpt-5.3-codex-spark`, `gpt-5.5`,
`gpt-5.6-luna`, `gpt-5.6-sol`, `gpt-5.6-terra`; `llama.cpp` exposes `qwen38`;
and the claude-bridge list contains `claude-fable-5`, `claude-opus-5`,
`claude-sonnet-5`. Note that claude-bridge models are absent from the store
because the package registers them at runtime.

### Verification

Every row of the table under "Verified runtime facts" is reproduced from the
installed source. No file has been modified yet.

---

## Task 2 — Reconcile the governed settings with the live machine

The governed `config/pi/settings.json` lags the live
`<pi agent dir>/settings.json`. The matrix must not land on a stale baseline,
so this is its own commit, before any matrix change.

The complete difference, both directions:

| Present live, absent governed | Value |
| --- | --- |
| `defaultTools` | `["read", "bash", "edit", "write", "grep", "find", "ls"]` |
| `subagents.agentOverrides` | the six external CLI agents, each `{ "disabled": true }` |
| package | `npm:@plannotator/pi-extension` |
| skills | 15 further entries from the pinned `mattpocock/skills` source |

The 15 further skills: `diagnosing-bugs`, `implement`,
`resolving-merge-conflicts`, `setup-matt-pocock-skills`, `tdd`, `to-spec`,
`to-tickets`, `triage`, `wizard`, `misc/git-guardrails-claude-code`,
`misc/migrate-to-shoehorn`, `misc/scaffold-exercises`, `misc/setup-pre-commit`,
`productivity/grill-me`, `productivity/writing-for-agents`.

**Present governed, absent live: none.** The live skills list is a strict
superset of the governed one, so reconciliation drops nothing.

### Steps

- [ ] Capture the difference as evidence before editing.

```sh
cd /path/to/maestro-pi-config
diff <(python3 -m json.tool config/pi/settings.json) \
     <(python3 -m json.tool "$HOME/.pi/agent/settings.json")
```

- [ ] Confirm no governed skill is missing from the live list.

```sh
python3 - <<'PY'
import json, os
def skills(p):
    d = json.load(open(p))
    for pkg in d["packages"]:
        if isinstance(pkg, dict) and "mattpocock" in pkg["source"]:
            return set(pkg["skills"])
    return set()
governed = skills("config/pi/settings.json")
live = skills(os.path.expanduser("~/.pi/agent/settings.json"))
print("governed only:", sorted(governed - live))
print("live only:", len(live - governed), "entries")
PY
```

Expected: `governed only: []`. If that list is non-empty, stop: something was
removed live that the repository still carries, and the reconciliation
direction has to be decided rather than assumed.

- [ ] Bring `config/pi/settings.json` up to the live state: add
      `defaultTools`, the `subagents.agentOverrides` block with the six
      disabled external agents, the `@plannotator/pi-extension` package, and
      the full skills selection. Key order follows the live file so a later
      `sync` produces no spurious diff.

- [ ] Add the missing package to `config/provision.txt` under the pi section,
      so a fresh machine installs what the settings file declares.

```text
pi npm:@plannotator/pi-extension
```

- [ ] Verify the governed file is now byte-identical to the live one.

```sh
diff config/pi/settings.json "$HOME/.pi/agent/settings.json" && echo IDENTICAL
```

Expected: `IDENTICAL`.

- [ ] Run the gates and commit.

```sh
just check
git diff --check
git add config/pi/settings.json config/provision.txt
git commit -m "chore: reconcile the governed pi settings with the machine"
```

The commit message body lists every difference resolved, in both directions,
including the explicit statement that nothing was dropped from the governed
side.

### Verification

`diff` reports the governed and live settings identical, `just check` passes,
and the commit body records the full difference.

### Note for the reviewer

`config/provision.txt` pins `uv graphifyy==0.9.51`, while the estate's reviewed
pin elsewhere is `graphifyy[mcp,openai]==0.9.53`. That is a real drift, but it
belongs to provisioning rather than orchestration. It is recorded here and
deliberately left alone, so this change stays surgical. Raise it as its own
task.

---

## Task 3 — Add the matrix and the two agent sources

### Steps

- [ ] Write the failing test first. Add `tests/agents.rs`, modelled on
      `tests/skills.rs`, which asserts the governed agent sources exist and
      declare what pi needs. It must fail before the files are written.

```rust
//! The governed agent sources are what pi loads as user-scope agents.
//!
//! A missing frontmatter field does not error: pi silently falls back to the
//! session model, so a routing decision would vanish without a failure anyone
//! sees. Hence these tests.

use std::fs;
use std::path::PathBuf;

fn agent(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config/pi/agents")
        .join(format!("{name}.md"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn each_agent_declares_itself_to_pi() {
    for name in ["spark", "adversary"] {
        let text = agent(name);
        assert!(text.starts_with("---"), "{name}: frontmatter is what makes pi load it");
        assert!(text.contains(&format!("name: {name}")), "{name}: pi registers the agent by this name");
    }
}

#[test]
fn each_agent_pins_a_model_and_a_cross_family_fallback() {
    for name in ["spark", "adversary"] {
        let text = agent(name);
        assert!(text.contains("model:"), "{name}: an unpinned agent inherits the session model");
        assert!(text.contains("fallbackModels:"), "{name}: a lane with no fallback dies on a quota error");
    }
}

#[test]
fn the_adversary_cannot_edit() {
    let text = agent("adversary");
    for forbidden in ["edit", "write", "bash"] {
        assert!(
            !text.contains(&format!(" {forbidden},")) && !text.contains(&format!(" {forbidden}\n")),
            "adversary: a review lane that can edit is not a review lane"
        );
    }
}
```

```sh
cargo test --test agents
```

Expected: failure, because `config/pi/agents/` does not exist. Record the
output.

- [ ] Create `config/pi/agents/spark.md` and `config/pi/agents/adversary.md`
      with the frontmatter exactly as the spec gives it.

- [ ] Wire the directory into the manifest. In `src/manifest.rs`, add one
      entry beside the skills entry:

```rust
Entry::dir("config/pi/agents", pi.join("agents")),
```

`Kind::Dir` already mirrors every file underneath, so no other code changes.

- [ ] Add the `subagents` block from the spec to
      `config/pi/settings.json`, merged with the six disabled external agents
      that Task 2 already placed there, and switch `defaultProvider` to
      `claude-bridge` and `defaultModel` to `claude-fable-5`.

- [ ] Confirm the settings file still parses as the runtime expects.

```sh
python3 -m json.tool config/pi/settings.json > /dev/null && echo "valid json"
```

- [ ] Run the tests and gates.

```sh
cargo test --test agents
cargo test
just check
git diff --check
```

Expected: `tests/agents.rs` now passes; the existing `plan_apply` suite still
passes, because the scratch-home tests will now also create the agents
directory.

- [ ] Commit.

```sh
git add config/pi/agents config/pi/settings.json src/manifest.rs tests/agents.rs
git commit -m "feat: route each agent role to a model with a cross-family fallback"
```

### Verification

`cargo test --test agents` fails before the agent files exist and passes after.
`just check` passes. The manifest carries an entry whose live path is
`<pi agent dir>/agents`.

---

## Task 4 — Enable the boundary watchdog

The spec states the watchdog is enabled by policy, so it is persisted at user
scope rather than set for one session.

Note from `docs/watchdog.md:106`: a `main.model` without a thinking suffix and
without `main.thinking` runs **with thinking off**. The thinking level is not
optional here.

### Steps

- [ ] Add the watchdog block to the governed `config/pi/settings.json`, inside
      `subagents`:

```jsonc
"watchdog": {
  "enabled": true,
  "main": { "model": "openai-codex/gpt-5.5", "thinking": "high" }
}
```

Cadence stays absent: the spec keeps cadence reviews off until boundary
reviews prove their value.

- [ ] Confirm the file still parses.

```sh
python3 -m json.tool config/pi/settings.json > /dev/null && echo "valid json"
```

- [ ] Run the gates and commit.

```sh
just check
git diff --check
git add config/pi/settings.json
git commit -m "feat: review each turn boundary with the complementary model family"
```

### Verification

`just check` passes and the governed settings carry the watchdog block with an
explicit thinking level.

---

## Task 5 — Apply, reload, and prove it on the machine

Configuration that has not been applied and exercised is a claim, not a
result.

### Steps

- [ ] Plan first, and read what it reports.

```sh
just plan
cat plan.out
```

Expected: `config/pi/settings.json` listed as a change, and the two agent
files listed as additions under `<pi agent dir>/agents/`. If the agent files
are absent from the plan, the manifest entry from Task 3 is missing or
misspelled; stop and fix that before applying.

- [ ] Apply the reviewed plan.

```sh
just apply
```

- [ ] Confirm the live files arrived.

```sh
diff config/pi/settings.json "$HOME/.pi/agent/settings.json" && echo IDENTICAL
ls "$HOME/.pi/agent/agents/"
```

Expected: `IDENTICAL`, and both `spark.md` and `adversary.md` present.

- [ ] **Operator step, requiring a session reload.** Pi reads settings and
      discovers agents at session start. None of the checks below mean
      anything until the session has been reloaded. Reload now, then continue.

- [ ] Confirm both new agents are discovered.

```text
subagent({ action: "list", capabilities: true })
```

Expected: `spark` and `adversary` appear alongside the six builtins; the six
external CLI agents remain absent.

- [ ] Dispatch one bounded run per role and read the resolved model from run
      status rather than from the agent's own report. One example, to be
      repeated per role:

```text
subagent({ workflowScript: `return runs.run("probe", { agent: "scout", task: "Print the repository root and stop." })`, async: true })
subagent({ action: "status", id: "<run id>" })
```

Expected resolved models: `scout` and `delegate` on
`claude-bridge/claude-sonnet-5`; `worker` on `openai-codex/gpt-5.6-luna`;
`researcher` and `oracle` on `openai-codex/gpt-5.6-sol`; `spark` on
`openai-codex/gpt-5.3-codex-spark`; `adversary` on
`claude-bridge/claude-opus-5`; `reviewer` on the family opposite the session
provider.

- [ ] Confirm the reviewer's provider routing specifically, since it is the
      only role whose model depends on the parent session. With the session on
      `claude-bridge`, the reviewer must resolve to
      `openai-codex/gpt-5.6-terra`.

- [ ] Confirm the runtime reports no configuration error.

```text
subagent({ action: "doctor" })
```

Expected: discovery lists every role above with no configuration error.

- [ ] Confirm the watchdog is live.

```text
/subagents-watchdog status
```

Expected: enabled, model `openai-codex/gpt-5.5`, thinking `high`, boundary
trigger, cadence off.

- [ ] **Record the fallback honestly.** Quota-driven fallback cannot be forced
      in a test: it fires only on a real retryable provider failure, and
      exhausting a subscription to observe it is not a reasonable check. Do
      not claim it verified. What is verified instead:

  1. every fallback model id resolves in the active registry, which is what
     `subagent({ action: "doctor" })` and the successful dispatches above
     establish;
  2. the chains are syntactically accepted, which settings loading proves —
     a malformed `fallbackModels` entry throws at load;
  3. the first real occurrence is observed rather than staged. When a
     provider quota is next reached, capture the run status showing the
     fallback attempt and append it to this plan as evidence.

  Until then, the fallback chains are configured and plausible, not proven.

### Verification

Agents list shows `spark` and `adversary`; each role's resolved model in run
status matches the matrix; the reviewer routes to the opposite family; doctor
reports no configuration error; watchdog status reports enabled with thinking
`high`. Fallback is recorded as configured-not-proven, with the observation
method named.

---

## Task 6 — Update the captured documentation

`config/README.md` carries a table mapping each governed path to its live
destination, and `config/INVENTORY.md` lists the pi packages. Both are now
incomplete.

### Steps

- [ ] Add the agents row to the table in `config/README.md`:

```text
| `pi/agents/` | `<pi agent dir>/agents/` | the two custom agent roles |
```

- [ ] Update the `pi/settings.json` row in the same table, which currently
      reads "packages, provider, default model, theme". It now also carries
      the routing matrix and the watchdog.

- [ ] Add `npm:@plannotator/pi-extension` to the pi packages list in
      `config/INVENTORY.md`.

- [ ] Run the gates and commit.

```sh
just check
git diff --check
git add config/README.md config/INVENTORY.md
git commit -m "docs: record the agent sources and the routing matrix"
```

### Verification

`just check` passes. Every governed path under `config/` appears in the
`config/README.md` table.

---

## What this plan does not do

- **Part 2, the steward.** It depends on a resident model endpoint from the
  `maestro-llamacpp` router, which does not exist yet.
- **Difficulty-based escalation.** The runtime has none. The matrix pairs
  deliver degradation on failure, not promotion on hardness, and this plan
  does not pretend otherwise.
- **A fallback for the main session.** None exists. A quota failure there
  remains a manual model switch.
- **A fallback for the watchdog.** The watchdog is a direct model call, not a
  subagent, and its configuration has no fallback field. During an
  `openai-codex` outage the watchdog stops until its model is switched by
  hand.
- **The `graphifyy` pin drift** noted in Task 2.
