//! What a restore would change, decided before anything is written.
//!
//! Modelled on `terraform plan` / `terraform apply`: a plan reports only real
//! changes, and a saved plan is carried out exactly as reviewed.
//!
//! A saved plan records what each target held when the plan was made. Applying
//! it re-reads both the repository and the machine and refuses if either moved,
//! so an apply can never quietly do something other than what was approved.

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

fn consider(
    plan: &mut Plan,
    target: PathBuf,
    wanted: String,
    executable: bool,
    source: PathBuf,
    templated: bool,
) {
    let (change, observed) = match fs::read_to_string(&target) {
        Ok(current) if current == wanted => {
            plan.unchanged += 1;
            return;
        }
        Ok(current) => (Change::Update, Some(digest(&current))),
        Err(_) => (Change::Create, None),
    };
    plan.sources.push((
        source.clone(),
        digest(&fs::read_to_string(&source).unwrap_or_default()),
    ));
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
                        entry.live.clone(),
                        expand(stored),
                        entry.executable,
                        repo_path.clone(),
                        entry.templated,
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
                            repo_path.join(&rel),
                            entry.templated,
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

// ---------------------------------------------------------------- persistence

const HEADER: &str = "pi-config plan v1";

/// A saved plan. One action per line, tab-separated, so it can be read without
/// a parser dependency and eyeballed without a tool.
///
/// Content is not stored: the plan names its source in the repository and the
/// digest that source had. Applying re-reads it and checks. That keeps the file
/// small and makes a repository edit after planning a refusal rather than a
/// surprise.
pub fn save(plan: &Plan, sources: &[(PathBuf, u64)], path: &Path) -> io::Result<()> {
    let mut out = String::from(HEADER);
    out.push('\n');
    for (action, (source, source_digest)) in plan.actions.iter().zip(sources) {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            match action.change {
                Change::Create => "create",
                Change::Update => "update",
            },
            action.target.display(),
            source.display(),
            source_digest,
            action
                .observed
                .map_or_else(|| "absent".to_owned(), |d| d.to_string()),
            action.executable,
            action.templated,
        ));
    }
    fs::write(path, out)
}

#[derive(Debug)]
pub struct SavedAction {
    pub change: Change,
    pub target: PathBuf,
    pub source: PathBuf,
    pub source_digest: u64,
    pub observed: Option<u64>,
    pub executable: bool,
    /// Carried in the plan rather than recomputed from the manifest: a manifest
    /// edited between plan and apply must not change what the plan does.
    pub templated: bool,
}

pub fn load(path: &Path) -> Result<Vec<SavedAction>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Err(format!("{} is not a pi-config plan", path.display()));
    }
    lines
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            let [
                change,
                target,
                source,
                source_digest,
                observed,
                executable,
                templated,
            ] = f[..]
            else {
                return Err(format!("malformed plan line: {line}"));
            };
            Ok(SavedAction {
                change: if change == "create" {
                    Change::Create
                } else {
                    Change::Update
                },
                target: PathBuf::from(target),
                source: PathBuf::from(source),
                source_digest: source_digest.parse().map_err(|_| "bad digest".to_owned())?,
                observed: (observed != "absent")
                    .then(|| observed.parse().map_err(|_| "bad digest".to_owned()))
                    .transpose()?,
                executable: executable == "true",
                templated: templated == "true",
            })
        })
        .collect()
}

/// Apply a saved plan, refusing if either side moved since it was made.
///
/// This is the whole reason to persist a plan: without these checks an apply
/// is just a re-plan wearing a reviewed plan's name.
pub fn apply_saved(actions: &[SavedAction], home: &str) -> Result<usize, String> {
    // Verify everything first, keeping the bytes that were checked. Reading a
    // second time in the write loop would mean writing content whose digest was
    // never verified -- which is the guarantee a saved plan exists to give.
    let mut verified: Vec<(&SavedAction, String)> = Vec::with_capacity(actions.len());
    for a in actions {
        let stored = fs::read_to_string(&a.source)
            .map_err(|e| format!("plan source {} is gone: {e}", a.source.display()))?;
        if digest(&stored) != a.source_digest {
            return Err(format!(
                "stale plan: {} changed in the repository since planning",
                a.source.display()
            ));
        }
        let now = fs::read_to_string(&a.target).ok().map(|c| digest(&c));
        if now != a.observed {
            return Err(format!(
                "stale plan: {} changed on this machine since planning",
                a.target.display()
            ));
        }
        let content = if a.templated {
            from_template(&stored, home)
        } else {
            stored
        };
        verified.push((a, content));
    }

    for (a, content) in &verified {
        if let Some(parent) = a.target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&a.target, content).map_err(|e| e.to_string())?;
        set_executable(&a.target, a.executable).map_err(|e| e.to_string())?;
    }
    Ok(verified.len())
}
