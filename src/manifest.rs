//! What is captured, and where it lives on this machine.
//!
//! Every live path is derived from the environment. Nothing is written down,
//! per maestro-core ADR-0001.

use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    /// Compared and copied by the contents this repository already carries.
    Dir,
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Path inside this repository, relative to its root.
    pub repo: &'static str,
    /// Absolute path on this machine.
    pub live: PathBuf,
    pub kind: Kind,
    /// Stored with the home directory replaced by `${HOME}`, because the
    /// value is machine-specific. Expanded on restore, collapsed on sync.
    pub templated: bool,
    /// Restored with the executable bit set.
    pub executable: bool,
}

impl Entry {
    fn file(repo: &'static str, live: PathBuf) -> Self {
        Self {
            repo,
            live,
            kind: Kind::File,
            templated: false,
            executable: false,
        }
    }

    fn dir(repo: &'static str, live: PathBuf) -> Self {
        Self {
            repo,
            live,
            kind: Kind::Dir,
            templated: false,
            executable: false,
        }
    }

    fn templated(mut self) -> Self {
        self.templated = true;
        self
    }

    fn executable(mut self) -> Self {
        self.executable = true;
        self
    }
}

/// The user's home directory, on any platform. `HOME` everywhere Unix-like,
/// `USERPROFILE` on Windows.
pub fn home() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Pi keeps its agent directory under the user's home. An explicit override
/// wins, so a machine with a relocated agent directory can still sync.
pub fn pi_agent_dir(home: &Path) -> PathBuf {
    env::var_os("PI_AGENT_DIR").map_or_else(|| home.join(".pi").join("agent"), PathBuf::from)
}

/// The tool-agnostic MCP config directory, following the adapter's order.
pub fn mcp_config_dir(home: &Path) -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map_or_else(|| home.join(".config"), PathBuf::from)
        .join("mcp")
}

/// A directory on `PATH` that the user owns.
pub fn user_bin_dir(home: &Path) -> PathBuf {
    env::var_os("PI_CONFIG_BIN_DIR").map_or_else(|| home.join(".local").join("bin"), PathBuf::from)
}

pub fn manifest(home: &Path) -> Vec<Entry> {
    let pi = pi_agent_dir(home);
    vec![
        Entry::file("config/pi/settings.json", pi.join("settings.json")),
        Entry::file(
            "config/pi/claude-bridge.json",
            pi.join("claude-bridge.json"),
        ),
        Entry::file("config/pi/models-store.json", pi.join("models-store.json")),
        Entry::dir("config/pi/skills", pi.join("skills")),
        Entry::file("config/mcp/mcp.json", mcp_config_dir(home).join("mcp.json")),
        Entry::file(
            "config/tools/mempalace/config.template.json",
            home.join(".mempalace").join("config.json"),
        )
        .templated(),
        Entry::file(
            "config/tools/codegraphcontext/config.yaml",
            home.join(".codegraphcontext").join("config.yaml"),
        ),
        Entry::file(
            "config/tools/codegraphcontext/env.template",
            home.join(".codegraphcontext").join(".env"),
        )
        .templated(),
        Entry::dir("config/bin", user_bin_dir(home)).executable(),
    ]
}

/// Collapse this machine's home directory to a placeholder.
pub fn to_template(text: &str, home: &str) -> String {
    text.replace(home, "${HOME}")
}

/// Expand the placeholder to this machine's home directory.
pub fn from_template(text: &str, home: &str) -> String {
    text.replace("${HOME}", home)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_derived_from_the_environment() {
        let home = Path::new("/somewhere");
        assert_eq!(pi_agent_dir(home), Path::new("/somewhere/.pi/agent"));
        assert_eq!(mcp_config_dir(home), Path::new("/somewhere/.config/mcp"));
        assert_eq!(user_bin_dir(home), Path::new("/somewhere/.local/bin"));
    }

    #[test]
    fn a_template_round_trips_through_the_home_directory() {
        let live = "/home/someone/.mempalace/palace";
        let stored = to_template(live, "/home/someone");
        assert_eq!(stored, "${HOME}/.mempalace/palace");
        assert_eq!(from_template(&stored, "/home/someone"), live);
    }

    #[test]
    fn every_entry_is_anchored_under_the_given_home() {
        let home = Path::new("/anchor");
        for entry in manifest(home) {
            assert!(
                entry.live.starts_with(home),
                "{} escaped the home directory: {}",
                entry.repo,
                entry.live.display()
            );
        }
    }
}
