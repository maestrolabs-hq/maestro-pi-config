//! Comparing, pulling in and writing back captured configuration.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::manifest::{Entry, Kind, to_template};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Same,
    Differs,
    AbsentLive,
    AbsentRepo,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Same => "  ok    ",
            Self::Differs => "  DRIFT ",
            Self::AbsentLive => "  absent",
            Self::AbsentRepo => "  NEW   ",
        }
    }
}

#[derive(Debug)]
pub struct Report {
    pub repo: &'static str,
    pub state: State,
    /// Paths that differ, relative to a directory entry.
    pub detail: Vec<String>,
}

/// Files under `root`, relative to it, sorted. A missing root yields nothing.
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

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// Repository form of a live file.
fn as_stored(text: &str, entry: &Entry, home: &str) -> String {
    if entry.templated {
        to_template(text, home)
    } else {
        text.to_owned()
    }
}

pub fn inspect(entry: &Entry, root: &Path, home: &str) -> Report {
    let repo_path = root.join(entry.repo);
    if !entry.live.exists() {
        return Report {
            repo: entry.repo,
            state: State::AbsentLive,
            detail: Vec::new(),
        };
    }
    if !repo_path.exists() {
        return Report {
            repo: entry.repo,
            state: State::AbsentRepo,
            detail: Vec::new(),
        };
    }

    match entry.kind {
        Kind::File => {
            let same = match (read(&repo_path), read(&entry.live)) {
                (Some(stored), Some(live)) => stored == as_stored(&live, entry, home),
                _ => false,
            };
            Report {
                repo: entry.repo,
                state: if same { State::Same } else { State::Differs },
                detail: if same {
                    Vec::new()
                } else {
                    vec![entry.repo.to_owned()]
                },
            }
        }
        // Only the files this repository already carries are tracked: the live
        // directory may hold unrelated things, and claiming those is not this
        // repository's business.
        Kind::Dir => {
            let differing: Vec<String> = files_under(&repo_path)
                .into_iter()
                .filter(|rel| {
                    let stored = read(&repo_path.join(rel));
                    let live = read(&entry.live.join(rel));
                    match (stored, live) {
                        (Some(s), Some(l)) => s != as_stored(&l, entry, home),
                        _ => true,
                    }
                })
                .map(|rel| rel.display().to_string())
                .collect();
            Report {
                repo: entry.repo,
                state: if differing.is_empty() {
                    State::Same
                } else {
                    State::Differs
                },
                detail: differing,
            }
        }
    }
}

pub fn status(entries: &[Entry], root: &Path, home: &str) -> Vec<Report> {
    entries.iter().map(|e| inspect(e, root, home)).collect()
}

/// Machine to repository. Writes only inside the repository.
pub fn sync(entries: &[Entry], root: &Path, home: &str) -> io::Result<Vec<String>> {
    let mut written = Vec::new();
    for entry in entries {
        if !entry.live.exists() {
            continue;
        }
        let repo_path = root.join(entry.repo);
        match entry.kind {
            Kind::File => {
                let Some(live) = read(&entry.live) else {
                    continue;
                };
                if let Some(parent) = repo_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&repo_path, as_stored(&live, entry, home))?;
                written.push(entry.repo.to_owned());
            }
            Kind::Dir => {
                for rel in files_under(&repo_path) {
                    let Some(live) = read(&entry.live.join(&rel)) else {
                        continue;
                    };
                    fs::write(repo_path.join(&rel), as_stored(&live, entry, home))?;
                    written.push(format!("{}/{}", entry.repo, rel.display()));
                }
            }
        }
    }
    Ok(written)
}
