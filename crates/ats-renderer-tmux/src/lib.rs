//! # `ats-renderer-tmux` — tmux pane border/title renderer (I-09)
//!
//! Reflects agent state on a tmux pane via `pane-border-style` (color)
//! and `pane-border-format` (symbol + label). Implements the [`Renderer`]
//! trait from `ats-renderer`.
//!
//! ## Pane safety (I-05 spike #2)
//! tmux < 3.7 leaks `set-option -p` to the entire window, violating pane
//! isolation. This renderer gates `pane_border` capability on tmux >= 3.7
//! and never calls tmux without `-t <pane>`.
//!
//! ## Allowlist (SPEC §14.2)
//! No provider-derived string ever reaches a shell. Pane identifiers
//! and format strings are validated before use.

use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;

use async_trait::async_trait;
use ats_config::theme::{Theme, ThemeEntry};
use ats_core::{ActivityLabel, AgentState, TerminalContext};
use ats_renderer::{
    HealthStatus, RenderTarget, Renderer, RendererCapabilities, RendererError, StateView,
};

const MIN_TMUX_VERSION: (u32, u32) = (3, 7);

const FORBIDDEN_CHARS: &[char] = &[';', '$', '`', '|', '&', '>', '<', '\n', '\r', '\\'];

#[derive(Default)]
struct CapturedState {
    border_style: Option<String>,
    border_format: Option<String>,
    border_status: Option<String>,
}

pub struct TmuxRenderer {
    theme: Option<Theme>,
    prior: Mutex<HashMap<String, CapturedState>>,
}

impl TmuxRenderer {
    pub fn new(theme: Option<Theme>) -> Self {
        Self {
            theme,
            prior: Mutex::new(HashMap::new()),
        }
    }

    fn resolve_entry(&self, state: AgentState) -> Option<ThemeEntry> {
        self.theme.as_ref().and_then(|t| t.resolve(state))
    }

    pub fn border_style_for(&self, state: AgentState) -> Option<String> {
        if state == AgentState::Idle {
            return None;
        }
        let entry = self.resolve_entry(state)?;
        let color = entry.color.as_ref()?;
        Some(format!("fg={color}"))
    }

    pub fn border_format_for(
        &self,
        state: AgentState,
        label: Option<&ActivityLabel>,
    ) -> Option<String> {
        if state == AgentState::Idle {
            return None;
        }
        let entry = self.resolve_entry(state);
        let symbol = entry.as_ref().map(|e| e.symbol.as_str()).unwrap_or("?");
        let state_label = entry.as_ref().map(|e| e.label.as_str()).unwrap_or("");

        let mut parts: Vec<&str> = Vec::new();
        if !symbol.is_empty() {
            parts.push(symbol);
        }
        if !state_label.is_empty() {
            parts.push(state_label);
        }
        if let Some(l) = label {
            let s = l.as_str();
            if !s.is_empty() {
                parts.push(s);
            }
        }
        if parts.is_empty() {
            return None;
        }
        Some(format!(" {} ", parts.join(" · ")))
    }

    pub fn validate_pane(pane: &str) -> bool {
        if pane.len() < 2 {
            return false;
        }
        if !pane.starts_with('%') && !pane.starts_with('=') {
            return false;
        }
        pane[1..].chars().all(|c| c.is_ascii_digit())
    }

    pub fn validate_arg(s: &str) -> bool {
        !s.contains(FORBIDDEN_CHARS)
    }

    pub fn parse_tmux_version(raw: &str) -> Option<(u32, u32)> {
        let start = raw.find(|c: char| c.is_ascii_digit())?;
        let tail = &raw[start..];
        let end = tail
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(tail.len());
        let mut parts = tail[..end].split('.');
        let major: u32 = parts.next()?.parse().ok()?;
        let minor: u32 = parts.next().unwrap_or("0").parse().ok()?;
        Some((major, minor))
    }

    fn check_tmux_version() -> Option<(u32, u32)> {
        let out = Command::new("tmux").arg("-V").output().ok()?;
        if !out.status.success() {
            return None;
        }
        let raw = String::from_utf8(out.stdout).ok()?;
        Self::parse_tmux_version(&raw)
    }

    fn tmux_exists() -> bool {
        Command::new("which")
            .arg("tmux")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn capture_pane_state(pane: &str) -> Option<CapturedState> {
        let border_style = Self::tmux_output(
            &["display-message", "-p", "-F", "#{pane-border-style}"],
            pane,
        );
        let border_format = Self::tmux_output(
            &["display-message", "-p", "-F", "#{pane-border-format}"],
            pane,
        );
        let border_status = Self::tmux_output(
            &["display-message", "-p", "-F", "#{pane-border-status}"],
            pane,
        );
        Some(CapturedState {
            border_style,
            border_format,
            border_status,
        })
    }

    fn tmux_output(args: &[&str], pane: &str) -> Option<String> {
        let mut cmd = Command::new("tmux");
        for a in args {
            cmd.arg(a);
        }
        cmd.arg("-t").arg(pane);
        let out = cmd.output().ok()?;
        if out.status.success() {
            Some(String::from_utf8(out.stdout).ok()?.trim().to_string())
        } else {
            None
        }
    }

    fn run_tmux(args: &[&str], pane: &str) -> Result<(), RendererError> {
        let mut cmd = Command::new("tmux");
        // Insert -t <pane> after the subcommand for correct arg order with set-option.
        if let Some((first, rest)) = args.split_first() {
            cmd.arg(first);
            cmd.arg("-t").arg(pane);
            for a in rest {
                cmd.arg(a);
            }
        } else {
            cmd.arg("-t").arg(pane);
        }
        let out = cmd
            .output()
            .map_err(|e| RendererError::Failed(format!("cannot run tmux: {e}")))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(RendererError::Failed(format!(
                "tmux command failed: {}",
                stderr.trim()
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl Renderer for TmuxRenderer {
    async fn detect(&self, ctx: &TerminalContext) -> Result<RendererCapabilities, RendererError> {
        if ctx.tmux_pane.is_none() {
            return Err(RendererError::Unsupported(
                "no TMUX_PANE in terminal context".into(),
            ));
        }

        let version = Self::check_tmux_version()
            .ok_or_else(|| RendererError::Failed("cannot determine tmux version".into()))?;

        let pane_safe = version >= MIN_TMUX_VERSION;

        Ok(RendererCapabilities {
            pane_border: pane_safe,
            reset_reliable: pane_safe,
            ..Default::default()
        })
    }

    async fn render(&self, view: &StateView) -> Result<(), RendererError> {
        let pane =
            view.target.terminal.tmux_pane.as_deref().ok_or_else(|| {
                RendererError::Unsupported("no tmux pane in render target".into())
            })?;

        if !Self::validate_pane(pane) {
            return Err(RendererError::Failed(format!("invalid tmux pane: {pane}")));
        }

        if !self.prior.lock().unwrap().contains_key(pane) {
            if let Some(captured) = Self::capture_pane_state(pane) {
                self.prior
                    .lock()
                    .unwrap()
                    .insert(pane.to_string(), captured);
            }
        }

        if view.state == AgentState::Idle {
            Self::run_tmux(&["set-option", "-p", "-u", "pane-border-style"], pane)?;
            Self::run_tmux(&["set-option", "-p", "-u", "pane-border-format"], pane)?;
            Self::run_tmux(&["set-option", "-p", "-u", "pane-border-status"], pane)?;
            return Ok(());
        }

        if let Some(style) = self.border_style_for(view.state) {
            Self::run_tmux(&["set-option", "-p", "pane-border-style", &style], pane)?;
        }

        if let Some(format) = self.border_format_for(view.state, view.label.as_ref()) {
            if Self::validate_arg(&format) {
                Self::run_tmux(&["set-option", "-p", "pane-border-status", "top"], pane)?;
                Self::run_tmux(&["set-option", "-p", "pane-border-format", &format], pane)?;
            }
        }

        Ok(())
    }

    async fn reset(&self, target: &RenderTarget) -> Result<(), RendererError> {
        let pane = target
            .terminal
            .tmux_pane
            .as_deref()
            .ok_or_else(|| RendererError::Unsupported("no tmux pane in reset target".into()))?;

        if !Self::validate_pane(pane) {
            return Err(RendererError::Failed(format!("invalid tmux pane: {pane}")));
        }

        let captured = self.prior.lock().unwrap().remove(pane);

        match captured {
            Some(ref cs) => {
                if let Some(ref style) = cs.border_style {
                    Self::run_tmux(&["set-option", "-p", "pane-border-style", style], pane)?;
                } else {
                    Self::run_tmux(&["set-option", "-p", "-u", "pane-border-style"], pane)?;
                }
                if let Some(ref fmt) = cs.border_format {
                    Self::run_tmux(&["set-option", "-p", "pane-border-format", fmt], pane)?;
                } else {
                    Self::run_tmux(&["set-option", "-p", "-u", "pane-border-format"], pane)?;
                }
                if let Some(ref status) = cs.border_status {
                    Self::run_tmux(&["set-option", "-p", "pane-border-status", status], pane)?;
                } else {
                    Self::run_tmux(&["set-option", "-p", "-u", "pane-border-status"], pane)?;
                }
            }
            None => {
                Self::run_tmux(&["set-option", "-p", "-u", "pane-border-style"], pane)?;
                Self::run_tmux(&["set-option", "-p", "-u", "pane-border-format"], pane)?;
                Self::run_tmux(&["set-option", "-p", "-u", "pane-border-status"], pane)?;
            }
        }

        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        if !Self::tmux_exists() {
            return HealthStatus::Unavailable {
                reason: "tmux not found".into(),
            };
        }
        match Self::check_tmux_version() {
            Some(v) if v >= MIN_TMUX_VERSION => HealthStatus::Healthy,
            Some(_) => HealthStatus::Degraded {
                reason: format!(
                    "tmux < {}.{} — pane border not isolated",
                    MIN_TMUX_VERSION.0, MIN_TMUX_VERSION.1
                ),
            },
            None => HealthStatus::Degraded {
                reason: "cannot determine tmux version".into(),
            },
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

    fn test_renderer() -> TmuxRenderer {
        TmuxRenderer::new(Some(default_theme()))
    }

    // ---------------------------------------------------------------
    // parse_tmux_version
    // ---------------------------------------------------------------

    #[test]
    fn parses_standard_tmux_versions() {
        assert_eq!(TmuxRenderer::parse_tmux_version("tmux 3.4"), Some((3, 4)));
        assert_eq!(TmuxRenderer::parse_tmux_version("tmux 3.5a"), Some((3, 5)));
        assert_eq!(TmuxRenderer::parse_tmux_version("tmux 3.7b"), Some((3, 7)));
        assert_eq!(
            TmuxRenderer::parse_tmux_version("tmux next-3.8"),
            Some((3, 8))
        );
        assert_eq!(TmuxRenderer::parse_tmux_version("tmux 4.0"), Some((4, 0)));
        assert_eq!(TmuxRenderer::parse_tmux_version("tmux 3.10"), Some((3, 10)));
    }

    #[test]
    fn parse_tmux_version_returns_none_for_unparseable() {
        assert_eq!(TmuxRenderer::parse_tmux_version("tmux master"), None);
        assert_eq!(TmuxRenderer::parse_tmux_version(""), None);
        assert_eq!(TmuxRenderer::parse_tmux_version("no version"), None);
    }

    #[test]
    fn pane_scope_boundary_is_3_7() {
        for (raw, safe) in [
            ("tmux 3.4", false),
            ("tmux 3.5a", false),
            ("tmux 3.6", false),
            ("tmux 3.7", true),
            ("tmux 3.7b", true),
            ("tmux next-3.8", true),
            ("tmux 4.0", true),
        ] {
            let version = TmuxRenderer::parse_tmux_version(raw).expect(raw);
            assert_eq!(
                version >= MIN_TMUX_VERSION,
                safe,
                "boundary check for {raw}"
            );
        }
    }

    // ---------------------------------------------------------------
    // validate_pane
    // ---------------------------------------------------------------

    #[test]
    fn validates_correct_pane_ids() {
        assert!(TmuxRenderer::validate_pane("%0"));
        assert!(TmuxRenderer::validate_pane("%12"));
        assert!(TmuxRenderer::validate_pane("%999"));
        assert!(TmuxRenderer::validate_pane("=0"));
        assert!(TmuxRenderer::validate_pane("=42"));
    }

    #[test]
    fn rejects_invalid_pane_ids() {
        assert!(!TmuxRenderer::validate_pane(""));
        assert!(!TmuxRenderer::validate_pane("%"));
        assert!(!TmuxRenderer::validate_pane("%abc"));
        assert!(!TmuxRenderer::validate_pane("12"));
        assert!(!TmuxRenderer::validate_pane("#12"));
        assert!(!TmuxRenderer::validate_pane("%1;echo x"));
    }

    // ---------------------------------------------------------------
    // validate_arg
    // ---------------------------------------------------------------

    #[test]
    fn validate_arg_allows_safe_strings() {
        assert!(TmuxRenderer::validate_arg("hello"));
        assert!(TmuxRenderer::validate_arg("● Working"));
        assert!(TmuxRenderer::validate_arg("Needs input · Running test"));
        assert!(TmuxRenderer::validate_arg("× Error"));
        assert!(TmuxRenderer::validate_arg(" · !! Risk · "));
    }

    #[test]
    fn validate_arg_rejects_injection_attempts() {
        assert!(!TmuxRenderer::validate_arg("hello; rm -rf /"));
        assert!(!TmuxRenderer::validate_arg("$(whoami)"));
        assert!(!TmuxRenderer::validate_arg("`ls`"));
        assert!(!TmuxRenderer::validate_arg("echo | cat"));
        assert!(!TmuxRenderer::validate_arg(">&2"));
        assert!(!TmuxRenderer::validate_arg("hello\nbad"));
    }

    // ---------------------------------------------------------------
    // border_style_for
    // ---------------------------------------------------------------

    #[test]
    fn idyle_returns_none() {
        let r = test_renderer();
        assert_eq!(r.border_style_for(AgentState::Idle), None);
    }

    #[test]
    fn active_states_return_color() {
        let r = test_renderer();
        assert!(r
            .border_style_for(AgentState::Working)
            .unwrap()
            .contains("#2457A6"));
        assert!(r
            .border_style_for(AgentState::Attention)
            .unwrap()
            .contains("#D97706"));
        assert!(r
            .border_style_for(AgentState::Risk)
            .unwrap()
            .contains("#B91C1C"));
        assert!(r
            .border_style_for(AgentState::Result)
            .unwrap()
            .contains("#15803D"));
        assert!(r
            .border_style_for(AgentState::Error)
            .unwrap()
            .contains("#9333EA"));
        assert!(r
            .border_style_for(AgentState::Unknown)
            .unwrap()
            .contains("#6B7280"));
    }

    #[test]
    fn no_theme_returns_none_for_all_states() {
        let r = TmuxRenderer::new(None);
        for state in [
            AgentState::Idle,
            AgentState::Working,
            AgentState::Attention,
            AgentState::Risk,
            AgentState::Result,
            AgentState::Error,
            AgentState::Unknown,
        ] {
            assert_eq!(
                r.border_style_for(state),
                None,
                "no theme should produce None for {state:?}"
            );
        }
    }

    // ---------------------------------------------------------------
    // border_format_for
    // ---------------------------------------------------------------

    #[test]
    fn idle_returns_none() {
        let r = test_renderer();
        assert_eq!(r.border_format_for(AgentState::Idle, None), None);
    }

    #[test]
    fn active_states_contain_symbol_and_label() {
        let r = test_renderer();
        let fmt = r.border_format_for(AgentState::Working, None).unwrap();
        assert!(fmt.contains("●"));
        assert!(fmt.contains("Working"));
    }

    #[test]
    fn risk_state_contains_double_exclamation() {
        let r = test_renderer();
        let fmt = r.border_format_for(AgentState::Risk, None).unwrap();
        assert!(fmt.contains("!!"));
        assert!(fmt.contains("Risk"));
    }

    #[test]
    fn includes_activity_label_when_present() {
        let r = test_renderer();
        let label = ActivityLabel::new("Running cargo test");
        let fmt = r
            .border_format_for(AgentState::Working, Some(&label))
            .unwrap();
        assert!(fmt.contains("Running cargo test"));
    }

    #[test]
    fn activity_label_is_trimmed_by_activitylabel() {
        let r = test_renderer();
        let long = "a".repeat(100);
        let label = ActivityLabel::new(&long);
        let fmt = r
            .border_format_for(AgentState::Working, Some(&label))
            .unwrap();
        assert!(fmt.len() <= 60);
    }

    #[test]
    fn empty_activity_label_still_shows_state() {
        let r = test_renderer();
        let label = ActivityLabel::new("");
        let fmt = r
            .border_format_for(AgentState::Working, Some(&label))
            .unwrap();
        assert!(fmt.contains("●"));
        assert!(fmt.contains("Working"));
    }

    // ---------------------------------------------------------------
    // detect prerequisite checks (logic-only, no tmux call)
    // ---------------------------------------------------------------

    fn ctx_with_pane(pane: &str) -> TerminalContext {
        TerminalContext {
            tmux_pane: Some(pane.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn detect_requires_pane_from_context() {
        let ctx = TerminalContext::default();
        assert!(ctx.tmux_pane.is_none());
    }

    #[test]
    fn detect_ctx_with_pane_has_pane() {
        let ctx = ctx_with_pane("%12");
        assert_eq!(ctx.tmux_pane.as_deref(), Some("%12"));
    }

    // ---------------------------------------------------------------
    // All 7 states have theme resolution
    // ---------------------------------------------------------------

    #[test]
    fn all_default_theme_states_have_color() {
        let r = test_renderer();
        let states_with_color = [
            AgentState::Working,
            AgentState::Attention,
            AgentState::Risk,
            AgentState::Result,
            AgentState::Error,
            AgentState::Unknown,
        ];
        for &state in &states_with_color {
            assert!(
                r.border_style_for(state).is_some(),
                "state {state:?} should have color in default theme"
            );
        }
    }

    #[test]
    fn all_default_theme_states_have_format() {
        let r = test_renderer();
        let states_with_format = [
            AgentState::Working,
            AgentState::Attention,
            AgentState::Risk,
            AgentState::Result,
            AgentState::Error,
            AgentState::Unknown,
        ];
        for &state in &states_with_format {
            assert!(
                r.border_format_for(state, None).is_some(),
                "state {state:?} should have format in default theme"
            );
        }
    }

    #[test]
    fn idle_has_no_color_and_no_format() {
        let r = test_renderer();
        assert_eq!(r.border_style_for(AgentState::Idle), None);
        assert_eq!(r.border_format_for(AgentState::Idle, None), None);
    }

    // ---------------------------------------------------------------
    // Two-representation rule (SPEC §10.3)
    // ---------------------------------------------------------------

    #[test]
    fn two_representation_rule_per_state() {
        let theme = default_theme();
        for state in [
            AgentState::Idle,
            AgentState::Working,
            AgentState::Attention,
            AgentState::Risk,
            AgentState::Result,
            AgentState::Error,
            AgentState::Unknown,
        ] {
            let entry = theme.resolve(state);
            if state == AgentState::Idle {
                assert!(entry.is_some());
                let e = entry.unwrap();
                assert!(e.color.is_none(), "Idle should have no color");
                assert!(!e.symbol.is_empty(), "Idle must have symbol");
                assert!(!e.label.is_empty(), "Idle must have label");
            } else {
                assert!(entry.is_some(), "{state:?} must have a theme entry");
                let e = entry.unwrap();
                let reps = [e.color.is_some(), !e.symbol.is_empty(), !e.label.is_empty()]
                    .iter()
                    .filter(|&&x| x)
                    .count();
                assert!(reps >= 2, "{state:?} has {reps} representations, need >= 2");
            }
        }
    }

    // ---------------------------------------------------------------
    // All 5 bundled themes resolve all states
    // ---------------------------------------------------------------

    #[test]
    fn all_bundled_themes_work_with_renderer() {
        for name in Theme::bundle_names() {
            let theme = Theme::load_bundled(name).expect("theme must load");
            let r = TmuxRenderer::new(Some(theme));
            for state in [
                AgentState::Idle,
                AgentState::Working,
                AgentState::Attention,
                AgentState::Risk,
                AgentState::Result,
                AgentState::Error,
                AgentState::Unknown,
            ] {
                let _entry = r.resolve_entry(state);
                let style = r.border_style_for(state);
                let format = r.border_format_for(state, None);
                if state == AgentState::Idle {
                    assert_eq!(style, None, "{name}: Idle should have no style");
                    assert_eq!(format, None, "{name}: Idle should have no format");
                } else {
                    let has_visual = style.is_some() || format.is_some();
                    assert!(
                        has_visual,
                        "{name}: {state:?} must have at least one of color or format"
                    );
                }
            }
        }
    }

    // ---------------------------------------------------------------
    // monochrome theme returns None for color
    // ---------------------------------------------------------------

    #[test]
    fn monochrome_theme_has_no_colors_but_has_symbols() {
        let theme = Theme::load_bundled("monochrome-symbols").unwrap();
        let r = TmuxRenderer::new(Some(theme));
        for &state in &[
            AgentState::Working,
            AgentState::Attention,
            AgentState::Risk,
            AgentState::Result,
            AgentState::Error,
            AgentState::Unknown,
        ] {
            assert_eq!(
                r.border_style_for(state),
                None,
                "monochrome theme should have no color for {state:?}"
            );
            assert!(
                r.border_format_for(state, None).is_some(),
                "monochrome theme should have symbols for {state:?}"
            );
        }
    }
}
