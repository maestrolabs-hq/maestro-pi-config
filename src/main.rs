//! `pi-config` — inspect, capture and restore this machine's pi configuration.
//!
//! Deliberately outside pi: restoring configuration has to work when pi is
//! broken or not yet installed.

mod cmd;
mod config;
mod manifest;
mod plan;
mod provision;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use manifest::{Layout, home, manifest};

/// The repository root: the directory holding this crate's `Cargo.toml`,
/// resolved at build time so the binary can run from anywhere.
///
/// `PI_CONFIG_REPO` overrides it. That exists so a test can point the tool at a
/// scratch copy and edit a source for real, rather than asserting against a
/// hand-written digest -- a test that computes the expected hash itself passes
/// even when the implementation's hash is stubbed to a constant.
fn repo_root() -> PathBuf {
    env::var_os("PI_CONFIG_REPO")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: pi-config <status|sync|plan [--out FILE]|apply [FILE|--auto-approve]|provision [--apply]>"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let Some(home_dir) = home() else {
        eprintln!("pi-config: no home directory in the environment (HOME or USERPROFILE)");
        return ExitCode::FAILURE;
    };
    let home_str = home_dir.display().to_string();
    let root = repo_root();
    let entries = manifest(&Layout::from_env(&home_dir));

    let args: Vec<String> = env::args().skip(1).collect();
    let Some(verb) = args.first() else {
        return usage();
    };

    match verb.as_str() {
        "status" => cmd::run_status(&entries, &root, &home_str),
        "sync" => cmd::run_sync(&entries, &root, &home_str),
        "plan" => cmd::run_plan(&entries, &root, &home_str, &args),
        "apply" => cmd::run_apply(&entries, &root, &home_str, &args),
        "provision" => cmd::run_provision(&root, &args),
        _ => usage(),
    }
}
