use ats_core::{AgentState, EventType};
use chrono::Duration;

pub fn priority(state: AgentState) -> u8 {
    match state {
        AgentState::Risk => 7,
        AgentState::Attention => 6,
        AgentState::Error => 5,
        AgentState::Result => 4,
        AgentState::Working => 3,
        AgentState::Idle => 2,
        AgentState::Unknown => 1,
    }
}

pub fn ttl_duration(state: AgentState) -> Duration {
    match state {
        AgentState::Working => Duration::minutes(30),
        AgentState::Attention => Duration::hours(4),
        AgentState::Risk => Duration::minutes(30),
        AgentState::Result => Duration::seconds(8),
        AgentState::Error => Duration::seconds(60),
        AgentState::Unknown => Duration::seconds(30),
        AgentState::Idle => Duration::zero(),
    }
}

pub fn state_from_event(event_type: EventType, is_risk: bool) -> AgentState {
    match event_type {
        EventType::AgentStarted => AgentState::Working,
        EventType::AgentWorking => AgentState::Working,
        EventType::AgentWaiting => AgentState::Working,
        EventType::AgentCompleted => AgentState::Result,
        EventType::AgentFailed => AgentState::Error,
        EventType::SessionStarted => AgentState::Idle,
        EventType::SessionStopped => AgentState::Idle,
        EventType::SessionFailed => AgentState::Error,
        EventType::SessionHeartbeat => AgentState::Working,
        EventType::SessionTimeout => AgentState::Unknown,
        EventType::ToolStarted => AgentState::Working,
        EventType::ToolCompleted => AgentState::Working,
        EventType::ToolFailed => AgentState::Error,
        EventType::UserInputRequired => AgentState::Attention,
        EventType::UserApprovalRequired => {
            if is_risk {
                AgentState::Risk
            } else {
                AgentState::Attention
            }
        }
        EventType::UserInputReceived => AgentState::Working,
        EventType::ProviderDisconnected => AgentState::Error,
        EventType::ProviderSchemaError => AgentState::Error,
        EventType::RendererFailed => AgentState::Error,
    }
}

#[allow(dead_code)]
pub fn event_is_provider_disconnect(event_type: EventType) -> bool {
    event_type == EventType::ProviderDisconnected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_order_matches_spec() {
        assert!(priority(AgentState::Risk) > priority(AgentState::Attention));
        assert!(priority(AgentState::Attention) > priority(AgentState::Error));
        assert!(priority(AgentState::Error) > priority(AgentState::Result));
        assert!(priority(AgentState::Result) > priority(AgentState::Working));
        assert!(priority(AgentState::Working) > priority(AgentState::Idle));
        assert!(priority(AgentState::Idle) > priority(AgentState::Unknown));
    }

    #[test]
    fn ttl_values_match_spec() {
        assert_eq!(ttl_duration(AgentState::Working), Duration::minutes(30));
        assert_eq!(ttl_duration(AgentState::Attention), Duration::hours(4));
        assert_eq!(ttl_duration(AgentState::Risk), Duration::minutes(30));
        assert_eq!(ttl_duration(AgentState::Result), Duration::seconds(8));
        assert_eq!(ttl_duration(AgentState::Error), Duration::seconds(60));
        assert_eq!(ttl_duration(AgentState::Unknown), Duration::seconds(30));
        assert_eq!(ttl_duration(AgentState::Idle), Duration::zero());
    }

    #[test]
    fn state_from_event_maps_all_event_types() {
        assert_eq!(
            state_from_event(EventType::AgentStarted, false),
            AgentState::Working
        );
        assert_eq!(
            state_from_event(EventType::AgentWorking, false),
            AgentState::Working
        );
        assert_eq!(
            state_from_event(EventType::AgentWaiting, false),
            AgentState::Working
        );
        assert_eq!(
            state_from_event(EventType::AgentCompleted, false),
            AgentState::Result
        );
        assert_eq!(
            state_from_event(EventType::AgentFailed, false),
            AgentState::Error
        );
        assert_eq!(
            state_from_event(EventType::UserInputRequired, false),
            AgentState::Attention
        );
        assert_eq!(
            state_from_event(EventType::UserApprovalRequired, false),
            AgentState::Attention
        );
        assert_eq!(
            state_from_event(EventType::UserApprovalRequired, true),
            AgentState::Risk
        );
        assert_eq!(
            state_from_event(EventType::UserInputReceived, false),
            AgentState::Working
        );
        assert_eq!(
            state_from_event(EventType::ToolStarted, false),
            AgentState::Working
        );
        assert_eq!(
            state_from_event(EventType::ToolCompleted, false),
            AgentState::Working
        );
        assert_eq!(
            state_from_event(EventType::ToolFailed, false),
            AgentState::Error
        );
        assert_eq!(
            state_from_event(EventType::SessionStarted, false),
            AgentState::Idle
        );
        assert_eq!(
            state_from_event(EventType::SessionStopped, false),
            AgentState::Idle
        );
        assert_eq!(
            state_from_event(EventType::SessionFailed, false),
            AgentState::Error
        );
        assert_eq!(
            state_from_event(EventType::SessionTimeout, false),
            AgentState::Unknown
        );
        assert_eq!(
            state_from_event(EventType::SessionHeartbeat, false),
            AgentState::Working
        );
        assert_eq!(
            state_from_event(EventType::ProviderDisconnected, false),
            AgentState::Error
        );
        assert_eq!(
            state_from_event(EventType::ProviderSchemaError, false),
            AgentState::Error
        );
        assert_eq!(
            state_from_event(EventType::RendererFailed, false),
            AgentState::Error
        );
    }
}
