//! `pi-config` — inspect, capture and restore this machine's pi configuration.
//!
//! Deliberately outside pi: restoring configuration has to work when pi is
//! broken or not yet installed.

mod config;
mod manifest;
mod plan;
mod provision;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use config::{State, status, sync};
use manifest::{home, manifest};

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

/// Only what changes. A plan that lists what it will not do buries what it will.
fn render(p: &plan::Plan) {
    if p.is_empty() {
        println!("No changes. This machine matches the repository.");
        if p.unchanged > 0 {
            println!("({} file(s) already in place.)", p.unchanged);
        }
        return;
    }
    println!("pi-config will perform the following actions:\n");
    for a in &p.actions {
        println!("  {} {}", a.change.symbol(), a.target.display());
    }
    println!(
        "\nPlan: {} to add, {} to change, {} unchanged.",
        p.creates(),
        p.updates(),
        p.unchanged
    );
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
                // Absent counts. A machine holding none of the files is not
                // "in sync" with a repository that holds all of them.
                if matches!(r.state, State::Differs | State::AbsentLive) {
                    drifted += 1;
                }
            }
            if drifted == 0 {
                println!("\nIn sync with this machine.");
                ExitCode::SUCCESS
            } else {
                let plural = if drifted == 1 {
                    "entry is"
                } else {
                    "entries are"
                };
                println!(
                    "\n{drifted} {plural} out of step. `just plan` shows what a restore would change."
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
        "plan" => {
            let p = plan::plan(&entries, &root, &home_str);
            render(&p);
            if let Some(i) = args.iter().position(|a| a == "--out") {
                let Some(out) = args.get(i + 1) else {
                    eprintln!("pi-config: --out needs a path");
                    return ExitCode::FAILURE;
                };
                if let Err(e) = plan::save(&p, &p.sources, std::path::Path::new(out)) {
                    eprintln!("pi-config: cannot write the plan: {e}");
                    return ExitCode::FAILURE;
                }
                if p.is_empty() {
                    println!(
                        "
Saved an empty plan to {out}."
                    );
                } else {
                    println!(
                        "
Saved to {out}. Carry it out with: pi-config apply {out}"
                    );
                }
            } else if !p.is_empty() {
                println!(
                    "
Run `just apply` to carry this out."
                );
            }
            ExitCode::SUCCESS
        }
        "apply" => {
            // A path argument means a saved plan: it was reviewed when it was
            // written, so it needs no second approval -- only proof that
            // nothing moved since.
            if let Some(file) = args.iter().skip(1).find(|a| !a.starts_with("--")) {
                let path = std::path::Path::new(file);
                let saved = match plan::load(path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("pi-config: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                println!("Carrying out the saved plan:\n");
                for a in &saved {
                    println!("  {} {}", a.change.symbol(), a.target.display());
                }
                println!();
                return match plan::apply_saved(&saved, &home_str) {
                    Ok(n) => {
                        println!("Apply complete. {n} file(s) written from {file}.");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("pi-config: {e}");
                        eprintln!("Re-plan and review again.");
                        ExitCode::FAILURE
                    }
                };
            }
            let p = plan::plan(&entries, &root, &home_str);
            render(&p);
            if p.is_empty() {
                return ExitCode::SUCCESS;
            }
            if !args.iter().any(|a| a == "--auto-approve") {
                println!(
                    "
Refusing to change {} file(s) unprompted.",
                    p.actions.len()
                );
                println!("Review the plan above, then: just apply --auto-approve");
                return ExitCode::FAILURE;
            }
            match plan::apply(&p) {
                Ok(()) => {
                    println!(
                        "
Apply complete. {} added, {} changed.",
                        p.creates(),
                        p.updates()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("pi-config: apply failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "provision" => {
            let apply = args.iter().any(|a| a == "--apply" || a == "--auto-approve");
            let text = match std::fs::read_to_string(root.join("config/provision.txt")) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("pi-config: cannot read the provisioning manifest: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let steps = match provision::parse(&text) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("pi-config: {e}");
                    return ExitCode::FAILURE;
                }
            };
            for s in &steps {
                if apply {
                    println!("  $ {}", s.rendered());
                    if let Err(e) = provision::run(s) {
                        eprintln!("pi-config: {e}");
                        return ExitCode::FAILURE;
                    }
                } else {
                    println!("  would run  {}", s.rendered());
                }
            }
            let manual = provision::manual(&text);
            if !manual.is_empty() {
                println!("\nFetch by hand, then put on PATH:");
                for m in &manual {
                    println!("  {m}");
                }
            }
            if apply {
                println!(
                    "\n{} step(s) ran. Next: pi-config restore --apply",
                    steps.len()
                );
            } else {
                println!(
                    "\n{} step(s) would run. Nothing changed. Re-run with --apply.",
                    steps.len()
                );
            }
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}
