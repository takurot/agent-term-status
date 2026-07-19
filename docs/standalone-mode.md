# Standalone Mode Fallback (SPEC §4.1)

When the `ats-daemon` socket is unreachable (daemon not running, crashed, or
during transitions), the CLI falls back to **standalone mode**: it renders
agent states directly without going through the daemon's event broker.

## Behavior

1. **Detection**: On each invocation, `ats event` / `ats ingest` / `ats reset`
   tries to connect to the daemon's Unix Domain Socket. If the connection
   fails, standalone mode activates.

2. **Standalone rendering**: The CLI creates a local `RenderingEngine` with the
   default theme and renders directly to the terminal. No deduplication, no
   TTL expiry, no notification suppression beyond basic cooldown.

3. **One-shot, best-effort**: Standalone writes are fire-and-forget. They are
   never retried if tmux, iTerm2, or the notification subsystem fails. Every
   standalone render call returns success (fail-open invariant).

4. **No persistence**: Standalone mode writes nothing to disk and produces no
   log files. It is an ephemeral fallback only.

## Handoff to Daemon

When the daemon starts up (or restarts after a crash):

1. The daemon queries the current **authoritative** broker state from its
   in-memory state engine. If no events were buffered while it was down,
   the state engine is fresh and all sessions are `Idle`.

2. The daemon issues a full re-render from its authoritative state,
   **overwriting any standalone-mode residue** (colors, titles, badges, etc.)
   that may have been left behind.

3. If the daemon detects stale state on startup (e.g., a pane border color
   that doesn't match its internal state), it performs a `reset → render`
   cycle to correct it. The daemon's state is always the source of truth.

## Rationale

- **Fail-open**: An agent should never stop working because a visualization
  daemon is down (SPEC §15).
- **Observability continuity**: Operators can still see state changes even
  during daemon maintenance, restarts, or crashes.
- **Clear ownership**: The daemon's re-render on startup ensures standalone
  residue is always corrected, avoiding confusing "stale paint" scenarios.

## Testing

Standalone mode is exercised by the integration test suite (I-24):

```bash
# Kill daemon, verify standalone render works
pkill ats-daemon
ats event working  # pane border turns blue via standalone

# Start daemon, verify it overwrites standalone state
ats-daemon &
sleep 1
ats event attention  # goes through daemon, overwrites standalone Working
```

The daemon handoff test verifies:
- Standalone renders while daemon is down
- Daemon startup re-renders from authoritative state
- No visual artifacts remain from the standalone session
