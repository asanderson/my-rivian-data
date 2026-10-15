# Phase I Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Finish all implementable Phase I subsystems and deliver reviewed desktop packages with explicit external acceptance gates.

**Architecture:** Preserve the approved native Rust/Axum plus embedded React/Rust-WASM architecture. Add portable Parallax decoding, native lifecycle-bound streaming/vault adapters, fixed-operation inventory and controlled write confirmation; reuse the existing protected local boundary.

**Tech Stack:** Rust 1.99.0, Axum/Tokio, reqwest/rustls, native WebSocket and native OS credential backends, React/TypeScript, wasm-bindgen, Node 24 build tools.

**Spec:** `docs/superpowers/specs/2026-10-14-phase-i-completion.md`

## Global constraints

- GPL-3.0-only with retained third-party notices and corresponding source.
- Native authority owns tokens, signing, destinations and per-account vehicle/resource access.
- Browser/WASM receives no reusable Rivian token or key; demo never accepts credentials.
- Keep loopback, Host/Origin, cookie/capability and CSRF checks on every new protected route.
- No arbitrary proxy/signing, automatic mutation retries, plaintext vault fallback or invented telemetry.
- Keep external real-account, enrollment, signing and independent-provider evidence visibly unverified.

## Review focus

- Account logout while credentials, vault IO or telemetry reconnect is pending: revoke old generations and suppress results.
- Switching vehicles with partial or late telemetry: never attribute another vehicle's reading to the selection.
- Missing/zero/unknown protobuf fields and timestamps: preserve absence and reject stale/malformed data.
- Locked OS store or unavailable Linux Secret Service: fail protected persistence without breaking memory-only sign-in.
- Ambiguous write or installer/update failure: no automatic mutation replay and preserve a verified rollback path.

### Task 1: Portable current-schema Parallax state

**Files:** `crates/rivian-core/src/parallax.rs`, core module export/dependencies, WASM wrapper and parity fixtures.
**Interfaces:** `TelemetryState::new(vehicle_id)->Result`, `apply(rvm,payload_base64,timestamp_ms)->Result<UpdateOutcome>`, `snapshot()->Value`, `developer_entries()->Vec<Value>` and fixed `READ_RVMS`.
- [x] Write failing tests for documented wire fields, explicit present zero, absent values, bad Base64/length/nesting, stale updates, invalid tires and contradictory merge ages.
- [x] Implement bounded decoders and field-aware merge; preserve source/schema provenance.
- [x] Verify native tests and compiled-WASM parity using identical synthetic wire fixtures.

### Task 2: Native authenticated telemetry and host lifecycle

**Files:** `crates/rivian-api/src/telemetry.rs`, native auth credential helper, host telemetry routes, UI shared telemetry hook/workflows.
**Interfaces:** owned `SubscriptionCredentials` with zeroizing tokens and epoch watch; `TelemetryService::start`, `snapshot`, `stop`; protected local routes defined in the spec.
- [x] Write mock socket tests for init/ACK/subscribe, ping/pong, reconnect, authentication failure, bounds, revocation and worker replacement.
- [x] Implement fixed TLS transport, safe errors, bounded retries and current-generation snapshot.
- [x] Integrate owner source timestamps and developer start/stop; prove account/vehicle/logout races in native and browser tests.

### Task 3: Versioned inventory and controlled operations

**Files:** `crates/rivian-core/src/inventory.rs`, `live.rs`, inventory fixtures, host catalog/admission/confirmation handlers, developer workbench.
**Interfaces:** inventory snapshot/enrichment and resource scope; exact reviewed-document admission; prepare/consume confirmation routes and fixed `rename-vehicle` mutation.
- [x] Write inventory completeness/provenance, bounded input, unknown/import mismatch and blocked-operation tests.
- [x] Implement reviewed additional reads with native resource authority; catalog remaining operations with honest gates.
- [x] Write one-use/expiry/account-switch/direct-dispatch bypass tests, implement native confirmation and verify exact variables reach the fixed write once.

### Task 4: Protected remembered sign-in and MFA

**Files:** `crates/rivian-api/src/vault.rs`, native client, host account routes, account form and storage tests.
**Interfaces:** `remember(bool)`, `restore_remembered()`, `vault_status()`, native-only secret bundle; safe vault status; bounded fixed MFA resend.
- [x] Prove unavailable/locked store, malformed/expired bundle, mismatched restored account and late-store-write/logout behavior with isolated test stores.
- [x] Implement explicit platform credential builders and opt-in native persistence, restoration and forgetting.
- [x] Verify browser credentials/storage boundaries, session-only fallback and remembered-session logout outcomes.

### Task 5: Desktop delivery, measurements and release review

**Files:** `packaging/**`, `scripts/package.mjs` and release/SBOM/benchmark/acceptance helpers, CI workflows, release/owner/developer documentation and completion ledger.
- [x] Add checked installer sources for Windows/macOS/Linux with clean-source identity, exact checksums, notices and SBOM.
- [x] Implement signing/notarization-required release workflow and protected rollback/update tooling; no PR can publish a signed release.
- [ ] Run deterministic benchmark and outside-source package acceptance; retain sanitized raw/summary evidence.
- [ ] Run full verification and independent source review, implement findings and publish PR without merging.
- [x] Mark external real-account/vehicle, certificate-signing and independent-provider acceptance accurately.
