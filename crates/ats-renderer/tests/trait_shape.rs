use async_trait::async_trait;
use ats_core::{ActivityLabel, AgentState, TerminalContext};
use ats_renderer::{
    HealthStatus, RenderTarget, Renderer, RendererCapabilities, RendererError, StateView,
};

/// Compile-only mock verifying the trait shape (I-03 DoD).
struct MockRenderer;

#[async_trait]
impl Renderer for MockRenderer {
    async fn detect(&self, ctx: &TerminalContext) -> Result<RendererCapabilities, RendererError> {
        if ctx.tmux_pane.is_some() {
            Ok(RendererCapabilities {
                pane_border: true,
                reset_reliable: true,
                ..RendererCapabilities::default()
            })
        } else {
            Err(RendererError::Unsupported(
                "no tmux pane in context".to_string(),
            ))
        }
    }

    async fn render(&self, view: &StateView) -> Result<(), RendererError> {
        if view.state == AgentState::Unknown {
            return Err(RendererError::Failed("cannot render unknown".to_string()));
        }
        Ok(())
    }

    async fn reset(&self, _target: &RenderTarget) -> Result<(), RendererError> {
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn renderer_is_object_safe_and_send_sync() {
    assert_send_sync::<Box<dyn Renderer>>();
    let _renderer: Box<dyn Renderer> = Box::new(MockRenderer);
}

#[test]
fn capabilities_default_to_all_disabled() {
    let caps = RendererCapabilities::default();
    assert!(
        !(caps.background
            || caps.tab_title
            || caps.tab_color
            || caps.badge
            || caps.cursor_color
            || caps.pane_border
            || caps.notification
            || caps.flash
            || caps.reset_reliable),
        "all capabilities must default to false: {caps:?}"
    );
}

#[tokio::test]
async fn mock_renderer_exercises_trait_surface() {
    let renderer: Box<dyn Renderer> = Box::new(MockRenderer);

    let tmux_ctx = TerminalContext {
        tmux_pane: Some("%12".to_string()),
        ..TerminalContext::default()
    };
    let caps = renderer.detect(&tmux_ctx).await.expect("detect succeeds");
    assert!(caps.pane_border);

    let bare_ctx = TerminalContext::default();
    let err = renderer.detect(&bare_ctx).await.expect_err("must fail");
    assert!(matches!(err, RendererError::Unsupported(_)));
    assert!(!err.to_string().is_empty());
    let _: &dyn std::error::Error = &err;

    let target = RenderTarget {
        session_id: "s-1".to_string(),
        terminal: tmux_ctx.clone(),
    };
    let view = StateView {
        state: AgentState::Working,
        label: Some(ActivityLabel::new("Running tests")),
        target: target.clone(),
    };
    renderer.render(&view).await.expect("render succeeds");
    renderer.reset(&target).await.expect("reset succeeds");
    assert_eq!(renderer.health_check().await, HealthStatus::Healthy);
}

#[test]
fn health_status_carries_degradation_reason() {
    let degraded = HealthStatus::Degraded {
        reason: "tmux not reachable".to_string(),
    };
    assert_ne!(degraded, HealthStatus::Healthy);
    let unavailable = HealthStatus::Unavailable {
        reason: "binary missing".to_string(),
    };
    assert_ne!(unavailable, HealthStatus::Healthy);
}
