//! Carrying out a plan. The only code in this program that writes to the
//! user's machine.

use std::fs;
use std::io;
use std::path::Path;

use super::file::SavedAction;
use super::{Plan, digest};
use crate::manifest::from_template;

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
