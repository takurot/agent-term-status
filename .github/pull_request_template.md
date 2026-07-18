## Summary

<!-- What does this PR change and why? Link the issue. -->

Closes #

## Changes

-

## Test plan

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes

## Checklist

- [ ] Tests written first (TDD) and cover the new behavior
- [ ] No prompt bodies / file contents / API keys / full paths are collected or stored (SPEC §14)
- [ ] Fail-open preserved: hook paths return success even on internal errors
