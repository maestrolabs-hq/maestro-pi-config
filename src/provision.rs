//! Installing what configuration needs before it can be restored.
//!
//! Restoring writes a `settings.json` naming packages and an `mcp.json`
//! naming binaries. Neither is any use if those are absent, so provisioning
//! runs first.

use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
pub struct Step {
    pub program: String,
    pub args: Vec<String>,
    /// The manifest line this came from, for reporting.
    pub source: String,
}

impl Step {
    pub fn rendered(&self) -> String {
        format!("{} {}", self.program, self.args.join(" "))
    }
}

fn step(program: &str, args: &[&str], source: &str) -> Step {
    Step {
        program: program.to_owned(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        source: source.to_owned(),
    }
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
pub fn run(step: &Step) -> Result<(), String> {
    let status = Command::new(&step.program)
        .args(&step.args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("{}: {e}", step.program))?;
    if status.success() {
        Ok(())
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
}
