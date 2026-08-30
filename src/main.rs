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

fn run_status(entries: &[manifest::Entry], root: &std::path::Path, home: &str) -> ExitCode {
    let reports = status(entries, root, home);
    let mut drifted = 0;
    for r in &reports {
        println!("{} {}", r.state.label(), r.repo);
        for d in &r.detail {
            println!("          {d}");
        }
        // Absent counts. A machine holding none of the files is not "in sync"
        // with a repository that holds all of them.
        if matches!(r.state, State::Differs | State::AbsentLive) {
            drifted += 1;
        }
    }
    if drifted == 0 {
        println!("\nIn sync with this machine.");
        return ExitCode::SUCCESS;
    }
    let plural = if drifted == 1 {
        "entry is"
    } else {
        "entries are"
    };
    println!("\n{drifted} {plural} out of step. `just plan` shows what a restore would change.");
    ExitCode::FAILURE
}

fn run_sync(entries: &[manifest::Entry], root: &std::path::Path, home: &str) -> ExitCode {
    match sync(entries, root, home) {
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
    }
}

fn run_plan(
    entries: &[manifest::Entry],
    root: &std::path::Path,
    home: &str,
    args: &[String],
) -> ExitCode {
    let p = plan::plan(entries, root, home);
    render(&p);

    let Some(i) = args.iter().position(|a| a == "--out") else {
        if !p.is_empty() {
            println!("\nRun `just apply` to carry this out.");
        }
        return ExitCode::SUCCESS;
    };
    let Some(out) = args.get(i + 1) else {
        eprintln!("pi-config: --out needs a path");
        return ExitCode::FAILURE;
    };
    if let Err(e) = plan::save(&p, &p.sources, std::path::Path::new(out)) {
        eprintln!("pi-config: cannot write the plan: {e}");
        return ExitCode::FAILURE;
    }
    if p.is_empty() {
        println!("\nSaved an empty plan to {out}.");
    } else {
        println!("\nSaved to {out}. Carry it out with: pi-config apply {out}");
    }
    ExitCode::SUCCESS
}

/// A saved plan was reviewed when it was written, so it needs no second
/// approval -- only proof that nothing moved since.
fn run_apply_saved(file: &str, home: &str) -> ExitCode {
    let saved = match plan::load(std::path::Path::new(file)) {
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
    match plan::apply_saved(&saved, home) {
        Ok(n) => {
            println!("Apply complete. {n} file(s) written from {file}.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("pi-config: {e}");
            eprintln!("Re-plan and review again.");
            ExitCode::FAILURE
        }
    }
}

fn run_apply(
    entries: &[manifest::Entry],
    root: &std::path::Path,
    home: &str,
    args: &[String],
) -> ExitCode {
    if let Some(file) = args.iter().skip(1).find(|a| !a.starts_with("--")) {
        return run_apply_saved(file, home);
    }

    let p = plan::plan(entries, root, home);
    render(&p);
    if p.is_empty() {
        return ExitCode::SUCCESS;
    }
    if !args.iter().any(|a| a == "--auto-approve") {
        println!(
            "\nRefusing to change {} file(s) unprompted.",
            p.actions.len()
        );
        println!("Review the plan above, then: just apply --auto-approve");
        return ExitCode::FAILURE;
    }
    match plan::apply(&p) {
        Ok(()) => {
            println!(
                "\nApply complete. {} added, {} changed.",
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

fn run_provision(root: &std::path::Path, args: &[String]) -> ExitCode {
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
        "status" => run_status(&entries, &root, &home_str),
        "sync" => run_sync(&entries, &root, &home_str),
        "plan" => run_plan(&entries, &root, &home_str, &args),
        "apply" => run_apply(&entries, &root, &home_str, &args),
        "provision" => run_provision(&root, &args),
        _ => usage(),
    }
}
