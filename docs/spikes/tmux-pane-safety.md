# Spike: tmux pane-targeting safety (I-05 #2)

**Decision: pane-scoped options (`set-option -p -t $TMUX_PANE`) are the
rendering primitive, but they are only safe on tmux ≥ 3.7. On older tmux
they silently apply at window scope and leak to every pane — the renderer
must version-gate the `pane_border` capability.**

## Question

MVP acceptance criterion §21 #5: state must never leak to other panes.
Does `tmux set-option -p -t %N pane-border-style` affect only pane `%N`?

## Experiment

Two-pane detached session (`%0`, `%1`), identical procedure per version
(source-built binaries, isolated `-L` sockets):

```
$ tmux set-option -p -t %0 pane-border-style 'fg=blue'
$ tmux show-options -p -t %0 pane-border-style
$ tmux show-options -p -t %1 pane-border-style
$ tmux show-options -w pane-border-style
```

| tmux | `%0` | `%1` (sibling) | window scope | verdict |
|------|------|----------------|--------------|---------|
| 3.4  | fg=blue | **fg=blue** | **fg=blue** | LEAKS |
| 3.5a | fg=blue | **fg=blue** | **fg=blue** | LEAKS |
| 3.6  | fg=blue | **fg=blue** | **fg=blue** | LEAKS |
| 3.7  | fg=blue | (empty) | (empty) | pane-scoped ✓ |
| 3.7b | fg=blue | (empty) | (empty) | pane-scoped ✓ |

This was first noticed when CI (ubuntu, tmux 3.4) failed the
sibling-isolation E2E test that passes locally on 3.7b — `set-option -p`
*accepts* the flags on old tmux but stores the value at window scope.

## Findings

1. On tmux ≥ 3.7, pane-scoped set (`-p`) writes only to the target pane's
   option table; sibling pane and window/global tables are untouched.
2. On tmux 3.4–3.6 the same command leaks to the whole window — silently
   (exit code 0). Version detection is the only defense.
3. `-u` (unset) cleanly restores default appearance — this is the reset
   primitive for `ats reset` (§21 #7).
4. `$TMUX_PANE` (e.g. `%12`) is a stable pane ID, not an index; it does not
   change when panes are reordered, and tmux rejects unknown IDs with a
   non-zero exit (safe failure).

## Decision

- `ats-renderer-tmux` uses exclusively pane-scoped options addressed by
  pane ID (`-p -t %N`): `pane-border-style`, `pane-border-format`; reset
  uses `-u`. Window- or global-scope options are forbidden in the renderer.
- `Renderer::detect()` (I-09) MUST parse `tmux -V` and report
  `pane_border: false` for tmux < 3.7. Rendering into window scope is
  never acceptable (acceptance §21 #5 outranks having any tmux output).
- The Phase 0 prototype (`ats event`) implements the same gate and refuses
  to render on tmux < 3.7 (fail-open, stderr note).

## Fallback

For tmux 3.4–3.6 users, the pane border channel stays off; state is still
visible through the other renderers (iTerm2 tab/badge, notifications).
If demand justifies it later, a window-level *opt-in* mode could render
only when the window has a single pane — explicitly out of MVP scope.
