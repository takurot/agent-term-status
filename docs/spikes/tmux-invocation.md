# Spike: tmux invocation strategy (I-05 #1)

**Decision: per-call `tmux` subprocess. Control mode is the documented fallback.**

## Question

Renderer updates must fit the 150 ms event→render budget (SPEC §16). Do we
need a persistent tmux control-mode (`tmux -C`) connection, or is spawning
`tmux <cmd>` per update fast enough?

## Measurement

100 sequential pane-scoped style updates on a detached session
(tmux 3.7b, Apple Silicon macOS, release-quality conditions not required
for an order-of-magnitude check):

```
tmux set-option -p -t %0 pane-border-style fg=colourN
min=5.15ms  p50=5.98ms  p95=7.20ms  max=8.93ms
```

## Analysis

- p95 ≈ 7 ms is **~5 % of the 150 ms budget**; even 3 renderer calls per
  event (border + title + format) stay under 25 ms.
- Subprocess model is stateless: no connection lifecycle, no reconnect
  logic on tmux server restart, no long-lived fd in the daemon.
- Control mode would save ~5 ms/call but adds a session-scoped client,
  output parsing, and failure modes (dangling control client on crash).

## Decision

Use `tmux` subprocess invocations from the renderer (`ats-renderer-tmux`),
batched per event where possible (`tmux set-option -p ... \; set-option ...`
one-shot chaining is available if call count ever matters).

## Fallback

If real-world profiling (I-23 perf harness) shows subprocess spawn cost
breaking the budget under load (e.g. ≥100 events/sec sustained across many
panes), switch to a single control-mode client owned by the daemon. The
`Renderer` trait (I-03) hides this behind `render()`, so the change is
contained in `ats-renderer-tmux`.
