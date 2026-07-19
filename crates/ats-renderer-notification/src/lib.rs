//! # `ats-renderer-notification` — macOS native notification renderer (I-11)
//!
//! Delivers macOS notifications for attention-grabbing states (ATTENTION,
//! RISK, RESULT, ERROR). Uses `terminal-notifier` as the delivery
//! mechanism; the app-bundle helper (`ats-notifier.app`, I-05 spike #4) is
//! deferred to a future phase.
//!
//! Policy: debounce (10s), quiet-hours suppression, RISK-always-notify.
//! All notification failures are fail-open (logged, never propagate to
//! agent).

use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;

use async_trait::async_trait;
use ats_config::theme::{Theme, ThemeEntry};
use ats_core::{ActivityLabel, AgentState, TerminalContext};
use ats_renderer::{
    HealthStatus, RenderTarget, Renderer, RendererCapabilities, RendererError, StateView,
};
use chrono::{DateTime, NaiveTime, Utc};

const NOTIFIER_BIN: &str = "terminal-notifier";
const DEBOUNCE_S: i64 = 10;
const RESULT_DEBOUNCE_S: i64 = 30;

pub fn find_notifier() -> Option<String> {
    let which = Command::new("which").arg(NOTIFIER_BIN).output().ok()?;
    if which.status.success() {
        let path = String::from_utf8(which.stdout).ok()?;
        Some(path.trim().to_string())
    } else {
        None
    }
}

pub struct NotificationRenderer {
    theme: Option<Theme>,
    notifier_path: Option<String>,
    quiet_hours_start: Option<NaiveTime>,
    quiet_hours_end: Option<NaiveTime>,
    quiet_hours_states: Vec<String>,
    last_notify: Mutex<HashMap<NotificationKey, DateTime<Utc>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct NotificationKey {
    state: AgentState,
    session_id: String,
}

impl NotificationRenderer {
    pub fn new(
        theme: Option<Theme>,
        quiet_hours_start: Option<NaiveTime>,
        quiet_hours_end: Option<NaiveTime>,
        quiet_hours_states: Vec<String>,
    ) -> Self {
        let notifier_path = find_notifier();
        Self {
            theme,
            notifier_path,
            quiet_hours_start,
            quiet_hours_end,
            quiet_hours_states,
            last_notify: Mutex::new(HashMap::new()),
        }
    }

    fn resolve_entry(&self, state: AgentState) -> Option<ThemeEntry> {
        self.theme.as_ref().and_then(|t| t.resolve(state))
    }

    fn should_notify(&self, state: AgentState) -> bool {
        match state {
            AgentState::Attention | AgentState::Risk | AgentState::Result | AgentState::Error => {
                true
            }
            AgentState::Working | AgentState::Idle | AgentState::Unknown => false,
        }
    }

    fn is_quiet_hours(&self) -> bool {
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

    fn quiet_hours_allows(&self, state: AgentState) -> bool {
        if !self.is_quiet_hours() {
            return true;
        }
        if self.quiet_hours_states.is_empty() {
            return state == AgentState::Risk;
        }
        let state_key = agent_state_to_key(state);
        self.quiet_hours_states.iter().any(|s| s == state_key)
    }

    fn is_debounced(&self, state: AgentState, session_id: &str) -> bool {
        let key = NotificationKey {
            state,
            session_id: session_id.to_string(),
        };
        let mut last = self.last_notify.lock().unwrap();
        if let Some(prev) = last.get(&key) {
            let elapsed = Utc::now().signed_duration_since(*prev).num_seconds();
            let threshold = if state == AgentState::Result {
                RESULT_DEBOUNCE_S
            } else {
                DEBOUNCE_S
            };
            if elapsed < threshold {
                return true;
            }
        }
        last.insert(key, Utc::now());
        false
    }

    fn build_title(&self, state: AgentState) -> String {
        let entry = self.resolve_entry(state);
        let symbol = entry.as_ref().map(|e| e.symbol.as_str()).unwrap_or("?");
        let label = entry.as_ref().map(|e| e.label.as_str()).unwrap_or("");
        format!("{symbol} {label}")
    }

    fn build_body(&self, state: AgentState, label: Option<&ActivityLabel>) -> String {
        let entry = self.resolve_entry(state);
        let state_label = entry.as_ref().map(|e| e.label.as_str()).unwrap_or("");
        match label {
            Some(l) if !l.as_str().is_empty() => {
                format!("{state_label}: {}", l.as_str())
            }
            _ => state_label.to_string(),
        }
    }

    fn build_sound(&self, state: AgentState) -> &'static str {
        match state {
            AgentState::Risk => "Basso",
            AgentState::Error => "Basso",
            _ => "default",
        }
    }

    fn send_notification(&self, title: &str, body: &str, sound: &str) -> Result<(), RendererError> {
        let path = self.notifier_path.as_deref().unwrap_or(NOTIFIER_BIN);
        let result = Command::new(path)
            .args([
                "-title",
                title,
                "-message",
                body,
                "-sound",
                sound,
                "-sender",
                "com.googlecode.iterm2",
            ])
            .output();
        match result {
            Ok(out) if out.status.success() => Ok(()),
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                Err(RendererError::Failed(format!(
                    "notification failed: {}",
                    stderr.trim()
                )))
            }
            Err(e) => Err(RendererError::Failed(format!("cannot run notifier: {e}"))),
        }
    }
}

fn agent_state_to_key(state: AgentState) -> &'static str {
    match state {
        AgentState::Idle => "idle",
        AgentState::Working => "working",
        AgentState::Attention => "attention",
        AgentState::Risk => "risk",
        AgentState::Result => "result",
        AgentState::Error => "error",
        AgentState::Unknown => "unknown",
    }
}

#[async_trait]
impl Renderer for NotificationRenderer {
    async fn detect(&self, _ctx: &TerminalContext) -> Result<RendererCapabilities, RendererError> {
        if self.notifier_path.is_some() {
            Ok(RendererCapabilities {
                notification: true,
                ..Default::default()
            })
        } else {
            Err(RendererError::Unsupported(format!(
                "{NOTIFIER_BIN} not found — notifications unavailable"
            )))
        }
    }

    async fn render(&self, view: &StateView) -> Result<(), RendererError> {
        if !self.should_notify(view.state) {
            return Ok(());
        }

        if !self.quiet_hours_allows(view.state) {
            return Ok(());
        }

        if self.is_debounced(view.state, &view.target.session_id) {
            return Ok(());
        }

        let title = self.build_title(view.state);
        let body = self.build_body(view.state, view.label.as_ref());
        let sound = self.build_sound(view.state);

        match self.send_notification(&title, &body, sound) {
            Ok(()) => Ok(()),
            Err(_) => Ok(()),
        }
    }

    async fn reset(&self, _target: &RenderTarget) -> Result<(), RendererError> {
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        if self.notifier_path.is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unavailable {
                reason: format!("{NOTIFIER_BIN} not found"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ats_config::theme::Theme;
    use ats_core::ActivityLabel;

    fn default_theme() -> Theme {
        Theme::load_bundled("default").expect("default theme must load")
    }

    fn test_renderer() -> NotificationRenderer {
        NotificationRenderer::new(Some(default_theme()), None, None, vec![])
    }

    // ---------------------------------------------------------------
    // should_notify
    // ---------------------------------------------------------------

    #[test]
    fn notify_enabled_for_attention_risk_result_error() {
        let r = test_renderer();
        assert!(r.should_notify(AgentState::Attention));
        assert!(r.should_notify(AgentState::Risk));
        assert!(r.should_notify(AgentState::Result));
        assert!(r.should_notify(AgentState::Error));
    }

    #[test]
    fn notify_disabled_for_working_idle_unknown() {
        let r = test_renderer();
        assert!(!r.should_notify(AgentState::Working));
        assert!(!r.should_notify(AgentState::Idle));
        assert!(!r.should_notify(AgentState::Unknown));
    }

    // ---------------------------------------------------------------
    // quiet hours
    // ---------------------------------------------------------------

    #[test]
    fn quiet_hours_disabled_when_none_configured() {
        let r = test_renderer();
        assert!(!r.is_quiet_hours());
    }

    #[test]
    fn quiet_hours_active_during_range() {
        let r = NotificationRenderer::new(
            Some(default_theme()),
            Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            Some(NaiveTime::from_hms_opt(23, 59, 59).unwrap()),
            vec!["risk".into()],
        );
        assert!(r.is_quiet_hours());
    }

    #[test]
    fn quiet_hours_risk_always_allowed_by_default() {
        let r = NotificationRenderer::new(
            Some(default_theme()),
            Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            Some(NaiveTime::from_hms_opt(23, 59, 59).unwrap()),
            vec![],
        );
        assert!(r.quiet_hours_allows(AgentState::Risk));
    }

    #[test]
    fn quiet_hours_suppresses_attention_when_not_in_allowlist() {
        let r = NotificationRenderer::new(
            Some(default_theme()),
            Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            Some(NaiveTime::from_hms_opt(23, 59, 59).unwrap()),
            vec!["risk".into()],
        );
        assert!(!r.quiet_hours_allows(AgentState::Attention));
    }

    #[test]
    fn quiet_hours_allows_configured_states() {
        let r = NotificationRenderer::new(
            Some(default_theme()),
            Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            Some(NaiveTime::from_hms_opt(23, 59, 59).unwrap()),
            vec!["risk".into(), "error".into()],
        );
        assert!(r.quiet_hours_allows(AgentState::Risk));
        assert!(r.quiet_hours_allows(AgentState::Error));
        assert!(!r.quiet_hours_allows(AgentState::Attention));
    }

    // ---------------------------------------------------------------
    // debounce
    // ---------------------------------------------------------------

    #[test]
    fn first_notification_not_debounced() {
        let r = test_renderer();
        assert!(!r.is_debounced(AgentState::Attention, "session-1"));
    }

    #[test]
    fn repeated_notification_is_debounced() {
        let r = test_renderer();
        assert!(!r.is_debounced(AgentState::Attention, "session-1"));
        assert!(r.is_debounced(AgentState::Attention, "session-1"));
    }

    #[test]
    fn different_sessions_not_debounced() {
        let r = test_renderer();
        assert!(!r.is_debounced(AgentState::Attention, "session-1"));
        assert!(!r.is_debounced(AgentState::Attention, "session-2"));
    }

    #[test]
    fn different_states_not_debounced() {
        let r = test_renderer();
        assert!(!r.is_debounced(AgentState::Attention, "session-1"));
        assert!(!r.is_debounced(AgentState::Risk, "session-1"));
    }

    // ---------------------------------------------------------------
    // title and body
    // ---------------------------------------------------------------

    #[test]
    fn title_contains_symbol_and_label() {
        let r = test_renderer();
        let title = r.build_title(AgentState::Attention);
        assert!(title.contains("!"));
        assert!(title.contains("Needs input"));
    }

    #[test]
    fn body_includes_activity_label() {
        let r = test_renderer();
        let label = ActivityLabel::new("Review deployment");
        let body = r.build_body(AgentState::Risk, Some(&label));
        assert!(body.contains("Review deployment"));
    }

    #[test]
    fn body_without_activity_label_shows_state() {
        let r = test_renderer();
        let body = r.build_body(AgentState::Error, None);
        assert!(body.contains("Error"));
    }

    // ---------------------------------------------------------------
    // sound
    // ---------------------------------------------------------------

    #[test]
    fn risk_uses_alert_sound() {
        let r = test_renderer();
        assert_eq!(r.build_sound(AgentState::Risk), "Basso");
        assert_eq!(r.build_sound(AgentState::Error), "Basso");
    }

    #[test]
    fn attention_uses_default_sound() {
        let r = test_renderer();
        assert_eq!(r.build_sound(AgentState::Attention), "default");
        assert_eq!(r.build_sound(AgentState::Result), "default");
    }

    // ---------------------------------------------------------------
    // RISK always notifies
    // ---------------------------------------------------------------

    #[test]
    fn risk_notifies_even_in_quiet_hours() {
        let r = NotificationRenderer::new(
            Some(default_theme()),
            Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            Some(NaiveTime::from_hms_opt(23, 59, 59).unwrap()),
            vec![],
        );
        assert!(r.quiet_hours_allows(AgentState::Risk));
    }

    // ---------------------------------------------------------------
    // All 5 bundled themes resolve
    // ---------------------------------------------------------------

    #[test]
    fn all_bundled_themes_build_title() {
        for name in Theme::bundle_names() {
            let theme = Theme::load_bundled(name).expect("theme must load");
            let r = NotificationRenderer::new(Some(theme), None, None, vec![]);
            for &state in &[
                AgentState::Attention,
                AgentState::Risk,
                AgentState::Result,
                AgentState::Error,
            ] {
                let title = r.build_title(state);
                assert!(
                    !title.is_empty(),
                    "{name}: {state:?} title must not be empty"
                );
            }
        }
    }

    // ---------------------------------------------------------------
    // Fail-open: render never propagates errors
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn render_fails_open_for_unknown_state() {
        let r = test_renderer();
        let ctx = TerminalContext::default();
        let target = RenderTarget {
            session_id: "test-session".into(),
            terminal: ctx,
        };
        let view = StateView {
            state: AgentState::Working,
            label: None,
            target,
        };
        let result = r.render(&view).await;
        assert!(
            result.is_ok(),
            "Working should not notify but must not error"
        );
    }

    #[tokio::test]
    async fn render_is_ok_for_notifiable_state() {
        let r = test_renderer();
        let ctx = TerminalContext::default();
        let target = RenderTarget {
            session_id: "test-session-attention".into(),
            terminal: ctx,
        };
        let view = StateView {
            state: AgentState::Attention,
            label: None,
            target,
        };
        let result = r.render(&view).await;
        assert!(
            result.is_ok(),
            "render must be fail-open even if notifier missing"
        );
    }

    #[tokio::test]
    async fn reset_is_always_ok() {
        let r = test_renderer();
        let target = RenderTarget {
            session_id: "any".into(),
            terminal: TerminalContext::default(),
        };
        let result = r.reset(&target).await;
        assert!(result.is_ok());
    }
}
