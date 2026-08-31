# Working in Maestro-Pi-Config

Every post-install change to a pi installation, captured so a machine can be rebuilt.

Written for an agent, and true for a person.

## Before anything else

Read `README.md` for what this is, and `docs/adr/` for what has already been
decided. A decision recorded there was made with reasons. Reopen it with new
evidence, not with a preference.

---

## The four rules

From [Andrej Karpathy's observations](https://x.com/karpathy/status/2015883857489522876)
on how coding agents actually fail. They bias toward caution over speed; on a
one-line change, use judgement.

### 1. Think before coding

State your assumptions **before** implementing. Where more than one reading of
the request exists, present them rather than silently picking one. If a simpler
approach exists, say so and push back. If something is unclear, stop and name
what is confusing.

The failure this prevents: confidently building the wrong thing, fast.

### 2. Simplicity first

Write the least code that solves the problem. No speculative abstraction, no
configuration hook for a single caller, no error handling for a state that
cannot occur. Three similar lines beat a premature abstraction.

The failure this prevents: a framework where a function was wanted.

### 3. Surgical changes

Touch what the task requires and nothing else. A bug fix is not an invitation
to reformat the file, rename the variables, or upgrade the dependency. Drive-by
changes hide the real diff and make a revert dangerous.

The failure this prevents: a two-line fix arriving as a two-hundred-line diff.

### 4. Goal-driven execution

Define what "done" looks like before starting, in terms someone else could
check. Then run that check and report what it printed. "Tests pass" is a claim;
the output is evidence.

The failure this prevents: declaring success without ever verifying it.

---

## The principles behind them

Named, because a shared name makes a code review one word long.

| Principle | What it means here |
| --- | --- |
| **YAGNI** — you aren't gonna need it | Build for the requirement in front of you. A feature added for a future that never arrives is pure cost. |
| **KISS** — keep it simple | Prefer the boring construct. Cleverness is a loan against the next reader's time. |
| **DRY** — don't repeat yourself | Factor out what is genuinely one idea. Not everything that *looks* alike *is* alike -- see WET below. |
| **WET** — write everything twice | Duplicate once and wait. The second occurrence teaches you the shape; the first only guesses it. |
| **Rule of three** | Extract on the third occurrence, not the second. Two points fit any line. |
| **Chesterton's fence** | Do not remove what you do not understand. Find out why it is there first; the answer is often in an ADR. |
| **Boy Scout rule** | Leave code better than you found it -- within the diff you already had reason to touch. Compatible with surgical changes, not an exception to them. |
| **Least astonishment** | A reader's first guess about what something does should be right. |
| **Single responsibility** | One reason to change. This is what the module-size and complexity gates approximate mechanically. |
| **Composition over inheritance** | Assemble behaviour; do not inherit it. Rust makes this easy and it still has to be chosen. |
| **Fail fast** | Refuse bad input at the boundary rather than carrying it inward and failing somewhere confusing. |
| **Make illegal states unrepresentable** | Encode the constraint in the type. A state that cannot be constructed needs no test. |
| **Parse, don't validate** | Convert unstructured input into a type that proves the check already happened. |
| **Principle of least privilege** | Every token, workflow and permission gets the minimum. Applied literally in this estate's CI. |
| **Separation of concerns** | A module has one job. The architecture gates fail the build when a layer reaches across a boundary. |
| **Zero, one, or many** | If something can happen twice it can happen *n* times. Do not hard-code two. |
| **Premature optimisation** | Measure first. A benchmark beats an intuition, and profiling beats both. |
| **Broken windows** | A tolerated small mess licenses the next one. This is why no gate is ever bypassed quietly. |

---

## Rules that are enforced, not suggested

**No absolute paths.** Anywhere -- code, configuration, task runner, workflow.
Derive from the environment at runtime. A test fails on this.

**English only.** Prose and identifiers. A test scans for Latin diacritics.

**Conventional commits.** `feat:`, `fix:`, `docs:`, `refactor:`, `test:`,
`ci:`, `build:`, `chore:`, `perf:`, `revert:`. An organisation ruleset refuses
anything else, and the changelog is generated from them.

**Write the failing test first, and watch it fail.** A test that passes the
moment it is written has proved nothing: it may be testing the implementation
that was just written rather than the behaviour that was wanted.

**Never weaken a gate to make it pass.** If a check blocks something correct,
say so in the pull request. A gate that is quietly bypassed is worse than no
gate, because the repository still looks guarded.

## The shape of a change

`main` takes changes only through a pull request. Direct pushes are refused by
the platform, for the maintainer too.

```text
git switch -c <topic>
just check            # the same commands CI runs, not equivalents
git push -u origin <topic>
gh pr create --fill
gh pr merge --squash --delete-branch
```

## What the gates will tell you

Locally, hooks run formatting and lint at commit time and the rest before a
push. In CI the fast tier blocks the merge; the heavy tier runs weekly and
reports.

Two gates use an allowlist rather than a threshold -- accepted code duplication
and retired vocabulary. Adding an entry is expected. Adding one without the
reason is not, and a second test fails when an entry stops being true, so the
list cannot rot into excuses.

## Things that will surprise you

**An enforced organisation security configuration overwrites settings applied
by hand.** If something must hold, it belongs in the configuration, not in a
one-off API call.

**Renaming a CI job renames a required context.** The ruleset naming the old one
blocks every pull request until it is updated. They change together.

**`governance plan` is the arbiter.** If it reports drift, the estate is wrong
somewhere -- including possibly the baseline itself.
