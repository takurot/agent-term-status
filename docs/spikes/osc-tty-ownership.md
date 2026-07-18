# Spike: OSC-to-TTY ownership (I-05 #5) — CRITICAL

**Decision: option (b) — the daemon writes OSC directly to the pane's TTY
device path (`/dev/ttysNNN`) captured at event time. No controlling TTY is
required. Short-lived CLI proxy is the fallback. This unblocks I-10/I-12.**

## Question

The daemon has no controlling TTY. Who emits OSC sequences for iTerm2
(titles/badges), which must reach a specific terminal session?

- (a) daemon replies to a short-lived CLI proxy still attached to the TTY
- (b) daemon opens the pane's TTY device by path and writes
- (c) other (e.g. iTerm2 Python API)

## Experiment (this machine, macOS)

From a process with **no controlling terminal**, targeting a tmux pane's
TTY owned by the same user:

```
open("/dev/ttys003", O_WRONLY | O_NOCTTY)  → fd ok
write(fd, "\x1b]2;ats-spike-title\x07")    → 20 bytes written
/dev/ttys003 mode = 0620, owner uid = current uid
```

Findings:

1. Same-user processes can open and write any of the user's PTYs by path;
   `O_NOCTTY` prevents accidentally acquiring it as controlling terminal.
2. tty devices are `crw--w----` (0620) owned by the session user — other
   users cannot write (no cross-user injection surface, SPEC §14).
3. The TTY path arrives for free: hooks capture it at event time into
   `TerminalContext.tty` (I-02), so the daemon needs no discovery.

## Decision

- `ats-renderer-iterm2` (and future OSC renderers) receive the target TTY
  path via `RenderTarget.terminal.tty` and write with
  `O_WRONLY | O_NOCTTY | O_NONBLOCK`.
- `O_NONBLOCK` + small writes (<1 KiB) prevent a wedged terminal from
  blocking the daemon; write failures surface as `renderer.failed`
  events, never crashes (fail-open, SPEC §15).
- Stale TTYs (session ended, path reused) are handled by the crash-recovery
  rules (SPEC §15): unknown session → no render, reset on session end.
- tmux rendering does NOT use this path at all — it goes through `tmux`
  commands (spike #1/#2), avoiding the passthrough problem entirely.

## Fallback

If direct TTY writes prove unreliable on newer macOS (e.g. sandbox
tightening), switch to (a): `ats` CLI keeps a short-lived connection open
after `ingest`, and the daemon replies with render instructions that the
CLI (still TTY-attached) executes before exiting. This costs latency on
the hook path, so it stays a fallback. The `Renderer` trait isolates the
change to the iTerm2 renderer.
