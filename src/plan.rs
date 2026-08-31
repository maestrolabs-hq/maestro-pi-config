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
pub mod destroy;
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
/// Every target the manifest resolves to on this machine, with the content it
/// should hold.
///
/// Extracted because `plan` and `destroy` ask different questions of the same
/// walk: one compares, the other removes. Two copies of this loop would drift
/// the first time an entry kind was added to only one of them.
fn candidates(entries: &[Entry], root: &Path, home: &str) -> Vec<Candidate> {
    let mut out = Vec::new();
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
        let mut push = |target: PathBuf, source: PathBuf| {
            if let Ok(stored) = fs::read_to_string(&source) {
                out.push(Candidate {
                    target,
                    wanted: expand(stored),
                    executable: entry.executable,
                    source,
                    templated: entry.templated,
                });
            }
        };
        match entry.kind {
            Kind::File => push(entry.live.clone(), repo_path.clone()),
            Kind::Dir => {
                for rel in files_under(&repo_path) {
                    push(entry.live.join(&rel), repo_path.join(&rel));
                }
            }
        }
    }
    out
}

pub fn plan(entries: &[Entry], root: &Path, home: &str) -> Plan {
    let mut plan = Plan::default();
    for candidate in candidates(entries, root, home) {
        consider(&mut plan, candidate);
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A scratch directory that cleans up after itself.
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pi-config-plan-{name}"));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).expect("mkdir");
        d
    }

    /// The executable bit has no entry in the manifest today. It is still
    /// covered here, because the machinery is what makes a captured `bin/`
    /// directory restorable, and a script that arrives without +x fails in a
    /// way that looks like a missing file.
    ///
    /// Tested against a fixture this function creates rather than against
    /// whatever happens to be in `config/`, so deleting a real script cannot
    /// silently delete the coverage with it -- which is exactly what happened
    /// to the two integration tests this replaces.
    #[test]
    fn a_file_that_lost_its_executable_bit_is_drift() {
        let dir = scratch("mode");
        let target = dir.join("script");
        fs::write(&target, "#!/bin/sh\ntrue\n").expect("write");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).expect("chmod");

        assert!(
            mode_is_correct(&target, true),
            "0755 should satisfy an executable entry"
        );

        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).expect("chmod");
        assert!(
            !mode_is_correct(&target, true),
            "0644 must not satisfy an executable entry -- the content is \
             identical, so nothing else would ever notice"
        );
        assert!(
            mode_is_correct(&target, false),
            "a file that was never meant to be executable is not drift"
        );
    }

    /// Identical content plus a wrong mode still has to produce an action,
    /// because apply only ever visits actions.
    #[test]
    fn identical_content_with_the_wrong_mode_still_plans_a_change() {
        let dir = scratch("mode-plan");
        let target = dir.join("script");
        let body = "#!/bin/sh\ntrue\n";
        fs::write(&target, body).expect("write");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).expect("chmod");

        let source = dir.join("source");
        fs::write(&source, body).expect("write");

        let mut plan = Plan::default();
        consider(
            &mut plan,
            Candidate {
                target: target.clone(),
                wanted: body.to_owned(),
                executable: true,
                source,
                templated: false,
            },
        );

        assert_eq!(plan.unchanged, 0, "a wrong mode is not 'unchanged'");
        assert_eq!(plan.actions.len(), 1, "it should plan exactly one change");
        assert_eq!(plan.actions[0].change, Change::Update);
    }
}
