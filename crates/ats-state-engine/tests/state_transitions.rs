use ats_core::{
    new_event_id, Activity, ActivityCategory, AgentState, EventType, NormalizedEvent, RiskLevel,
    SessionIdentity, SCHEMA_VERSION,
};
use ats_state_engine::StateEngine;
use chrono::{DateTime, Duration, Utc};
use serde_json::Map;

fn make_event(
    event_type: EventType,
    session_id: &str,
    timestamp: DateTime<Utc>,
) -> NormalizedEvent {
    NormalizedEvent {
        schema_version: SCHEMA_VERSION.to_string(),
        event_id: new_event_id(),
        timestamp,
        provider: "test".to_string(),
        provider_version: None,
        event_type,
        session: SessionIdentity {
            id: session_id.to_string(),
            ..Default::default()
        },
        activity: None,
        metadata: Map::new(),
    }
}

fn make_event_with_risk(
    event_type: EventType,
    session_id: &str,
    risk: RiskLevel,
    timestamp: DateTime<Utc>,
) -> NormalizedEvent {
    let mut event = make_event(event_type, session_id, timestamp);
    event.activity = Some(Activity {
        category: ActivityCategory::Unknown,
        label: None,
        tool_name: None,
        risk: Some(risk),
    });
    event
}

fn make_event_with_parent(
    event_type: EventType,
    session_id: &str,
    parent_id: &str,
    timestamp: DateTime<Utc>,
) -> NormalizedEvent {
    let mut event = make_event(event_type, session_id, timestamp);
    event.session.parent_id = Some(parent_id.to_string());
    event
}

fn now() -> DateTime<Utc> {
    "2024-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
}

mod state_transitions {
    use super::*;

    #[test]
    fn idle_to_working_on_agent_started() {
        let mut engine = StateEngine::new();
        let t = now();
        let event = make_event(EventType::AgentStarted, "s1", t);
        let transition = engine.ingest(&event, t).unwrap();
        assert_eq!(transition.previous, AgentState::Idle);
        assert_eq!(transition.new, AgentState::Working);
        assert_eq!(engine.get_state("s1"), Some(AgentState::Working));
    }

    #[test]
    fn working_stays_working_on_agent_working() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine.ingest(&make_event(EventType::AgentWorking, "s1", t), t);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Working));
    }

    #[test]
    fn working_to_attention_on_input_required() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine
            .ingest(&make_event(EventType::UserInputRequired, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Working);
        assert_eq!(transition.new, AgentState::Attention);
    }

    #[test]
    fn working_to_attention_on_approval_required() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine
            .ingest(&make_event(EventType::UserApprovalRequired, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Working);
        assert_eq!(transition.new, AgentState::Attention);
    }

    #[test]
    fn attention_to_risk_on_high_risk_approval() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserApprovalRequired, "s1", t), t);
        let transition = engine
            .ingest(
                &make_event_with_risk(EventType::UserApprovalRequired, "s1", RiskLevel::High, t),
                t,
            )
            .unwrap();
        assert_eq!(transition.previous, AgentState::Attention);
        assert_eq!(transition.new, AgentState::Risk);
    }

    #[test]
    fn attention_to_risk_on_critical_risk_approval() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserApprovalRequired, "s1", t), t);
        let transition = engine
            .ingest(
                &make_event_with_risk(
                    EventType::UserApprovalRequired,
                    "s1",
                    RiskLevel::Critical,
                    t,
                ),
                t,
            )
            .unwrap();
        assert_eq!(transition.previous, AgentState::Attention);
        assert_eq!(transition.new, AgentState::Risk);
    }

    #[test]
    fn working_to_result_on_agent_completed() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine
            .ingest(&make_event(EventType::AgentCompleted, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Working);
        assert_eq!(transition.new, AgentState::Result);
    }

    #[test]
    fn any_state_to_error_on_agent_failed() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine
            .ingest(&make_event(EventType::AgentFailed, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Working);
        assert_eq!(transition.new, AgentState::Error);
    }

    #[test]
    fn idle_to_error_on_session_failed() {
        let mut engine = StateEngine::new();
        let t = now();
        let transition = engine
            .ingest(&make_event(EventType::SessionFailed, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Idle);
        assert_eq!(transition.new, AgentState::Error);
    }

    #[test]
    fn idle_stays_idle_on_session_started() {
        let mut engine = StateEngine::new();
        let t = now();
        let transition = engine.ingest(&make_event(EventType::SessionStarted, "s1", t), t);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Idle));
    }

    #[test]
    fn working_stays_working_on_tool_started() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine.ingest(&make_event(EventType::ToolStarted, "s1", t), t);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Working));
    }

    #[test]
    fn working_to_error_on_tool_failed() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine
            .ingest(&make_event(EventType::ToolFailed, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Working);
        assert_eq!(transition.new, AgentState::Error);
    }

    #[test]
    fn session_stopped_from_working() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine.ingest(&make_event(EventType::SessionStopped, "s1", t), t);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Working));
    }

    #[test]
    fn session_timeout_produces_unknown() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine
            .ingest(&make_event(EventType::SessionTimeout, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Working);
        assert_eq!(transition.new, AgentState::Unknown);
    }

    #[test]
    fn provider_disconnected_produces_error() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine
            .ingest(&make_event(EventType::ProviderDisconnected, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Working);
        assert_eq!(transition.new, AgentState::Error);
    }

    #[test]
    fn provider_schema_error_produces_error() {
        let mut engine = StateEngine::new();
        let t = now();
        let transition = engine
            .ingest(&make_event(EventType::ProviderSchemaError, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Idle);
        assert_eq!(transition.new, AgentState::Error);
    }

    #[test]
    fn attention_to_working_on_input_received() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserInputRequired, "s1", t), t);
        let transition = engine
            .ingest(&make_event(EventType::UserInputReceived, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Attention);
        assert_eq!(transition.new, AgentState::Working);
    }

    #[test]
    fn attention_persists_over_working_events() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserApprovalRequired, "s1", t), t);
        let transition = engine.ingest(&make_event(EventType::AgentWorking, "s1", t), t);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Attention));
    }

    #[test]
    fn risk_persists_over_working_events() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(
            &make_event_with_risk(
                EventType::UserApprovalRequired,
                "s1",
                RiskLevel::Critical,
                t,
            ),
            t,
        );
        let transition = engine.ingest(&make_event(EventType::AgentWorking, "s1", t), t);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Risk));
    }

    #[test]
    fn error_provider_disconnect_does_not_override_attention() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserInputRequired, "s1", t), t);
        let transition = engine.ingest(&make_event(EventType::ProviderDisconnected, "s1", t), t);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Attention));
    }

    #[test]
    fn error_from_non_provider_source_overrides_attention() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserInputRequired, "s1", t), t);
        let transition = engine
            .ingest(&make_event(EventType::AgentFailed, "s1", t), t)
            .unwrap();
        assert_eq!(transition.previous, AgentState::Attention);
        assert_eq!(transition.new, AgentState::Error);
    }

    #[test]
    fn error_persists_over_lower_priority_events() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::AgentFailed, "s1", t), t);
        let transition = engine.ingest(&make_event(EventType::AgentWorking, "s1", t), t);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Error));
    }

    #[test]
    fn error_resumes_attention_when_provider_reconnects() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserInputRequired, "s1", t), t);
        engine.ingest(&make_event(EventType::ProviderDisconnected, "s1", t), t);
        let transition = engine.ingest(&make_event(EventType::UserApprovalRequired, "s1", t), t);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Attention));
    }

    #[test]
    fn session_heartbeat_does_not_change_state() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        let transition = engine.ingest(&make_event(EventType::SessionHeartbeat, "s1", t), t);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Working));
    }
}

mod idempotency {
    use super::*;

    #[test]
    fn same_event_id_twice_is_noop() {
        let mut engine = StateEngine::new();
        let t = now();
        let event = make_event(EventType::AgentStarted, "s1", t);
        let first = engine.ingest(&event, t);
        assert!(first.is_some());

        let second = engine.ingest(&event, t);
        assert!(second.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Working));
    }

    #[test]
    fn idempotent_replay_does_not_corrupt_state() {
        let mut engine = StateEngine::new();
        let t = now();
        let event1 = make_event(EventType::AgentStarted, "s1", t);
        let event2 = make_event(EventType::UserInputRequired, "s1", t);

        engine.ingest(&event1, t);
        engine.ingest(&event2, t);
        assert_eq!(engine.get_state("s1"), Some(AgentState::Attention));

        engine.ingest(&event1, t);
        engine.ingest(&event2, t);
        assert_eq!(engine.get_state("s1"), Some(AgentState::Attention));
    }

    #[test]
    fn duplicate_event_id_produces_no_transition() {
        let mut engine = StateEngine::new();
        let t = now();
        let event = make_event(EventType::AgentStarted, "s1", t);

        let first = engine.ingest(&event, t);
        assert!(first.is_some());

        let second = engine.ingest(&event, t);
        assert!(second.is_none());
    }
}

mod ttl {
    use super::*;

    #[test]
    fn working_expires_to_unknown_then_idle() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);

        let t1 = t + Duration::minutes(31);
        let transitions = engine.expire_ttls(t1);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].previous, AgentState::Working);
        assert_eq!(transitions[0].new, AgentState::Unknown);
        assert_eq!(engine.get_state("s1"), Some(AgentState::Unknown));

        let t2 = t1 + Duration::seconds(31);
        let transitions = engine.expire_ttls(t2);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].previous, AgentState::Unknown);
        assert_eq!(transitions[0].new, AgentState::Idle);
        assert_eq!(engine.get_state("s1"), None);
    }

    #[test]
    fn attention_expires_to_unknown_then_idle() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserInputRequired, "s1", t), t);

        let t1 = t + Duration::hours(5);
        let transitions = engine.expire_ttls(t1);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].previous, AgentState::Attention);
        assert_eq!(transitions[0].new, AgentState::Unknown);
    }

    #[test]
    fn risk_expires_to_unknown_then_idle() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(
            &make_event_with_risk(
                EventType::UserApprovalRequired,
                "s1",
                RiskLevel::Critical,
                t,
            ),
            t,
        );

        let t1 = t + Duration::minutes(31);
        let transitions = engine.expire_ttls(t1);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].previous, AgentState::Risk);
        assert_eq!(transitions[0].new, AgentState::Unknown);
    }

    #[test]
    fn result_expires_to_unknown_then_idle() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::AgentCompleted, "s1", t), t);

        let t1 = t + Duration::seconds(9);
        let transitions = engine.expire_ttls(t1);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].previous, AgentState::Result);
        assert_eq!(transitions[0].new, AgentState::Unknown);
    }

    #[test]
    fn error_expires_to_unknown_then_idle() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::AgentFailed, "s1", t), t);

        let t1 = t + Duration::seconds(61);
        let transitions = engine.expire_ttls(t1);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].previous, AgentState::Error);
        assert_eq!(transitions[0].new, AgentState::Unknown);
    }

    #[test]
    fn ttl_precision_within_one_second() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentCompleted, "s1", t), t);

        let t_before = t + Duration::seconds(7);
        let transitions = engine.expire_ttls(t_before);
        assert_eq!(transitions.len(), 0);

        let t_after = t + Duration::seconds(9);
        let transitions = engine.expire_ttls(t_after);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].new, AgentState::Unknown);
    }

    #[test]
    fn ttl_never_goes_directly_to_idle() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);

        let t1 = t + Duration::minutes(35);
        let transitions = engine.expire_ttls(t1);
        assert!(!transitions.is_empty());
        assert_eq!(transitions[0].new, AgentState::Unknown);
    }

    #[test]
    fn unknown_ttl_expires_to_idle() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::SessionTimeout, "s1", t), t);

        let t1 = t + Duration::seconds(31);
        let transitions = engine.expire_ttls(t1);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].previous, AgentState::Unknown);
        assert_eq!(transitions[0].new, AgentState::Idle);
    }

    #[test]
    fn idle_never_expires() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::SessionStarted, "s1", t), t);

        let t1 = t + Duration::hours(100);
        let transitions = engine.expire_ttls(t1);
        assert_eq!(transitions.len(), 0);
    }

    #[test]
    fn multiple_sessions_expire_independently() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentCompleted, "s1", t), t);
        engine.ingest(&make_event(EventType::AgentCompleted, "s2", t), t);

        let t1 = t + Duration::seconds(9);
        let transitions = engine.expire_ttls(t1);
        assert_eq!(transitions.len(), 2);
        for txn in &transitions {
            assert_eq!(txn.new, AgentState::Unknown);
        }
    }
}

mod parent_subagent {
    use super::*;

    #[test]
    fn child_transition_updates_parent_aggregation() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(
            &make_event_with_parent(EventType::AgentStarted, "child1", "parent1", t),
            t,
        );
        let aggregated = engine.get_aggregated_state("parent1");
        assert_eq!(aggregated, Some(AgentState::Working));
    }

    #[test]
    fn parent_aggregation_uses_max_priority_of_children() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(
            &make_event_with_parent(EventType::AgentStarted, "child1", "parent1", t),
            t,
        );
        engine.ingest(
            &make_event_with_parent(EventType::UserInputRequired, "child2", "parent1", t),
            t,
        );
        let aggregated = engine.get_aggregated_state("parent1");
        assert_eq!(aggregated, Some(AgentState::Attention));
    }

    #[test]
    fn parent_aggregation_returns_none_without_children() {
        let engine = StateEngine::new();
        assert_eq!(engine.get_aggregated_state("no-such-parent"), None);
    }

    #[test]
    fn risk_child_makes_parent_risk() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(
            &make_event_with_parent(EventType::AgentStarted, "child1", "parent1", t),
            t,
        );
        engine.ingest(
            &make_event_with_parent(EventType::UserApprovalRequired, "child2", "parent1", t),
            t,
        );
        engine.ingest(
            &make_event_with_risk(
                EventType::UserApprovalRequired,
                "child2",
                RiskLevel::Critical,
                t,
            ),
            t,
        );
        let aggregated = engine.get_aggregated_state("parent1");
        assert_eq!(aggregated, Some(AgentState::Risk));
    }
}

mod heartbeat {
    use super::*;

    #[test]
    fn heartbeat_extends_ttl() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);

        let t_mid = t + Duration::minutes(15);
        engine.heartbeat("s1", t_mid);

        let t_before_original = t + Duration::minutes(29);
        let transitions = engine.expire_ttls(t_before_original);
        assert!(transitions.is_empty());

        let t_after_extension = t_mid + Duration::minutes(29);
        let transitions = engine.expire_ttls(t_after_extension);
        assert!(transitions.is_empty());

        let t_expire = t_mid + Duration::minutes(31);
        let transitions = engine.expire_ttls(t_expire);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].new, AgentState::Unknown);
    }
}

mod out_of_order {
    use super::*;

    #[test]
    fn out_of_order_events_dont_corrupt_state() {
        let mut engine = StateEngine::new();
        let t = now();
        let t_late = t - Duration::seconds(10);

        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        assert_eq!(engine.get_state("s1"), Some(AgentState::Working));

        let transition = engine.ingest(&make_event(EventType::AgentStarted, "s1", t_late), t_late);
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Working));
    }
}

mod multi_session_isolation {
    use super::*;

    #[test]
    fn sessions_dont_leak_state() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserInputRequired, "s2", t), t);

        assert_eq!(engine.get_state("s1"), Some(AgentState::Working));
        assert_eq!(engine.get_state("s2"), Some(AgentState::Attention));
    }

    #[test]
    fn session_count_tracks_active_sessions() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::AgentStarted, "s2", t), t);
        assert_eq!(engine.session_count(), 2);
    }
}

mod risk_transitions {
    use super::*;

    #[test]
    fn low_risk_does_not_escalate_to_risk_state() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserApprovalRequired, "s1", t), t);
        let transition = engine.ingest(
            &make_event_with_risk(EventType::UserApprovalRequired, "s1", RiskLevel::Low, t),
            t,
        );
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Attention));
    }

    #[test]
    fn medium_risk_does_not_escalate_to_risk_state() {
        let mut engine = StateEngine::new();
        let t = now();
        engine.ingest(&make_event(EventType::AgentStarted, "s1", t), t);
        engine.ingest(&make_event(EventType::UserApprovalRequired, "s1", t), t);
        let transition = engine.ingest(
            &make_event_with_risk(EventType::UserApprovalRequired, "s1", RiskLevel::Medium, t),
            t,
        );
        assert!(transition.is_none());
        assert_eq!(engine.get_state("s1"), Some(AgentState::Attention));
    }
}
