# TODO

Findings from an adversarial review, ordered by what they cost if left alone.

This tool writes to the user's machine. Every P0 and P1 below is on that write
path, which is the only part of the estate that can destroy something the user
cannot get back.

---

## P0 -- can lose data or execute unreviewed writes

### 0. `destroy` deletes directories it never created, climbing past `$HOME`

`src/plan/destroy.rs:75-83`

```rust
fn prune_empty_parents(from: &Path) {
    let mut dir = from.parent();
    while let Some(d) = dir {
        if fs::remove_dir(d).is_err() { return; }
        dir = d.parent();
    }
}
```

There is no root bound. The loop stops only when a directory is non-empty --
not when it leaves the layout this tool owns. Run against a sandbox home with a
sentinel planted so the climb could not escape:

```text
--- destroy ---
Destroy: 8 to remove, 0 kept.
Destroy complete. 8 file(s) removed.
--- verdict ---
>>> HOME (<scratch>/box/home) WAS DELETED BY destroy
>>> PARENT OF HOME (<scratch>/box) WAS DELETED BY destroy
```

It removed the home directory and the directory above it. Neither was created
by this tool. It stopped only because of the sentinel.

Three written guarantees say this cannot happen:

- `src/plan/destroy.rs:55` -- "only the ones that become empty as a result. A
  directory holding anything else was not ours alone."
- `config/README.md:70` -- "the worst case is that it leaves more behind than
  you expected, **never less**."
- `src/cmd/destroy.rs:16` -- "That makes destroy safe to run without first
  checking what you changed."

The bound implemented is "non-empty", not "inside the layout". On a populated
machine `$HOME` has other contents and the climb stops -- but `~/.config`
becoming empty is ordinary, and `~/.config` is not ours to remove. In a
container or a fresh account, `$HOME` goes.

No test catches it. `destroy_is_idempotent` (`tests/plan_apply.rs:310`) runs
destroy twice and asserts the second prints "Nothing to remove" -- which it
does, because the directory is gone. Every test's scratch home lives under
`std::env::temp_dir()`, so the suite performs this upward climb against the
system temp directory on every `cargo test`.

**Fix:** bound the climb. Pass the `Layout` root into `plan::destroy::run` and
stop when `d` is no longer a strict descendant -- `d.starts_with(root) && d !=
root`. Write the failing test first: apply into a scratch home, destroy, assert
the home directory still exists.

### 1. A saved plan is an unauthenticated instruction to write anywhere

`src/plan/file.rs:56`, `src/cmd.rs:104`

`plan.out` is a tab-separated file naming absolute target paths, content
sources, and an executable bit. `apply <file>` reads it and writes exactly what
it says, with **no approval prompt** -- deliberately, because writing the plan
was the approval.

That holds only if the file cannot be substituted. Nothing authenticates it. A
plan naming `~/.bashrc` or `~/.ssh/authorized_keys` at mode 0755 applies
silently. The staleness checks do not help: they compare the digest recorded in
the file against the machine, so a hostile plan is self-consistent by
construction.

The threat model is narrow -- an attacker who can write `plan.out` can usually
write the targets directly -- but the asymmetry is bad. We turned a file into
executable intent and documented that it needs no confirmation.

**Fix:** confine targets to paths the manifest actually claims. Reject any
action whose target is not a known live path. Cheap, and it makes the file
inert outside its purpose.

### 2. An unreadable existing file is planned as a create, then clobbered

`src/plan.rs:142`

```rust
let (change, observed) = match fs::read_to_string(&target) {
    ...
    Err(_) => (Change::Create, None),
};
```

`read_to_string` fails on invalid UTF-8 and on permission errors, not only on
absence. Any such target is reported as `+ path` -- "to add" -- for a file that
already exists, and then overwritten.

The saved-plan safety net does not catch it. `apply.rs:42` uses the same lossy
read:

```rust
let now = fs::read_to_string(&a.target).ok().map(|c| digest(&c));
if now != a.observed {
```

`None == None`, so the "changed since planning" guard passes for exactly the
files whose contents were never observed. This is the one path that can destroy
user data without saying so.

**Fix:** match `e.kind() == ErrorKind::NotFound` for the `Create` arm. Fail the
plan on any other error.

---

## P1 -- the guarantees are weaker than stated

### 3. FNV-1a is load-bearing for an approval gate its own comment disclaims

`src/plan.rs:52-60`

```rust
/// FNV-1a. Not for security -- only to notice that bytes moved.
```

It is the sole check that a saved plan still matches the machine. If the
comment is right, the gate is decorative; if the gate matters, the comment is
wrong. Both cannot hold.

Also: the prime is wrong. `0x1000_0000_01b3` is 13 hex digits. The FNV-1a
64-bit prime is `0x0000_0100_0000_01b3`. What we have is a different, weaker
mixing constant. It still detects accidental change, which is what we use it
for -- but nobody chose it.

**Fix:** correct the constant, and rewrite the comment to say what it does:
detects accidental change, not adversarial substitution. Then fix #1 properly,
since that is the real defence.

### 4. Writes follow symlinks, for content and for mode

`src/plan/apply.rs:18-19, 63-64`

`fs::write` and the mode change both follow a symlink at the target. A symlink
planted at any managed path redirects the write, and `set_executable` chmods
the destination.

**Fix:** `O_NOFOLLOW` on the target, or `symlink_metadata` before writing.

### 5. Apply is neither atomic per file nor recoverable across files

`src/plan/apply.rs:59-66`

`fs::write` truncates then writes: an interruption leaves a truncated file. And
the error carries no path, because `io::Error`'s `Display` omits it -- so a
mid-apply failure surfaces as `apply failed: Permission denied (os error 13)`
with N files already written and no way to know which.

`apply_saved` verifies everything before writing anything, then throws that care
away in the write loop.

**Fix:** write to a temp file in the same directory and rename. Include the
path in the mapped error. On failure, print what was already written.

### 6. TOCTOU window is the whole apply, not an instant

`src/plan/apply.rs:42` verifies; `:63` writes. Every file is verified first,
then all are written. The gap is the duration of the apply.

Acceptable for a single-user config tool. Worth a comment saying it is a
deliberate choice rather than an oversight.

---

## P2

### 7. `${HOME}` expansion produces invalid JSON on Windows, and the test cannot fail

The template stores `${HOME}` and expands it on restore. On Windows the
expansion inserts a path containing backslashes into a JSON string without
escaping, producing a file that does not parse. The test covering the round
trip asserts on the template form rather than the expanded output, so it passes
on every platform regardless.

**Fix:** escape the expansion for the target format, and assert on the parsed
result rather than the string.

### 8. baseline.txt documents one `.pre-commit-config.yaml` divergence; there are four

`maestro-governance/baseline.txt` scopes the entry to two repositories and
explains one deliberate difference. The three copies actually differ four ways:

| | maestro-core | maestro-governance | maestro-pi-config |
| --- | --- | --- | --- |
| `cargo-test` entry | `cargo test --all-targets` | `cargo test --all-targets` | `cargo test` |
| `cargo-similarity` entry | `--test standards` | `--test standards` | `--test duplication` |
| structural check | `check-toml` | `check-toml` | `check-json` |

**Fix:** document each divergence or remove it. A scoped baseline entry is how
a real difference stays visible as a decision; three undocumented ones are how
it hides as drift.

### 9. `config/README.md` documents ten deleted scripts as present

It also states "Nothing here contains an absolute path" while `config/`
contains them -- the gate exempts the directory, which is why nothing noticed.

**Fix:** regenerate the inventory from what is actually captured.

### 10. The `config/` exemption is directory-shaped, not provenance-shaped

Every gate skips `config/` because it holds captured third-party state. The
exemption is by path, so anything placed there is exempt regardless of origin.

**Fix:** state the intent in one place and have each gate cite it, or mark
captured files explicitly and exempt on the mark.

### 11. Dead captured state

`config/shell/cargo-env.sh` and `Layout.user_bin` are captured and referenced
by nothing.

**Fix:** delete, or record why they are kept.

### 12. `provision.txt` claims versions are pinned; six of thirteen are not

`config/provision.txt:4`

```text
# Versions are pinned. Moving one is a deliberate edit, not a drift.
```

Thirteen pi packages, seven with `@version`. The other six float. Provisioning
two machines a week apart gives two different environments -- exactly what this
file exists to prevent.

**Fix:** pin the six, or narrow the claim to the ones that are pinned.

---

## Round 2 -- the write path, executed against scratch machines

### 13. `sync` captures a credentials file into a public repository

`src/manifest.rs:140` puts `~/.codegraphcontext/.env` in the capture manifest,
stored as `config/tools/codegraphcontext/env.template`. `src/config.rs:152`
writes it verbatim; `as_stored` collapses the home directory and nothing else.
Four realistic credentials planted and staged:

```text
== does .gitignore stop it? ==   NOT IGNORED -- it is staged
== gitleaks 8.24.3 protect --staged ==
  OPENAI_API_KEY=sk-proj-...              caught
  ANTHROPIC_API_KEY=sk-ant-api03-...      MISSED
  FALKORDB_PASSWORD=hunter2               MISSED
  NEO4J_URI=neo4j+s://user:pw@host:7687   MISSED
leaks found: 1
```

`.gitignore:73` ignores `.env` and `.env.*`; the stored filename is
`env.template`, so those rules never apply. The version tested is the one
pinned at `.pre-commit-config.yaml:36`.

`config/README.md:121` states *"Credentials never leave the machine they were
issued on."* The live file is empty today -- that is the state of one machine,
not a property of the design.

**Why it matters:** this is a public repository and `just sync` is one word. The
only backstop is entropy-based and misses the two commonest shapes, a bare
password and a URI with inline credentials, in the one captured file whose
purpose is to hold secrets.

**Fix:** drop the file from the manifest and keep only the annotated template,
as `config/INVENTORY.md:58` already does for `auth.json` and `hosts.yml`. If it
must stay, add a `redacted` flag that blanks every `KEY=value` not on an
allowlist, and capture through that rather than trusting a scanner.

### 14. An empty environment override makes every managed path relative

`src/manifest.rs:67` guards the home directory against an empty value;
`:104-116` applies no such guard to `PI_AGENT_DIR`, `XDG_CONFIG_HOME` or
`PI_CONFIG_BIN_DIR`. `env::var_os` returns `Some("")` for a set-but-empty
variable -- the ordinary result of `export XDG_CONFIG_HOME="$UNSET_VAR"`. Full
round trip from an unrelated working directory:

```text
$ env XDG_CONFIG_HOME= PI_AGENT_DIR= HOME=$SB/home pi-config apply --auto-approve
  + settings.json          <- relative, not absolute
  + mcp/mcp.json
Apply complete. 6 added, 2 changed.        <- the user's own files overwritten
$ pi-config destroy --auto-approve
Destroy complete. 8 file(s) removed.       <- working directory emptied
```

`Entry.live` is documented as *"Absolute path on this machine"* and the module
header cites ADR-0001, but the derivation yields a relative path and nothing
checks. The two verbs compose into data loss: `apply` first overwrites the
foreign file so it matches the repository, which converts `destroy`'s
protection -- keep anything that no longer matches -- into permission to remove
it.

`every_entry_is_anchored_under_the_given_home` (`src/manifest.rs:195`) asserts
exactly the invariant `from_env` breaks, but calls `Layout::under`, the one
constructor that cannot break it.

**Fix:** apply the empty-value filter to all three overrides, assert
`entry.live.is_absolute()` at the manifest boundary, and point the test at
`from_env`.

### 15. A hand-installed skill is never captured, and both verbs report success

`src/config.rs:156` and `src/plan.rs:192` walk the files the *repository*
already carries, so a file that exists only on the machine is invisible to
every verb:

```text
== user installs a new skill ==   .pi/agent/skills/my-new-skill/SKILL.md
== status ==  exit=0  In sync with this machine.
== sync ==    exit=0  8 file(s) pulled in.
== is it in the repository? ==    no
```

`NORTHSTAR.md:13` promises *"A machine can be rebuilt from the repository."*
Skills are the category most likely to be added by hand, and this is the one
failure where the user gets no signal at all.

**Fix:** for a directory entry, walk the union of live and repository trees;
report a live-only file as new and pull it in.

### 16. Other proven write-path defects

- A saved plan replayed under a different home writes to the old machine's
  paths with the new machine's home in the contents.
- A symlink appearing between plan and apply defeats the staleness check and
  writes outside the home directory.
- A saved plan applies over a file it swore it would refuse to touch.
- An unreadable captured file puts the tool in permanent, unfixable drift.
- `to_template` corrupts any path that merely *starts with* the home directory
  string.
- On Windows, capture never collapses the home directory in a JSON file at all.
- The directory walk follows symlinks with no cycle detection or depth bound.
- `sync` silently skips files it cannot read and reports success.
- `cargo test` deletes `$TMPDIR`; PR #21 moves that suite onto Windows and
  macOS on every pull request.

### 17. Untested guarantees, proved by mutation

Each of these was deleted from a scratch copy and the suite still passed:

- `apply_saved`'s all-or-nothing property.
- `destroy`'s "unreadable is not absent" guard.
- The plan-file header check.
- A whole manifest entry.
- The executable-bit write path -- only the duplication gate noticed.
- The saved-plan "machine moved" guarantee, for the case it exists to cover.
