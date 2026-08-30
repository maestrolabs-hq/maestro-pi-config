//! `pi-config` — inspect, capture and restore this machine's pi configuration.
//!
//! Deliberately outside pi: restoring configuration has to work when pi is
//! broken or not yet installed.

mod config;
mod manifest;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use config::{State, restore, status, sync};
use manifest::{home, manifest};

/// The repository root: the directory holding this crate's `Cargo.toml`,
/// resolved at build time so the binary can run from anywhere.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn usage() -> ExitCode {
    eprintln!("usage: pi-config <status|sync|restore [--apply]>");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let Some(home_dir) = home() else {
        eprintln!("pi-config: no home directory in the environment (HOME or USERPROFILE)");
        return ExitCode::FAILURE;
    };
    let home_str = home_dir.display().to_string();
    let root = repo_root();
    let entries = manifest(&home_dir);

    let args: Vec<String> = env::args().skip(1).collect();
    let Some(verb) = args.first() else {
        return usage();
    };

    match verb.as_str() {
        "status" => {
            let reports = status(&entries, &root, &home_str);
            let mut drifted = 0;
            for r in &reports {
                println!("{} {}", r.state.label(), r.repo);
                for d in &r.detail {
                    println!("          {d}");
                }
                if r.state == State::Differs {
                    drifted += 1;
                }
            }
            if drifted == 0 {
                println!("\nIn sync with this machine.");
                ExitCode::SUCCESS
            } else {
                let plural = if drifted == 1 {
                    "entry has"
                } else {
                    "entries have"
                };
                println!(
                    "\n{drifted} {plural} drifted. `just sync` pulls the machine's version in."
                );
                ExitCode::FAILURE
            }
        }
        "sync" => match sync(&entries, &root, &home_str) {
            Ok(written) => {
                for w in &written {
                    println!("  wrote {w}");
                }
                println!(
                    "\n{} file(s) pulled in. Review with `git diff`.",
                    written.len()
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("pi-config: sync failed: {e}");
                ExitCode::FAILURE
            }
        },
        "restore" => {
            let apply = args.iter().any(|a| a == "--apply");
            match restore(&entries, &root, &home_str, apply) {
                Ok(touched) => {
                    let verb = if apply { "wrote" } else { "would write" };
                    for t in &touched {
                        println!("  {verb} {t}");
                    }
                    if apply {
                        println!("\n{} file(s) restored.", touched.len());
                    } else {
                        println!(
                            "\n{} file(s) would be written. Nothing changed. Re-run with --apply.",
                            touched.len()
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("pi-config: restore failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => usage(),
    }
}
