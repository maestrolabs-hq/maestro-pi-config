//! One function per verb. Each takes what it needs, prints its own output
//! and returns the exit code; none of them parse arguments.

mod destroy;
pub use destroy::run_destroy;

use std::path::Path;
use std::process::ExitCode;

use crate::config::{State, status, sync};
use crate::manifest::Entry;
use crate::{plan, provision};

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

pub fn run_status(entries: &[Entry], root: &Path, home: &str) -> ExitCode {
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

pub fn run_sync(entries: &[Entry], root: &Path, home: &str) -> ExitCode {
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

pub fn run_plan(entries: &[Entry], root: &Path, home: &str, args: &[String]) -> ExitCode {
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
pub fn run_apply_saved(file: &str, home: &str) -> ExitCode {
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

/// Nothing is written or removed without an explicit flag.
///
/// One function rather than one per verb, because the rule is the same and the
/// duplication gate was right to object: an apply that demands approval beside
/// a destroy that forgot to would be a surprise in the more dangerous
/// direction.
fn approved(args: &[String], count: usize, act: &str, recipe: &str) -> bool {
    if args.iter().any(|a| a == "--auto-approve") {
        return true;
    }
    println!("\nRefusing to {act} {count} file(s) unprompted.");
    println!("Review the list above, then: just {recipe} --auto-approve");
    false
}

pub fn run_apply(entries: &[Entry], root: &Path, home: &str, args: &[String]) -> ExitCode {
    if let Some(file) = args.iter().skip(1).find(|a| !a.starts_with("--")) {
        return run_apply_saved(file, home);
    }

    let p = plan::plan(entries, root, home);
    render(&p);
    if p.is_empty() {
        return ExitCode::SUCCESS;
    }
    if !approved(args, p.actions.len(), "change", "apply") {
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

pub fn run_provision(root: &Path, args: &[String]) -> ExitCode {
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
