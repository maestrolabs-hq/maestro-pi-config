# Working in maestro-pi-config

Every post-install change to a pi installation, captured so a machine can be rebuilt.

Written for an agent, and true for a person.

## Before anything else

Read `README.md` for what this is, and `docs/adr/` for what has already been
decided. A decision recorded there was made with reasons; reopen it with new
evidence rather than by preference.

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

## Rules that are enforced, not suggested

**No absolute paths.** Anywhere -- code, configuration, task runner, workflow.
Derive from the environment at runtime. A test fails on this.

**English only.** Prose and identifiers. A test scans for Latin diacritics.

**Write the failing test first, and watch it fail.** A test that passes the
moment it is written has proved nothing: it may be testing the implementation
that was just written rather than the behaviour that was wanted.

**Do not weaken a gate to make it pass.** If a check blocks something correct,
say so in the pull request. A gate that is quietly bypassed is worse than no
gate, because the repository still looks guarded.

## What the gates will tell you

Locally, hooks run formatting and lint at commit time and the rest before a
push. In CI the fast tier blocks the merge; the heavy tier runs weekly and
reports.

Two gates use an allowlist rather than a threshold -- accepted code
duplication, and retired vocabulary. Adding an entry is expected. Adding one
without the reason is not, and a second test fails when an entry stops being
true, so the list cannot rot into excuses.

## Things that will surprise you

**An enforced organisation security configuration overwrites settings applied
by hand.** If something must hold, it belongs in the configuration, not in a
one-off API call.

**Renaming a CI job renames a required context.** The ruleset naming the old
one blocks every pull request until it is updated. They change together.

## When you are done

State what you ran and what it printed. "Tests pass" is a claim; the output is
evidence.
