# Task Plan for MVP 0.1

Generated from `docs/SPEC.md` v1.1.  
Reviewed by a critical review pass ([details](https://github.com/takurot/agent-term-status/issues?q=is%3Aissue+label%3Aphase-0+label%3Aphase-1+sort%3Acreated-asc)).  
All tasks exist as GitHub issues with full scope, DoD, and dependency annotations.

---

## Phase 0 — Foundation & Spikes

| # | Issue | Dependencies |
|---|-------|-------------|
| I-01 | Workspace scaffolding & CI baseline | — (root) |
| I-02 | `ats-core` — pure data model | I-01 |
| I-03 | `ats-provider` + `ats-renderer` trait crates | I-02 |
| I-04 | Claude Code hook fixture capture | I-01 |
| I-05 | Phase 0 spike — terminal integration decisions | I-02, I-03 |

Phase 0 deliverables: repo skeleton with CI green, shared data types, trait contracts, real hook fixture corpus, and resolved architectural decisions (tmux invocation, iTerm2 OSC, notification viability, OSC-to-TTY ownership).

---

## Phase 1 — Core Architecture

| # | Issue | Dependencies |
|---|-------|-------------|
| I-06 | `ats-config` — YAML config & themes | I-02 |
| I-07 | `ats-provider-claude` — Hook adapter + install | I-03, I-04 |
| I-08 | `ats-state-engine` | I-02 |
| I-09 | `ats-renderer-tmux` | I-03, I-05 |
| I-10 | `ats-renderer-iterm2` | I-03, I-05 |
| I-11 | `ats-renderer-notification` (macOS native) | I-05 |
| I-12 | Rendering Engine + NotificationDispatcher | I-03, I-06, I-08 |

---

## Phase 1 — Daemon

| # | Issue | Dependencies |
|---|-------|-------------|
| I-13 | `ats-daemon` socket server | I-02, I-03 |
| I-14 | Daemon broker logic | I-07, I-08, I-12, I-13 |
| I-15 | Daemon logging init with redaction layer | I-06, I-13 |
| I-16 | Daemon auto-start (launchd) | I-13 |

---

## Phase 1 — CLI

| # | Issue | Dependencies |
|---|-------|-------------|
| I-17 | CLI hook-path commands (`ingest`, `event`, `reset`) | I-07, I-08, I-13, I-14 |
| I-18 | CLI query commands (`status`, `list`, `logs`) | I-14 |
| I-19 | CLI theme commands | I-06 |
| I-20 | CLI daemon subcommands | I-13, I-16 |
| I-21 | `ats doctor` | I-07, I-09, I-10, I-11, I-12, I-14 |
| I-22 | Shell completions + version stamping | I-01 |

---

## Phase 1 — Quality & Release

| # | Issue | Dependencies |
|---|-------|-------------|
| I-23 | Test & perf harness | I-04, I-13 |
| I-24 | E2E + fault injection suite | I-07, I-09–I-12, I-14, I-17, I-23 |
| I-25 | Threat model doc + security DoD audit | I-02, I-06, I-07, I-09, I-10, I-13, I-15 |
| I-26 | Standalone mode fallback | I-12, I-17 |
| I-27 | Packaging & distribution | all |
| I-28 | MVP acceptance verification | all |

---

## Acceptance Criteria Traceability (SPEC §21)

| # | Criterion | Primary issues |
|---|-----------|---------------|
| 1 | install doesn't break config | I-07, I-24 |
| 2 | Claude start → WORKING | I-04, I-07, I-08, I-09, I-12, I-17, I-24 |
| 3 | ATTENTION display (+ RISK emphasis) | I-07, I-08, I-09, I-12 |
| 4 | RESULT + auto-return | I-08, I-09, I-24 |
| 5 | No pane leakage | I-05, I-09, I-24 |
| 6 | Fail-open | I-17, I-26, I-24 |
| 7 | Manual reset | I-17, I-09, I-10 |
| 8 | No persistence by default | I-06, I-15, I-25 |
| 9 | doctor detects misconfig | I-21 |
| 10 | 30-min soak, no residue | I-08, I-24, I-28 |

---

## Key Design Decisions from Review

1. Two trait crates added (`ats-provider`, `ats-renderer`) to keep `ats-core` as pure data model and give Phase 2/3 SDK work a clean foundation.
2. Security invariants distributed as DoD on owning tasks, not batched as a final pass. Replaced by threat-model audit (I-25).
3. Tests are per-task DoD, with cross-cutting E2E/fault-injection in I-24.
4. Phase 0 includes five decision spikes (tmux invocation, pane safety, iTerm2 OSC, native notification viability, OSC-to-TTY ownership) to surface architectural risks before Phase 1 work.
5. RISK classifier in I-07 is intentionally conservative (explicit allowlist) per SPEC §13.4.
6. Claude Code fixture corpus (I-04) drives parser development to avoid building against an imagined schema.
