# Phase I completion design

This implements the approved October 7 `rivian-cross-platform-application-plan.pdf`, version 4, especially pages 6–8 and 14, and the owner's October 14 instruction to finish Phase I. The selected architecture and execution authorization already exist. Implementation proceeds in an isolated feature worktree with scoped agents and review; no new architecture approval is required.

## Outcome and scope

My Rivian Data gives owners access to their own Rivian account and vehicle data through a self-contained Rust/Axum executable with embedded React and Rust/WASM, on Linux, Windows and Intel/Apple Silicon macOS. Complete modern telemetry, the reviewed operation inventory, optional native protected session storage, controlled writes, installation/release integrity and acceptance tooling. Existing owner areas and separate developer explorer remain the product structure.

The original plan permits enrollment-dependent commands to remain cataloged and blocked until a legitimate desktop enrollment path is proven. No other phone's secrets are imported. Android/iOS, advanced history/background collectors, push, live media and deeper pairing remain later scope.

## Telemetry

Portable `rivian-core::parallax::TelemetryState` decodes documented protobuf wire fields from the pinned current community schema. Missing physical measurement tags/unknown enums stay unknown; the documented non-optional tire-validity control uses its proto3 zero default. Explicit present measurement zero remains zero, invalid values are rejected, and partial updates merge only by original per-field timestamps. Payloads are bounded to 32 KiB and developer entries are bounded and separate from owner state.

Native `rivian-api::telemetry::TelemetryService` connects only to the reviewed Rivian WebSocket URL using native sensitive session headers and graphql-transport-ws. Ownership and account lifecycle are checked before connecting and throughout handshake, reads and reconnect. Start/stop replaces the prior vehicle worker, bounded reconnection resubscribes, authentication failures stop, and logout cancels work and clears readings.

Protected local routes: POST `/api/telemetry/start` with `vehicle_id`, POST `/api/telemetry/stop`, and GET `/api/telemetry`. They use the existing cookie, per-origin tab capability, Host/Origin and CSRF protections. Browser polling reads a native snapshot every two seconds with one request in flight. It never receives Rivian tokens. Owner state never crosses vehicle/account generations; original field timestamps distinguish observation from retrieval.

## Operation inventory and controlled writes

A versioned inventory covers the pinned RivDocs App operations, pinned client GraphQL root fields and named documents, Parallax RVMs and command vocabulary. Every entry reports source/review date, endpoint/transport, document/schema, prerequisites, implementation and verification status, retry policy and live test evidence (initially absent). Unknown private operations are not claimed known.

Executable operations retain fixed native documents and strict bounded variables. Resource-scoped reads additionally check native account-owned order/charger identifiers. Expert document admission matches reviewed compiled documents; it never creates an arbitrary GraphQL or URL proxy.

`rename-vehicle` is the initial reviewed write not requiring key enrollment. POST `/api/commands/prepare` validates variables, current ownership and account generation, returns the exact operation/variables plus a random confirmation identifier valid for two minutes. POST `/api/commands/execute` consumes that identifier once, rechecks session/account generation and authority, sends the fixed mutation once, and distinguishes success from ambiguous transport outcome. Ordinary `/api/execute` refuses mutations. The UI requires an explicit confirmation. Physical commands remain gated by enrollment evidence.

## Native protected sessions

Default sign-in remains memory-only. An explicit Remember sign-in choice stores only the native session bundle in Windows Credential Manager, macOS Keychain or Linux encrypted Secret Service. No plaintext file, browser storage or keyring mock fallback is permitted. The native adapter explicitly selects each platform backend. Locked/unavailable stores produce a safe status and never prevent session-only sign-in.

Restore validates stored structure/local retention limits, creates a new app session and discovers current account authority before reporting authenticated. No unsupported refresh grant is invented. Logout/forget serialize native store changes with account generation changes so pending writes cannot resurrect remembered authority. Explicit disconnect/end-session forget the remembered account; stopping the process preserves opted-in storage. MFA resend uses only a verified fixed contract and bounded attempts.

## Desktop release and evidence

Build Windows per-user installers, macOS .app/DMG for Intel and ARM64, Linux DEB/RPM and portable packages. Preserve corresponding source, dependency notices, exact artifact checksums and a locked-dependency SBOM. Development packages are explicitly unsigned. Signed release jobs fail closed without required owner-supplied signing/notarization credentials and never publish from pull-request code. Install/update/rollback tooling verifies source identity and package integrity before replacing installed versions.

Synthetic benchmark and local real-account acceptance tools record measurements/outcomes without passwords, tokens, raw private responses or launch capabilities. Real account/vehicle tests, actual certificate signing/notarization and independent-provider review remain external evidence gates until performed; tooling or mock success never closes those gates.

## Completion evidence

Run native/WASM parser/validator parity, malformed and out-of-order fixtures, HTTP and WebSocket contract tests, logout/account/vehicle race regressions, vault lifecycle tests using isolated fake stores, owner/developer browser workflows, strict Rust/TypeScript checks, dependency advisory scans, package checks, benchmark outputs and platform CI. Review requirements and implementation independently, fix findings and rerun reproductions. Record the exact source revision and all unfinished external acceptance gates in `docs/PHASE-I-COMPLETION.md`.
