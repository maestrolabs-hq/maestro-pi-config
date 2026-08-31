//! Removing what `apply` wrote, and refusing to remove what it did not.
//!
//! `apply` converges presence: it writes what is missing and corrects what
//! differs. Nothing ever removed a target, so a file this tool wrote outlived
//! the manifest entry that produced it. Deleting an entry stopped the file
//! being managed; it did not stop the file existing.
//!
//! Terraform destroys freely because it owns its resources. This writes into
//! someone's home directory, where a file can hold work that exists nowhere
//! else. So the rule is narrower than terraform's: remove a target only when
//! it still holds exactly what the repository says it should. A target that
//! was edited after it was written is no longer purely ours, and is kept and
//! reported rather than deleted.

use super::{Candidate, candidates};
use crate::manifest::Entry;
use std::fs;
use std::path::{Path, PathBuf};

/// What a destroy would do, before it does any of it.
#[derive(Debug, Default)]
pub struct Destroy {
    /// Targets holding exactly what the repository says. Safe to remove.
    pub remove: Vec<PathBuf>,
    /// Targets that exist but differ. Kept, because the difference is
    /// someone's work and this tool did not put it there.
    pub kept: Vec<PathBuf>,
}

impl Destroy {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.remove.is_empty() && self.kept.is_empty()
    }
}

/// Classify every managed target as removable, kept, or already gone.
#[must_use]
pub fn plan(entries: &[Entry], root: &Path, home: &str) -> Destroy {
    let mut out = Destroy::default();
    for Candidate { target, wanted, .. } in candidates(entries, root, home) {
        match fs::read_to_string(&target) {
            Ok(current) if current == wanted => out.remove.push(target),
            // Unreadable is not absent. A target we cannot compare is one we
            // have no business deleting.
            Ok(_) | Err(_) if target.exists() => out.kept.push(target),
            _ => {}
        }
    }
    out
}

/// Remove the targets a destroy plan classified as removable.
///
/// Empty parent directories are removed too, but only the ones that become
/// empty as a result. A directory holding anything else was not ours alone.
///
/// # Errors
///
/// Returns the path and the reason on the first removal that fails, having
/// already removed the ones before it. There is no rollback: recreating a file
/// this tool just deleted is what `apply` is for.
pub fn run(destroy: &Destroy) -> Result<usize, String> {
    for target in &destroy.remove {
        fs::remove_file(target).map_err(|e| format!("removing {}: {e}", target.display()))?;
        prune_empty_parents(target);
    }
    Ok(destroy.remove.len())
}

/// Walk up from a removed file, deleting directories that are now empty.
///
/// Failure is the stop condition, not an error: a non-empty directory refuses
/// to be removed, which is exactly the signal to stop climbing.
fn prune_empty_parents(from: &Path) {
    let mut dir = from.parent();
    while let Some(d) = dir {
        if fs::remove_dir(d).is_err() {
            return;
        }
        dir = d.parent();
    }
}
