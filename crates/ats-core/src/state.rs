use serde::{Deserialize, Serialize};

/// Normalized agent state shown to the user (SPEC §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    /// Session is waiting; no user action needed.
    Idle,
    /// AI is processing; no user action needed.
    Working,
    /// Waiting for user input or approval; action required.
    Attention,
    /// High-risk operation pending approval; review immediately.
    Risk,
    /// Completed successfully.
    Result,
    /// Failure or integration issue.
    Error,
    /// State cannot be determined.
    Unknown,
}
