use ats_core::AgentState;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashSet;
use uuid::Uuid;

use crate::priority::{priority, state_from_event, ttl_duration};

#[derive(Debug, Clone)]
pub(super) struct SessionState {
    pub state: AgentState,
    pub error_type: Option<ats_core::EventType>,
    pub updated_at: DateTime<Utc>,
    pub ttl: Duration,
    pub seen_event_ids: HashSet<Uuid>,
}

impl SessionState {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            state: AgentState::Idle,
            error_type: None,
            updated_at: now,
            ttl: Duration::zero(),
            seen_event_ids: HashSet::new(),
        }
    }

    pub fn apply(
        &mut self,
        event_id: Uuid,
        event_type: ats_core::EventType,
        is_risk: bool,
        timestamp: DateTime<Utc>,
    ) -> TransitionResult {
        if !self.seen_event_ids.insert(event_id) {
            return TransitionResult::Idempotent;
        }

        let proposed = state_from_event(event_type, is_risk);
        let previous = self.state;

        let effective = resolve_effective_state(self.state, proposed, self.error_type, event_type);

        if effective == previous {
            self.updated_at = timestamp;
            self.ttl = ttl_duration(effective);
            return TransitionResult::Unchanged;
        }

        self.state = effective;
        self.updated_at = timestamp;
        self.ttl = ttl_duration(effective);

        if effective == AgentState::Error {
            self.error_type = Some(event_type);
        } else {
            self.error_type = None;
        }

        TransitionResult::Changed {
            previous,
            new: effective,
        }
    }

    pub fn check_ttl(&mut self, now: DateTime<Utc>) -> Option<AgentState> {
        if self.state == AgentState::Idle {
            return None;
        }

        if self.ttl == Duration::zero() {
            return None;
        }

        let elapsed = now.signed_duration_since(self.updated_at);
        if elapsed < self.ttl {
            return None;
        }

        if self.state == AgentState::Unknown {
            self.state = AgentState::Idle;
            self.ttl = Duration::zero();
            self.updated_at = now;
            self.error_type = None;
            return Some(AgentState::Idle);
        }

        self.state = AgentState::Unknown;
        self.ttl = ttl_duration(AgentState::Unknown);
        self.updated_at = now;
        self.error_type = None;
        Some(AgentState::Unknown)
    }

    pub fn extend_heartbeat(&mut self, now: DateTime<Utc>) {
        if self.state != AgentState::Idle && self.state != AgentState::Unknown {
            self.updated_at = now;
            self.ttl = ttl_duration(self.state);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TransitionResult {
    Changed {
        previous: AgentState,
        new: AgentState,
    },
    Unchanged,
    Idempotent,
}

fn forced_transition(event_type: ats_core::EventType) -> bool {
    matches!(
        event_type,
        ats_core::EventType::SessionTimeout
            | ats_core::EventType::AgentFailed
            | ats_core::EventType::SessionFailed
            | ats_core::EventType::ToolFailed
            | ats_core::EventType::ProviderSchemaError
            | ats_core::EventType::RendererFailed
            | ats_core::EventType::UserInputReceived
    )
}

fn resolve_effective_state(
    current: AgentState,
    proposed: AgentState,
    _current_error_type: Option<ats_core::EventType>,
    proposed_event: ats_core::EventType,
) -> AgentState {
    if proposed_event == ats_core::EventType::ProviderDisconnected
        && current == AgentState::Attention
    {
        return AgentState::Attention;
    }

    if forced_transition(proposed_event) {
        return proposed;
    }

    let proposed_prio = priority(proposed);

    if proposed_prio > priority(current) {
        return proposed;
    }

    if proposed == current {
        return current;
    }

    current
}
