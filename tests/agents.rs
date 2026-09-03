//! The governed agent sources are what pi loads as user-scope agents.
//!
//! A missing frontmatter field does not error: pi silently falls back to the
//! session model, so a routing decision would vanish without a failure anyone
//! sees. Hence these tests.

use std::fs;
use std::path::PathBuf;

fn agent(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config/pi/agents")
        .join(format!("{name}.md"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn each_agent_declares_itself_to_pi() {
    for name in ["spark", "adversary"] {
        let text = agent(name);
        assert!(
            text.starts_with("---"),
            "{name}: frontmatter is what makes pi load it"
        );
        assert!(
            text.contains(&format!("name: {name}")),
            "{name}: pi registers the agent by this name"
        );
    }
}

#[test]
fn each_agent_pins_a_model_and_a_cross_family_fallback() {
    for name in ["spark", "adversary"] {
        let text = agent(name);
        assert!(
            text.contains("model:"),
            "{name}: an unpinned agent inherits the session model"
        );
        assert!(
            text.contains("fallbackModels:"),
            "{name}: a lane with no fallback dies on a quota error"
        );
    }
}

#[test]
fn the_adversary_cannot_edit() {
    let text = agent("adversary");
    for forbidden in ["edit", "write", "bash"] {
        assert!(
            !text.contains(&format!(" {forbidden},")) && !text.contains(&format!(" {forbidden}\n")),
            "adversary: a review lane that can edit is not a review lane"
        );
    }
}
