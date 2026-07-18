//! **Phase 0 spike prototype** (I-05) — standalone tmux pane-border demo.
//!
//! `ats event <state>` maps an agent state to a tmux pane border color on
//! the pane identified by `$TMUX_PANE`, with no daemon involved. This
//! validates the tmux invocation strategy and pane-targeting safety
//! decisions (see `docs/spikes/`). It will be replaced by the real event
//! pipeline in I-17; do not extend it.
//!
//! Invariants honored even in the prototype:
//! - fail-open: always exits 0 (SPEC §9.2, §15)
//! - pane-scoped options only (`set-option -p`), never window/global

use std::process::Command;

use ats_core::AgentState;

/// Parses the lowercase state names accepted by `ats event` (SPEC §5.1.2).
fn parse_state(raw: &str) -> Option<AgentState> {
    serde_json::from_value(serde_json::Value::String(raw.to_string())).ok()
}

/// Default theme border colors (SPEC §11 default theme).
fn border_style(state: AgentState) -> Option<&'static str> {
    match state {
        AgentState::Working => Some("fg=blue"),
        AgentState::Attention => Some("fg=orange"),
        AgentState::Risk => Some("fg=red"),
        AgentState::Result => Some("fg=green"),
        AgentState::Error => Some("fg=magenta"),
        AgentState::Unknown => Some("fg=colour244"),
        AgentState::Idle => None, // reset to terminal default
    }
}

/// Runs the demo. Never returns a failure exit code (fail-open).
pub fn run(state_arg: &str) {
    let Some(state) = parse_state(state_arg) else {
        eprintln!("ats event (prototype): unknown state {state_arg:?}, ignoring");
        return;
    };

    // SPEC §6.4.2: TMUX_PANE is the pane-targeting key. The TMUX socket
    // env var (if any) is inherited by the tmux client subprocess as-is.
    let Ok(pane) = std::env::var("TMUX_PANE") else {
        eprintln!("ats event (prototype): TMUX_PANE not set, nothing to render");
        return;
    };

    let result = match border_style(state) {
        Some(style) => Command::new("tmux")
            .args(["set-option", "-p", "-t", &pane, "pane-border-style", style])
            .output(),
        None => Command::new("tmux")
            .args(["set-option", "-p", "-t", &pane, "-u", "pane-border-style"])
            .output(),
    };

    match result {
        Ok(out) if out.status.success() => {}
        Ok(out) => eprintln!(
            "ats event (prototype): tmux failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        Err(err) => eprintln!("ats event (prototype): cannot run tmux: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_lowercase_state_names() {
        for (raw, expected) in [
            ("idle", AgentState::Idle),
            ("working", AgentState::Working),
            ("attention", AgentState::Attention),
            ("risk", AgentState::Risk),
            ("result", AgentState::Result),
            ("error", AgentState::Error),
            ("unknown", AgentState::Unknown),
        ] {
            assert_eq!(parse_state(raw), Some(expected), "state {raw}");
        }
        assert_eq!(parse_state("exploded"), None);
        assert_eq!(parse_state("WORKING"), None, "uppercase is not accepted");
    }

    #[test]
    fn maps_states_to_default_theme_colors() {
        assert_eq!(border_style(AgentState::Working), Some("fg=blue"));
        assert_eq!(border_style(AgentState::Attention), Some("fg=orange"));
        assert_eq!(border_style(AgentState::Risk), Some("fg=red"));
        assert_eq!(border_style(AgentState::Result), Some("fg=green"));
        assert_eq!(border_style(AgentState::Error), Some("fg=magenta"));
        assert_eq!(border_style(AgentState::Unknown), Some("fg=colour244"));
        assert_eq!(border_style(AgentState::Idle), None, "idle resets");
    }
}
