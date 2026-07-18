# Contributing

Development workflow, task plan, and conventions live in:

- [docs/WORKFLOW.md](docs/WORKFLOW.md) — issue-driven development flow (TDD, PR, CI, merge)
- [docs/TASK_PLAN.md](docs/TASK_PLAN.md) — task breakdown and dependencies
- [docs/SPEC.md](docs/SPEC.md) — full specification

## Quick reference

```bash
cargo build --workspace                                   # build all crates
cargo test --workspace                                    # run all tests
cargo clippy --all-targets --all-features -- -D warnings  # lint
cargo fmt --all -- --check                                # format check
```

- Branch from `main`: `feature/issue-<number>-<short-description>`
- Commit style: `<type>(<scope>): <description>` (scope = crate name)
- All tasks are GitHub issues with scope, DoD, and dependencies in the body.
