use ats_core::{
    new_event_id, Activity, ActivityCategory, AgentState, EventType, NormalizedEvent, RiskLevel,
    SessionIdentity, SCHEMA_VERSION,
};
use ats_state_engine::StateEngine;
use chrono::{DateTime, Duration, Utc};
use proptest::prelude::*;
use serde_json::Map;

fn event_type_strategy() -> impl Strategy<Value = EventType> {
    prop_oneof![
        Just(EventType::AgentStarted),
        Just(EventType::AgentWorking),
        Just(EventType::AgentWaiting),
        Just(EventType::AgentCompleted),
        Just(EventType::AgentFailed),
        Just(EventType::SessionStarted),
        Just(EventType::SessionStopped),
        Just(EventType::SessionFailed),
        Just(EventType::SessionHeartbeat),
        Just(EventType::SessionTimeout),
        Just(EventType::ToolStarted),
        Just(EventType::ToolCompleted),
        Just(EventType::ToolFailed),
        Just(EventType::UserInputRequired),
        Just(EventType::UserApprovalRequired),
        Just(EventType::UserInputReceived),
        Just(EventType::ProviderDisconnected),
        Just(EventType::ProviderSchemaError),
        Just(EventType::RendererFailed),
    ]
}

fn risk_level_strategy() -> impl Strategy<Value = Option<RiskLevel>> {
    prop_oneof![
        5 => Just(None),
        1 => Just(Some(RiskLevel::Low)),
        1 => Just(Some(RiskLevel::Medium)),
        1 => Just(Some(RiskLevel::High)),
        1 => Just(Some(RiskLevel::Critical)),
    ]
}

fn event_with_meta(
    event_type: EventType,
    session_id: &str,
    risk: Option<RiskLevel>,
    timestamp: DateTime<Utc>,
) -> NormalizedEvent {
    let activity = risk.map(|r| Activity {
        category: ActivityCategory::Unknown,
        label: None,
        tool_name: None,
        risk: Some(r),
    });
    NormalizedEvent {
        schema_version: SCHEMA_VERSION.to_string(),
        event_id: new_event_id(),
        timestamp,
        provider: "proptest".to_string(),
        provider_version: None,
        event_type,
        session: SessionIdentity {
            id: session_id.to_string(),
            ..Default::default()
        },
        activity,
        metadata: Map::new(),
    }
}

proptest! {
    #[test]
    fn duplicate_event_id_always_idempotent(
        event_type in event_type_strategy(),
        risk in risk_level_strategy(),
    ) {
        let mut engine = StateEngine::new();
        let t: DateTime<Utc> = "2024-01-01T00:00:00Z".parse().unwrap();
        let event = event_with_meta(event_type, "s1", risk, t);

        let first = engine.ingest(&event, t);
        let second = engine.ingest(&event, t);

        assert!(second.is_none(), "duplicate event_id must produce no transition");
        if let Some(txn) = first {
            assert_eq!(engine.get_state("s1"), Some(txn.new),
                "state after first ingest must equal transition.new");
        }
    }

    #[test]
    fn priority_order_is_respected(
        event_types in prop::collection::vec(event_type_strategy(), 1..20),
        risks in prop::collection::vec(risk_level_strategy(), 1..20),
    ) {
        let mut engine = StateEngine::new();
        let t: DateTime<Utc> = "2024-01-01T00:00:00Z".parse().unwrap();

        let events: Vec<_> = event_types.iter()
            .zip(risks.iter())
            .enumerate()
            .map(|(i, (&et, &risk))| {
                let ts = t + Duration::seconds(i as i64);
                event_with_meta(et, "s1", risk, ts)
            })
            .collect();

        for event in &events {
            engine.ingest(event, event.timestamp);
        }

        // The state should never be UNKNOWN unless session.timeout occurred
        // and no higher-prio event came after
        if let Some(state) = engine.get_state("s1") {
            assert!(state != AgentState::Unknown || events.iter().any(|e| e.event_type == EventType::SessionTimeout),
                "state must not be UNKNOWN unless SessionTimeout occurred");
        }
    }

    #[test]
    fn ttl_always_reaches_idle_through_unknown(
        event_type in event_type_strategy(),
        risk in risk_level_strategy(),
    ) {
        let mut engine = StateEngine::new();
        let t: DateTime<Utc> = "2024-01-01T00:00:00Z".parse().unwrap();
        let event = event_with_meta(event_type, "s1", risk, t);

        engine.ingest(&event, t);

        // IDLE has no TTL — invariant is trivially satisfied
        if engine.get_state("s1").is_none_or(|s| s == AgentState::Idle) {
            return Ok(());
        }

        let mut reached_idle = false;
        for step in 0..12 {
            let far = t + Duration::days(step + 1);
            let transitions = engine.expire_ttls(far);
            for txn in &transitions {
                if txn.new == AgentState::Idle {
                    assert_eq!(txn.previous, AgentState::Unknown,
                        "state must go through UNKNOWN before IDLE");
                    reached_idle = true;
                }
            }
            if reached_idle {
                break;
            }
        }
        assert!(reached_idle, "TTL did not reach IDLE within 12 steps");
    }

    #[test]
    fn event_id_is_preserved_across_replay(
        event_types in prop::collection::vec(event_type_strategy(), 1..30),
        risks in prop::collection::vec(risk_level_strategy(), 1..30),
    ) {
        let mut engine = StateEngine::new();
        let t: DateTime<Utc> = "2024-01-01T00:00:00Z".parse().unwrap();

        let events: Vec<_> = event_types.iter()
            .zip(risks.iter())
            .enumerate()
            .map(|(i, (&et, &risk))| {
                let ts = t + Duration::seconds(i as i64);
                event_with_meta(et, "s1", risk, ts)
            })
            .collect();

        // Apply all events once
        for event in &events {
            engine.ingest(event, event.timestamp);
        }
        let state_after_first = engine.get_state("s1");

        // Replay all events
        for event in &events {
            let result = engine.ingest(event, event.timestamp);
            assert!(result.is_none(), "replayed event_id must be idempotent");
        }

        assert_eq!(engine.get_state("s1"), state_after_first,
            "state after replay must equal state after first application");
    }

    #[test]
    fn sessions_are_isolated(
        events_s1 in prop::collection::vec(event_type_strategy(), 0..10),
        events_s2 in prop::collection::vec(event_type_strategy(), 0..10),
        risks_s1 in prop::collection::vec(risk_level_strategy(), 0..10),
        risks_s2 in prop::collection::vec(risk_level_strategy(), 0..10),
    ) {
        let mut engine = StateEngine::new();
        let t: DateTime<Utc> = "2024-01-01T00:00:00Z".parse().unwrap();

        let events_a: Vec<_> = events_s1.iter()
            .zip(risks_s1.iter())
            .enumerate()
            .map(|(i, (&et, &risk))| {
                event_with_meta(et, "A", risk, t + Duration::seconds(i as i64))
            })
            .collect();

        let events_b: Vec<_> = events_s2.iter()
            .zip(risks_s2.iter())
            .enumerate()
            .map(|(i, (&et, &risk))| {
                event_with_meta(et, "B", risk, t + Duration::seconds(i as i64))
            })
            .collect();

        // Apply only session A
        for event in &events_a {
            engine.ingest(event, event.timestamp);
        }
        let state_a = engine.get_state("A");

        // Apply session B
        for event in &events_b {
            engine.ingest(event, event.timestamp);
        }

        // Session A state must not be affected by session B
        assert_eq!(engine.get_state("A"), state_a,
            "session A state must not be affected by session B events");
    }
}
