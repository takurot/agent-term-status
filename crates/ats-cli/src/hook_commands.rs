use std::io::Read;

use ats_core::{
    new_event_id, now_utc, Activity, ActivityCategory, ActivityLabel, AgentState, EventType,
    NormalizedEvent, SessionIdentity, TerminalContext,
};

use crate::socket_client;

pub fn run_ingest(provider: &str) {
    let mut input = String::new();
    if std::io::stdin()
        .take(65536)
        .read_to_string(&mut input)
        .is_err()
    {
        return;
    }

    let value: serde_json::Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return,
    };

    let event = match build_event(provider, &value) {
        Ok(e) => e,
        Err(_) => return,
    };

    let payload = match serde_json::to_vec(&event) {
        Ok(p) => p,
        Err(_) => return,
    };

    let _ = socket_client::send_frame_to_daemon(&payload);
}

#[allow(dead_code)]
pub fn run_event(state_str: &str, activity_label: Option<&str>, session_id: Option<&str>) {
    let state: AgentState =
        match serde_json::from_value(serde_json::Value::String(state_str.to_lowercase())) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("unknown state: {state_str}");
                return;
            }
        };

    let event = build_manual_event(state, activity_label, session_id);
    let payload = match serde_json::to_vec(&event) {
        Ok(p) => p,
        Err(_) => return,
    };

    let sent = socket_client::send_frame_to_daemon(&payload).is_ok();

    if !sent {
        run_standalone_render(state, activity_label);
    }
}

#[allow(dead_code)]
pub fn run_reset(all: bool, session_id: Option<&str>) {
    let target = if all {
        "all".to_string()
    } else if let Some(id) = session_id {
        id.to_string()
    } else {
        eprintln!("Specify --all or --session <id>");
        return;
    };

    let event = NormalizedEvent {
        schema_version: "1.0".to_string(),
        event_id: new_event_id(),
        timestamp: now_utc(),
        provider: "cli".to_string(),
        provider_version: None,
        event_type: EventType::SessionTimeout,
        session: SessionIdentity {
            id: target.clone(),
            parent_id: None,
            workspace: None,
            terminal: None,
        },
        activity: Some(Activity {
            category: ActivityCategory::Unknown,
            label: Some(ActivityLabel::new("reset")),
            tool_name: None,
            risk: None,
        }),
        metadata: Default::default(),
    };

    let payload = match serde_json::to_vec(&event) {
        Ok(p) => p,
        Err(_) => return,
    };

    let sent = socket_client::send_frame_to_daemon(&payload).is_ok();
    if !sent {
        eprintln!("Daemon not reachable. Nothing to reset.");
    } else {
        println!("Reset sent: {target}");
    }
}

fn build_event(provider: &str, value: &serde_json::Value) -> Result<NormalizedEvent, ()> {
    let mut event: NormalizedEvent = serde_json::from_value(value.clone()).map_err(|_| ())?;
    event.provider = provider.to_string();
    if event.event_id.is_nil() {
        event.event_id = new_event_id();
    }
    if event.schema_version.is_empty() {
        event.schema_version = "1.0".to_string();
    }
    Ok(event)
}

#[allow(dead_code)]
fn build_manual_event(
    state: AgentState,
    activity_label: Option<&str>,
    session_id: Option<&str>,
) -> NormalizedEvent {
    let event_type = state_to_event_type(state);

    NormalizedEvent {
        schema_version: "1.0".to_string(),
        event_id: new_event_id(),
        timestamp: now_utc(),
        provider: "cli".to_string(),
        provider_version: None,
        event_type,
        session: SessionIdentity {
            id: session_id.unwrap_or("default").to_string(),
            parent_id: None,
            workspace: None,
            terminal: Some(TerminalContext {
                tty: std::env::var("TTY").ok(),
                term_program: std::env::var("TERM_PROGRAM").ok(),
                tmux_pane: std::env::var("TMUX_PANE").ok(),
            }),
        },
        activity: activity_label.map(|label| Activity {
            category: ActivityCategory::Unknown,
            label: Some(ActivityLabel::new(label)),
            tool_name: None,
            risk: None,
        }),
        metadata: Default::default(),
    }
}

#[allow(dead_code)]
fn state_to_event_type(state: AgentState) -> EventType {
    match state {
        AgentState::Working => EventType::AgentWorking,
        AgentState::Attention => EventType::AgentWaiting,
        AgentState::Risk => EventType::AgentFailed,
        AgentState::Result => EventType::AgentCompleted,
        AgentState::Error => EventType::AgentFailed,
        AgentState::Unknown => EventType::AgentFailed,
        AgentState::Idle => EventType::SessionStopped,
    }
}

#[allow(dead_code)]
fn run_standalone_render(state: AgentState, _activity_label: Option<&str>) {
    let state_str = match state {
        AgentState::Idle => "idle",
        AgentState::Working => "working",
        AgentState::Attention => "attention",
        AgentState::Risk => "risk",
        AgentState::Result => "result",
        AgentState::Error => "error",
        AgentState::Unknown => "unknown",
    };

    crate::event_prototype::run(state_str);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_event_adds_missing_fields() {
        let value = serde_json::json!({
            "schema_version": "1.0",
            "event_id": "00000000-0000-0000-0000-000000000000",
            "timestamp": "2025-01-01T00:00:00Z",
            "provider": "",
            "event_type": "agent.working",
            "session": {
                "id": "test-session"
            }
        });
        let event = build_event("test-provider", &value).unwrap();
        assert_eq!(event.provider, "test-provider");
        assert!(!event.event_id.is_nil());
    }

    #[test]
    fn build_event_rejects_invalid_json() {
        let value = serde_json::json!({"not": "an event"});
        assert!(build_event("test", &value).is_err());
    }

    #[test]
    fn build_manual_event_creates_valid_event() {
        let event = build_manual_event(AgentState::Working, Some("testing"), Some("s1"));
        assert_eq!(event.provider, "cli");
        assert_eq!(event.session.id, "s1");
        assert!(event.activity.is_some());
    }

    #[test]
    fn state_to_event_type_maps_all_states() {
        use AgentState::*;
        for state in [Idle, Working, Attention, Risk, Result, Error, Unknown] {
            let _ = state_to_event_type(state);
        }
    }
}
