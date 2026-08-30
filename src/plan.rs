//! What a restore would change, decided before anything is written.
//!
//! Modelled on `terraform plan` / `terraform apply`: a plan reports only real
//! changes, and an apply prints the same plan before acting.
//!
//! Terraform can save a plan so an apply carries out exactly what was
//! reviewed. That is not done here: an apply re-plans and prints it, and the
//! only gap it leaves is the seconds between reading the output and confirming.
//! A plan file would add a format, a staleness rule and an artifact to ignore,
//! to close a window that a local config restore does not really have.

use std::fs;
use std::io;
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
}

#[derive(Debug, Default)]
pub struct Plan {
    pub actions: Vec<Action>,
    /// Targets already matching the repository. Counted, never listed: a plan
    /// that prints what it will not do buries what it will.
    pub unchanged: usize,
}

impl Plan {
    pub fn creates(&self) -> usize {
        self.actions
            .iter()
            .filter(|a| a.change == Change::Create)
            .count()
    }

    pub fn updates(&self) -> usize {
        self.actions
            .iter()
            .filter(|a| a.change == Change::Update)
            .count()
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

fn consider(plan: &mut Plan, target: PathBuf, wanted: String, executable: bool) {
    match fs::read_to_string(&target) {
        Ok(current) if current == wanted => plan.unchanged += 1,
        Ok(_) => plan.actions.push(Action {
            change: Change::Update,
            target,
            content: wanted,
            executable,
        }),
        Err(_) => plan.actions.push(Action {
            change: Change::Create,
            target,
            content: wanted,
            executable,
        }),
    }
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
                        entry.live.clone(),
                        expand(stored),
                        entry.executable,
                    );
                }
            }
            Kind::Dir => {
                for rel in files_under(&repo_path) {
                    if let Ok(stored) = fs::read_to_string(repo_path.join(&rel)) {
                        consider(
                            &mut plan,
                            entry.live.join(&rel),
                            expand(stored),
                            entry.executable,
                        );
                    }
                }
            }
        }
    }
    plan
}

/// Carry out a plan. Every action was decided before this ran.
pub fn apply(plan: &Plan) -> io::Result<()> {
    for action in &plan.actions {
        if let Some(parent) = action.target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&action.target, &action.content)?;
        set_executable(&action.target, action.executable)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if executable {
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _executable: bool) -> io::Result<()> {
    Ok(())
}
