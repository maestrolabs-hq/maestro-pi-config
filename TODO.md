# TODO

Findings from an adversarial review, ordered by what they cost if left alone.

This tool writes to the user's machine. Every P0 and P1 below is on that write
path, which is the only part of the estate that can destroy something the user
cannot get back.

---

## P0 -- can lose data or execute unreviewed writes

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

### 7. `provision.txt` claims versions are pinned; six of thirteen are not

`config/provision.txt:4`

```text
# Versions are pinned. Moving one is a deliberate edit, not a drift.
```

Thirteen pi packages, seven with `@version`. The other six float. Provisioning
two machines a week apart gives two different environments -- exactly what this
file exists to prevent.

**Fix:** pin the six, or narrow the claim to the ones that are pinned.
