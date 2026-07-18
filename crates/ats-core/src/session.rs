use serde::{Deserialize, Serialize};

/// Session identity for routing state to the right terminal target (SPEC §6.4).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionIdentity {
    /// Provider-supplied session ID (required, SPEC §6.1.1).
    pub id: String,
    /// Parent session ID when this session is a subagent (SPEC §6.4.3).
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Workspace path. Not stored by default (`store_workspace_paths: false`).
    #[serde(default)]
    pub workspace: Option<String>,
    /// Terminal context used for pane/tab targeting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<TerminalContext>,
}

/// Terminal identification captured at event time (SPEC §6.1, §6.4).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalContext {
    /// TTY device path, e.g. `/dev/ttys004`.
    #[serde(default)]
    pub tty: Option<String>,
    /// Terminal program, e.g. `iTerm.app`.
    #[serde(default)]
    pub term_program: Option<String>,
    /// tmux pane ID, e.g. `%12` (SPEC §6.4.2).
    #[serde(default)]
    pub tmux_pane: Option<String>,
}
