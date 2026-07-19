# Threat Model (SPEC §14)

This document enumerates the threat surface of `agent-term-status` and maps
each threat to the architectural defense. It is required for I-25: Threat
model doc + distributed security DoD audit.

## Scope

The tool has two main execution contexts:
1. **Hook path** (`ats ingest` / `ats event` / `ats reset`): invoked by AI
   agent hook fire at high frequency. Must return within 50ms and never block
   the agent (fail-open).
2. **Daemon** (`ats-daemon`): a long-lived background process that receives
   events over a Unix Domain Socket (UDS) and dispatches rendering commands to
   terminal renderers.

## Trust Boundaries

```
+-------------------+       UDS (0600)       +--------------------+
| AI Agent (Claude) | ----> | ats CLI (hook) | ----> | ats-daemon |
+-------------------+       +----------------+       +-------------+
                                       |                         |
                                       v                         v
                              Terminal renderers        State engine
                              (tmux, iTerm2,            (in-memory)
                               notifications)
```

### Boundary 1: Agent → Hook CLI
- The agent spawns the `ats` binary and pipes JSON on stdin (`ats ingest`)
  or passes state as CLI args (`ats event`).
- **Threat**: Malformed JSON, oversized payloads, JSON bombs.
- **Defense**: `ats-core` validates all input via serde typed deserialization at the daemon boundary (`serde_json::from_slice` in `buffer_event`); a JSON Schema test suite (`crates/ats-core/tests/schema_validation.rs`) provides independent verification against `schemas/event-v1.schema.json`.
    Oversized payloads (>64 KiB) are rejected before parsing. All hooks
    return exit code 0 (fail-open, SPEC §15).

### Boundary 2: Hook CLI → Daemon
- The CLI connects to the daemon over a Unix Domain Socket at
  `$XDG_RUNTIME_DIR/agent-term-status.sock` (or fallback).
- **Threat**: Unauthorized local processes connecting to the socket.
- **Defense**: Socket is created with mode `0600` (owner-only). The PID file
  guards against duplicate daemon instances.

### Boundary 3: Daemon → Terminal Renderers
- The daemon invokes `tmux`, emits OSC sequences (iTerm2), or calls
  notification APIs.
- **Threat**: Shell injection via manipulated event fields.
- **Defense**: All provider-derived strings (labels, symbols, pane IDs) are
  sanitized before use. `TmuxRenderer::validate_arg` rejects characters
  (`;`, `$`, `` ` ``, `|`, `&`, etc.) that could enable injection. Pane
  IDs are validated to match `%[0-9]+` or `=[0-9]+`.

## Threat Catalog

| # | Threat | Severity | Defense | Issue |
|---|--------|----------|---------|-------|
| T1 | JSON bomb via stdin (ingest) | High | 64 KiB frame cap, serde deserialization | I-07, I-13 |
| T2 | Invalid tmux pane ID injected in `TMUX_PANE` | High | Regex validation (`%[0-9]+`) | I-09 |
| T3 | Shell injection via activity label in border format | High | Forbidden char filter (`validate_arg`) | I-09 |
| T4 | Unauthorized UDS client (other local user) | High | Socket mode `0600`, parent dir `0700` | I-13 |
| T5 | Duplicate daemon (stale PID/socket) | Medium | PID file (`O_EXCL`-style via `PidFile`), stale detection | I-13 |
| T6 | Home path leak in log output | Medium | Redaction layer strips `/Users/<name>` → `~` | I-15 |
| T7 | Prompt body / file contents in events | High | NOT collected by default (privacy invariant) | I-07 |
| T8 | API keys / secrets in command strings | Critical | NOT collected by default; RISK classifier only tags command class | I-07 |
| T9 | Daemon memory exhaustion (event flood) | Medium | Channel bounded to 1024 events, backpressure on socket reads | I-13, I-14 |
| T10 | Stale render residue after daemon restart | Low | Daemon does NOT re-render on restart (fresh broker starts empty, waits for next event); standalone mode is one-shot | I-14, I-26 |
| T11 | Control characters in OSC sequences | Medium | Sanitizer strips C0, C1, DEL, surrogate chars | I-10 |
| T12 | OSC sequence injection via theme fields | Medium | `ats doctor` validates bundled themes (compiled-in, no file permissions concern); external theme files loaded via `load_from_path` do not enforce `0600` | I-06, I-21 |

## Privacy Invariants (SPEC §14.2)

These are always-on protections. No configuration can weaken them:

1. **No prompt bodies**: The hook adapter extracts metadata only (session ID,
   hook event name, tool class). Prompt text is never read or transmitted.

2. **No file contents**: Paths are reduced to basenames via
   `ActivityLabel::from_path` (I-02).

3. **No command strings**: The RISK classifier (I-07) operates on
   whitelisted command prefixes only; it never captures or stores the full
   command.

4. **No API keys**: The tool never reads or transmits environment variables
   that may contain secrets.

5. **No full paths**: Home directory paths in log output are replaced with
   `~` by the redaction layer (I-15).

## Secure Defaults

| Setting | Default | Rationale |
|---------|---------|-----------|
| Socket permissions | `0600` | Owner-only; prevents other users on same host |
| PID file permissions | `0600` | Prevents PID tampering |
| State directory permissions | `0700` | Agent-term-status owns this directory exclusively |
| Log redaction | On | Strips home paths from all log output |
| Activity label storage | Off | SPEC §14.2 default: `privacy.store_activity_labels: false` |
| Frame size limit | 64 KiB | Prevents memory exhaustion from oversized events |
| Event channel capacity | 1024 | Backpressure prevents unbounded memory growth |
| RISK classifier | Allowlist-only | Only known-dangerous command prefixes trigger RISK |

## Audit Checklist

For each new feature or code change touching the hook path, daemon, or
renderers, verify:

- [ ] No new path that reads prompt body text
- [ ] No new path that captures command arguments (only command class)
- [ ] Any new log field passes through the redaction layer
- [ ] Any new tmux argument passes through `validate_arg`
- [ ] Any new CLI input is validated before use (never trusted raw)
- [ ] Fail-open: no error path aborts the agent process
- [ ] No secrets or full paths in test fixtures (verified by I-04 corpus check)
