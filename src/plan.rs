//! What a restore would change, decided before anything is written.
//!
//! Modelled on `terraform plan` / `terraform apply`: a plan reports only real
//! changes, and a saved plan is carried out exactly as reviewed.
//!
//! A saved plan records what each target held when the plan was made. Applying
//! it re-reads both the repository and the machine and refuses if either moved,
//! so an apply can never quietly do something other than what was approved.
//!
//! This module decides. `file` persists a decision, `apply` carries one out --
//! and `apply` is the only code here that writes to the user's machine.

mod apply;
mod file;

pub use apply::{apply, apply_saved};
pub use file::{load, save};

use std::fs;
use std::path::{Path, PathBuf};

use crate::manifest::{Entry, Kind, from_template};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// The machine does not have this file.
    Create,
    /// The machine has it, with different content.
    Update,
}

impl Change {
    pub fn symbol(self) -> char {
        match self {
            Self::Create => '+',
            Self::Update => '~',
        }
    }
}

#[derive(Debug, Clone)]
pub struct Action {
    pub change: Change,
    pub target: PathBuf,
    pub content: String,
    pub executable: bool,
    /// What the target held when this was planned. `None` means it did not
    /// exist. Applying checks this before writing.
    pub observed: Option<u64>,
    pub templated: bool,
}

/// FNV-1a. Not for security -- only to notice that bytes moved. Written out
/// rather than taken from `DefaultHasher`, whose output Rust does not promise
/// to keep stable, and a plan file has to outlive the process that wrote it.
pub fn digest(bytes: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

#[derive(Debug, Default)]
pub struct Plan {
    pub actions: Vec<Action>,
    /// Where each action's content came from, positionally aligned with
    /// `actions`. Only needed when the plan is saved.
    pub sources: Vec<(PathBuf, u64)>,
    /// Targets already matching the repository. Counted, never listed: a plan
    /// that prints what it will not do buries what it will.
    pub unchanged: usize,
}

impl Plan {
    fn count(&self, change: Change) -> usize {
        self.actions.iter().filter(|a| a.change == change).count()
    }

    pub fn creates(&self) -> usize {
        self.count(Change::Create)
    }

    pub fn updates(&self) -> usize {
        self.count(Change::Update)
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// One file a restore might touch. A struct rather than six positional
/// arguments, two of which are adjacent `PathBuf`s -- transposing the target
/// and its source would type-check and write the wrong file.
struct Candidate {
    target: PathBuf,
    wanted: String,
    executable: bool,
    source: PathBuf,
    templated: bool,
}

fn consider(plan: &mut Plan, candidate: Candidate) {
    let Candidate {
        target,
        wanted,
        executable,
        source,
        templated,
    } = candidate;
    let (change, observed) = match fs::read_to_string(&target) {
        // Identical content is not enough: a script that lost its executable
        // bit has the same bytes and is still broken. Nothing else would ever
        // repair it, because apply only visits actions.
        Ok(current) if current == wanted && mode_is_correct(&target, executable) => {
            plan.unchanged += 1;
            return;
        }
        Ok(current) => (Change::Update, Some(digest(&current))),
        Err(_) => (Change::Create, None),
    };
    let source_digest = digest(&fs::read_to_string(&source).unwrap_or_default());
    plan.sources.push((source, source_digest));
    plan.actions.push(Action {
        change,
        target,
        content: wanted,
        executable,
        observed,
        templated,
    });
}

/// Decide what a restore would change. Reads only.
pub fn plan(entries: &[Entry], root: &Path, home: &str) -> Plan {
    let mut plan = Plan::default();
    for entry in entries {
        let repo_path = root.join(entry.repo);
        if !repo_path.exists() {
            continue;
        }
        let expand = |text: String| {
            if entry.templated {
                from_template(&text, home)
            } else {
                text
            }
        };
        match entry.kind {
            Kind::File => {
                if let Ok(stored) = fs::read_to_string(&repo_path) {
                    consider(
                        &mut plan,
                        Candidate {
                            target: entry.live.clone(),
                            wanted: expand(stored),
                            executable: entry.executable,
                            source: repo_path.clone(),
                            templated: entry.templated,
                        },
                    );
                }
            }
            Kind::Dir => {
                for rel in files_under(&repo_path) {
                    if let Ok(stored) = fs::read_to_string(repo_path.join(&rel)) {
                        consider(
                            &mut plan,
                            Candidate {
                                target: entry.live.join(&rel),
                                wanted: expand(stored),
                                executable: entry.executable,
                                source: repo_path.join(&rel),
                                templated: entry.templated,
                            },
                        );
                    }
                }
            }
        }
    }
    plan
}

#[cfg(unix)]
fn mode_is_correct(path: &Path, executable: bool) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if !executable {
        return true;
    }
    fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn mode_is_correct(_path: &Path, _executable: bool) -> bool {
    true
}
