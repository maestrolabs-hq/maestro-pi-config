//! Installing what configuration needs before it can be restored.
//!
//! Restoring writes a `settings.json` naming packages and an `mcp.json`
//! naming binaries. Neither is any use if those are absent, so provisioning
//! runs first.

use std::env;
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
pub struct Step {
    pub program: String,
    pub args: Vec<String>,
    /// The manifest line this came from, for reporting.
    pub source: String,
    /// Whether an absent `program` fails provisioning. False for a step that
    /// is only applicable on some platforms (systemd exists on Linux only):
    /// there, a missing program means "not applicable here," not "broken."
    pub required: bool,
}

impl Step {
    pub fn rendered(&self) -> String {
        format!("{} {}", self.program, self.args.join(" "))
    }
}

/// The outcome of running a step that did not fail.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Ran,
    /// An optional step whose program is not on `PATH` on this platform.
    Skipped(String),
}

fn step(program: &str, args: &[&str], source: &str) -> Step {
    Step {
        program: program.to_owned(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        source: source.to_owned(),
        required: true,
    }
}

impl Step {
    /// Marks a step as platform-conditional: see the `required` field.
    fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

/// Whether `program` resolves on `PATH`, without invoking it. Hand-rolled
/// rather than a dependency: this crate needs the answer for exactly two
/// program names, not a general-purpose lookup.
fn on_path(program: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|path| env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// Parse the manifest. Unknown kinds are an error rather than a silent skip:
/// a typo that installs nothing is worse than one that stops.
pub fn parse(manifest: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for (n, raw) in manifest.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(kind), Some(arg)) = (parts.next(), parts.next()) else {
            return Err(format!(
                "line {}: expected `<kind> <argument>`: {raw}",
                n + 1
            ));
        };
        steps.push(match kind {
            "uv" => step("uv", &["tool", "install", arg], line),
            "cargo" => step("cargo", &["binstall", "-y", arg], line),
            "rustup-component" => step("rustup", &["component", "add", arg], line),
            "pi" => step("pi", &["install", arg], line),
            // Herdr's autostart unit: enabling and lingering are separate
            // loginctl/systemctl verbs, so they are separate manifest lines.
            // Both are optional -- systemd is a Linux concept, and a machine
            // without it is not broken for lacking them.
            "systemd-user-enable" => step("systemctl", &["--user", "enable", arg], line).optional(),
            // `arg` is a required token under the `<kind> <argument>`
            // grammar but is not forwarded: `loginctl enable-linger` with no
            // user argument targets the invoking user, which is always the
            // right target for a per-machine provisioning manifest -- baking
            // in a literal username here would be exactly the machine-specific
            // fact this repository's templating exists to avoid.
            "systemd-user-linger" => step("loginctl", &["enable-linger"], line).optional(),
            // A binary is a URL to fetch by hand: every project releases
            // differently, and guessing an asset name silently installs the
            // wrong thing.
            "binary" => continue,
            other => return Err(format!("line {}: unknown kind `{other}`", n + 1)),
        });
    }
    Ok(steps)
}

/// Lines describing binaries that must be fetched by hand.
pub fn manual(manifest: &str) -> Vec<String> {
    manifest
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| l.starts_with("binary "))
        .map(|l| l.trim_start_matches("binary ").to_owned())
        .collect()
}

/// Run a step. Output goes to the terminal: an install that fails should say
/// why in its own words.
///
/// An optional step whose program is absent from `PATH` is skipped rather
/// than run: on the platforms where that program does not exist at all
/// (systemd is Linux-only), absence means "not applicable," not "broken."
/// A required step's absence is still a hard failure, unchanged from before.
pub fn run(step: &Step) -> Result<Outcome, String> {
    if !step.required && !on_path(&step.program) {
        return Ok(Outcome::Skipped(format!(
            "{} not found on PATH; skipping (Linux/systemd only)",
            step.program
        )));
    }
    let status = Command::new(&step.program)
        .args(&step.args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("{}: {e}", step.program))?;
    if status.success() {
        Ok(Outcome::Ran)
    } else {
        Err(format!("{} exited {status}", step.rendered()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        assert!(
            parse("# just a comment\n\n   \n")
                .expect("parse")
                .is_empty()
        );
    }

    #[test]
    fn each_kind_maps_to_its_installer() {
        let steps =
            parse("uv mempalace==3.8.0\ncargo prek@0.5.0\npi npm:pi-lens\nrustup-component clippy")
                .expect("parse");
        let rendered: Vec<String> = steps.iter().map(Step::rendered).collect();
        assert_eq!(
            rendered,
            vec![
                "uv tool install mempalace==3.8.0",
                "cargo binstall -y prek@0.5.0",
                "pi install npm:pi-lens",
                "rustup component add clippy",
            ]
        );
    }

    #[test]
    fn a_trailing_comment_does_not_become_an_argument() {
        let steps = parse("uv mempalace==3.8.0  # the memory tool").expect("parse");
        assert_eq!(steps[0].rendered(), "uv tool install mempalace==3.8.0");
    }

    #[test]
    fn an_unknown_kind_stops_rather_than_installing_nothing() {
        let err = parse("npm something").expect_err("should reject");
        assert!(err.contains("unknown kind"), "{err}");
    }

    #[test]
    fn binaries_are_reported_for_manual_installation() {
        let m = "binary gh https://example.invalid/cli\nuv mempalace==3.8.0";
        assert!(
            parse(m).expect("parse").len() == 1,
            "a binary is not an automated step"
        );
        assert_eq!(manual(m), vec!["gh https://example.invalid/cli"]);
    }

    #[test]
    fn systemd_user_enable_maps_to_systemctl_and_is_optional() {
        let steps = parse("systemd-user-enable herdr.service").expect("parse");
        assert_eq!(steps[0].rendered(), "systemctl --user enable herdr.service");
        assert!(
            !steps[0].required,
            "a systemd step must not fail provisioning on a platform without systemd"
        );
    }

    #[test]
    fn systemd_user_linger_maps_to_loginctl_enable_linger_and_is_optional() {
        let steps = parse("systemd-user-linger enable").expect("parse");
        assert_eq!(steps[0].rendered(), "loginctl enable-linger");
        assert!(!steps[0].required);
    }

    #[test]
    fn run_skips_an_optional_step_whose_program_is_absent() {
        let s = Step {
            program: "definitely-not-a-real-binary-xyz".to_owned(),
            args: vec![],
            source: "test".to_owned(),
            required: false,
        };
        match run(&s) {
            Ok(Outcome::Skipped(reason)) => assert!(reason.contains("PATH")),
            other => panic!("expected Skipped, got {other:?}"),
        }
    }

    #[test]
    fn run_still_fails_a_required_step_whose_program_is_absent() {
        let s = Step {
            program: "definitely-not-a-real-binary-xyz".to_owned(),
            args: vec![],
            source: "test".to_owned(),
            required: true,
        };
        assert!(
            run(&s).is_err(),
            "a required step's missing program must still fail provisioning"
        );
    }
}
