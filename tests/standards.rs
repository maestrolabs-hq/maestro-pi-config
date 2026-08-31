//! Size limits. A test, not a promise.

use std::fs;
use std::path::{Path, PathBuf};

/// Python's governance bar uses 200. Rust needs more room for the same
/// content -- closing braces, explicit types, match arms, derive attributes --
/// so this sits a quarter higher.
///
/// It is a dumping-ground tripwire, not a design rule. The design rules are
/// per-function and live in `Cargo.toml`: `too_many_lines`,
/// `cognitive_complexity`, `too_many_arguments`. A module of twenty small
/// clear functions is fine; one of three sprawling ones is not, and this
/// limit would not notice the difference. Those lints would.
const MAX_MODULE_LINES: usize = 250;

fn src_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            src_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Lines before the first test module. Rust colocates unit tests with the code
/// they cover; counting them would charge a module for being tested.
///
/// Any cfg naming `test` counts, not just the bare `#[cfg(test)]`. Matching the
/// literal string charged `plan.rs` for a `#[cfg(all(test, unix))]` module --
/// the gate doing the exact thing this comment says it must not.
fn code_lines(path: &Path) -> usize {
    let text = fs::read_to_string(path).expect("read");
    text.lines()
        .position(|l| {
            let l = l.trim_start();
            l.starts_with("#[cfg(")
                && l.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|tok| tok == "test")
        })
        .unwrap_or_else(|| text.lines().count())
}

#[test]
fn no_module_becomes_a_dumping_ground() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    src_files(&root, &mut files);
    assert!(
        !files.is_empty(),
        "no module found under {}",
        root.display()
    );

    let over: Vec<String> = files
        .iter()
        .filter_map(|f| {
            let n = code_lines(f);
            (n > MAX_MODULE_LINES)
                .then(|| format!("  {}: {n} lines (max {MAX_MODULE_LINES})", f.display()))
        })
        .collect();
    assert!(over.is_empty(), "Module too large:\n{}\n", over.join("\n"));
}
