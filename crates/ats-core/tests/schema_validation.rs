use std::path::Path;

use ats_core::{new_event_id, now_utc, EventType, NormalizedEvent, SessionIdentity};
use serde_json::{json, Value};

fn schema() -> jsonschema::Validator {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/event-v1.schema.json");
    let raw = std::fs::read_to_string(&path).expect("read schemas/event-v1.schema.json");
    let schema: Value = serde_json::from_str(&raw).expect("schema is valid JSON");
    jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .expect("schema compiles")
}

fn sample_event() -> NormalizedEvent {
    NormalizedEvent {
        schema_version: "1.0".to_string(),
        event_id: new_event_id(),
        timestamp: now_utc(),
        provider: "claude".to_string(),
        provider_version: Some("1.0".to_string()),
        event_type: EventType::ToolStarted,
        session: SessionIdentity {
            id: "session-1".to_string(),
            ..SessionIdentity::default()
        },
        activity: None,
        metadata: serde_json::Map::new(),
    }
}

#[test]
fn generated_event_passes_schema() {
    let validator = schema();
    let value = serde_json::to_value(sample_event()).expect("serialize");
    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|e| format!("{e} at {}", e.instance_path))
        .collect();
    assert!(errors.is_empty(), "schema violations: {errors:?}");
}

#[test]
fn spec_sample_event_passes_schema() {
    let validator = schema();
    let value = json!({
      "schema_version": "1.0",
      "event_id": "018f2b70-5f14-7fb7-a880-123456789abc",
      "timestamp": "2026-07-18T07:15:31.123Z",
      "provider": "claude",
      "provider_version": "1.0",
      "event_type": "tool.started",
      "session": {
        "id": "provider-session-id",
        "parent_id": null,
        "workspace": "/Users/user/src/project",
        "terminal": {
          "tty": "/dev/ttys004",
          "term_program": "iTerm.app",
          "tmux_pane": "%12"
        }
      },
      "activity": {
        "category": "shell",
        "label": "Running tests",
        "tool_name": "Bash",
        "risk": "low"
      },
      "metadata": {}
    });
    assert!(validator.validate(&value).is_ok());
}

#[test]
fn event_missing_session_id_fails_schema() {
    let validator = schema();
    let mut value = serde_json::to_value(sample_event()).expect("serialize");
    value["session"]
        .as_object_mut()
        .expect("session object")
        .remove("id");
    assert!(
        validator.validate(&value).is_err(),
        "missing session.id must fail"
    );
}

#[test]
fn event_with_unknown_event_type_fails_schema() {
    let validator = schema();
    let mut value = serde_json::to_value(sample_event()).expect("serialize");
    value["event_type"] = json!("tool.exploded");
    assert!(
        validator.validate(&value).is_err(),
        "unknown event_type must fail"
    );
}

#[test]
fn event_with_overlong_label_fails_schema() {
    let validator = schema();
    let mut value = serde_json::to_value(sample_event()).expect("serialize");
    value["activity"] = json!({
        "category": "shell",
        "label": "x".repeat(41),
        "tool_name": null,
        "risk": null
    });
    assert!(
        validator.validate(&value).is_err(),
        "41-char label must fail"
    );
}

#[test]
fn event_with_unknown_top_level_field_fails_schema() {
    let validator = schema();
    let mut value = serde_json::to_value(sample_event()).expect("serialize");
    value["prompt_body"] = json!("must never be transported");
    assert!(
        validator.validate(&value).is_err(),
        "unknown top-level fields must be rejected (strict schema, privacy)"
    );
}
