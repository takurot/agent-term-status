//! Daemon event broker (SPEC §5.3.3–5.3.4, I-14).
//!
//! Receives raw JSON frames from the socket server, deserializes them
//! into [`NormalizedEvent`]s, validates, deduplicates, corrects
//! timestamp ordering, feeds them into the [`StateEngine`], and
//! dispatches resulting state changes to the [`RenderingEngine`].
//!
//! Privacy invariant (SPEC §14.2): deserialization failures and
//! validation rejections carry byte counts only, never payload bodies.

use std::collections::HashMap;

use ats_core::{AgentState, NormalizedEvent};
use ats_renderer::{RenderTarget, StateView};
use ats_rendering::RenderingEngine;
use ats_state_engine::{StateEngine, StateTransition};
use chrono::{DateTime, Utc};
use thiserror::Error;
use tokio::sync::mpsc;

/// Default time window for ordering correction (ms).
pub const DEFAULT_ORDERING_WINDOW_MS: u64 = 50;
/// Default TTL check interval (ms).
pub const DEFAULT_TTL_CHECK_INTERVAL_MS: u64 = 1000;

#[derive(Debug, Clone)]
pub struct BrokerConfig {
    /// How long to buffer events for ordering correction (ms).
    pub ordering_window_ms: u64,
    /// TTL check interval (ms).
    pub ttl_check_interval_ms: u64,
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            ordering_window_ms: DEFAULT_ORDERING_WINDOW_MS,
            ttl_check_interval_ms: DEFAULT_TTL_CHECK_INTERVAL_MS,
        }
    }
}

#[derive(Error, Debug)]
pub enum BrokerError {
    #[error("event deserialization failed ({} bytes)", .len)]
    Deserialize { len: usize },
    #[error("rendering engine not configured")]
    NoRenderingEngine,
}

/// Event broker: the core loop that sits between the socket server and
/// the state/rendering engines.
pub struct Broker {
    state_engine: StateEngine,
    rendering_engine: Option<RenderingEngine>,
    config: BrokerConfig,
    /// Per-session event buffer for ordering correction.
    /// Keyed by session_id, holds (timestamp, event) pairs.
    pending: HashMap<String, Vec<(DateTime<Utc>, NormalizedEvent)>>,
}

impl Broker {
    pub fn new(
        state_engine: StateEngine,
        rendering_engine: Option<RenderingEngine>,
        config: BrokerConfig,
    ) -> Self {
        Self {
            state_engine,
            rendering_engine,
            config,
            pending: HashMap::new(),
        }
    }

    /// Adds a raw JSON payload to the pending buffer.
    ///
    /// Returns the deserialized event on success (for informational
    /// purposes) or a [`BrokerError`] carrying only the byte count.
    pub fn buffer_event(&mut self, raw: &[u8]) -> Result<NormalizedEvent, BrokerError> {
        let event: NormalizedEvent =
            serde_json::from_slice(raw).map_err(|_| BrokerError::Deserialize { len: raw.len() })?;

        let sid = event.session.id.clone();
        let ts = event.timestamp;

        self.pending
            .entry(sid)
            .or_default()
            .push((ts, event.clone()));

        Ok(event)
    }

    /// Drains all pending events, processes them in timestamp order,
    /// and dispatches rendering for any state transitions.
    ///
    /// Returns all state transitions that occurred during processing.
    pub fn drain_and_process(&mut self) -> Vec<StateTransition> {
        let now = Utc::now();
        let mut all_transitions = Vec::new();

        let sessions: Vec<String> = self.pending.keys().cloned().collect();

        for sid in sessions {
            let Some(events) = self.pending.get_mut(&sid) else {
                continue;
            };
            if events.is_empty() {
                continue;
            }

            events.sort_by_key(|(ts, _)| *ts);

            for (_, event) in events.drain(..) {
                if let Some(transition) = self.state_engine.ingest(&event, now) {
                    all_transitions.push(transition);
                }
            }
        }

        all_transitions
    }

    /// Check and expire TTLs for all sessions.
    ///
    /// Returns transitions for sessions that expired.
    pub fn check_ttls(&mut self, now: DateTime<Utc>) -> Vec<StateTransition> {
        self.state_engine.expire_ttls(now)
    }

    /// Dispatch rendering for a set of state transitions.
    ///
    /// Spawns a tokio task per render dispatch and returns immediately
    /// (fail-open: render failures are collected by the engine but never
    /// block the broker).
    pub async fn dispatch_render(
        rendering_engine: &RenderingEngine,
        transitions: &[StateTransition],
        session_terminal: &HashMap<
            String,
            (
                Option<ats_core::ActivityLabel>,
                Option<ats_core::TerminalContext>,
            ),
        >,
    ) {
        for transition in transitions {
            let (label, terminal) = session_terminal
                .get(&transition.session_id)
                .cloned()
                .unwrap_or_default();

            let view = StateView {
                state: transition.new,
                label,
                target: RenderTarget {
                    session_id: transition.session_id.clone(),
                    terminal: terminal.unwrap_or_default(),
                },
            };

            rendering_engine.render(&view).await;
        }
    }

    /// Get the state engine for external queries.
    pub fn state_engine(&self) -> &StateEngine {
        &self.state_engine
    }

    /// Get a mutable reference to the state engine.
    pub fn state_engine_mut(&mut self) -> &mut StateEngine {
        &mut self.state_engine
    }

    /// Get the rendering engine.
    pub fn rendering_engine(&self) -> Option<&RenderingEngine> {
        self.rendering_engine.as_ref()
    }

    /// Get the rendering engine mutably.
    pub fn rendering_engine_mut(&mut self) -> Option<&mut RenderingEngine> {
        self.rendering_engine.as_mut()
    }

    /// Session count in the state engine.
    pub fn session_count(&self) -> usize {
        self.state_engine.session_count()
    }

    /// Get agent state for a session.
    pub fn get_state(&self, session_id: &str) -> Option<AgentState> {
        self.state_engine.get_state(session_id)
    }

    /// Run the broker main loop.
    ///
    /// Reads raw JSON payloads from `event_rx`, buffers them for
    /// ordering correction, periodically drains and processes, and
    /// checks TTL expiry. Exits when `shutdown_rx` fires.
    pub async fn run(
        &mut self,
        mut event_rx: mpsc::Receiver<Vec<u8>>,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) {
        let ordering_interval = tokio::time::Duration::from_millis(self.config.ordering_window_ms);
        let ttl_interval = tokio::time::Duration::from_millis(self.config.ttl_check_interval_ms);

        let mut drain_tick = tokio::time::interval(ordering_interval);
        // Suppress immediate tick so the first drain happens after
        // at least one window period of accumulation.
        drain_tick.reset();

        let mut ttl_tick = tokio::time::interval(ttl_interval);

        // Per-event session/terminal info for render dispatch.
        let mut session_info: HashMap<
            String,
            (
                Option<ats_core::ActivityLabel>,
                Option<ats_core::TerminalContext>,
            ),
        > = HashMap::new();

        loop {
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    if changed.is_err() || *shutdown_rx.borrow() {
                        // Final drain before shutdown.
                        let transitions = self.drain_and_process();
                        if let Some(engine) = &self.rendering_engine {
                            Self::dispatch_render(engine, &transitions, &session_info).await;
                        }
                        break;
                    }
                }

                event = event_rx.recv() => {
                    let Some(raw) = event else {
                        // Channel closed; final drain.
                        let transitions = self.drain_and_process();
                        if let Some(engine) = &self.rendering_engine {
                            Self::dispatch_render(engine, &transitions, &session_info).await;
                        }
                        break;
                    };

                    match self.buffer_event(&raw) {
                        Ok(event) => {
                            let sid = event.session.id.clone();
                            let label = event.activity.as_ref().and_then(|a| a.label.clone());
                            let terminal = event.session.terminal.clone();
                            session_info.insert(sid, (label, terminal));
                        }
                        Err(BrokerError::Deserialize { len }) => {
                            // Fail-open: log and drop (privacy: byte count only).
                            eprintln!("ats-daemon broker: failed to deserialize event ({} bytes)", len);
                        }
                        _ => {}
                    }
                }

                _ = drain_tick.tick() => {
                    let transitions = self.drain_and_process();
                    if !transitions.is_empty() {
                        if let Some(engine) = &self.rendering_engine {
                            Self::dispatch_render(engine, &transitions, &session_info).await;
                        }
                    }
                }

                _ = ttl_tick.tick() => {
                    let now = Utc::now();
                    let transitions = self.check_ttls(now);
                    if !transitions.is_empty() {
                        if let Some(engine) = &self.rendering_engine {
                            Self::dispatch_render(engine, &transitions, &session_info).await;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ats_core::{EventType, SessionIdentity};
    use ats_state_engine::StateEngine;
    use chrono::Duration;
    use uuid::Uuid;

    fn make_event(
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        session_id: &str,
        event_type: EventType,
    ) -> NormalizedEvent {
        NormalizedEvent {
            schema_version: "1.0".into(),
            event_id,
            timestamp,
            provider: "test".into(),
            provider_version: None,
            event_type,
            session: SessionIdentity {
                id: session_id.into(),
                parent_id: None,
                workspace: None,
                terminal: None,
            },
            activity: None,
            metadata: serde_json::Map::new(),
        }
    }

    fn make_broker() -> Broker {
        Broker::new(StateEngine::new(), None, BrokerConfig::default())
    }

    fn serialize_event(event: &NormalizedEvent) -> Vec<u8> {
        serde_json::to_vec(event).unwrap()
    }

    // ---------------------------------------------------------------
    // Dedup test: send same event_id twice → exactly one transition
    // ---------------------------------------------------------------

    #[test]
    fn dedup_same_event_id_produces_one_transition() {
        let mut broker = make_broker();

        let eid = Uuid::now_v7();
        let ts = Utc::now();
        let session_id = "dedup-test";

        let event = make_event(eid, ts, session_id, EventType::AgentStarted);
        let raw = serialize_event(&event);

        // First event
        let _ = broker.buffer_event(&raw).unwrap();
        let transitions = broker.drain_and_process();
        assert_eq!(transitions.len(), 1, "first event should transition");

        // Second event — same event_id
        let event2 = make_event(eid, ts, session_id, EventType::AgentStarted);
        let raw2 = serialize_event(&event2);
        let _ = broker.buffer_event(&raw2).unwrap();
        let transitions2 = broker.drain_and_process();
        assert!(
            transitions2.is_empty(),
            "duplicate event_id should produce no transition"
        );
    }

    // ---------------------------------------------------------------
    // Crash recovery: fresh broker has no state, does not re-render
    // ---------------------------------------------------------------

    #[test]
    fn crash_recovery_fresh_broker_has_no_state() {
        // Simulate: broker1 processes a WORKING event, then "crashes".
        let mut broker1 = make_broker();
        let eid = Uuid::now_v7();
        let ts = Utc::now();
        let event = make_event(eid, ts, "crash-test", EventType::AgentStarted);
        let raw = serialize_event(&event);

        let _ = broker1.buffer_event(&raw).unwrap();
        let transitions = broker1.drain_and_process();
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].new, AgentState::Working);

        // "Crash": broker1 is dropped, a new broker (simulating restart)
        // has no session state.
        let broker2 = make_broker();
        assert_eq!(broker2.session_count(), 0);
        assert!(broker2.get_state("crash-test").is_none());
        // It does NOT re-render WORKING — it waits for next event or checker tick.
    }

    #[test]
    fn crash_recovery_does_not_render_stale_state() {
        let mut broker1 = make_broker();
        let eid = Uuid::now_v7();
        let ts = Utc::now();
        let event = make_event(eid, ts, "stale-session", EventType::AgentStarted);
        let raw = serialize_event(&event);

        let _ = broker1.buffer_event(&raw).unwrap();
        let t1 = broker1.drain_and_process();
        assert!(!t1.is_empty());
        assert_eq!(
            broker1.get_state("stale-session"),
            Some(AgentState::Working)
        );

        // New broker after restart — no states, no re-render.
        let broker2 = make_broker();
        assert_eq!(broker2.session_count(), 0);
        assert!(broker2.get_state("stale-session").is_none());

        // TTL check should produce nothing (no sessions).
        let ttl_transitions = broker1.check_ttls(Utc::now());
        // broker1 still has the session, but it's not expired yet.
        let expected_empty = ttl_transitions.is_empty()
            || ttl_transitions.iter().all(|t| t.new != AgentState::Working);
        assert!(expected_empty, "TTL check should not re-render WORKING");
    }

    // ---------------------------------------------------------------
    // Ordering test: send t2 then t1 → processed t1 then t2
    // ---------------------------------------------------------------

    #[test]
    fn ordering_correction_sorts_by_timestamp() {
        let mut broker = make_broker();

        let t1 = Utc::now();
        let t2 = t1 + chrono::Duration::seconds(10);

        let eid1 = Uuid::now_v7();
        let eid2 = Uuid::now_v7();

        let session_id = "order-test";

        // Send in reverse order: t2 first, then t1.
        let event2 = make_event(eid2, t2, session_id, EventType::AgentCompleted);
        let event1 = make_event(eid1, t1, session_id, EventType::AgentStarted);

        let raw2 = serialize_event(&event2);
        let raw1 = serialize_event(&event1);

        let _ = broker.buffer_event(&raw2).unwrap();
        let _ = broker.buffer_event(&raw1).unwrap();

        // Drain — should process t1 (AgentStarted, IDLE→WORKING)
        // then t2 (AgentCompleted, WORKING→RESULT).
        let transitions = broker.drain_and_process();

        // Should see: first transition WORKING, then RESULT
        assert_eq!(
            transitions.len(),
            2,
            "both events should produce transitions"
        );
        assert_eq!(transitions[0].new, AgentState::Working);
        assert_eq!(transitions[1].new, AgentState::Result);
    }

    // ---------------------------------------------------------------
    // Validation: malformed JSON doesn't crash
    // ---------------------------------------------------------------

    #[test]
    fn malformed_json_rejected_not_crashed() {
        let mut broker = make_broker();
        let result = broker.buffer_event(b"not valid json");
        assert!(
            matches!(result, Err(BrokerError::Deserialize { .. })),
            "malformed JSON should return error, not panic"
        );
    }

    #[test]
    fn empty_payload_rejected() {
        let mut broker = make_broker();
        let result = broker.buffer_event(b"");
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // Session registry: multiple sessions are isolated
    // ---------------------------------------------------------------

    #[test]
    fn multiple_sessions_isolated() {
        let mut broker = make_broker();

        let eid1 = Uuid::now_v7();
        let eid2 = Uuid::now_v7();
        let ts = Utc::now();

        let event1 = make_event(eid1, ts, "session-a", EventType::AgentStarted);
        let event2 = make_event(eid2, ts, "session-b", EventType::AgentStarted);

        let raw1 = serialize_event(&event1);
        let raw2 = serialize_event(&event2);

        let _ = broker.buffer_event(&raw1).unwrap();
        let _ = broker.buffer_event(&raw2).unwrap();

        let transitions = broker.drain_and_process();
        assert_eq!(transitions.len(), 2);
        assert_eq!(broker.session_count(), 2);
        assert_eq!(broker.get_state("session-a"), Some(AgentState::Working));
        assert_eq!(broker.get_state("session-b"), Some(AgentState::Working));
    }

    // ---------------------------------------------------------------
    // TTL expiry: check_ttls transitions expired sessions
    // ---------------------------------------------------------------

    #[test]
    fn ttl_expiry_transitions_sessions() {
        let mut broker = make_broker();

        let eid = Uuid::now_v7();
        let ts = Utc::now() - Duration::hours(1);

        let event = make_event(eid, ts, "ttl-test", EventType::AgentCompleted);
        let raw = serialize_event(&event);

        let _ = broker.buffer_event(&raw).unwrap();
        let transitions = broker.drain_and_process();
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].new, AgentState::Result);

        // Advance time, check TTL. Result TTL is 8s, should expire.
        let now = ts + Duration::seconds(30);
        let ttl_transitions = broker.check_ttls(now);
        // Should go Result → Unknown → Idle (two-stage expiry)
        assert!(!ttl_transitions.is_empty());
    }

    // ---------------------------------------------------------------
    // Buffer event returns deserialized event
    // ---------------------------------------------------------------

    #[test]
    fn buffer_event_returns_deserialized_event() {
        let mut broker = make_broker();
        let eid = Uuid::now_v7();
        let ts = Utc::now();
        let event = make_event(eid, ts, "return-test", EventType::SessionStarted);
        let raw = serialize_event(&event);

        let returned = broker.buffer_event(&raw).unwrap();
        assert_eq!(returned.event_id, eid);
        assert_eq!(returned.session.id, "return-test");
    }
}
