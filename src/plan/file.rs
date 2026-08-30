//! The saved-plan file format.
//!
//! One action per line, tab-separated, so it can be read without a parser
//! dependency and eyeballed without a tool.

use std::fmt::Write;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{Change, Plan};

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
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
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
        );
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
