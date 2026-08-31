//! Copy-paste detection, via `similarity-rs` (APTED tree edit distance, so it
//! compares structure rather than text and is not fooled by renaming).
//!
//! Structural similarity is not the same as duplication worth removing.
//! `to_template` and `from_template` are 89% alike and must stay apart:
//! merging them would produce one function with a flag that flips its meaning.
//! So this gate is an allowlist, not a threshold -- every accepted pair is a
//! written decision, and anything new fails.

use std::collections::BTreeSet;
use std::process::Command;

const THRESHOLD: &str = "0.85";

/// Pairs we have looked at and chosen to keep, with the reason.
const ACCEPTED: &[(&str, &str)] = &[
    (
        "to_template <-> from_template",
        "Inverses, one line each. Merging them means a direction flag, which is worse.",
    ),
    (
        "file <-> dir",
        "Two named constructors delegating to Entry::new. The names are the point.",
    ),
    (
        "inspect <-> sync",
        "Same skeleton -- match on entry.kind -- doing entirely different work: one \
         compares and reports, the other reads and writes. Structural, not logical.",
    ),
    (
        "a_plan_is_refused_once_the_machine_moves_under_it <-> \
         a_plan_is_refused_once_the_repository_moves_under_it",
        "Parallel scenarios. A test that has to be assembled from helpers to be \
         understood has stopped documenting the behaviour it covers.",
    ),
    (
        "no_prompt_template_tells_the_agent_to_ultracode <-> \
         no_prompt_template_uses_the_loop_slash_command",
        "Two guards over the same file for different banned words. Same reason.",
    ),
    (
        "run_plan <-> run_apply",
        "Parallel verbs of one CLI: resolve a subject, render it, decide. They \
         are alike because they are alike. The one rule worth sharing -- that \
         nothing happens without --auto-approve -- is already `approved`; \
         collapsing the rest needs a trait over the subject type and makes both \
         harder to read than the repetition does. run_apply <-> run_destroy \
         sits just under the threshold for the same reason.",
    ),
];

/// `path:lines function name <-> path:lines function name` -> `name <-> name`.
/// Line numbers move whenever anything above them moves.
fn pair_name(line: &str) -> Option<String> {
    // `Classes: Entry <-> Entry` names the type two methods share, not a pair.
    if line.trim_start().starts_with("Classes:") {
        return None;
    }
    let (left, right) = line.split_once(" <-> ")?;
    let name = |s: &str| s.split_whitespace().last().map(str::to_owned);
    Some(format!("{} <-> {}", name(left)?, name(right)?))
}

fn detected() -> BTreeSet<String> {
    let out = Command::new("similarity-rs")
        .args(["--threshold", THRESHOLD, "src", "tests"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect(
            "similarity-rs must be installed: cargo binstall similarity-rs. \
             A gate that skips when its tool is missing reports green while \
             looking at nothing.",
        );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(pair_name)
        .collect()
}

#[test]
fn no_duplication_is_unaccounted_for() {
    let found = detected();
    let accepted: BTreeSet<&str> = ACCEPTED.iter().map(|(p, _)| *p).collect();

    let unexplained: Vec<&String> = found
        .iter()
        .filter(|p| !accepted.contains(p.as_str()))
        .collect();

    assert!(
        unexplained.is_empty(),
        "Duplication with no recorded decision:\n\n{}\n\n\
         Either factor out what is shared, or add the pair to ACCEPTED in this \
         file with the reason it should stay.\n",
        unexplained
            .iter()
            .map(|p| format!("  {p}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// An allowlist nobody prunes becomes a list of excuses for code that no longer
/// exists, and the next real duplicate hides among them.
#[test]
fn no_accepted_pair_has_gone_stale() {
    let found = detected();
    let stale: Vec<&str> = ACCEPTED
        .iter()
        .map(|(p, _)| *p)
        .filter(|p| !found.contains(*p))
        .collect();

    assert!(
        stale.is_empty(),
        "ACCEPTED lists pairs that are no longer duplicated:\n\n{}\n\n\
         Remove them.\n",
        stale
            .iter()
            .map(|p| format!("  {p}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
