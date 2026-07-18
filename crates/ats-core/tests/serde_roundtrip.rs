use ats_core::{
    Activity, ActivityCategory, ActivityLabel, AgentState, EventType, NormalizedEvent, RiskLevel,
    SessionIdentity, TerminalContext,
};
use serde_json::{json, Value};

/// SPEC §6.1 の Normalized Event サンプル。
fn spec_sample() -> Value {
    json!({
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
    })
}

#[test]
fn normalized_event_roundtrips_spec_sample() {
    let event: NormalizedEvent = serde_json::from_value(spec_sample()).expect("deserialize");
    let back = serde_json::to_value(&event).expect("serialize");
    assert_eq!(back, spec_sample());
}

#[test]
fn normalized_event_deserializes_with_only_required_fields() {
    let minimal = json!({
      "schema_version": "1.0",
      "event_id": "018f2b70-5f14-7fb7-a880-123456789abc",
      "timestamp": "2026-07-18T07:15:31Z",
      "provider": "claude",
      "event_type": "session.started",
      "session": { "id": "s-1" }
    });
    let event: NormalizedEvent = serde_json::from_value(minimal).expect("deserialize minimal");
    assert_eq!(event.event_type, EventType::SessionStarted);
    assert_eq!(event.session.id, "s-1");
    assert!(event.session.parent_id.is_none());
    assert!(event.activity.is_none());
    assert!(event.metadata.is_empty());

    let back = serde_json::to_value(&event).expect("serialize");
    let reparsed: NormalizedEvent = serde_json::from_value(back).expect("roundtrip");
    assert_eq!(event, reparsed);
}

#[test]
fn agent_state_serializes_lowercase_and_roundtrips() {
    let states = [
        (AgentState::Idle, "idle"),
        (AgentState::Working, "working"),
        (AgentState::Attention, "attention"),
        (AgentState::Risk, "risk"),
        (AgentState::Result, "result"),
        (AgentState::Error, "error"),
        (AgentState::Unknown, "unknown"),
    ];
    for (state, expected) in states {
        let s = serde_json::to_value(state).expect("serialize");
        assert_eq!(s, json!(expected));
        let back: AgentState = serde_json::from_value(s).expect("deserialize");
        assert_eq!(back, state);
    }
}

#[test]
fn event_type_covers_full_spec_taxonomy_and_roundtrips() {
    let cases = [
        (EventType::SessionStarted, "session.started"),
        (EventType::SessionStopped, "session.stopped"),
        (EventType::SessionFailed, "session.failed"),
        (EventType::SessionHeartbeat, "session.heartbeat"),
        (EventType::AgentStarted, "agent.started"),
        (EventType::AgentWorking, "agent.working"),
        (EventType::AgentWaiting, "agent.waiting"),
        (EventType::AgentCompleted, "agent.completed"),
        (EventType::AgentFailed, "agent.failed"),
        (EventType::ToolStarted, "tool.started"),
        (EventType::ToolCompleted, "tool.completed"),
        (EventType::ToolFailed, "tool.failed"),
        (EventType::UserInputRequired, "user.input_required"),
        (EventType::UserApprovalRequired, "user.approval_required"),
        (EventType::UserInputReceived, "user.input_received"),
        (EventType::ProviderDisconnected, "provider.disconnected"),
        (EventType::ProviderSchemaError, "provider.schema_error"),
        (EventType::RendererFailed, "renderer.failed"),
        (EventType::SessionTimeout, "session.timeout"),
    ];
    for (event_type, expected) in cases {
        let s = serde_json::to_value(event_type).expect("serialize");
        assert_eq!(s, json!(expected));
        let back: EventType = serde_json::from_value(s).expect("deserialize");
        assert_eq!(back, event_type);
    }
}

#[test]
fn activity_category_and_risk_level_roundtrip() {
    let categories = [
        (ActivityCategory::Thinking, "thinking"),
        (ActivityCategory::Reading, "reading"),
        (ActivityCategory::Searching, "searching"),
        (ActivityCategory::Editing, "editing"),
        (ActivityCategory::Shell, "shell"),
        (ActivityCategory::Testing, "testing"),
        (ActivityCategory::Network, "network"),
        (ActivityCategory::VersionControl, "version_control"),
        (ActivityCategory::Deployment, "deployment"),
        (ActivityCategory::Unknown, "unknown"),
    ];
    for (category, expected) in categories {
        let s = serde_json::to_value(category).expect("serialize");
        assert_eq!(s, json!(expected));
        let back: ActivityCategory = serde_json::from_value(s).expect("deserialize");
        assert_eq!(back, category);
    }

    let risks = [
        (RiskLevel::Low, "low"),
        (RiskLevel::Medium, "medium"),
        (RiskLevel::High, "high"),
        (RiskLevel::Critical, "critical"),
        (RiskLevel::Unknown, "unknown"),
    ];
    for (risk, expected) in risks {
        let s = serde_json::to_value(risk).expect("serialize");
        assert_eq!(s, json!(expected));
        let back: RiskLevel = serde_json::from_value(s).expect("deserialize");
        assert_eq!(back, risk);
    }
}

#[test]
fn session_identity_and_terminal_context_roundtrip() {
    let session = SessionIdentity {
        id: "s-42".to_string(),
        parent_id: Some("s-1".to_string()),
        workspace: Some("/tmp/ws".to_string()),
        terminal: TerminalContext {
            tty: Some("/dev/ttys004".to_string()),
            term_program: Some("iTerm.app".to_string()),
            tmux_pane: Some("%12".to_string()),
        },
    };
    let s = serde_json::to_value(&session).expect("serialize");
    let back: SessionIdentity = serde_json::from_value(s).expect("deserialize");
    assert_eq!(back, session);
}

#[test]
fn activity_roundtrips() {
    let activity = Activity {
        category: ActivityCategory::Testing,
        label: Some(ActivityLabel::new("Running tests")),
        tool_name: Some("Bash".to_string()),
        risk: Some(RiskLevel::Low),
    };
    let s = serde_json::to_value(&activity).expect("serialize");
    let back: Activity = serde_json::from_value(s).expect("deserialize");
    assert_eq!(back, activity);
}

#[test]
fn timestamp_serializes_as_rfc3339_utc() {
    let event: NormalizedEvent = serde_json::from_value(spec_sample()).expect("deserialize sample");
    let back = serde_json::to_value(&event).expect("serialize");
    let ts = back["timestamp"].as_str().expect("timestamp string");
    assert!(
        ts.ends_with('Z'),
        "timestamp must be UTC with Z suffix: {ts}"
    );
    assert!(
        ts.starts_with("2026-07-18T07:15:31"),
        "timestamp value preserved: {ts}"
    );
}
