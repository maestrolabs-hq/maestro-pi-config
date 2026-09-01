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
    fn new(repo: &'static str, live: PathBuf, kind: Kind) -> Self {
        Self {
            repo,
            live,
            kind,
            templated: false,
            executable: false,
        }
    }

    fn file(repo: &'static str, live: PathBuf) -> Self {
        Self::new(repo, live, Kind::File)
    }

    fn dir(repo: &'static str, live: PathBuf) -> Self {
        Self::new(repo, live, Kind::Dir)
    }

    fn templated(mut self) -> Self {
        self.templated = true;
        self
    }

    /// No entry sets this today: `config/bin` was the only one, and its
    /// scripts were removed. Kept because restoring a captured `bin/`
    /// directory without the mode is worse than not restoring it -- the file
    /// arrives, and fails in a way that reads like it is missing.
    #[cfg_attr(not(test), expect(dead_code, reason = "no entry needs it yet"))]
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

/// Where this machine keeps the things we capture.
///
/// Resolved once, from the environment, at the edge of the program. Everything
/// below takes it as data, so no function deeper in has to reach for a variable
/// -- which is what made the old helpers untestable: they could only be
/// exercised by mutating the process environment, which is racy across
/// parallel tests and unsafe from Rust 2024. Two of them silently passed here
/// and failed in CI, where `XDG_CONFIG_HOME` happens to be set.
#[derive(Debug, Clone)]
pub struct Layout {
    pub home: PathBuf,
    pub pi_agent: PathBuf,
    pub mcp_config: PathBuf,
    pub user_bin: PathBuf,
}

impl Layout {
    /// The default arrangement: everything under one home directory.
    pub fn under(home: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
            pi_agent: home.join(".pi").join("agent"),
            mcp_config: home.join(".config").join("mcp"),
            user_bin: home.join(".local").join("bin"),
        }
    }

    /// The arrangement this machine actually has: the default, with any
    /// explicit override applied, so a relocated directory can still be
    /// captured and restored.
    pub fn from_env(home: &Path) -> Self {
        let mut layout = Self::under(home);
        if let Some(dir) = env::var_os("PI_AGENT_DIR") {
            layout.pi_agent = PathBuf::from(dir);
        }
        if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
            layout.mcp_config = PathBuf::from(dir).join("mcp");
        }
        if let Some(dir) = env::var_os("PI_CONFIG_BIN_DIR") {
            layout.user_bin = PathBuf::from(dir);
        }
        layout
    }
}

pub fn manifest(layout: &Layout) -> Vec<Entry> {
    let pi = &layout.pi_agent;
    let home = &layout.home;
    vec![
        Entry::file("config/pi/settings.json", pi.join("settings.json")),
        Entry::file(
            "config/pi/claude-bridge.json",
            pi.join("claude-bridge.json"),
        ),
        Entry::file("config/pi/models-store.json", pi.join("models-store.json")),
        Entry::dir("config/pi/skills", pi.join("skills")),
        Entry::file("config/mcp/mcp.json", layout.mcp_config.join("mcp.json")),
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

    /// A Layout built as data, with no environment involved, so the test
    /// cannot pass or fail depending on the machine it runs on. The old
    /// version read `XDG_CONFIG_HOME` from the process and passed only because
    /// it happened to be unset here; CI, where it is set, failed.
    #[test]
    fn a_layout_places_every_directory_under_its_home() {
        let l = Layout::under(Path::new("/somewhere"));
        assert_eq!(l.pi_agent, Path::new("/somewhere/.pi/agent"));
        assert_eq!(l.mcp_config, Path::new("/somewhere/.config/mcp"));
        assert_eq!(l.user_bin, Path::new("/somewhere/.local/bin"));
    }

    /// Each directory can be relocated on its own, and doing so must not move
    /// the others.
    #[test]
    fn an_override_moves_one_directory_and_leaves_the_rest() {
        let l = Layout {
            mcp_config: PathBuf::from("/elsewhere/mcp"),
            ..Layout::under(Path::new("/somewhere"))
        };
        assert_eq!(l.mcp_config, Path::new("/elsewhere/mcp"));
        assert_eq!(l.pi_agent, Path::new("/somewhere/.pi/agent"));
    }

    #[test]
    fn a_template_round_trips_through_the_home_directory() {
        let live = "/somewhere/.mempalace/palace";
        let stored = to_template(live, "/somewhere");
        assert_eq!(stored, "${HOME}/.mempalace/palace");
        assert_eq!(from_template(&stored, "/somewhere"), live);
    }

    #[test]
    fn every_entry_is_anchored_under_the_given_home() {
        let home = Path::new("/anchor");
        for entry in manifest(&Layout::under(home)) {
            assert!(
                entry.live.starts_with(home),
                "{} escaped the home directory: {}",
                entry.repo,
                entry.live.display()
            );
        }
    }
}

#[cfg(test)]
mod executable_tests {
    use super::*;

    /// Guards the builder itself, so the flag cannot quietly stop being set
    /// while `plan` still reads it.
    #[test]
    fn the_builder_sets_the_flag() {
        let plain = Entry::file("config/x", PathBuf::from("/tmp/x"));
        assert!(!plain.executable, "entries are not executable by default");

        let script = Entry::file("config/x", PathBuf::from("/tmp/x")).executable();
        assert!(script.executable, "the builder must set it");
    }
}
