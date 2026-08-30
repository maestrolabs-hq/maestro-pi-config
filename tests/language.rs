//! English-only gate.
//!
//! Pragmatic heuristic: French text virtually always carries accented
//! characters, so scanning for Latin diacritics catches a regression cheaply
//! without any language detection. It will not catch French written without
//! accents, which is the accepted limit of a cheap check.
//!
//! `config/` is exempt: it is captured third-party configuration, and must
//! stay byte-identical to its source rather than conform to our prose rules.

use std::fs;
use std::path::{Path, PathBuf};

/// Latin-1 accented letters (A-grave through y-umlaut, skipping the
/// multiplication and division signs) plus the OE ligatures. Built from code
/// points so this file itself stays accent-free and cannot fail its own test.
const RANGES: &[(u32, u32)] = &[
    (0x00C0, 0x00D6),
    (0x00D8, 0x00F6),
    (0x00F8, 0x00FF),
    (0x0152, 0x0153),
];

fn is_accented(c: char) -> bool {
    RANGES
        .iter()
        .any(|(lo, hi)| (*lo..=*hi).contains(&(c as u32)))
}

fn scanned(dir: &Path, out: &mut Vec<PathBuf>) {
    let skip = ["target", "node_modules", ".git", "config"];
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if skip.contains(&name.as_str()) {
            continue;
        }
        if path.is_dir() {
            scanned(&path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("rs" | "md" | "toml" | "json" | "yaml" | "yml" | "ts")
        ) {
            out.push(path);
        }
    }
}

#[test]
fn all_prose_is_english() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    scanned(&root, &mut files);
    assert!(!files.is_empty(), "nothing was scanned");

    let mut found = Vec::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            if line.chars().any(is_accented) {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                found.push(format!("  {}:{}: {}", rel.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "Accented characters, which usually means French:\n\n{}\n",
        found.join("\n")
    );
}
