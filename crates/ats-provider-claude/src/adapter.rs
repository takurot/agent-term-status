use ats_core::{
    Activity, ActivityCategory, ActivityLabel, EventType, NormalizedEvent, RiskLevel,
    SessionIdentity, TerminalContext, SCHEMA_VERSION,
};
use ats_provider::{ProviderAdapter, ProviderError, ValidationResult};
use serde_json::Value;

use crate::risk::RiskClassifier;

const PROVIDER_NAME: &str = "claude";

pub struct ClaudeAdapter {
    version: String,
    risk_classifier: RiskClassifier,
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            risk_classifier: RiskClassifier::default(),
        }
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAdapter for ClaudeAdapter {
    fn name(&self) -> &str {
        PROVIDER_NAME
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn parse(&self, input: Value) -> Result<Vec<NormalizedEvent>, ProviderError> {
        let obj = input
            .as_object()
            .ok_or_else(|| ProviderError::Malformed("input is not a JSON object".to_string()))?;

        let hook_type = obj
            .get("hook_event_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ProviderError::MissingRequiredField("hook_event_name".to_string()))?;

        let session = self.derive_session(&input);
        let timestamp = ats_core::now_utc();
        let event_id = ats_core::new_event_id();
        let provider_version = detect_version(obj);

        match hook_type {
            "SessionStart" => Ok(vec![NormalizedEvent {
                schema_version: SCHEMA_VERSION.to_string(),
                event_id,
                timestamp,
                provider: PROVIDER_NAME.to_string(),
                provider_version,
                event_type: EventType::SessionStarted,
                session,
                activity: None,
                metadata: serde_json::Map::new(),
            }]),

            "UserPromptSubmit" => Ok(vec![NormalizedEvent {
                schema_version: SCHEMA_VERSION.to_string(),
                event_id,
                timestamp,
                provider: PROVIDER_NAME.to_string(),
                provider_version,
                event_type: EventType::AgentStarted,
                session,
                activity: None,
                metadata: serde_json::Map::new(),
            }]),

            "PreToolUse" => {
                let tool_name = obj.get("tool_name").and_then(|v| v.as_str());
                let command = tool_input_command(obj);
                let risk = self.evaluate_risk(tool_name, command.as_deref());
                let label = command.as_deref().map(ActivityLabel::new);

                let activity = Some(Activity {
                    category: categorize_tool(tool_name),
                    label,
                    tool_name: tool_name.map(|s| s.to_string()),
                    risk,
                });

                Ok(vec![NormalizedEvent {
                    schema_version: SCHEMA_VERSION.to_string(),
                    event_id,
                    timestamp,
                    provider: PROVIDER_NAME.to_string(),
                    provider_version,
                    event_type: EventType::ToolStarted,
                    session,
                    activity,
                    metadata: serde_json::Map::new(),
                }])
            }

            "PostToolUse" => {
                let tool_name = obj.get("tool_name").and_then(|v| v.as_str());
                let command = tool_input_command(obj);
                let label = command.as_deref().map(ActivityLabel::new);

                let activity = Some(Activity {
                    category: categorize_tool(tool_name),
                    label,
                    tool_name: tool_name.map(|s| s.to_string()),
                    risk: None,
                });

                Ok(vec![NormalizedEvent {
                    schema_version: SCHEMA_VERSION.to_string(),
                    event_id,
                    timestamp,
                    provider: PROVIDER_NAME.to_string(),
                    provider_version,
                    event_type: EventType::ToolCompleted,
                    session,
                    activity,
                    metadata: serde_json::Map::new(),
                }])
            }

            "PostToolUseFailure" => {
                let tool_name = obj.get("tool_name").and_then(|v| v.as_str());
                let command = tool_input_command(obj);
                let label = command.as_deref().map(ActivityLabel::new);

                let activity = Some(Activity {
                    category: categorize_tool(tool_name),
                    label,
                    tool_name: tool_name.map(|s| s.to_string()),
                    risk: None,
                });

                Ok(vec![NormalizedEvent {
                    schema_version: SCHEMA_VERSION.to_string(),
                    event_id,
                    timestamp,
                    provider: PROVIDER_NAME.to_string(),
                    provider_version,
                    event_type: EventType::ToolFailed,
                    session,
                    activity,
                    metadata: serde_json::Map::new(),
                }])
            }

            "Notification" => {
                let message = obj.get("message").and_then(|v| v.as_str()).unwrap_or("");
                let event_type = classify_notification(message);

                let label = if message.len() <= 40 {
                    ActivityLabel::new(message)
                } else {
                    ActivityLabel::new(&message[..40])
                };

                let activity = Some(Activity {
                    category: ActivityCategory::Unknown,
                    label: Some(label),
                    tool_name: None,
                    risk: None,
                });

                Ok(vec![NormalizedEvent {
                    schema_version: SCHEMA_VERSION.to_string(),
                    event_id,
                    timestamp,
                    provider: PROVIDER_NAME.to_string(),
                    provider_version,
                    event_type,
                    session,
                    activity,
                    metadata: serde_json::Map::new(),
                }])
            }

            "Stop" => Ok(vec![NormalizedEvent {
                schema_version: SCHEMA_VERSION.to_string(),
                event_id,
                timestamp,
                provider: PROVIDER_NAME.to_string(),
                provider_version,
                event_type: EventType::AgentCompleted,
                session,
                activity: None,
                metadata: serde_json::Map::new(),
            }]),

            "SessionEnd" => Ok(vec![NormalizedEvent {
                schema_version: SCHEMA_VERSION.to_string(),
                event_id,
                timestamp,
                provider: PROVIDER_NAME.to_string(),
                provider_version,
                event_type: EventType::SessionStopped,
                session,
                activity: None,
                metadata: serde_json::Map::new(),
            }]),

            _ => {
                let schema_error = NormalizedEvent {
                    schema_version: SCHEMA_VERSION.to_string(),
                    event_id,
                    timestamp,
                    provider: PROVIDER_NAME.to_string(),
                    provider_version,
                    event_type: EventType::ProviderSchemaError,
                    session,
                    activity: None,
                    metadata: {
                        let mut m = serde_json::Map::new();
                        m.insert(
                            "reason".to_string(),
                            serde_json::Value::String(format!(
                                "unknown hook_event_name: {hook_type}"
                            )),
                        );
                        m
                    },
                };
                Ok(vec![schema_error])
            }
        }
    }

    fn validate(&self, input: &Value) -> ValidationResult {
        let obj = match input.as_object() {
            Some(o) => o,
            None => {
                return ValidationResult::Invalid {
                    reason: "input is not a JSON object".to_string(),
                }
            }
        };

        if !obj.contains_key("hook_event_name") {
            return ValidationResult::Invalid {
                reason: "missing hook_event_name".to_string(),
            };
        }

        let hook_type = match obj.get("hook_event_name").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return ValidationResult::Invalid {
                    reason: "hook_event_name is not a string".to_string(),
                }
            }
        };

        let valid = [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "PostToolUseFailure",
            "Notification",
            "Stop",
            "SessionEnd",
        ];

        if valid.contains(&hook_type) {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid {
                reason: format!("unknown hook_event_name: {hook_type}"),
            }
        }
    }

    fn derive_session(&self, input: &Value) -> SessionIdentity {
        let obj = match input.as_object() {
            Some(o) => o,
            None => return SessionIdentity::default(),
        };

        let id = obj
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("unknown-{}", ats_core::new_event_id()));

        let workspace = obj
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let tmux_pane = std::env::var("TMUX_PANE").ok();

        let tty = std::env::var("TTY")
            .ok()
            .or_else(|| std::env::var("SSH_TTY").ok());

        let term_program = std::env::var("TERM_PROGRAM").ok();

        SessionIdentity {
            id,
            parent_id: None,
            workspace,
            terminal: Some(TerminalContext {
                tty,
                term_program,
                tmux_pane,
            }),
        }
    }
}

impl ClaudeAdapter {
    fn evaluate_risk(&self, tool_name: Option<&str>, command: Option<&str>) -> Option<RiskLevel> {
        if tool_name != Some("Bash") {
            return None;
        }
        let cmd = command?;
        self.risk_classifier.classify(cmd)
    }
}

fn tool_input_command(obj: &serde_json::Map<String, Value>) -> Option<String> {
    obj.get("tool_input")
        .and_then(|v| v.as_object())
        .and_then(|ti| ti.get("command"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn categorize_tool(tool_name: Option<&str>) -> ActivityCategory {
    match tool_name {
        Some("Bash") | Some("Shell") => ActivityCategory::Shell,
        Some("Read") | Some("Glob") | Some("Grep") => ActivityCategory::Reading,
        Some("Write") | Some("Edit") => ActivityCategory::Editing,
        Some("WebFetch") | Some("WebSearch") => ActivityCategory::Network,
        Some("Task") | Some("TodoWrite") => ActivityCategory::Thinking,
        Some("BashOutput") | Some("KillShell") => ActivityCategory::Shell,
        Some("NotebookEdit") => ActivityCategory::Editing,
        _ => ActivityCategory::Unknown,
    }
}

fn classify_notification(message: &str) -> EventType {
    let lower = message.to_lowercase();
    if lower.contains("permission") || lower.contains("approval") || lower.contains("allow") {
        EventType::UserApprovalRequired
    } else {
        EventType::UserInputRequired
    }
}

fn detect_version(obj: &serde_json::Map<String, Value>) -> Option<String> {
    obj.get("claude_code_version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_fixture(hook: &str, extra: Value) -> Value {
        let mut base = json!({
            "session_id": "test-session",
            "transcript_path": "/tmp/test.jsonl",
            "cwd": "/tmp/test",
            "hook_event_name": hook,
        });
        if let Value::Object(extra_obj) = extra {
            if let Value::Object(ref mut base_obj) = base {
                for (k, v) in extra_obj {
                    base_obj.insert(k, v);
                }
            }
        }
        base
    }

    #[test]
    fn session_start() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture("SessionStart", json!({"source": "startup"}));
        let events = adapter.parse(input).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::SessionStarted);
        assert_eq!(events[0].session.id, "test-session");
    }

    #[test]
    fn user_prompt_submit() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture(
            "UserPromptSubmit",
            json!({"prompt_id": "abc", "permission_mode": "default"}),
        );
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].event_type, EventType::AgentStarted);
    }

    #[test]
    fn pre_tool_use_bash() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture(
            "PreToolUse",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": "echo hello", "description": "test"},
                "tool_use_id": "tool_1",
                "prompt_id": "abc",
                "permission_mode": "default",
            }),
        );
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].event_type, EventType::ToolStarted);
        let activity = events[0].activity.as_ref().unwrap();
        assert_eq!(activity.tool_name, Some("Bash".to_string()));
        assert_eq!(activity.category, ActivityCategory::Shell);
    }

    #[test]
    fn pre_tool_use_risk_command() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture(
            "PreToolUse",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": "rm -rf /tmp/test", "description": "test"},
                "tool_use_id": "tool_1",
                "prompt_id": "abc",
                "permission_mode": "default",
            }),
        );
        let events = adapter.parse(input).unwrap();
        let activity = events[0].activity.as_ref().unwrap();
        assert!(activity.risk.is_some());
    }

    #[test]
    fn pre_tool_use_benign_command() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture(
            "PreToolUse",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": "echo hello", "description": "test"},
                "tool_use_id": "tool_1",
                "prompt_id": "abc",
                "permission_mode": "default",
            }),
        );
        let events = adapter.parse(input).unwrap();
        let activity = events[0].activity.as_ref().unwrap();
        assert!(activity.risk.is_none());
    }

    #[test]
    fn post_tool_use() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture(
            "PostToolUse",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": "echo done", "description": "test"},
                "tool_use_id": "tool_1",
                "prompt_id": "abc",
                "permission_mode": "default",
                "tool_response": {"stdout": "done"},
                "duration_ms": 100,
            }),
        );
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].event_type, EventType::ToolCompleted);
    }

    #[test]
    fn post_tool_use_failure() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture(
            "PostToolUseFailure",
            json!({
                "tool_name": "Bash",
                "tool_input": {"command": "false", "description": "test"},
                "tool_use_id": "tool_1",
                "prompt_id": "abc",
                "permission_mode": "default",
            }),
        );
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].event_type, EventType::ToolFailed);
    }

    #[test]
    fn notification_permission() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture(
            "Notification",
            json!({"message": "Claude needs your permission"}),
        );
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].event_type, EventType::UserApprovalRequired);
    }

    #[test]
    fn notification_input_required() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture(
            "Notification",
            json!({"message": "Claude is waiting for your input"}),
        );
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].event_type, EventType::UserInputRequired);
    }

    #[test]
    fn stop() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture(
            "Stop",
            json!({"stop_hook_active": false, "prompt_id": "abc", "permission_mode": "default"}),
        );
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].event_type, EventType::AgentCompleted);
    }

    #[test]
    fn session_end() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture("SessionEnd", json!({"reason": "other", "prompt_id": "abc"}));
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].event_type, EventType::SessionStopped);
    }

    #[test]
    fn unknown_hook_emits_schema_error() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture("UnknownHook", json!({}));
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].event_type, EventType::ProviderSchemaError);
    }

    #[test]
    fn missing_hook_event_name_is_error() {
        let adapter = ClaudeAdapter::new();
        let input = json!({"session_id": "test"});
        let result = adapter.parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn non_object_input_is_malformed() {
        let adapter = ClaudeAdapter::new();
        let input = json!("not an object");
        let result = adapter.parse(input);
        assert!(matches!(result, Err(ProviderError::Malformed(_))));
    }

    #[test]
    fn validation_rejects_unknown_hook() {
        let adapter = ClaudeAdapter::new();
        let input = json!({"hook_event_name": "UnknownHook"});
        let result = adapter.validate(&input);
        assert!(!result.is_valid());
    }

    #[test]
    fn validation_accepts_known_hook() {
        let adapter = ClaudeAdapter::new();
        let input = json!({"hook_event_name": "SessionStart"});
        let result = adapter.validate(&input);
        assert!(result.is_valid());
    }

    #[test]
    fn session_derive_from_payload() {
        let adapter = ClaudeAdapter::new();
        let input = json!({
            "session_id": "abc-123",
            "cwd": "/home/user/project",
            "hook_event_name": "SessionStart",
        });
        let session = adapter.derive_session(&input);
        assert_eq!(session.id, "abc-123");
        assert_eq!(session.workspace, Some("/home/user/project".to_string()));
    }

    #[test]
    fn session_fallback_on_missing_session_id() {
        let adapter = ClaudeAdapter::new();
        let input = json!({"hook_event_name": "SessionStart"});
        let session = adapter.derive_session(&input);
        assert!(session.id.starts_with("unknown-"));
    }

    #[test]
    fn schema_version_is_set() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture("SessionStart", json!({"source": "startup"}));
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].schema_version, "1.0");
    }

    #[test]
    fn provider_fields_are_set() {
        let adapter = ClaudeAdapter::new();
        let input = make_fixture("SessionStart", json!({"source": "startup"}));
        let events = adapter.parse(input).unwrap();
        assert_eq!(events[0].provider, "claude");
    }

    #[test]
    fn tool_categorization() {
        assert_eq!(categorize_tool(Some("Bash")), ActivityCategory::Shell);
        assert_eq!(categorize_tool(Some("Read")), ActivityCategory::Reading);
        assert_eq!(categorize_tool(Some("Write")), ActivityCategory::Editing);
        assert_eq!(categorize_tool(Some("Edit")), ActivityCategory::Editing);
        assert_eq!(categorize_tool(Some("Glob")), ActivityCategory::Reading);
        assert_eq!(categorize_tool(Some("Grep")), ActivityCategory::Reading);
        assert_eq!(categorize_tool(Some("WebFetch")), ActivityCategory::Network);
        assert_eq!(
            categorize_tool(Some("WebSearch")),
            ActivityCategory::Network
        );
        assert_eq!(categorize_tool(Some("Task")), ActivityCategory::Thinking);
        assert_eq!(
            categorize_tool(Some("TodoWrite")),
            ActivityCategory::Thinking
        );
        assert_eq!(categorize_tool(None), ActivityCategory::Unknown);
        assert_eq!(
            categorize_tool(Some("UnknownTool")),
            ActivityCategory::Unknown
        );
    }

    #[test]
    fn notification_classification() {
        assert_eq!(
            classify_notification("Claude needs your permission"),
            EventType::UserApprovalRequired
        );
        assert_eq!(
            classify_notification("waiting for approval"),
            EventType::UserApprovalRequired
        );
        assert_eq!(
            classify_notification("allow this action?"),
            EventType::UserApprovalRequired
        );
        assert_eq!(
            classify_notification("Claude is waiting for your input"),
            EventType::UserInputRequired
        );
    }
}
