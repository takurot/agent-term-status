//! `ats-core` — pure data model shared by every other `ats-*` crate.
//!
//! Contains only serializable types and small pure helpers (SPEC §6, §7).
//! No I/O, no async, no traits from other layers.

mod activity;
mod event;
mod id;
mod session;
mod state;

pub use activity::{Activity, ActivityCategory, ActivityLabel, RiskLevel};
pub use event::{EventType, NormalizedEvent};
pub use id::{new_event_id, now_utc};
pub use session::{SessionIdentity, TerminalContext};
pub use state::AgentState;

/// Current Normalized Event schema version (SPEC §6.1).
pub const SCHEMA_VERSION: &str = "1.0";
