//! The captured gauntlet-loop skill is an adaptation, not a copy.
//!
//! Upstream is written for Claude Code and uses two primitives pi does not
//! have: the `/loop` slash command and `ultracode`. Our copy replaces both.
//!
//! Nothing enforces that on its own. Re-capturing from a pristine clone, or
//! restoring the wrong file, would silently reinstate instructions pi cannot
//! follow -- and the failure would be a model quietly ignoring a line, not an
//! error anyone sees. Hence these tests.

use std::fs;
use std::path::PathBuf;

fn skill() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/pi/skills/gauntlet-loop/SKILL.md");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The one line allowed to name the Claude Code primitives is the note
/// explaining that they were replaced.
const EXPLANATION: &str = "This copy is adapted for pi.";

#[test]
fn no_prompt_template_tells_the_agent_to_ultracode() {
    let text = skill();
    for (n, line) in text.lines().enumerate() {
        if line.contains("ultracode") {
            assert!(
                line.contains(EXPLANATION),
                "line {}: ultracode outside the adaptation note: {line}",
                n + 1
            );
        }
    }
}

#[test]
fn no_prompt_template_uses_the_loop_slash_command() {
    let text = skill();
    for (n, line) in text.lines().enumerate() {
        if line.contains("/loop") {
            assert!(
                line.contains(EXPLANATION),
                "line {}: /loop outside the adaptation note: {line}",
                n + 1
            );
        }
    }
}

#[test]
fn every_prompt_template_dispatches_through_subagents() {
    let text = skill();
    let replacement = text
        .matches("Run the builders and critics as parallel subagents.")
        .count();
    assert_eq!(
        replacement, 3,
        "expected the replacement line in all three prompt templates, found {replacement}"
    );
}

#[test]
fn the_skill_still_declares_itself_to_pi() {
    let text = skill();
    assert!(
        text.starts_with("---"),
        "frontmatter is what makes pi load it"
    );
    assert!(
        text.contains("name: gauntlet-loop"),
        "pi registers the skill by this name"
    );
}
