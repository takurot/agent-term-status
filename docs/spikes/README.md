# Phase 0 Spikes (I-05)

Written decisions resolving the architectural unknowns before Phase 1.
Each note records the measurement/experiment, the decision, and a
documented fallback.

| # | Topic | Decision (short) | Unblocks |
|---|-------|------------------|----------|
| 1 | [tmux-invocation.md](tmux-invocation.md) | per-call `tmux` subprocess (p95 ≈ 7 ms ≪ 150 ms budget) | I-09 |
| 2 | [tmux-pane-safety.md](tmux-pane-safety.md) | pane-scoped `set-option -p -t %N` proven leak-free; `-u` resets | I-09 |
| 3 | [iterm2-osc.md](iterm2-osc.md) | OSC 0/2 + 1337 badge outside tmux; passthrough off by default → capability-detect | I-10, I-12 |
| 4 | [macos-notifications.md](macos-notifications.md) | app-bundle helper (`ats-notifier.app`) with UNUserNotificationCenter; OSC 9 fallback | I-11 |
| 5 | [osc-tty-ownership.md](osc-tty-ownership.md) | daemon writes to `/dev/ttysNNN` with `O_NOCTTY` (proven); CLI-proxy fallback | I-10, I-12 |

Demo: `ats event <state>` (Phase 0 prototype in `ats-cli`) drives the tmux
pane border standalone — see `crates/ats-cli/src/event_prototype.rs` and
its E2E test `crates/ats-cli/tests/event_prototype.rs`.
