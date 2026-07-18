use ats_core::EventType;
use ats_provider::ProviderAdapter;
use ats_provider_claude::ClaudeAdapter;
use std::fs;
use std::path::{Path, PathBuf};

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

#[test]
fn every_fixture_parses_without_error() {
    let adapter = ClaudeAdapter::new();
    let fixtures = all_fixture_files();
    assert!(
        fixtures.len() >= 30,
        "expected >= 30 fixtures, got {}",
        fixtures.len()
    );

    let mut parsed = 0;
    let mut schema_errors = 0;

    for fixture in &fixtures {
        let content = fs::read_to_string(fixture).expect("read fixture");
        let input: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");

        let validation = adapter.validate(&input);
        let result = adapter.parse(input);

        let fixture_name = fixture
            .strip_prefix(fixtures_root())
            .unwrap_or(fixture)
            .display()
            .to_string();

        match result {
            Ok(events) => {
                for event in &events {
                    if event.event_type == EventType::ProviderSchemaError {
                        schema_errors += 1;
                    } else {
                        assert_eq!(event.provider, "claude");
                        assert_eq!(event.schema_version, "1.0");
                        assert!(!event.session.id.is_empty());
                        parsed += 1;
                    }
                }
            }
            Err(e) => {
                panic!("fixture {fixture_name}: parse failed with error: {e:?}");
            }
        }

        if !validation.is_valid() {
            let name = fixture_name;
            assert!(
                name.contains("synthetic") || name.contains("missing"),
                "fixture {name}: validation failed but fixture is not synthetic"
            );
        }
    }

    assert!(parsed > 0, "no fixtures were parsed successfully");
    eprintln!(
        "Parsed {parsed} events, {schema_errors} schema errors from {} fixtures",
        fixtures.len()
    );
}

#[test]
fn synthetic_missing_session_id_emits_schema_error_or_fallback() {
    let adapter = ClaudeAdapter::new();
    let fixture_path = fixtures_root()
        .join("2.1.214")
        .join("SessionStart")
        .join("missing-field.synthetic.json");
    let content = fs::read_to_string(fixture_path).unwrap();
    let input: serde_json::Value = serde_json::from_str(&content).unwrap();
    let events = adapter.parse(input).unwrap();
    assert!(!events.is_empty());
    for event in &events {
        assert_eq!(event.provider, "claude");
    }
}

#[test]
fn synthetic_unknown_fields_are_tolerated() {
    let adapter = ClaudeAdapter::new();
    let fixture_path = fixtures_root()
        .join("2.1.214")
        .join("SessionStart")
        .join("unknown-field.synthetic.json");
    let content = fs::read_to_string(fixture_path).unwrap();
    let input: serde_json::Value = serde_json::from_str(&content).unwrap();
    let events = adapter.parse(input).unwrap();
    assert!(!events.is_empty());
    assert_eq!(events[0].event_type, EventType::SessionStarted);
}

#[test]
fn risk_classifier_applied_to_bash_pretooluse() {
    let adapter = ClaudeAdapter::new();
    let risky = r#"{"session_id":"test","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/test","description":"test"},"tool_use_id":"t1","cwd":"/tmp","transcript_path":"/tmp/t.jsonl"}"#;
    let input: serde_json::Value = serde_json::from_str(risky).unwrap();
    let events = adapter.parse(input).unwrap();
    let activity = events[0].activity.as_ref().unwrap();
    assert!(activity.risk.is_some());

    let benign = r#"{"session_id":"test","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"echo hello","description":"test"},"tool_use_id":"t1","cwd":"/tmp","transcript_path":"/tmp/t.jsonl"}"#;
    let input: serde_json::Value = serde_json::from_str(benign).unwrap();
    let events = adapter.parse(input).unwrap();
    let activity = events[0].activity.as_ref().unwrap();
    assert!(activity.risk.is_none());
}

#[test]
fn notification_classification() {
    let adapter = ClaudeAdapter::new();

    let permission = r#"{"session_id":"test","hook_event_name":"Notification","message":"Claude needs your permission to use Bash","cwd":"/tmp","transcript_path":"/tmp/t.jsonl"}"#;
    let input: serde_json::Value = serde_json::from_str(permission).unwrap();
    let events = adapter.parse(input).unwrap();
    assert_eq!(events[0].event_type, EventType::UserApprovalRequired);

    let input_req = r#"{"session_id":"test","hook_event_name":"Notification","message":"Claude is waiting for your input","cwd":"/tmp","transcript_path":"/tmp/t.jsonl"}"#;
    let input: serde_json::Value = serde_json::from_str(input_req).unwrap();
    let events = adapter.parse(input).unwrap();
    assert_eq!(events[0].event_type, EventType::UserInputRequired);
}

#[test]
fn all_eight_hook_types_produce_correct_event_types() {
    let adapter = ClaudeAdapter::new();

    let expectations = vec![
        ("SessionStart", EventType::SessionStarted),
        ("UserPromptSubmit", EventType::AgentStarted),
        ("PreToolUse", EventType::ToolStarted),
        ("PostToolUse", EventType::ToolCompleted),
        ("PostToolUseFailure", EventType::ToolFailed),
        ("Stop", EventType::AgentCompleted),
        ("SessionEnd", EventType::SessionStopped),
    ];

    for (hook, expected) in expectations {
        let input = match hook {
            "PreToolUse" | "PostToolUse" | "PostToolUseFailure" => {
                serde_json::json!({
                    "session_id": "test",
                    "hook_event_name": hook,
                    "tool_name": "Bash",
                    "tool_input": {"command": "echo test", "description": "test"},
                    "tool_use_id": "t1",
                    "cwd": "/tmp",
                    "transcript_path": "/tmp/t.jsonl"
                })
            }
            "Stop" => {
                serde_json::json!({
                    "session_id": "test",
                    "hook_event_name": hook,
                    "cwd": "/tmp",
                    "transcript_path": "/tmp/t.jsonl",
                    "stop_hook_active": false
                })
            }
            _ => {
                serde_json::json!({
                    "session_id": "test",
                    "hook_event_name": hook,
                    "cwd": "/tmp",
                    "transcript_path": "/tmp/t.jsonl"
                })
            }
        };
        let events = adapter.parse(input).unwrap();
        assert_eq!(
            events[0].event_type, expected,
            "hook {hook} produced {:?}, expected {expected:?}",
            events[0].event_type
        );
    }
}
