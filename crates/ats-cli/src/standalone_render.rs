use std::os::unix::net::UnixStream;
use std::path::Path;

use ats_config::theme::Theme;
use ats_core::{ActivityLabel, AgentState, TerminalContext};
use ats_renderer::{RenderTarget, Renderer, StateView};
use ats_renderer_tmux::TmuxRenderer;

/// Check if the daemon socket is reachable.
pub fn daemon_reachable(socket_path: &Path) -> bool {
    UnixStream::connect(socket_path).is_ok()
}

/// Resolve daemon socket path from environment.
pub fn daemon_socket_path() -> Option<std::path::PathBuf> {
    ats_daemon::DaemonPaths::resolve_with_env(
        std::env::var("XDG_RUNTIME_DIR").ok().as_deref(),
        dirs::home_dir().as_deref(),
    )
    .socket_path
    .into()
}

/// Build terminal context from the current process environment.
fn build_terminal_context() -> TerminalContext {
    TerminalContext {
        tmux_pane: std::env::var("TMUX_PANE").ok(),
        ..Default::default()
    }
}

/// Render a state directly (standalone mode) using the tmux renderer.
///
/// No dedup, no TTL, no notification suppression. This is a one-shot
/// best-effort render that daemon will overwrite on next authoritative
/// state sync.
pub async fn standalone_render(state: AgentState, label: Option<&str>) {
    let theme = Theme::load_bundled("default")
        .unwrap_or_else(|_| Theme::load_bundled("color-safe").expect("bundled theme is missing"));

    let renderer = TmuxRenderer::new(Some(theme));

    let ctx = build_terminal_context();
    let pane = match ctx.tmux_pane.as_deref() {
        Some(p) => p.to_string(),
        None => {
            eprintln!("standalone: TMUX_PANE not set, nothing to render");
            return;
        }
    };

    let target = RenderTarget {
        session_id: "standalone".to_string(),
        terminal: ctx,
    };
    let view = StateView {
        state,
        label: label.map(ActivityLabel::new),
        target,
    };

    if let Err(e) = renderer.render(&view).await {
        eprintln!("standalone: render failed (fail-open): {e}");
    } else {
        eprintln!("standalone: rendered {state:?} on pane {pane}");
    }
}

/// Reset all renderers for the current terminal context.
pub async fn standalone_reset() {
    let ctx = build_terminal_context();
    if ctx.tmux_pane.is_none() {
        eprintln!("standalone: TMUX_PANE not set, nothing to reset");
        return;
    }

    let target = RenderTarget {
        session_id: "standalone".to_string(),
        terminal: ctx,
    };

    let renderer = TmuxRenderer::new(None);
    if let Err(e) = renderer.reset(&target).await {
        eprintln!("standalone: reset failed (fail-open): {e}");
    } else {
        eprintln!("standalone: reset complete");
    }
}
