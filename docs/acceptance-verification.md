# MVP 0.1 Acceptance Verification (I-28)

Per SPEC §21, all 10 criteria must be satisfied for MVP completion.
This assessment is based on code merged to `main` (I-01 through I-14).

## Verification Status (assessed against `main`)

| # | Criterion | Status | Caveat |
|---|-----------|--------|--------|
| 1 | `ats install claude` preserves existing settings | Partially Implemented | Install logic exists in I-07 (merged): `crates/ats-provider-claude/src/install.rs` with tests for preserving settings. The `ats install` CLI entry point is in I-17 (not merged). |
| 2 | Claude Code start → WORKING on target pane | Not Yet Implemented | Requires I-17 hook-path commands (`ats ingest`) and I-26 standalone mode (both not merged). Core daemon event pipeline (I-13, I-14) is merged but the hook trigger is missing. |
| 3 | ATTENTION display (+ RISK emphasis) | Partially Implemented | Core rendering pipeline merged: state engine (I-08), tmux renderer (I-09), RISK classifier (I-07). Full end-to-end hook trigger path requires I-17 (not merged). |
| 4 | RESULT + auto-return (TTL → IDLE) | Implemented | TTL expiry mechanism in state engine (I-08, merged): `result_expires_to_unknown_then_idle`, `ttl_precision_within_one_second` tests on main. |
| 5 | No pane leakage | Partially Implemented | Tmux pane isolation is implemented in I-09 (merged) via `target-session` + `target-pane` scoping. Comprehensive E2E validation (`e2e_multi_pane_isolation`) is in I-24 (not merged). |
| 6 | Fail-open: agent works without daemon | Not Yet Implemented | Standalone mode fallback is in I-26 (not merged). Hook-path fail-open (`ats ingest`/`ats event` returning exit 0 on errors) is in I-17 (not merged). On main, the prototype `event` command exists but is not integrated into the agent hook fire flow. |
| 7 | Manual reset (`ats reset`) | Not Yet Implemented | `ats reset` subcommand is in I-17 (not merged). `standalone_reset` is in I-26 (not merged). No reset functionality exists on main. |
| 8 | No persistence by default | Implemented | `privacy.store_activity_labels: false` default set in I-06 (merged). Log redaction layer (I-15, not merged) provides complementary home-path redaction. |
| 9 | `ats doctor` detects misconfig | Not Yet Implemented | `ats doctor` module is in I-18/I-21 (not merged). No diagnostic tooling exists on main. |
| 10 | 30-min soak, no state residue | Partially Implemented | TTL expiry mechanism (I-08, merged) provides the core idempotency guarantee. Property-based tests (`proptest_invariants`) are in I-23 (not merged). |

## Test Coverage Summary (on `main`)

| Layer | Count (main) | Notes |
|-------|-------------|-------|
| Unit (in-crate) | ~200 | Event transformations, state transitions, TTL, sanitization, risk classification, theme resolution, config validation, pane isolation |
| Integration | ~40 | Schema validation, socket server, frame protocol, PID file, fixture corpus |
| E2E | 0 | No E2E tests on main (I-24 not merged) |
| Property-based | 0 | No proptest on main (I-23 not merged) |
| Security | ~4 | Socket mode 0600, PID file locking, frame size cap |

## Dependency Verification

| Issue | PR | Description | Merged to main? |
|-------|----|-------------|:---:|
| I-01 | #29 | Workspace scaffolding | ✓ |
| I-02 | #30 | Core data model | ✓ |
| I-03 | #31 | Trait crates | ✓ |
| I-04 | #32 | Hook fixture corpus | ✓ |
| I-05 | #33 | Terminal spikes | ✓ |
| I-06 | #34 | Config types + themes | ✓ |
| I-07 | #35 | Claude Code hook adapter | ✓ |
| I-08 | #36 | State engine | ✓ |
| I-09 | #37 | Tmux renderer | ✓ |
| I-10 | #38 | iTerm2 renderer | ✓ |
| I-11 | #39 | Notification renderer | ✓ |
| I-12 | #40 | Rendering engine | ✓ |
| I-13 | #41 | Daemon socket server | ✓ |
| I-14 | #42 | Daemon broker | ✓ |
| I-15 | #44 | Logging + redaction | ✗ |
| I-16 | #45 | launchd autostart | ✗ |
| I-17 | #46 | Hook-path commands | ✗ |
| I-18/I-21 | #47 | Query commands + doctor | ✗ |
| I-19 | #48 | Theme commands | ✗ |
| I-20 | #50 | Daemon subcommands | ✗ |
| I-22 | #49 | Shell completions | ✗ |
| I-23 | #43 | Test harness | ✗ |
| I-24 | #54 | E2E + fault injection | ✗ |
| I-25 | #52 | Threat model | ✗ |
| I-26 | #51 | Standalone mode | ✗ |
| I-27 | #53 | Packaging + dist | ✗ |
| I-28 | This doc | Acceptance verification | ✗ |

## Conclusion

**MVP 0.1 is NOT ready for release** based on what is merged to `main`.

Only 2 of 10 SPEC §21 acceptance criteria are fully satisfied (criteria 4 and 8).
Four criteria are partially satisfied (1, 3, 5, 10) — core infrastructure is in
place but end-to-end integration requires unmerged branches. Four criteria are
not yet satisfied (2, 6, 7, 9) — the required modules do not exist on main.

All 14 PRs for I-15 through I-28 must be merged before the acceptance criteria
can be fully verified. The core platform (I-01 through I-14, all merged) provides
a solid foundation: the state engine, event data model, renderer pipeline, and
daemon infrastructure are complete and tested.
