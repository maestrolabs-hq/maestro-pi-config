//! The `destroy` verb: removing what `apply` wrote.
//!
//! Separate from the other verbs because it is the only one that deletes, and
//! because `cmd.rs` had grown past the size a module is allowed to reach.

use super::approved;
use crate::manifest::Entry;
use crate::plan;
use std::path::Path;
use std::process::ExitCode;

/// Remove what `apply` wrote.
///
/// The refusal is the interesting part: a target that was edited after it was
/// written holds work this tool did not put there, so it is reported and kept.
/// That makes destroy safe to run without first checking what you changed.
pub fn run_destroy(entries: &[Entry], root: &Path, home: &str, args: &[String]) -> ExitCode {
    let d = plan::destroy::plan(entries, root, home);

    if d.is_empty() {
        println!("Nothing to remove. This machine holds no file this tool wrote.");
        return ExitCode::SUCCESS;
    }

    if !d.remove.is_empty() {
        println!("pi-config will remove the following files:\n");
        for target in &d.remove {
            println!("  - {}", target.display());
        }
    }
    if !d.kept.is_empty() {
        println!("\nAnd will keep these, because they no longer match the repository:\n");
        for target in &d.kept {
            println!("  ! {}", target.display());
        }
        println!("\nSomething edited them after they were written, and that edit");
        println!("exists nowhere else. Remove them by hand if you meant to.");
    }
    println!(
        "\nDestroy: {} to remove, {} kept.",
        d.remove.len(),
        d.kept.len()
    );

    if d.remove.is_empty() {
        return ExitCode::SUCCESS;
    }
    if !approved(args, d.remove.len(), "remove", "destroy") {
        return ExitCode::FAILURE;
    }
    match plan::destroy::run(&d) {
        Ok(n) => {
            println!("\nDestroy complete. {n} file(s) removed.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("pi-config: destroy failed: {e}");
            eprintln!("Some files were already removed. Run destroy again, or apply to restore.");
            ExitCode::FAILURE
        }
    }
}
