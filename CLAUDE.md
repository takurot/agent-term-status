Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

## 5. Bug Fix Discipline

**After fixing a bug, grep for sibling paths before closing the task.**

When patching one code path in a transformation, normalization, or dispatch module:
- Grep the same file for other functions/branches that produce the same output type.
- Pay special attention to: `match` branches, parallel helper functions, `if node_type ==` style guards.
- A fix that passes tests can still miss a sibling path if the tests only exercise the primary path.

```bash
# Example: fixing PII masking in one function — check the whole file
grep -n "label: Some\|node_value\|redact" src/sre/normalization.rs
```

## 6. Test the Exact Path

**Tests for a bug fix must hit the specific code path being fixed, not a proxy.**

If fixing function A, assert on A's direct output — not on a caller that aggregates A's result with other functions. Indirect tests can pass even when the fixed path is still broken.

```rust
// Weak: asserts on the aggregating caller's output (may not exercise the fixed path)
assert_eq!(container.label, Some("***"));

// Strong: asserts on the raw output of the fixed path directly
assert_eq!(container.children[0].label, Some("***"));
```

## 7. nextest Profile Fields Are Not Inherited

**`[profile.ci]` does not inherit from `[profile.default]` — repeat every field.**

nextest named profiles start from nextest's built-in defaults, not from `[profile.default]`. A field set only in `[profile.default]` silently falls back to the built-in default in any other profile.

```toml
# WRONG — failure-output silently becomes "never" in CI
[profile.default]
failure-output = "immediate"

[profile.ci]
retries = { backoff = "exponential", count = 3, delay = "2s", max-delay = "60s" }
# failure-output missing → not inherited, falls back to built-in "never"

# CORRECT — add a NOTE and repeat every required field
[profile.ci]
# NOTE: nextest profiles do NOT inherit from [profile.default]. Repeat all fields.
retries = { backoff = "exponential", count = 3, delay = "2s", max-delay = "60s" }
failure-output = "immediate"
fail-fast = false
```

## 8. cargo-deny v2 Setup Checklist

When adding `cargo-deny` to a Rust workspace, four things will trip you up:

1. **Removed keys**: `unlicensed` and `copyleft` are gone in v2. Use `allow = [...]` — anything not listed is implicitly denied.

2. **Workspace crates without `license` field** → `error[unlicensed]`. Fix once at the workspace level:
   ```toml
   # Root Cargo.toml
   [workspace.package]
   license = "MIT"
   # Each member Cargo.toml
   license.workspace = true
   ```

3. **`cargo install cargo-audit/cargo-deny` takes 3–5 min in CI**. Use pre-built binary actions instead:
   ```yaml
   - uses: rustsec/audit-check@v2          # security-audit (informational)
     continue-on-error: true
   - uses: EmbarkStudios/cargo-deny-action@v2  # enforcing gate
   ```

4. **`mapfile` is bash-only**. Add `shell: bash` explicitly to any step that uses it:
   ```yaml
   - name: Lint shell scripts
     shell: bash
     run: |
       mapfile -t scripts < <(find . -name "*.sh" ...)
   ```

## 9. Swatinem/rust-cache Parallel-Job Strategy

When adding `Swatinem/rust-cache` to a CI with multiple parallel Rust jobs, three things matter:

1. **No `shared-key` across uncoordinated parallel jobs**. All jobs race to write the same key; the last writer wins and may overwrite a full workspace cache with a single-crate job's partial cache.
   ```yaml
   # WRONG — 22 jobs fight over one cache slot
   - uses: Swatinem/rust-cache@v2
     with:
       shared-key: "workspace"

   # CORRECT — use default per-job keys
   - uses: Swatinem/rust-cache@v2
   ```

2. **`save-if` on PR-only and benchmark jobs** to prevent benchmark artifacts from polluting the shared cache:
   ```yaml
   - uses: Swatinem/rust-cache@v2
     with:
       save-if: ${{ github.ref == 'refs/heads/main' }}
   ```

3. **Add `rust-toolchain.toml`** so the cache key is deterministic and toolchain changes are visible:
   ```toml
   [toolchain]
   channel = "stable"
   ```

## 10. Bash Install Script Portability

When writing a curl-pipe install script (`scripts/install.sh`) targeting macOS and Linux:

1. **Use `| bash`, never `| sh`** in README install commands. On Linux, `/bin/sh` is `dash` which ignores the shebang and fails on `set -o pipefail`:
   ```bash
   # WRONG — breaks on Linux (dash has no pipefail)
   curl -fsSL https://example.com/install.sh | sh

   # CORRECT
   curl -fsSL https://example.com/install.sh | bash
   ```

2. **Add `shasum -a 256` fallback** for macOS where `sha256sum` is not in PATH:
   ```bash
   if command -v sha256sum >/dev/null 2>&1; then
       ACTUAL="$(sha256sum "${BINARY}" | awk '{print $1}')"
   else
       ACTUAL="$(shasum -a 256 "${BINARY}" | awk '{print $1}')"
   fi
   ```
   In README examples, `shasum -a 256 -c` works on both platforms.

3. **Always `mkdir -p "${INSTALL_DIR}"`** before `mv` — custom install dirs may not exist.

## 11. Rust Test Env Isolation (thread-safe env-var testing)

`std::env::set_var` / `remove_var` are thread-unsafe in parallel test runners and are documented as unsound in Rust 1.81+. Never use them in tests.

**Instead, pass the env value as a parameter:**

```rust
// Production function — accepts optional override
fn chrome_path_detail_with_env(chrome_env: Option<&str>) -> (bool, String) {
    if let Some(path) = chrome_env {
        // use path directly, no global env read
        return if Path::new(path).exists() { ... } else { ... };
    }
    // fall through to real detection
}

// Thin wrapper for production use
fn chrome_path_detail() -> (bool, String) {
    chrome_path_detail_with_env(std::env::var("CHROME_PATH").ok().as_deref())
}

// Tests inject value directly — no set_var, no global mutation
#[test]
fn chrome_check_with_nonexistent_path_fails() {
    let (passed, detail) = chrome_path_detail_with_env(Some("/nonexistent/chrome"));
    assert!(!passed);
    assert!(detail.contains("file not found"));
}
```

Apply this pattern to any function that reads env vars and needs unit testing.

## 12. Rust Capture-Before-Propagation (preserve partial metering/audit state)

When a fallible sequence accumulates counts or state in a helper object, using `?` to
propagate an error silently drops everything the sequence completed before it failed.

**Always capture accumulated state before the `?`:**

```rust
// WRONG — partial delta/count lost if engine.run() returns Err
let report = engine.run(&skill, &mut runtime)?;
self.last_delta = runtime.into_delta();  // never reached on error

// CORRECT — capture unconditionally, then propagate
let result = engine.run(&skill, &mut runtime);
self.last_delta = runtime.into_delta();  // always runs
let report = result?;
```

This applies to any operation that accumulates metering counters, audit events, or
progress state across a sequence of steps. Watch for `runtime.into_X()` or
`accumulator.take()` calls that appear after a `?` — they are silently skipped on
error paths.

## 13. cargo-llvm-cov CI Setup (4 Gotchas)

When adding a `cargo-llvm-cov` coverage job to GitHub Actions, four non-obvious
failures will bite you:

1. **`--text` is required** — `cargo llvm-cov report` without `--text` exits silently
   (zero bytes on stdout). GitHub Step Summary will be an empty code block.

2. **`save-if: "false"` (string) causes "Unable to resolve action, repository not found"**
   — passing a string literal to `Swatenim/rust-cache`'s `save-if` input corrupts
   action lookup during the "Prepare all required actions" phase. Either omit the
   cache step entirely or use an expression: `save-if: ${{ github.ref == 'refs/heads/main' }}`.

3. **Add `--ignore-filename-regex '(tests/|test\.rs$)'`** to all three invocations
   (collect + text report + lcov). Without it, test file bodies inflate the coverage
   percentage and the 70% threshold becomes meaningless.

4. **No `Swatenim/rust-cache` in the coverage job** — LLVM-instrumented objects are
   incompatible with non-instrumented builds. If the coverage job writes to the shared
   cache slot it forces every other job to fully recompile.

Always add `needs: [test]` so coverage only runs after the correctness gate passes.

## 14. In-repo NFR Baseline Pattern (Benchmark Trend Tracking)

When adding performance regression detection to CI benchmark jobs without external services:

1. **Check baseline JSON into the repo** (`nfr-baseline/*.json`) — mirrors the format the benchmark test writes. Update manually with `scripts/update_nfr_baseline.sh` after intentional improvements. No cross-run artifact sharing needed.

2. **Use merged thresholds (current ∪ baseline)** so the baseline can declare soft trend-only thresholds for metrics the tests don't enforce:
   ```python
   merged_thresholds = {**metric.get("thresholds", {}), **baseline.get("thresholds", {})}
   ```
   Example: capacity tests only assert `success_rate_min_min`, but the baseline file can add `capture_p95_ms_max` so duration drift is also detected.

3. **Use `continue-on-error: true`** on the trend CI step — the hard threshold gate stays in the test assertions. The trend step adds visibility without becoming a second noisy fail gate.

4. **Skip count/target keys** (`trials`, `sessions_target`) — these are not performance metrics and should be excluded from regression comparison.
