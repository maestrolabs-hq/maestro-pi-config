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
    let home = scratch("stale-repo");
    let file = home.join("plan.out");
    let f = file.display().to_string();
    run(&home, &["plan", "--out", &f]);

    // Rewrite the plan to name a source that no longer matches its digest.
    let text = fs::read_to_string(&file).expect("read plan");
    let doctored: String = text
        .lines()
        .map(|l| {
            let mut f: Vec<&str> = l.split('\t').collect();
            if f.len() == 6 {
                f[3] = "1";
                f.join("\t")
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&file, doctored).expect("write plan");

    let out = Command::new(env!("CARGO_BIN_EXE_pi-config"))
        .args(["apply", &f])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .output()
        .expect("run");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a plan whose source moved must not apply"
    );
    assert!(
        err.contains("changed in the repository"),
        "it should say why:\n{err}"
    );
}
