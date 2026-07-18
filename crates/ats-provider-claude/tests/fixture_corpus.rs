//! Fixture corpus integrity tests (I-04).
//!
//! Guards the Claude Code hook fixture corpus that drives I-07 parser
//! development: structure, size, JSON validity, and privacy sanitization
//! (SPEC §14.2). Parser correctness tests live in I-07.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// All hook types the corpus must cover (SPEC §9.1).
const HOOK_TYPES: [&str; 8] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "Stop",
    "SessionEnd",
];

/// The Claude Code version whose corpus must be complete.
const PRIMARY_VERSION: &str = "2.1.214";

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude")
}

fn all_fixture_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("read fixtures dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "json") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&fixtures_root(), &mut files);
    files.sort();
    files
}

fn version_dirs() -> Vec<PathBuf> {
    fs::read_dir(fixtures_root())
        .expect("fixtures root exists")
        .filter_map(|e| {
            let path = e.expect("dir entry").path();
            path.is_dir().then_some(path)
        })
        .collect()
}

#[test]
fn corpus_has_at_least_two_claude_versions() {
    let versions: BTreeSet<String> = version_dirs()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(
        versions.len() >= 2,
        "need >= 2 Claude Code versions to surface drift, found: {versions:?}"
    );
    assert!(
        versions.contains(PRIMARY_VERSION),
        "primary version {PRIMARY_VERSION} missing: {versions:?}"
    );
}

#[test]
fn corpus_has_at_least_thirty_fixtures() {
    let files = all_fixture_files();
    assert!(
        files.len() >= 30,
        "DoD requires >= 30 fixtures, found {}",
        files.len()
    );
}

#[test]
fn every_fixture_parses_as_json_object_with_hook_event_name() {
    let files = all_fixture_files();
    assert!(!files.is_empty(), "no fixtures found");
    for file in files {
        let raw = fs::read_to_string(&file).expect("read fixture");
        let value: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", file.display()));
        let object = value
            .as_object()
            .unwrap_or_else(|| panic!("{} is not a JSON object", file.display()));
        // Synthetic missing-field variants may drop any field except the
        // hook name itself, which is how I-07 routes payloads.
        assert!(
            object.contains_key("hook_event_name"),
            "{} lacks hook_event_name",
            file.display()
        );
    }
}

#[test]
fn primary_version_covers_all_hook_types_with_variants() {
    let primary = fixtures_root().join(PRIMARY_VERSION);
    for hook in HOOK_TYPES {
        let dir = primary.join(hook);
        assert!(dir.is_dir(), "missing hook dir: {}", dir.display());
        let names: Vec<String> = fs::read_dir(&dir)
            .expect("read hook dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json"))
            .collect();
        assert!(
            !names.is_empty(),
            "no fixtures for hook {hook} in primary version"
        );
        // Notification cannot fire in headless `claude -p` sessions; its
        // corpus entries are documented synthetic payloads (see README.md).
        if hook != "Notification" {
            assert!(
                names.iter().any(|n| !n.contains(".synthetic")),
                "hook {hook} needs at least one real (non-synthetic) capture"
            );
        }
        assert!(
            names.iter().any(|n| n.contains("missing-field.synthetic")),
            "hook {hook} needs a missing-field synthetic variant"
        );
        assert!(
            names.iter().any(|n| n.contains("unknown-field.synthetic")),
            "hook {hook} needs an unknown-field synthetic variant"
        );
    }
}

#[test]
fn fixtures_contain_no_private_paths_or_secrets() {
    for file in all_fixture_files() {
        let raw = fs::read_to_string(&file).expect("read fixture");

        for (idx, _) in raw.match_indices("/Users/") {
            let rest = &raw[idx + "/Users/".len()..];
            assert!(
                rest.starts_with("testuser"),
                "{} leaks a real home directory",
                file.display()
            );
        }
        assert!(
            !raw.contains("/var/folders/") && !raw.contains("/private/var/"),
            "{} leaks a machine temp path",
            file.display()
        );
        assert!(
            !raw.contains("~/") && !raw.contains("\"~"),
            "{} contains unexpanded home shorthand",
            file.display()
        );
        assert!(
            !raw.contains("sk-ant-") && !raw.contains("sk-proj-"),
            "{} contains an API-key-shaped string",
            file.display()
        );
    }
}
