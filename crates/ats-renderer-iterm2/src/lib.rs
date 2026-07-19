//! # `ats-renderer-iterm2` — iTerm2 OSC renderer (I-10)
//!
//! Reflects agent state in iTerm2 via OSC sequences: tab/window title
//! (OSC 0/2) and badge (OSC 1337 SetBadgeFormat). Implements the
//! [`Renderer`] trait from `ats-renderer`.
//!
//! ## OSC-to-TTY ownership (I-05 spike #5)
//! Writes OSC directly to the pane TTY device path (`/dev/ttysNNN`)
//! with `O_WRONLY | O_NOCTTY | O_NONBLOCK`. Same-user PTYs are
//! open-writable; no cross-user injection surface.
//!
//! ## tmux interaction (I-05 spike #3)
//! Inside tmux (`TMUX_PANE` set), raw OSC is consumed by tmux, not
//! iTerm2. Passthrough wrapping only works with `allow-passthrough on`
//! which is off by default. In MVP, inside-tmux renders are silently
//! skipped; the tmux renderer (I-09) is the primary channel there.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::Mutex;

use async_trait::async_trait;
use ats_config::theme::{Theme, ThemeEntry};
use ats_core::{ActivityLabel, AgentState, TerminalContext};
use ats_renderer::{
    HealthStatus, RenderTarget, Renderer, RendererCapabilities, RendererError, StateView,
};

const MAX_OSC_STRING_LEN: usize = 256;

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Clone)]
struct CapturedState {
    #[allow(dead_code)]
    title: Option<String>,
    #[allow(dead_code)]
    badge: Option<String>,
}

pub struct ITerm2Renderer {
    theme: Option<Theme>,
    prior: Mutex<HashMap<String, CapturedState>>,
}

impl ITerm2Renderer {
    pub fn new(theme: Option<Theme>) -> Self {
        Self {
            theme,
            prior: Mutex::new(HashMap::new()),
        }
    }

    fn resolve_entry(&self, state: AgentState) -> Option<ThemeEntry> {
        self.theme.as_ref().and_then(|t| t.resolve(state))
    }

    pub fn sanitize_osc_string(s: &str) -> String {
        s.chars()
            .filter(|c| {
                let cp = *c as u32;
                cp >= 0x20 && cp != 0x7F && !(0x80..=0x9F).contains(&cp)
            })
            .take(MAX_OSC_STRING_LEN)
            .collect()
    }

    pub fn base64_encode(data: &[u8]) -> String {
        let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let n = (b0 << 16) | (b1 << 8) | b2;
            result.push(BASE64_CHARS[(n >> 18) as usize & 0x3F] as char);
            result.push(BASE64_CHARS[(n >> 12) as usize & 0x3F] as char);
            result.push(if chunk.len() > 1 {
                BASE64_CHARS[(n >> 6) as usize & 0x3F] as char
            } else {
                '='
            });
            result.push(if chunk.len() > 2 {
                BASE64_CHARS[n as usize & 0x3F] as char
            } else {
                '='
            });
        }
        result
    }

    fn build_title(&self, state: AgentState, label: Option<&ActivityLabel>) -> Option<String> {
        if state == AgentState::Idle {
            return Some(String::new());
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
        let raw = parts.join(" · ");
        let sanitized = Self::sanitize_osc_string(&raw);
        Some(sanitized)
    }

    fn build_badge(&self, state: AgentState) -> Option<String> {
        if state == AgentState::Idle {
            return Some(String::new());
        }
        let entry = self.resolve_entry(state)?;
        let badge_text = entry.symbol.as_str();
        if badge_text.is_empty() {
            return None;
        }
        let sanitized = Self::sanitize_osc_string(badge_text);
        Some(Self::base64_encode(sanitized.as_bytes()))
    }

    fn osc_set_title(title: &str) -> Vec<u8> {
        let mut seq = vec![0x1b, b']'];
        seq.extend_from_slice(b"2;");
        seq.extend_from_slice(title.as_bytes());
        seq.push(0x07);
        seq
    }

    fn osc_set_badge(badge: &str) -> Vec<u8> {
        let mut seq = vec![0x1b, b']'];
        seq.extend_from_slice(b"1337;SetBadgeFormat=");
        seq.extend_from_slice(badge.as_bytes());
        seq.push(0x07);
        seq
    }

    fn write_to_tty(tty: &str, bytes: &[u8]) -> Result<(), RendererError> {
        let mut file = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
            .open(tty)
            .map_err(|e| RendererError::Failed(format!("cannot open TTY {tty}: {e}")))?;
        file.write_all(bytes)
            .map_err(|e| RendererError::Failed(format!("write to TTY {tty} failed: {e}")))?;
        file.flush()
            .map_err(|e| RendererError::Failed(format!("flush TTY {tty} failed: {e}")))?;
        Ok(())
    }

    fn is_iterm2(ctx: &TerminalContext) -> bool {
        ctx.term_program.as_deref() == Some("iTerm.app")
    }

    fn is_inside_tmux(ctx: &TerminalContext) -> bool {
        ctx.tmux_pane.is_some()
    }
}

#[async_trait]
impl Renderer for ITerm2Renderer {
    async fn detect(&self, ctx: &TerminalContext) -> Result<RendererCapabilities, RendererError> {
        if !Self::is_iterm2(ctx) {
            return Err(RendererError::Unsupported("not iTerm2 terminal".into()));
        }

        if ctx.tty.is_none() {
            return Err(RendererError::Unsupported(
                "no TTY in terminal context".into(),
            ));
        }

        let inside_tmux = Self::is_inside_tmux(ctx);
        let can_title = !inside_tmux;
        let can_badge = !inside_tmux;

        Ok(RendererCapabilities {
            tab_title: can_title,
            badge: can_badge,
            reset_reliable: !inside_tmux,
            ..Default::default()
        })
    }

    async fn render(&self, view: &StateView) -> Result<(), RendererError> {
        let tty = view
            .target
            .terminal
            .tty
            .as_deref()
            .ok_or_else(|| RendererError::Unsupported("no TTY in render target".into()))?;

        if !Self::is_iterm2(&view.target.terminal) {
            return Err(RendererError::Unsupported(
                "renderer only supports iTerm2".into(),
            ));
        }

        if Self::is_inside_tmux(&view.target.terminal) {
            return Ok(());
        }

        {
            let mut prior = self.prior.lock().unwrap();
            if !prior.contains_key(view.target.session_id.as_str()) {
                prior.insert(
                    view.target.session_id.clone(),
                    CapturedState {
                        title: None,
                        badge: None,
                    },
                );
            }
        }

        if let Some(title) = self.build_title(view.state, view.label.as_ref()) {
            let seq = Self::osc_set_title(&title);
            Self::write_to_tty(tty, &seq)?;
        }

        if let Some(badge) = self.build_badge(view.state) {
            let seq = Self::osc_set_badge(&badge);
            Self::write_to_tty(tty, &seq)?;
        }

        Ok(())
    }

    async fn reset(&self, target: &RenderTarget) -> Result<(), RendererError> {
        let tty = target
            .terminal
            .tty
            .as_deref()
            .ok_or_else(|| RendererError::Unsupported("no TTY in reset target".into()))?;

        if !Self::is_iterm2(&target.terminal) {
            return Ok(());
        }

        if Self::is_inside_tmux(&target.terminal) {
            return Ok(());
        }

        let cap = self
            .prior
            .lock()
            .unwrap()
            .remove(target.session_id.as_str());

        if let Some(ref cs) = cap {
            if let Some(ref title) = cs.title {
                if !title.is_empty() {
                    let seq = Self::osc_set_title(title);
                    Self::write_to_tty(tty, &seq)?;
                }
            }
        }

        let seq = Self::osc_set_title("");
        Self::write_to_tty(tty, &seq)?;

        let seq = Self::osc_set_badge("");
        Self::write_to_tty(tty, &seq)?;

        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
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

    fn test_renderer() -> ITerm2Renderer {
        ITerm2Renderer::new(Some(default_theme()))
    }

    // ---------------------------------------------------------------
    // sanitize_osc_string
    // ---------------------------------------------------------------

    #[test]
    fn sanitize_passes_printable_ascii() {
        assert_eq!(ITerm2Renderer::sanitize_osc_string("Hello"), "Hello");
        assert_eq!(ITerm2Renderer::sanitize_osc_string("test 123"), "test 123");
    }

    #[test]
    fn sanitize_strips_control_characters() {
        assert_eq!(ITerm2Renderer::sanitize_osc_string("hel\x1blo"), "hello");
        assert_eq!(
            ITerm2Renderer::sanitize_osc_string("hi\x07there"),
            "hithere"
        );
        assert_eq!(ITerm2Renderer::sanitize_osc_string("a\x7fb"), "ab");
    }

    #[test]
    fn sanitize_strips_bel() {
        assert_eq!(ITerm2Renderer::sanitize_osc_string("\x07badge"), "badge");
    }

    #[test]
    fn sanitize_strips_newlines_and_tabs() {
        assert_eq!(
            ITerm2Renderer::sanitize_osc_string("line1\nline2\r\nline3\tx"),
            "line1line2line3x"
        );
    }

    #[test]
    fn sanitize_enforces_length_cap() {
        let long = "x".repeat(500);
        let result = ITerm2Renderer::sanitize_osc_string(&long);
        assert!(result.len() <= MAX_OSC_STRING_LEN);
    }

    #[test]
    fn sanitize_handles_empty() {
        assert_eq!(ITerm2Renderer::sanitize_osc_string(""), "");
    }

    // ---------------------------------------------------------------
    // base64_encode
    // ---------------------------------------------------------------

    #[test]
    fn base64_encode_empty() {
        assert_eq!(ITerm2Renderer::base64_encode(b""), "");
    }

    #[test]
    fn base64_encode_hello() {
        assert_eq!(ITerm2Renderer::base64_encode(b"Hello"), "SGVsbG8=");
    }

    #[test]
    fn base64_encode_symbols() {
        let encoded = ITerm2Renderer::base64_encode("●".as_bytes());
        assert!(!encoded.is_empty());
        let encoded = ITerm2Renderer::base64_encode("!".as_bytes());
        assert_eq!(encoded, "IQ==");
    }

    #[test]
    fn base64_encode_padding() {
        assert_eq!(ITerm2Renderer::base64_encode(b"f"), "Zg==");
        assert_eq!(ITerm2Renderer::base64_encode(b"fo"), "Zm8=");
        assert_eq!(ITerm2Renderer::base64_encode(b"foo"), "Zm9v");
    }

    #[test]
    fn base64_encode_longer() {
        assert_eq!(
            ITerm2Renderer::base64_encode(b"Man is distinguished"),
            "TWFuIGlzIGRpc3Rpbmd1aXNoZWQ="
        );
    }

    // ---------------------------------------------------------------
    // build_title
    // ---------------------------------------------------------------

    #[test]
    fn title_for_idle_is_empty_string() {
        let r = test_renderer();
        assert_eq!(r.build_title(AgentState::Idle, None), Some(String::new()));
    }

    #[test]
    fn title_contains_symbol_and_label() {
        let r = test_renderer();
        let title = r.build_title(AgentState::Working, None).unwrap();
        assert!(!title.is_empty());
        assert!(title.contains("Working"));
    }

    #[test]
    fn title_includes_activity_label() {
        let r = test_renderer();
        let label = ActivityLabel::new("Running tests");
        let title = r.build_title(AgentState::Attention, Some(&label)).unwrap();
        assert!(title.contains("Running tests"));
    }

    #[test]
    fn title_does_not_contain_control_chars() {
        let r = test_renderer();
        let label = ActivityLabel::new("test");
        let title = r.build_title(AgentState::Working, Some(&label)).unwrap();
        for c in title.chars() {
            let cp = c as u32;
            assert!(
                cp >= 0x20 && cp != 0x7F,
                "title contained control char U+{cp:04X}"
            );
        }
    }

    // ---------------------------------------------------------------
    // build_badge
    // ---------------------------------------------------------------

    #[test]
    fn badge_for_idle_is_empty_string() {
        let r = test_renderer();
        assert_eq!(r.build_badge(AgentState::Idle), Some(String::new()));
    }

    #[test]
    fn badge_is_base64_encoded_symbol() {
        let r = test_renderer();
        let badge = r.build_badge(AgentState::Working).unwrap();
        assert!(!badge.is_empty());
        let decoded =
            String::from_utf8(base64_decode_test(&badge).expect("badge must decode")).unwrap();
        assert!(decoded.contains("●"));
    }

    #[test]
    fn badge_only_contains_base64_chars() {
        let r = test_renderer();
        let badge = r.build_badge(AgentState::Risk).unwrap();
        for c in badge.chars() {
            assert!(
                c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=',
                "badge contains non-base64 char: {c:?}"
            );
        }
    }

    // ---------------------------------------------------------------
    // is_iterm2 detection
    // ---------------------------------------------------------------

    #[test]
    fn detects_iterm2() {
        let ctx = TerminalContext {
            term_program: Some("iTerm.app".into()),
            tty: Some("/dev/ttys001".into()),
            ..Default::default()
        };
        assert!(ITerm2Renderer::is_iterm2(&ctx));
    }

    #[test]
    fn rejects_terminal_app() {
        let ctx = TerminalContext {
            term_program: Some("Terminal.app".into()),
            ..Default::default()
        };
        assert!(!ITerm2Renderer::is_iterm2(&ctx));
    }

    #[test]
    fn rejects_missing_term_program() {
        let ctx = TerminalContext::default();
        assert!(!ITerm2Renderer::is_iterm2(&ctx));
    }

    // ---------------------------------------------------------------
    // tmux detection
    // ---------------------------------------------------------------

    #[test]
    fn detects_inside_tmux() {
        let ctx = TerminalContext {
            tmux_pane: Some("%12".into()),
            ..Default::default()
        };
        assert!(ITerm2Renderer::is_inside_tmux(&ctx));
    }

    #[test]
    fn not_inside_tmux_when_no_pane() {
        let ctx = TerminalContext::default();
        assert!(!ITerm2Renderer::is_inside_tmux(&ctx));
    }

    // ---------------------------------------------------------------
    // OSC sequence construction
    // ---------------------------------------------------------------

    #[test]
    fn osc_title_starts_with_esc_bracket() {
        let seq = ITerm2Renderer::osc_set_title("test");
        assert_eq!(seq[0], 0x1b);
        assert_eq!(seq[1], b']');
        assert_eq!(seq[2], b'2');
        assert_eq!(seq[3], b';');
    }

    #[test]
    fn osc_title_ends_with_bel() {
        let seq = ITerm2Renderer::osc_set_title("test");
        assert_eq!(seq.last(), Some(&0x07));
    }

    #[test]
    fn osc_title_contains_provided_text() {
        let seq = ITerm2Renderer::osc_set_title("hello world");
        let s = String::from_utf8(seq).unwrap();
        assert!(s.contains("hello world"));
    }

    #[test]
    fn osc_badge_contains_setbadgeformat() {
        let seq = ITerm2Renderer::osc_set_badge("dGVzdA==");
        let s = String::from_utf8(seq).unwrap();
        assert!(s.contains("1337;SetBadgeFormat="));
        assert!(s.contains("dGVzdA=="));
    }

    #[test]
    fn empty_badge_resets() {
        let seq = ITerm2Renderer::osc_set_badge("");
        let s = String::from_utf8(seq).unwrap();
        assert!(s.contains("1337;SetBadgeFormat="));
    }

    // ---------------------------------------------------------------
    // All 7 states
    // ---------------------------------------------------------------

    #[test]
    fn all_states_produce_non_empty_title() {
        let r = test_renderer();
        for &state in &[
            AgentState::Working,
            AgentState::Attention,
            AgentState::Risk,
            AgentState::Result,
            AgentState::Error,
            AgentState::Unknown,
        ] {
            let title = r.build_title(state, None);
            assert!(
                title.as_ref().is_some_and(|t| !t.is_empty()),
                "{state:?} should have non-empty title"
            );
        }
    }

    #[test]
    fn all_states_produce_non_empty_badge() {
        let r = test_renderer();
        for &state in &[
            AgentState::Working,
            AgentState::Attention,
            AgentState::Risk,
            AgentState::Result,
            AgentState::Error,
            AgentState::Unknown,
        ] {
            let badge = r.build_badge(state);
            assert!(
                badge.as_ref().is_some_and(|b| !b.is_empty()),
                "{state:?} should have non-empty badge"
            );
        }
    }

    #[test]
    fn idle_resets_both() {
        let r = test_renderer();
        assert_eq!(r.build_title(AgentState::Idle, None), Some(String::new()));
        assert_eq!(r.build_badge(AgentState::Idle), Some(String::new()));
    }

    // ---------------------------------------------------------------
    // Two-representation rule
    // ---------------------------------------------------------------

    #[test]
    fn all_active_states_have_color_and_badge() {
        let r = test_renderer();
        for &state in &[
            AgentState::Working,
            AgentState::Attention,
            AgentState::Risk,
            AgentState::Result,
            AgentState::Error,
            AgentState::Unknown,
        ] {
            let title = r.build_title(state, None).unwrap();
            let badge = r.build_badge(state).unwrap();
            assert!(!title.is_empty(), "{state:?} title must not be empty");
            assert!(!badge.is_empty(), "{state:?} badge must not be empty");
        }
    }

    // ---------------------------------------------------------------
    // All 5 bundled themes
    // ---------------------------------------------------------------

    #[test]
    fn all_bundled_themes_produce_valid_output() {
        for name in Theme::bundle_names() {
            let theme = Theme::load_bundled(name).expect("theme must load");
            let r = ITerm2Renderer::new(Some(theme));
            for &state in &[
                AgentState::Working,
                AgentState::Attention,
                AgentState::Risk,
                AgentState::Result,
                AgentState::Error,
                AgentState::Unknown,
            ] {
                let title = r.build_title(state, None);
                let badge = r.build_badge(state);
                let has_title = title.as_ref().is_some_and(|t| !t.is_empty());
                let has_badge = badge.as_ref().is_some_and(|b| !b.is_empty());
                assert!(
                    has_title || has_badge,
                    "{name}: {state:?} must have title or badge"
                );
            }
        }
    }

    // ---------------------------------------------------------------
    // Monochrome theme
    // ---------------------------------------------------------------

    #[test]
    fn monochrome_theme_has_no_colors_but_has_text() {
        let theme = Theme::load_bundled("monochrome-symbols").unwrap();
        let r = ITerm2Renderer::new(Some(theme));
        for &state in &[
            AgentState::Working,
            AgentState::Attention,
            AgentState::Risk,
            AgentState::Result,
            AgentState::Error,
            AgentState::Unknown,
        ] {
            let title = r.build_title(state, None);
            let badge = r.build_badge(state);
            assert!(
                title.as_ref().is_some_and(|t| !t.is_empty()),
                "monochrome {state:?} must have title"
            );
            assert!(
                badge.as_ref().is_some_and(|b| !b.is_empty()),
                "monochrome {state:?} must have badge"
            );
        }
    }

    // helper for test decoding
    fn base64_decode_test(encoded: &str) -> Option<Vec<u8>> {
        let mut result = Vec::new();
        let mut buf = 0u32;
        let mut bits = 0;
        for c in encoded.chars() {
            if c == '=' {
                break;
            }
            let val = BASE64_CHARS.iter().position(|&x| x == c as u8)? as u32;
            buf = (buf << 6) | val;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                result.push((buf >> bits) as u8);
                buf &= (1 << bits) - 1;
            }
        }
        Some(result)
    }
}
