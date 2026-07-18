use ats_core::{NormalizedEvent, RiskLevel};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::machine::{SessionState, TransitionResult};
use crate::priority::priority;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTransition {
    pub session_id: String,
    pub previous: ats_core::AgentState,
    pub new: ats_core::AgentState,
}

pub struct StateEngine {
    sessions: HashMap<String, SessionState>,
    parent_map: HashMap<String, String>,
}

impl Default for StateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl StateEngine {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            parent_map: HashMap::new(),
        }
    }

    /// Ingest a normalized event. Returns the state transition for this
    /// session if the state changed, or `None` if idempotent/unchanged.
    pub fn ingest(
        &mut self,
        event: &NormalizedEvent,
        now: DateTime<Utc>,
    ) -> Option<StateTransition> {
        let session_id = event.session.id.clone();

        let is_risk = event
            .activity
            .as_ref()
            .and_then(|a| a.risk.as_ref())
            .map(|r| matches!(r, RiskLevel::High | RiskLevel::Critical))
            .unwrap_or(false);

        let session = self
            .sessions
            .entry(session_id.clone())
            .or_insert_with(|| SessionState::new(now));

        match session.apply(event.event_id, event.event_type, is_risk, event.timestamp) {
            TransitionResult::Changed { previous, new } => {
                if let Some(parent_id) = &event.session.parent_id {
                    if parent_id != &session_id {
                        self.parent_map
                            .insert(session_id.clone(), parent_id.clone());
                    }
                }

                Some(StateTransition {
                    session_id,
                    previous,
                    new,
                })
            }
            TransitionResult::Unchanged | TransitionResult::Idempotent => {
                if let Some(parent_id) = &event.session.parent_id {
                    if parent_id != &session_id {
                        self.parent_map
                            .insert(session_id.clone(), parent_id.clone());
                    }
                }
                None
            }
        }
    }

    /// Expire TTLs for all sessions. Returns transitions for sessions
    /// whose state changed due to TTL expiry.
    pub fn expire_ttls(&mut self, now: DateTime<Utc>) -> Vec<StateTransition> {
        let mut transitions = Vec::new();
        let mut expired_ids = Vec::new();

        for (id, session) in self.sessions.iter_mut() {
            let previous = session.state;
            if let Some(_new_state) = session.check_ttl(now) {
                transitions.push(StateTransition {
                    session_id: id.clone(),
                    previous,
                    new: session.state,
                });
                if session.state == ats_core::AgentState::Idle
                    && !self.parent_map.iter().any(|(_, p)| p == id)
                {
                    expired_ids.push(id.clone());
                }
            }
        }

        for id in expired_ids {
            self.remove_session(&id);
        }

        transitions
    }

    /// Get the current state for a session.
    pub fn get_state(&self, session_id: &str) -> Option<ats_core::AgentState> {
        self.sessions.get(session_id).map(|s| s.state)
    }

    /// Get the aggregated state for a parent session, derived from
    /// the max-priority state of all its child sessions.
    pub fn get_aggregated_state(&self, parent_id: &str) -> Option<ats_core::AgentState> {
        let children: Vec<_> = self
            .sessions
            .iter()
            .filter(|(child_id, _)| {
                self.parent_map
                    .get(*child_id)
                    .map(|p| p == parent_id)
                    .unwrap_or(false)
            })
            .map(|(_, session)| session.state)
            .collect();

        if children.is_empty() {
            return None;
        }

        let max_state = children
            .into_iter()
            .max_by_key(|s| priority(*s))
            .expect("children non-empty");

        Some(max_state)
    }

    /// Extend TTL for a session (heartbeat assist).
    pub fn heartbeat(&mut self, session_id: &str, now: DateTime<Utc>) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.extend_heartbeat(now);
        }
    }

    pub(super) fn remove_session(&mut self, session_id: &str) -> Option<SessionState> {
        self.parent_map.remove(session_id);
        self.sessions.remove(session_id)
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}
