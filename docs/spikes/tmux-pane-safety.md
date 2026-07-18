# Spike: tmux pane-targeting safety (I-05 #2)

**Decision: pane-scoped options (`set-option -p -t $TMUX_PANE`) are safe;
they provably do not leak to sibling panes or the window.**

## Question

MVP acceptance criterion §21 #5: state must never leak to other panes.
Does `tmux set-option -p -t %N pane-border-style` affect only pane `%N`?

## Experiment (tmux 3.7b)

Two-pane detached session (`%0`, `%1`):

```
$ tmux set-option -p -t %0 pane-border-style 'fg=orange'
$ tmux set-option -p -t %0 pane-border-format ' ATTENTION '

$ tmux show-options -p -t %0 pane-border-style   → pane-border-style fg=orange
$ tmux show-options -p -t %1 pane-border-style   → (empty: untouched)
$ tmux show-options -w -t <win> pane-border-style → (empty: window untouched)

$ tmux set-option -p -t %0 -u pane-border-style   # reset
$ tmux show-options -p -t %0 pane-border-style   → (empty: default restored)
```

## Findings

1. Pane-scoped set (`-p`) writes only to the target pane's option table;
   sibling pane and window/global tables are untouched.
2. `-u` (unset) cleanly restores default appearance — this is the reset
   primitive for `ats reset` (§21 #7).
3. `$TMUX_PANE` (e.g. `%12`) is a stable pane ID, not an index; it does not
   change when panes are reordered, and tmux rejects unknown IDs with a
   non-zero exit (safe failure).

## Decision

`ats-renderer-tmux` uses exclusively pane-scoped options addressed by pane
ID (`-p -t %N`): `pane-border-style`, `pane-border-format`. Reset uses
`-u`. Window- or global-scope options are forbidden in the renderer.

## Fallback

If a tmux version regresses pane-scoped option isolation, fall back to
`select-pane -t %N -P 'fg=...'` (older pane-colouring path) and gate the
renderer on `tmux -V` detection in `Renderer::detect()`. The E2E leak test
(I-24) guards this permanently.
