# Spike: macOS native notification viability (I-05 #4) — CRITICAL

**Decision: ship a minimal app-bundle helper (`ats-notifier.app`) driven by
the daemon; use `UNUserNotificationCenter` via `objc2-user-notifications`
inside the bundle. OSC 9 terminal notifications are the zero-permission
fallback. This unblocks I-11.**

## Constraint (measured on this machine)

```
$ swift -e 'import Foundation; print(Bundle.main.bundleIdentifier ?? "nil")'
bundleIdentifier: nil (CLI binary, no bundle)
```

`UNUserNotificationCenter` requires a main bundle with an identifier;
calling it from a bare CLI binary aborts with
`bundleProxyForCurrentProcess is nil` — a hard blocker for calling the
modern API directly from `ats`/`ats-daemon`. SPEC §17 forbids `osascript`.

## Options considered

| Option | Verdict |
|--------|---------|
| `objc2-user-notifications` from bare CLI | ✗ crashes (nil bundle) |
| `notify-rust` / `mac-notification-sys` (legacy `NSUserNotification`) | △ works from CLI by impersonating another bundle ID; API deprecated since 10.14; delivery not guaranteed on new macOS |
| **App-bundle helper** (`ats-notifier.app` containing a tiny Rust binary using `objc2-user-notifications`) | ✓ modern API, own bundle ID (`dev.ats.notifier`), own permission entry in System Settings |
| OSC 9 (`ESC ] 9 ; msg BEL`, iTerm2) | ✓ no permissions, renders as native notification; iTerm2-only, no persistence |

## Decision

1. `ats-renderer-notification` (I-11) talks to a helper binary packaged as
   `ats-notifier.app` (built by `cargo` + a build script that lays out
   `Contents/MacOS` + `Info.plist`; distributed inside the Homebrew keg,
   symlink-safe because it is invoked by absolute path).
2. Permission flow: first notification triggers the standard macOS prompt
   attributed to "ats notifier". `ats doctor` reports the current
   `UNAuthorizationStatus` (denied/authorized/not-determined).
3. The helper is stateless: `ats-notifier <state> <title> <body>` per
   invocation; latency is off the hook path (daemon-side, async).

## Fallback (documented per DoD)

- If bundle-helper distribution via Homebrew proves fragile: use OSC 9 for
  iTerm2 users (capability-detected) and log-only elsewhere; notifications
  become a degraded capability (`notification: false` in
  `RendererCapabilities`), never a crash.
- Quiet-hours and state filtering (SPEC §12) live in the dispatcher
  (I-12), not the helper, so a fallback swap does not touch policy code.
