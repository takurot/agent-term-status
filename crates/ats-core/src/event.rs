use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::activity::Activity;
use crate::session::SessionIdentity;

/// Normalized event exchanged between providers, daemon, and renderers (SPEC §6.1).
///
/// Unknown fields are rejected at the type level (defense in depth for the
/// privacy invariant, SPEC §6.1.2); `metadata` is the only extension point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedEvent {
    /// Schema version, e.g. `"1.0"`.
    pub schema_version: String,
    /// UUIDv7 event ID (time-sortable).
    pub event_id: Uuid,
    /// RFC 3339 UTC timestamp.
    pub timestamp: DateTime<Utc>,
    /// Provider name, e.g. `claude`.
    pub provider: String,
    /// Provider schema/CLI version.
    #[serde(default)]
    pub provider_version: Option<String>,
    /// Strongly-typed event type (SPEC §7).
    pub event_type: EventType,
    /// Session identity for terminal targeting.
    pub session: SessionIdentity,
    /// Optional activity detail.
    #[serde(default)]
    pub activity: Option<Activity>,
    /// Free-form extension point; must never carry prompt bodies or secrets.
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

/// Event type taxonomy (SPEC §7.1–§7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    #[serde(rename = "session.started")]
    SessionStarted,
    #[serde(rename = "session.stopped")]
    SessionStopped,
    #[serde(rename = "session.failed")]
    SessionFailed,
    #[serde(rename = "session.heartbeat")]
    SessionHeartbeat,
    #[serde(rename = "agent.started")]
    AgentStarted,
    #[serde(rename = "agent.working")]
    AgentWorking,
    #[serde(rename = "agent.waiting")]
    AgentWaiting,
    #[serde(rename = "agent.completed")]
    AgentCompleted,
    #[serde(rename = "agent.failed")]
    AgentFailed,
    #[serde(rename = "tool.started")]
    ToolStarted,
    #[serde(rename = "tool.completed")]
    ToolCompleted,
    #[serde(rename = "tool.failed")]
    ToolFailed,
    #[serde(rename = "user.input_required")]
    UserInputRequired,
    #[serde(rename = "user.approval_required")]
    UserApprovalRequired,
    #[serde(rename = "user.input_received")]
    UserInputReceived,
    #[serde(rename = "provider.disconnected")]
    ProviderDisconnected,
    #[serde(rename = "provider.schema_error")]
    ProviderSchemaError,
    #[serde(rename = "renderer.failed")]
    RendererFailed,
    #[serde(rename = "session.timeout")]
    SessionTimeout,
}
