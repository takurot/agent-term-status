# agent-term-status

> Visualize AI coding agent state in your terminal — so you know at a glance whether a pane needs your attention.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

`agent-term-status` (`ats`) is a local CLI tool that reflects the state of AI coding agents (such as Claude Code) running in your terminal via colors, pane borders, tab titles, badges, and notifications.

The central question it answers is not *"what is the AI doing internally?"* but:

> **Do I need to pay attention to this terminal right now?**

It is purpose-built for developers who run multiple AI coding agents in parallel across tmux panes / iTerm2 tabs and want to identify which one is waiting for input, which has finished, and which has errored — without switching context.

---

## Highlights

- **Provider integration** — Claude Code Hooks (MVP), with OpenCode / Codex / generic providers planned.
- **Normalized agent state** — Events from any provider are folded into 7 user-action states: `IDLE`, `WORKING`, `ATTENTION`, `RISK`, `RESULT`, `ERROR`, `UNKNOWN`.
- **State Engine** — Handles priority resolution, TTL, timeouts, parent/subagent aggregation, and de-duplication from incomplete/out-of-order events.
- **Local-first** — Runs entirely on your machine via a daemon over a Unix Domain Socket. No cloud, no telemetry.
- **Renderers** —
  - **tmux**: pane border color + pane title
  - **iTerm2**: badge, tab title, optional profile switching
  - **macOS Notification**: native notifications via UserNotifications (no `osascript`)
- **Fail-open by design** — If `ats` crashes or is uninstalled, the AI agent keeps working. Hooks return success even on internal errors.
- **Privacy-preserving** — Prompt bodies, file contents, command strings, API keys, and full paths are never collected or stored by default.

---

## Agent States

| State | Meaning | User action |
|-------|---------|-------------|
| `IDLE` | Session waiting | None |
| `WORKING` | AI is processing | None |
| `ATTENTION` | Waiting for input/approval | Required |
| `RISK` | High-risk operation pending approval | Review immediately |
| `RESULT` | Completed successfully | Optional |
| `ERROR` | Failure or integration issue | Recommended |
| `UNKNOWN` | State cannot be determined | Check |

`RISK` is implemented internally but rendered in the MVP UI as an emphasized `ATTENTION`. It is advertised as an "attention amplifier," not as a safety mechanism.

Display priority when multiple states coexist:

```
RISK > ATTENTION > ERROR > RESULT > WORKING > IDLE > UNKNOWN
```

---

## Architecture

```
Event Sources (Claude Code Hooks / Manual CLI)
    ↓ Native Event (JSON stdin)
Provider Adapter (claude-provider)
    ↓ Normalized Event
Local Event Broker (Daemon, Unix Domain Socket)
    ↓
State Engine (state machine / priority / TTL / aggregation)
    ↓ Agent State
Rendering Engine (theme resolution / capability detection / rate limiting)
    ↓
iTerm2  ·  tmux  ·  macOS Notification
```

Core assets in priority order: **Provider layer**, **State Engine**, **Renderer layer**. Themes and control sequences are additive on top of these.

---

## Installation

> MVP distribution targets Homebrew Tap and GitHub Releases.

```bash
# Homebrew (planned)
brew install <tap>/agent-term-status

# cargo install (planned)
cargo install agent-term-status
```

The package installs the `agent-term-status` binary and an `ats` symlink.

---

## Quick Start

```bash
# 1. Install Claude Code hooks (user scope by default)
ats install claude --scope user

# 2. Start the daemon
ats daemon start

# 3. Verify setup
ats doctor

# 4. Use Claude Code in a tmux pane / iTerm2 tab —
#    pane border color and title will reflect the agent state.
```

Manual usage (for debugging/testing):

```bash
ats event working --activity "Running tests"
ats event attention
ats event result
ats status
ats list
ats reset --all
```

---

## CLI Reference

```
ats install <provider> [--scope user|project|local] [--dry-run]
ats uninstall <provider> [--scope user|project|local]
ats ingest --provider <name>            # receive JSON from stdin (Hook entry point)
ats event <state> [--activity <label>]  # manual state send
ats reset [--all | --session <id>]
ats status [--session <id>]
ats list [--json]
ats doctor
ats theme list
ats theme preview <name>
ats theme apply <name>
ats daemon start [--foreground]
ats daemon stop
ats daemon status
ats logs [--tail] [--level <level>]
```

Hook-triggered commands (`ingest`, `event`) target a completion time of **50 ms or less**.

---

## Configuration

Paths (XDG-compliant):

- User config: `~/.config/agent-term-status/config.yaml`
- Project config (optional, overrides user): `./.agent-term-status/config.yaml`
- Logs: `~/.local/state/agent-term-status/logs/`
- Unix Socket: `$XDG_RUNTIME_DIR/agent-term-status.sock`

Example:

```yaml
version: 1

daemon:
  enabled: true
  log_level: warn
  event_retention: 24h

privacy:
  store_activity_labels: false
  store_workspace_paths: false
  redact_home_directory: true

renderers:
  tmux:
    enabled: auto
    pane_border: true
    pane_title: true
  iterm2:
    enabled: auto
    badge: true
    tab_title: true
  notifications:
    enabled: true
    states: [attention, risk, result, error]
    quiet_hours:
      start: "22:00"
      end: "07:00"
      allow: [risk]

providers:
  claude:
    enabled: true
```

Bundled themes: `default`, `color-safe`, `low-distraction`, `high-contrast`, `monochrome-symbols`.

---

## Themes

Each state uses at least two representations (color + symbol/label) so information survives without relying on color alone.

| State | Color | Icon | Notification |
|-------|-------|------|--------------|
| IDLE | default | `○` | none |
| WORKING | blue | `●` | none |
| ATTENTION | orange | `!` | yes |
| RISK | red | `!!` | strong |
| RESULT | green | `+` | optional |
| ERROR | magenta / dark red | `×` | yes |
| UNKNOWN | gray | `?` | generally none |

An ASCII icon set is the default; an emoji set is available as an option to avoid display-width issues.

---

## Performance Targets

| Item | Target |
|------|--------|
| Hook CLI runtime (`ats ingest`) | ≤ 50 ms |
| Event latency (hook → render) | ≤ 150 ms |
| Daemon resident memory | ≤ 30 MB |
| Idle CPU usage | < 0.1% |
| Event throughput | ≥ 100 events/sec |
| Daemon startup time | ≤ 300 ms |

---

## Technology Stack

- **Language**: Rust (latest stable)
- **Async runtime**: Tokio
- **CLI parser**: clap
- **Serialization**: serde / serde_json / serde_yaml
- **Local transport**: Unix Domain Socket (length-prefixed JSON, 64 KiB/event cap)
- **Logging**: tracing / tracing-subscriber
- **UUID**: UUIDv7
- **macOS notifications**: native UserNotifications API (no `osascript` dependency)

---

## Security & Reliability

- Strict schema validation on incoming JSON; control characters stripped from all display strings
- Unix socket restricted to file mode `0600` (user-only)
- Hook inputs are never shell-evaluated; commands are never re-executed
- Log redaction enabled by default; bodies are not persisted
- **Fail-open**: agent processing continues even if `ats` is down
- **Idempotent**: duplicate `event_id`s never cause duplicate state transitions or notifications
- **Atomic config writes**: temp file + validate + rename
- **Crash recovery**: stale states are not re-rendered; unknown sessions go UNKNOWN → IDLE
- **Reset guarantee**: `ats reset --all` always restores default visuals
- Runs with user privileges only — no `sudo`, no privilege escalation

---

## Project Status

| Phase | Scope |
|-------|-------|
| **Phase 0** — Tech spike | Hook ingestion, tmux pane targeting, border color, reset |
| **Phase 1** — MVP 0.1 *(current)* | Claude Code provider, State Engine + Daemon, tmux/iTerm2/Notification renderers, install/doctor/reset/status/list, 5 themes, Homebrew distribution |
| Phase 2 | OpenCode provider, multi-session UI, subagent aggregation, WezTerm/Ghostty renderers, theme sharing |
| Phase 3 | Provider/Renderer SDK, generic webhook/socket API, VS Code/Cursor integration, menu bar app, policy engine (RISK) |

---

## Documentation

- [IDEA.md](docs/IDEA.md) — Original concept and motivation
- [SPEC.md](docs/SPEC.md) — Full specification (architecture, data model, state machine, security, testing)

---

## License

[MIT](LICENSE)
