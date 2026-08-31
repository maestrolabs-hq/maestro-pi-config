# Context

Glossary for maestro-pi-config. Terms only, no implementation detail.

Everything below describes something this repository does today. Terms that
belong to Maestro's design rather than to this tool live in
[maestro-core](https://github.com/maestrolabs-hq/maestro-core), and are not
restated here -- a glossary that defines another project's unbuilt vocabulary
goes stale without anyone noticing.

## Pi modification

Any change to how pi behaves: settings, MCP server registrations, skills,
packages, provider setup, runtime extensions. Every one is recorded here, so a
machine can be rebuilt from this repository.

## Entry

One thing the manifest tracks: a file or a directory, its path on the machine,
and whether it is templated or executable. The manifest is the complete list of
what this tool considers part of a pi installation.

## Layout

Where a machine keeps the directories entries live under. Resolved once from
the environment, then passed as data -- so no function deeper in has to reach
for a variable, and a test can construct one literally.

## Template

A captured file with this machine's home directory replaced by `${HOME}`. What
makes a capture portable: a path baked into a stored file would restore onto
the wrong machine, or onto no machine at all.

## Capture

Reading the live configuration into the repository. `sync` does this. The
result is a git diff, reviewed like any other change.

## Plan

What a restore would change, decided before anything is written. Reports only
real differences. A saved plan records what each target held when it was made,
so applying it can refuse if either the machine or the repository moved since.

## Apply

Carrying out a plan. Refuses without explicit approval, and refuses a saved
plan whose world has shifted. The only code here that writes to the machine.

## Provision

Installing what must exist before configuration means anything: toolchains,
binaries, runtimes. Separate from apply because a missing tool is a different
failure from a stale setting, and reads differently in a report.

## Status

Whether this machine still matches the repository. Absent counts as drift: a
machine holding none of the files is not in sync with a repository holding all
of them.
