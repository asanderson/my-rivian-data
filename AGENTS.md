# My Rivian Data

The project's purpose is to give Rivian owners easy access to the data associated
with the vehicles they own: "Your vehicles. Your data."
This is an original implementation of the selected Rust/Axum + React/Rust-WASM plan.
Phase 1 connects the standalone desktop web app to the unofficial Rivian API.
Keep explicit `--demo` mode for credential-free samples and deterministic tests.
Do not claim real-account verification, full API coverage, command execution,
official affiliation, independent-vendor review, or signed cross-platform packages
unless corresponding evidence actually exists.

## Boundaries

- Keep the default interface and opening documentation owner-focused, grouped by
  functional area. Put raw API tools and implementation details in the separate
  developer view and subsequent developer documentation.
- Label sample readings and missing data honestly; never infer health diagnostics
  from battery charge or other incomplete telemetry.

- Keep portable models, catalog validation and fixtures in `rivian-core`.
- `rivian-wasm` wraps the same bounded, non-secret validation logic.
- Native Rust owns application authority. Revalidate every operation in the host.
- Never accept Rivian credentials in demo mode; never log or persist secrets.
- In live mode, send login inputs only to native Rust over the protected local
  session. Keep Rivian session tokens native and memory-only. Do not invent a
  refresh-token exchange or treat an unverified API as supported.
- Account-owned vehicle IDs must be checked natively before dispatch; browser
  validation and a syntactically valid ID never establish account authority.
- Bind the host to IPv4 loopback only. Preserve Host, Origin, session and CSRF checks.
- No generic URL proxy, arbitrary signing, shell, filesystem or token-read endpoints.
- Preserve the repository GPLv3 license and retain third-party notices.
- Mobile will link the native core into Tauri; do not design it around an Axum server.

## Verification

Run formatting, clippy, Rust tests, TypeScript checking, release builds and the
native/WASM parity suite. Add security regression checks for real boundary changes.
Keep source and dependency locks; exclude build caches from source deliverables.
Record unrun OS/device and live-account checks in `docs/STATUS.md`.
