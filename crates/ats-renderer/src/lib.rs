//! `ats-renderer` — the Renderer trait surface (SPEC §5.5.2).
//!
//! Renderers turn resolved agent state into terminal-specific output
//! (tmux pane borders, iTerm2 badges, notifications, ...). This crate
//! holds only the trait and its view/result types so renderer
//! implementations and the rendering engine can evolve independently.
//! Depends only on `ats-core` (plus `async-trait`).

use async_trait::async_trait;
use ats_core::{ActivityLabel, AgentState, TerminalContext};

/// Renderer interface defined in SPEC §5.5.2.
///
/// Implementations MUST:
/// - only emit allowlisted control sequences (SPEC §14.2),
/// - never propagate failures to the agent (fail-open, SPEC §15):
///   errors are reported as [`RendererError`] and surfaced as
///   `renderer.failed` events by the rendering engine,
/// - set [`RendererCapabilities::reset_reliable`] honestly so the engine
///   can pick fallback representations (SPEC §10.3).
#[async_trait]
pub trait Renderer: Send + Sync {
    /// Detects which capabilities are available for the given terminal.
    async fn detect(&self, ctx: &TerminalContext) -> Result<RendererCapabilities, RendererError>;

    /// Renders the resolved state view onto the target terminal.
    async fn render(&self, view: &StateView) -> Result<(), RendererError>;

    /// Restores the target terminal to its default appearance.
    async fn reset(&self, target: &RenderTarget) -> Result<(), RendererError>;

    /// Reports whether this renderer is currently usable.
    async fn health_check(&self) -> HealthStatus;
}

/// Capability flags per renderer (SPEC §10.1). Defaults to all-off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererCapabilities {
    /// Can change the terminal background color.
    pub background: bool,
    /// Can set the tab / window title.
    pub tab_title: bool,
    /// Can set the tab color.
    pub tab_color: bool,
    /// Can display a badge (e.g. iTerm2 OSC 1337).
    pub badge: bool,
    /// Can change the cursor color.
    pub cursor_color: bool,
    /// Can style pane borders (tmux).
    pub pane_border: bool,
    /// Can emit user notifications.
    pub notification: bool,
    /// Can flash / bell for attention.
    pub flash: bool,
    /// Reset is guaranteed to restore the default appearance (SPEC §10.3).
    pub reset_reliable: bool,
}

/// Resolved, render-ready state for one session (input to [`Renderer::render`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateView {
    /// State to visualize.
    pub state: AgentState,
    /// Optional sanitized activity label (e.g. `WORKING · Running tests`).
    pub label: Option<ActivityLabel>,
    /// Where to render.
    pub target: RenderTarget,
}

/// Addressing information for one terminal surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTarget {
    /// Session the state belongs to.
    pub session_id: String,
    /// Terminal identification (tty / term program / tmux pane, SPEC §6.4).
    pub terminal: TerminalContext,
}

/// Renderer failure. Never aborts the agent; the rendering engine converts
/// these into `renderer.failed` events (SPEC §5.5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererError {
    /// The renderer cannot serve this terminal context at all.
    Unsupported(String),
    /// The renderer attempted to draw and failed.
    Failed(String),
}

impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(detail) => write!(f, "renderer unsupported: {detail}"),
            Self::Failed(detail) => write!(f, "renderer failed: {detail}"),
        }
    }
}

impl std::error::Error for RendererError {}

/// Liveness of a renderer as reported by [`Renderer::health_check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// Fully operational.
    Healthy,
    /// Usable but with reduced fidelity.
    Degraded {
        /// Log-safe description of the degradation.
        reason: String,
    },
    /// Not usable right now.
    Unavailable {
        /// Log-safe description of why the renderer is unusable.
        reason: String,
    },
}
