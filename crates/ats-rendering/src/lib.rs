//! # `ats-rendering` — Rendering Engine + NotificationDispatcher (I-12)
//!
//! Orchestrates multiple [`Renderer`] implementations: capability
//! detection, theme resolution, rate limiting, and dispatch. Sits
//! between the daemon event broker and individual renderers (SPEC §5.5).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ats_config::theme::{Theme, ThemeEntry};
use ats_core::AgentState;
use ats_renderer::{
    HealthStatus, RenderTarget, Renderer, RendererCapabilities, RendererError, StateView,
};
use chrono::{DateTime, Utc};

/// Default cooldown between same-state renders on the same target (ms).
pub const DEFAULT_RENDER_COOLDOWN_MS: i64 = 250;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RateLimitKey {
    session_id: String,
    state: AgentState,
}

/// Configuration for the rendering engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub render_cooldown_ms: i64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            render_cooldown_ms: DEFAULT_RENDER_COOLDOWN_MS,
        }
    }
}

/// Resolved visual properties for a state, ready to dispatch.
#[derive(Debug, Clone)]
pub struct ResolvedView {
    pub state: AgentState,
    pub color: Option<String>,
    pub label: String,
    pub symbol: String,
    pub notification: bool,
}

/// Result of a single renderer dispatch.
#[derive(Debug, Clone)]
pub struct RenderResult {
    pub renderer_name: String,
    pub error: Option<RendererError>,
}

/// The rendering engine orchestrating multiple [`Renderer`]s.
pub struct RenderingEngine {
    renderers: Vec<(String, Box<dyn Renderer>)>,
    theme: Option<Theme>,
    config: EngineConfig,
    last_render: Arc<Mutex<HashMap<RateLimitKey, DateTime<Utc>>>>,
}

impl RenderingEngine {
    pub fn new(theme: Option<Theme>, config: EngineConfig) -> Self {
        Self {
            renderers: Vec::new(),
            theme,
            config,
            last_render: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_renderer(&mut self, name: impl Into<String>, renderer: Box<dyn Renderer>) {
        self.renderers.push((name.into(), renderer));
    }

    fn resolve_entry(&self, state: AgentState) -> Option<ThemeEntry> {
        self.theme.as_ref().and_then(|t| t.resolve(state))
    }

    /// Produce the resolved visual view for a state.
    pub fn resolve_view(&self, state: AgentState) -> ResolvedView {
        let entry = self.resolve_entry(state);
        ResolvedView {
            state,
            color: entry.as_ref().and_then(|e| e.color.clone()),
            label: entry.as_ref().map(|e| e.label.clone()).unwrap_or_default(),
            symbol: entry.as_ref().map(|e| e.symbol.clone()).unwrap_or_default(),
            notification: entry.map(|e| e.notification).unwrap_or(false),
        }
    }

    /// Check if a render should be suppressed due to rate limiting.
    pub fn is_rate_limited(&self, session_id: &str, state: AgentState) -> bool {
        let key = RateLimitKey {
            session_id: session_id.to_string(),
            state,
        };
        let mut last = self.last_render.lock().unwrap();
        if let Some(prev) = last.get(&key) {
            let elapsed = Utc::now().signed_duration_since(*prev).num_milliseconds();
            if elapsed < self.config.render_cooldown_ms {
                return true;
            }
        }
        last.insert(key, Utc::now());
        false
    }

    /// Detect capabilities from all registered renderers for a given context.
    pub async fn detect_all(
        &self,
        ctx: &ats_core::TerminalContext,
    ) -> Vec<(&str, Result<RendererCapabilities, RendererError>)> {
        let mut results = Vec::new();
        for (name, renderer) in &self.renderers {
            let cap = renderer.detect(ctx).await;
            results.push((name.as_str(), cap));
        }
        results
    }

    /// Render a state view through all registered renderers.
    /// Returns results per-renderer (including errors for RenderFailed events).
    pub async fn render(&self, view: &StateView) -> Vec<RenderResult> {
        if self.is_rate_limited(&view.target.session_id, view.state) {
            return Vec::new();
        }

        let mut results = Vec::new();
        for (name, renderer) in &self.renderers {
            match renderer.render(view).await {
                Ok(()) => {
                    results.push(RenderResult {
                        renderer_name: name.clone(),
                        error: None,
                    });
                }
                Err(e) => {
                    results.push(RenderResult {
                        renderer_name: name.clone(),
                        error: Some(e),
                    });
                }
            }
        }
        results
    }

    /// Reset all renderers for a target.
    pub async fn reset(&self, target: &RenderTarget) -> Vec<RenderResult> {
        let mut results = Vec::new();
        for (name, renderer) in &self.renderers {
            match renderer.reset(target).await {
                Ok(()) => {
                    results.push(RenderResult {
                        renderer_name: name.clone(),
                        error: None,
                    });
                }
                Err(e) => {
                    results.push(RenderResult {
                        renderer_name: name.clone(),
                        error: Some(e),
                    });
                }
            }
        }
        results
    }

    /// Check health of all renderers.
    pub async fn health_check_all(&self) -> Vec<(&str, HealthStatus)> {
        let mut results = Vec::new();
        for (name, renderer) in &self.renderers {
            results.push((name.as_str(), renderer.health_check().await));
        }
        results
    }

    /// Return the number of registered renderers.
    pub fn renderer_count(&self) -> usize {
        self.renderers.len()
    }

    /// Return registered renderer names.
    pub fn renderer_names(&self) -> Vec<&str> {
        self.renderers.iter().map(|(n, _)| n.as_str()).collect()
    }
}

/// Notification policy: when to suppress a notification.
#[derive(Debug, Clone)]
pub struct NotificationPolicy {
    /// State must be in this set to trigger a notification.
    pub notify_states: Vec<AgentState>,
    /// Quiet hours start (local time).
    pub quiet_hours_start: Option<chrono::NaiveTime>,
    /// Quiet hours end (local time).
    pub quiet_hours_end: Option<chrono::NaiveTime>,
    /// States allowed during quiet hours (default: [Risk]).
    pub quiet_hours_allow: Vec<AgentState>,
}

impl Default for NotificationPolicy {
    fn default() -> Self {
        Self {
            notify_states: vec![
                AgentState::Attention,
                AgentState::Risk,
                AgentState::Result,
                AgentState::Error,
            ],
            quiet_hours_start: None,
            quiet_hours_end: None,
            quiet_hours_allow: vec![AgentState::Risk],
        }
    }
}

impl NotificationPolicy {
    pub fn should_notify(&self, state: AgentState) -> bool {
        self.notify_states.contains(&state)
    }

    pub fn is_quiet_hours(&self) -> bool {
        let (Some(start), Some(end)) = (self.quiet_hours_start, self.quiet_hours_end) else {
            return false;
        };
        let now = chrono::Local::now().time();
        if start <= end {
            now >= start && now < end
        } else {
            now >= start || now < end
        }
    }

    pub fn quiet_hours_allows(&self, state: AgentState) -> bool {
        if !self.is_quiet_hours() {
            return true;
        }
        self.quiet_hours_allow.contains(&state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ats_config::theme::Theme;
    use ats_core::TerminalContext;

    fn default_theme() -> Theme {
        Theme::load_bundled("default").expect("default theme must load")
    }

    // ---------------------------------------------------------------
    // resolve_view
    // ---------------------------------------------------------------

    #[test]
    fn resolve_view_returns_visual_properties() {
        let engine = RenderingEngine::new(Some(default_theme()), EngineConfig::default());
        let view = engine.resolve_view(AgentState::Working);
        assert_eq!(view.state, AgentState::Working);
        assert!(view.color.is_some());
        assert!(!view.symbol.is_empty());
    }

    #[test]
    fn resolve_view_for_idle_has_no_color() {
        let engine = RenderingEngine::new(Some(default_theme()), EngineConfig::default());
        let view = engine.resolve_view(AgentState::Idle);
        assert_eq!(view.state, AgentState::Idle);
        assert!(view.color.is_none());
        assert!(!view.symbol.is_empty());
    }

    #[test]
    fn resolve_view_no_theme_returns_defaults() {
        let engine = RenderingEngine::new(None, EngineConfig::default());
        let view = engine.resolve_view(AgentState::Attention);
        assert_eq!(view.state, AgentState::Attention);
        assert!(view.color.is_none());
    }

    // ---------------------------------------------------------------
    // rate limiting
    // ---------------------------------------------------------------

    #[test]
    fn first_render_not_rate_limited() {
        let engine = RenderingEngine::new(None, EngineConfig::default());
        assert!(!engine.is_rate_limited("session-1", AgentState::Working));
    }

    #[test]
    fn same_state_same_session_is_rate_limited() {
        let engine = RenderingEngine::new(None, EngineConfig::default());
        assert!(!engine.is_rate_limited("session-1", AgentState::Working));
        assert!(engine.is_rate_limited("session-1", AgentState::Working));
    }

    #[test]
    fn different_session_not_rate_limited() {
        let engine = RenderingEngine::new(None, EngineConfig::default());
        assert!(!engine.is_rate_limited("session-1", AgentState::Working));
        assert!(!engine.is_rate_limited("session-2", AgentState::Working));
    }

    #[test]
    fn different_state_not_rate_limited() {
        let engine = RenderingEngine::new(None, EngineConfig::default());
        assert!(!engine.is_rate_limited("session-1", AgentState::Working));
        assert!(!engine.is_rate_limited("session-1", AgentState::Attention));
    }

    // ---------------------------------------------------------------
    // renderer registration
    // ---------------------------------------------------------------

    #[test]
    fn empty_engine_has_zero_renderers() {
        let engine = RenderingEngine::new(None, EngineConfig::default());
        assert_eq!(engine.renderer_count(), 0);
    }

    #[test]
    fn engine_returns_registered_names() {
        let mut engine = RenderingEngine::new(None, EngineConfig::default());
        let renderer: Box<dyn Renderer> = Box::new(NoopRenderer::new("r1"));
        engine.add_renderer("r1", renderer);
        assert_eq!(engine.renderer_names(), vec!["r1"]);
        assert_eq!(engine.renderer_count(), 1);
    }

    // ---------------------------------------------------------------
    // NotificationPolicy
    // ---------------------------------------------------------------

    #[test]
    fn policy_notifies_attention_risk_result_error() {
        let policy = NotificationPolicy::default();
        assert!(policy.should_notify(AgentState::Attention));
        assert!(policy.should_notify(AgentState::Risk));
        assert!(policy.should_notify(AgentState::Result));
        assert!(policy.should_notify(AgentState::Error));
    }

    #[test]
    fn policy_suppresses_working() {
        let policy = NotificationPolicy::default();
        assert!(!policy.should_notify(AgentState::Working));
    }

    #[test]
    fn quiet_hours_disabled_by_default() {
        let policy = NotificationPolicy::default();
        assert!(!policy.is_quiet_hours());
    }

    #[test]
    fn quiet_hours_risk_allowed() {
        let policy = NotificationPolicy {
            quiet_hours_start: Some(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            quiet_hours_end: Some(chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap()),
            ..Default::default()
        };
        assert!(policy.quiet_hours_allows(AgentState::Risk));
        assert!(!policy.quiet_hours_allows(AgentState::Attention));
    }

    // ---------------------------------------------------------------
    // detect_all with real terminal context
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn detect_all_returns_results_per_renderer() {
        let mut engine = RenderingEngine::new(None, EngineConfig::default());
        engine.add_renderer("r1", Box::new(NoopRenderer::new("r1")));
        engine.add_renderer("r2", Box::new(NoopRenderer::new("r2")));

        let ctx = TerminalContext {
            tmux_pane: Some("%0".into()),
            ..Default::default()
        };
        let results = engine.detect_all(&ctx).await;
        assert_eq!(results.len(), 2);
    }

    // ---------------------------------------------------------------
    // RenderFailed events via render result
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn render_returns_errors_as_render_results() {
        let mut engine = RenderingEngine::new(None, EngineConfig::default());
        engine.add_renderer("failer", Box::new(FailingRenderer));

        let target = RenderTarget {
            session_id: "test-session".into(),
            terminal: TerminalContext::default(),
        };
        let view = StateView {
            state: AgentState::Working,
            label: None,
            target,
        };
        let results = engine.render(&view).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].error.is_some());
    }

    #[tokio::test]
    async fn reset_returns_errors_per_renderer() {
        let mut engine = RenderingEngine::new(None, EngineConfig::default());
        engine.add_renderer("failer", Box::new(FailingRenderer));

        let target = RenderTarget {
            session_id: "any".into(),
            terminal: TerminalContext::default(),
        };
        let results = engine.reset(&target).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].error.is_some());
    }

    // ---------------------------------------------------------------
    // Rate limiting integration with render
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn rate_limited_render_returns_empty() {
        let mut engine = RenderingEngine::new(None, EngineConfig::default());
        engine.add_renderer("r1", Box::new(NoopRenderer::new("r1")));

        let target = RenderTarget {
            session_id: "session-1".into(),
            terminal: TerminalContext::default(),
        };
        let view = StateView {
            state: AgentState::Working,
            label: None,
            target: target.clone(),
        };

        let first = engine.render(&view).await;
        assert!(!first.is_empty(), "first render should go through");

        let second = engine.render(&view).await;
        assert!(second.is_empty(), "second render should be rate limited");
    }

    // ---------------------------------------------------------------
    // NoopRenderer for tests
    // ---------------------------------------------------------------

    use async_trait::async_trait;

    struct NoopRenderer {
        #[allow(dead_code)]
        name: String,
    }

    impl NoopRenderer {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl Renderer for NoopRenderer {
        async fn detect(
            &self,
            _ctx: &TerminalContext,
        ) -> Result<RendererCapabilities, RendererError> {
            Ok(RendererCapabilities {
                pane_border: true,
                ..Default::default()
            })
        }

        async fn render(&self, _view: &StateView) -> Result<(), RendererError> {
            Ok(())
        }

        async fn reset(&self, _target: &RenderTarget) -> Result<(), RendererError> {
            Ok(())
        }

        async fn health_check(&self) -> HealthStatus {
            HealthStatus::Healthy
        }
    }

    struct FailingRenderer;

    #[async_trait]
    impl Renderer for FailingRenderer {
        async fn detect(
            &self,
            _ctx: &TerminalContext,
        ) -> Result<RendererCapabilities, RendererError> {
            Err(RendererError::Unsupported("always fails".into()))
        }

        async fn render(&self, _view: &StateView) -> Result<(), RendererError> {
            Err(RendererError::Failed("simulated render failure".into()))
        }

        async fn reset(&self, _target: &RenderTarget) -> Result<(), RendererError> {
            Err(RendererError::Failed("simulated reset failure".into()))
        }

        async fn health_check(&self) -> HealthStatus {
            HealthStatus::Unavailable {
                reason: "simulated".into(),
            }
        }
    }
}
