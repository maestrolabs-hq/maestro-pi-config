//! Plan reports only real changes; apply refuses to act unprompted.
//!
//! Every test drives the binary with a scratch `HOME`, which both keeps them
//! away from the real machine and proves the tool is environment-driven.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pi-config-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn run(home: &PathBuf, args: &[&str]) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_pi-config"))
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("PI_AGENT_DIR")
        .env_remove("PI_CONFIG_BIN_DIR")
        .output()
        .expect("run pi-config");
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
