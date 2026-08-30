//! Restore must not write unless asked, and drift must be exact.
//!
//! These run against a scratch directory, never the machine's real config.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maestro-pi-config-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

/// Runs the binary with a fake home, so nothing outside the scratch dir is read
/// or written. This is the property worth testing: the tool is driven entirely
/// by the environment.
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
fn restore_writes_nothing_without_apply() {
    let home = scratch("dry");
    let target = home.join(".pi/agent/settings.json");

    let (stdout, ok) = run(&home, &["restore"]);
    assert!(ok, "dry run should succeed");
    assert!(
        stdout.contains("would write"),
        "it should say what it would do:\n{stdout}"
    );
    assert!(
        stdout.contains("Nothing changed"),
        "it should say nothing changed:\n{stdout}"
    );
    assert!(
        !target.exists(),
        "dry run must not create {}",
        target.display()
    );
}

#[test]
fn restore_with_apply_writes_the_captured_files() {
    let home = scratch("apply");
    let target = home.join(".pi/agent/settings.json");

    let (_, ok) = run(&home, &["restore", "--apply"]);
    assert!(ok, "apply should succeed");
    assert!(target.exists(), "apply must create {}", target.display());

    let restored = fs::read_to_string(&target).expect("read restored");
    let captured = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/pi/settings.json"),
    )
    .expect("read captured");
    assert_eq!(
        restored, captured,
        "a restore must reproduce the captured bytes exactly"
    );
}

#[test]
fn a_templated_entry_expands_the_home_directory() {
    let home = scratch("template");
    run(&home, &["restore", "--apply"]);

    let restored = fs::read_to_string(home.join(".mempalace/config.json")).expect("mempalace");
    assert!(
        !restored.contains("${HOME}"),
        "the placeholder must be expanded on restore:\n{restored}"
    );
    assert!(
        restored.contains(&home.display().to_string()),
        "it should point at this machine's home:\n{restored}"
    );
}

#[test]
fn status_reports_drift_when_the_machine_has_nothing() {
    let home = scratch("status");
    let (stdout, ok) = run(&home, &["status"]);
    assert!(
        ok,
        "an empty machine is not drift, every entry is simply absent:\n{stdout}"
    );
    assert!(
        stdout.contains("absent"),
        "entries should read as absent:\n{stdout}"
    );
}
