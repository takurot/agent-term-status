# Claude Code Hook Fixture Corpus (I-04)

Real Claude Code hook `stdin` JSON payloads that drive `ats-provider-claude`
parser development (I-07). Captured per SPEC §9.1; sanitized per SPEC §14.2.

## Layout

```
tests/fixtures/claude/<claude-code-version>/<HookName>/<scenario>.json
```

- `*.json` (no `.synthetic` marker) — captured verbatim from a real
  `claude -p` session, then sanitized (see below).
- `*.synthetic.json` — hand-derived from a real capture (or from Anthropic
  hook documentation where headless capture is impossible). Used for
  forward-compatibility testing.

## Captured versions

| Version | How captured |
|---------|--------------|
| `2.1.214` | Locally installed Claude Code, headless `claude -p` sessions |
| `2.0.77` | `npm install @anthropic-ai/claude-code@2.0.77` into a temp prefix |

## Scenarios

| Scenario | Session | Hooks exercised |
|----------|---------|-----------------|
| `no-tool-*` | prompt answered without tools | SessionStart, UserPromptSubmit, Stop, SessionEnd |
| `tool-success-*` | `echo fixture-test` via Bash (allowlisted) | + PreToolUse, PostToolUse |
| `tool-failure-*` | `false` via Bash (exit 1, allowlisted) | + PreToolUse, PostToolUseFailure |
| `permission-denied-*` | non-allowlisted Bash in headless mode | PreToolUse only (tool blocked, no PostToolUse) |
| `capture-*` | 2.0.77 runs of the three scenarios above (unattributed) | session-level hooks |
| `missing-field.synthetic` | real payload minus `session_id` / `cwd` | forward-compat: missing fields |
| `unknown-field.synthetic` | real payload plus unknown future fields | forward-compat: unknown fields |

## Observed schema (summary)

Common to every hook: `session_id`, `transcript_path`, `cwd`,
`hook_event_name`.

| Hook | Extra fields (2.1.214) |
|------|------------------------|
| `SessionStart` | `source` (`"startup"`) |
| `UserPromptSubmit` | `prompt`, `prompt_id`, `permission_mode` |
| `PreToolUse` | `tool_name`, `tool_input`, `tool_use_id`, `prompt_id`, `permission_mode` |
| `PostToolUse` | + `tool_response`, `duration_ms` |
| `PostToolUseFailure` | + `error`, `is_interrupt`, `duration_ms` |
| `Notification` | `message` (synthetic; see below) |
| `Stop` | `stop_hook_active`, `last_assistant_message`, `background_tasks`, `session_crons`, `prompt_id`, `permission_mode` |
| `SessionEnd` | `reason`, `prompt_id` |

### Version drift observed (2.0.77 → 2.1.214)

- `prompt_id` added to `UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
  `PostToolUseFailure`, `Stop`, `SessionEnd`.
- `duration_ms` added to `PostToolUse` / `PostToolUseFailure`.
- `last_assistant_message`, `background_tasks`, `session_crons` added to `Stop`.
- `PostToolUseFailure` fires in **both** versions on non-zero tool exit
  (SPEC §9.1 mapping is valid for both).

Parsers (I-07) must tolerate both shapes: ignore unknown fields, never
require version-specific extras (SPEC §5.2.2).

### Notification caveat

`Notification` does not fire in headless (`claude -p`) sessions — permission
prompts fail the tool instead, and idle prompts require an interactive TTY.
Its fixtures are `*.synthetic.json` payloads modeled on the Anthropic hook
documentation (`message` field). Replace with real captures once an
interactive capture session is run (tracked in I-07).

## Sanitization applied (SPEC §14.2)

Before commit, every capture was rewritten:

- real home directory → `/Users/testuser`
- macOS temp sandbox path (`/private/var/folders/...`) → `/Users/testuser/sandbox/project`
- munged transcript project dir → `-Users-testuser-sandbox-project`
- prompts in `UserPromptSubmit.prompt` / `Stop.last_assistant_message` are
  the synthetic test prompts themselves (no user data)

`sanitize-check.sh` enforces: no `/Users/<real-user>`, no `/var/folders/`,
no `~` shorthand, no API-key-shaped strings. It runs in CI via
`crates/ats-provider-claude/tests/fixture_corpus.rs`, which re-implements
the same checks in Rust plus corpus-shape guarantees (≥30 fixtures,
≥2 versions, all 8 hook types with missing-/unknown-field variants).
