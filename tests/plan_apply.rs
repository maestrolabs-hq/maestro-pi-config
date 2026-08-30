//! Plan reports only real changes; apply refuses to act unprompted.
//!
//! Every test drives the binary with a scratch `HOME`, which both keeps them
//! away from the real machine and proves the tool is environment-driven.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pi-config-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn run(home: &Path, args: &[&str]) -> (String, bool) {
    run_in(home, None, args)
}

fn run_in(home: &Path, repo: Option<&Path>, args: &[&str]) -> (String, bool) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pi-config"));
    cmd.args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("PI_AGENT_DIR")
        .env_remove("PI_CONFIG_BIN_DIR");
    if let Some(repo) = repo {
        cmd.env("PI_CONFIG_REPO", repo);
    }
    let out = cmd.output().expect("run pi-config");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.success(),
    )
}

#[test]
fn a_bare_machine_plans_only_creations() {
    let home = scratch("bare");
    let (out, ok) = run(&home, &["plan"]);
    assert!(ok, "planning is read-only and should succeed:\n{out}");
    assert!(out.contains("to add"), "it should count creations:\n{out}");
    assert!(!out.contains(" ~ "), "nothing exists to update:\n{out}");
    assert_eq!(
        fs::read_dir(&home).expect("scratch").count(),
        0,
        "plan must not write"
    );
}

#[test]
fn apply_refuses_without_approval() {
    let home = scratch("refuse");
    let (out, ok) = run(&home, &["apply"]);
    assert!(
        !ok,
        "an unapproved apply must fail, not silently do nothing"
    );
    assert!(out.contains("Refusing"), "it should say why:\n{out}");
    assert_eq!(
        fs::read_dir(&home).expect("scratch").count(),
        0,
        "nothing may be written"
    );
}

#[test]
fn apply_is_idempotent() {
    let home = scratch("idempotent");
    let (_, ok) = run(&home, &["apply", "--auto-approve"]);
    assert!(ok, "first apply should succeed");

    let (out, ok) = run(&home, &["plan"]);
    assert!(ok);
    assert!(
        out.contains("No changes"),
        "a second plan should find nothing left to do:\n{out}"
    );
}

#[test]
fn only_the_changed_file_appears_in_the_plan() {
    let home = scratch("drift");
    run(&home, &["apply", "--auto-approve"]);

    let settings = home.join(".pi/agent/settings.json");
    fs::write(&settings, "tampered").expect("tamper");

    let (out, _) = run(&home, &["plan"]);
    assert!(out.contains("~ "), "a modified file is an update:\n{out}");
    assert!(out.contains("1 to change"), "exactly one change:\n{out}");
    assert!(out.contains("0 to add"), "nothing is missing:\n{out}");
    assert!(
        !out.contains("mcp.json"),
        "untouched files must not be listed; a plan that lists what it will not \
         do buries what it will:\n{out}"
    );
}

#[test]
fn a_templated_entry_expands_the_home_directory() {
    let home = scratch("template");
    run(&home, &["apply", "--auto-approve"]);

    let restored = fs::read_to_string(home.join(".mempalace/config.json")).expect("mempalace");
    assert!(
        !restored.contains("${HOME}"),
        "the placeholder must expand:\n{restored}"
    );
    assert!(
        restored.contains(&home.display().to_string()),
        "it should point at this machine's home:\n{restored}"
    );
}

#[test]
fn applied_files_match_the_captured_bytes_exactly() {
    let home = scratch("bytes");
    run(&home, &["apply", "--auto-approve"]);

    let applied = fs::read_to_string(home.join(".pi/agent/settings.json")).expect("applied");
    let captured = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/pi/settings.json"),
    )
    .expect("captured");
    assert_eq!(
        applied, captured,
        "an apply must reproduce the captured bytes exactly"
    );
}

#[test]
fn a_saved_plan_is_applied_without_a_second_approval() {
    let home = scratch("saved");
    let file = home.join("plan.out");
    let f = file.display().to_string();

    let (out, ok) = run(&home, &["plan", "--out", &f]);
    assert!(ok, "{out}");
    assert!(file.exists(), "the plan should have been written");

    // No --auto-approve: reviewing produced the file, that is the approval.
    let (out, ok) = run(&home, &["apply", &f]);
    assert!(ok, "a saved plan should apply:\n{out}");
    assert!(
        home.join(".pi/agent/settings.json").exists(),
        "it should have written"
    );
}

#[test]
fn a_plan_is_refused_once_the_machine_moves_under_it() {
    let home = scratch("stale-machine");
    let file = home.join("plan.out");
    let f = file.display().to_string();
    run(&home, &["plan", "--out", &f]);

    let target = home.join(".pi/agent/settings.json");
    fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
    fs::write(&target, "someone else wrote this").expect("tamper");

    let out = Command::new(env!("CARGO_BIN_EXE_pi-config"))
        .args(["apply", &f])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .output()
        .expect("run");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "a stale plan must not apply");
    assert!(err.contains("stale plan"), "it should say why:\n{err}");
    assert_eq!(
        fs::read_to_string(&target).expect("read"),
        "someone else wrote this",
        "nothing may be written when any action is stale"
    );
}

#[test]
fn a_plan_is_refused_once_the_repository_moves_under_it() {
    // Point the tool at a scratch copy of the repository so a source can be
    // edited for real. Asserting against a digest the test computes itself
    // would pass even with the implementation's hash stubbed to a constant --
    // which is exactly what the earlier version of this test did.
    let home = scratch("stale-repo");
    let repo = home.join("repo");
    copy_repo_into(&repo);

    let plan = home.join("plan.out");
    let f = plan.display().to_string();
    let (_, ok) = run_in(&home, Some(&repo), &["plan", "--out", &f]);
    assert!(ok, "planning against the copy should work");

    // Edit a source the plan actually names. A constant hash cannot tell this
    // from the original, so the refusal below only happens if the digest is real.
    let source = repo.join("config/pi/settings.json");
    fs::write(&source, "{\"edited\": \"after planning\"}").expect("edit source");

    let out = Command::new(env!("CARGO_BIN_EXE_pi-config"))
        .args(["apply", &f])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("PI_CONFIG_REPO", &repo)
        .output()
        .expect("run");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a source edited after planning must not apply"
    );
    assert!(
        err.contains("changed in the repository"),
        "it should say why:\n{err}"
    );
}

fn copy_repo_into(dest: &Path) {
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("mkdir");
        for e in fs::read_dir(from).expect("read_dir").flatten() {
            let (s, d) = (e.path(), to.join(e.file_name()));
            if s.is_dir() {
                copy(&s, &d);
            } else {
                fs::copy(&s, &d).expect("copy");
            }
        }
    }
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
    copy(&src, &dest.join("config"));
}

#[test]
fn a_saved_plan_expands_home_in_directory_entries_too() {
    let home = scratch("saved-template-dir");
    let file = home.join("plan.out");
    let f = file.display().to_string();

    run(&home, &["plan", "--out", &f]);
    let (out, ok) = run(&home, &["apply", &f]);
    assert!(ok, "{out}");

    let script = fs::read_to_string(home.join(".local/bin/start-tts")).expect("script");
    assert!(
        !script.contains("${HOME}"),
        "a saved plan must expand the placeholder, not write it literally:\n{script}"
    );
    assert!(
        script.contains(&home.display().to_string()),
        "it should point at this machine's home:\n{script}"
    );
}

/// Applying a plan must leave nothing to do. This is what "converged" means,
/// and it is the property the templating bug broke silently.
#[test]
fn a_saved_plan_converges() {
    let home = scratch("saved-converge");
    let file = home.join("plan.out");
    let f = file.display().to_string();

    run(&home, &["plan", "--out", &f]);
    run(&home, &["apply", &f]);

    let (out, ok) = run(&home, &["plan"]);
    assert!(ok, "{out}");
    assert!(
        out.contains("No changes"),
        "after applying a saved plan nothing should remain:\n{out}"
    );
}

/// A script that lost its executable bit has identical content. Nothing else
/// would ever repair it: apply only visits actions, and actions only existed
/// when content differed.
#[cfg(unix)]
#[test]
fn losing_the_executable_bit_is_drift() {
    use std::os::unix::fs::PermissionsExt;
    let home = scratch("mode-drift");
    run(&home, &["apply", "--auto-approve"]);

    let script = home.join(".local/bin/start-tts");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o644)).expect("strip +x");

    let (out, _) = run(&home, &["plan"]);
    assert!(out.contains("1 to change"), "a lost mode is drift:\n{out}");

    run(&home, &["apply", "--auto-approve"]);
    let mode = fs::metadata(&script).expect("stat").permissions().mode();
    assert!(
        mode & 0o111 != 0,
        "apply must restore the executable bit, got {mode:o}"
    );
}
