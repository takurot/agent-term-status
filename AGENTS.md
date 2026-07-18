# Agent Instructions

AI coding agents working in this repository must follow:

- [CLAUDE.md](CLAUDE.md) — behavioral guidelines (think before coding, simplicity, surgical changes, TDD discipline)
- [docs/WORKFLOW.md](docs/WORKFLOW.md) — issue-driven development workflow
- [CONTRIBUTING.md](CONTRIBUTING.md) — commands and conventions

Project invariants (never break):

- **Fail-open**: hook paths (`ats ingest` / `ats event`) return success even on internal errors.
- **Privacy**: prompt bodies, file contents, command strings, API keys, and full paths are never collected or stored by default.
- **Hook latency**: `ats ingest` / `ats event` complete within 50 ms.
