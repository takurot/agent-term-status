# Spike: iTerm2 OSC sequences (I-05 #3)

**Decision: OSC 0/2 for tab titles, OSC 1337 SetBadgeFormat for badges,
profile switching deferred to opt-in. Inside tmux, write to the pane TTY
only via tmux passthrough — which is OFF by default and must be detected.**

## Sequences (verified against iTerm2 escape-code docs)

| Purpose | Sequence | Reset |
|---------|----------|-------|
| Tab/window title | `ESC ] 2 ; title BEL` (OSC 0 sets both) | empty title |
| Badge | `ESC ] 1337 ; SetBadgeFormat=<base64> BEL` | empty base64 payload |
| Profile switch (opt-in) | `ESC ] 1337 ; SetProfile=<name> BEL` | `SetProfile=` + original |

- Badge format payload is **base64-encoded UTF-8**; sanitized label goes
  through `ActivityLabel` first (SPEC §6.3.2), then base64 — control-char
  injection is structurally impossible.
- Non-iTerm2 terminals ignore unknown OSC 1337 (verified: Terminal.app and
  tmux swallow it without visual corruption). OSC 0/2 is universally safe.
- Detection: `TERM_PROGRAM=iTerm.app` env captured at hook time in
  `TerminalContext.term_program` (never sniffed at render time — the
  daemon has no terminal env of its own).

## tmux interaction (measured, tmux 3.7b)

```
$ tmux show-options -g allow-passthrough
allow-passthrough off        ← default
```

Consequences:

1. When the pane lives inside tmux, raw OSC written to the pane TTY is
   consumed by tmux, not iTerm2.
2. Passthrough wrapping (`ESC Ptmux; ESC <seq> ESC \`) only works when the
   user has `set -g allow-passthrough on`.

## Decision

- Outside tmux: write OSC 0/2 + OSC 1337 directly to the session TTY (see
  `osc-tty-ownership.md` for who writes).
- Inside tmux: **do not** attempt badge/title OSC in MVP; tmux pane
  border/title (spike #2) is the primary channel. If
  `allow-passthrough on` is detected (`tmux show-options -g`), the iTerm2
  renderer MAY wrap sequences; otherwise it reports the capability as
  absent in `RendererCapabilities` (`badge: false, tab_title: false`).
- Profile switching ships behind `renderers.iterm2.profile_switch` config,
  default off (SPEC §10.2.1 marks it optional) — restore is not reliable
  if iTerm2 closes mid-session, so `reset_reliable: false` when enabled.

## Fallback

If OSC 1337 badge proves unreliable across iTerm2 versions, drop to
title-only rendering (OSC 2 works everywhere) and keep badges as a
Phase 2 capability. Detection stays in `Renderer::detect()` so the
rendering engine (I-12) needs no changes.
