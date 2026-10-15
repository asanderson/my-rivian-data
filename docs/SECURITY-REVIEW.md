# Phase I security boundary and review

Boundary documentation updated: 2026-10-14. Scope: the native Rust/Axum loopback
service, Rivian HTTP/WSS transport, fixed operation catalog, Rust/WASM validation,
React views, explicit OS-vault persistence, confirmed nickname write and desktop
artifact pipeline.

Source review and deterministic tests provide bounded evidence. They are not an
external penetration test, certification or proof of live Rivian compatibility.
The requested **independent Anthropic review remains outstanding**. Separate
agents from the implementation provider do not satisfy that independent-provider
gate. No real account credentials or vehicles were used in the recorded synthetic
checks. Current execution evidence belongs in [status](STATUS.md).

## Trust boundary

The app protects a loopback service from unrelated web pages and rejects
cookie-only replay from another local port. Native Rust alone holds Rivian
sessions and grants account/resource authority. WASM validation and catalog
membership cannot grant it. The browser is an owner interface with a separate
local capability, not an upstream credential store.

The model assumes a trusted OS, browser and browser extensions. Same-origin
script execution, malicious extensions, process memory access, debuggers, swap
and crash dumps can defeat that assumption. Memory-only defaults reduce deliberate
persistence; they do not guarantee erasure of every HTTP-library or OS copy.
Opt-in OS-vault protection inherits the credential store and the user's OS account.

## Threat vectors and controls

| Threat | Implemented boundary | Residual risk or limit |
| --- | --- | --- |
| Malicious page or DNS rebinding | IPv4 loopback, exact Host/Origin, fetch-metadata checks and no permissive CORS | A compromised browser or OS is outside this boundary |
| Cookie leakage to another local port | Independent application capability on every protected call, including session recovery; origin-scoped tab storage | Same-origin script or a privileged extension can read the local capability |
| Bootstrap replay or CSRF | Random single-use five-minute launch capability, HttpOnly SameSite cookie, separate app capability, CSRF and native expiry | A deliberately printed launch link is a temporary credential |
| Delayed body, queued work or stale account result | Native epoch captured before body extraction, reauthorization before dispatch/result release, cancellation and session-owned native clients | Cancellation cannot undo a request already accepted remotely |
| Cross-account or cross-resource disclosure | Native account identity, owned vehicles, fresh order/wallbox checks, fixed documents and response-ID checks where available | Rivian remains the authority; legacy state schema has no vehicle-ID echo |
| Arbitrary GraphQL or proxy abuse | Exact endpoint/name/document admission only resolves to a fixed local ID; execution rebuilds the admitted document | A newly added contract requires source review and bounded schemas |
| Credential or token disclosure | Native-only token bundle, no token-read route, transient password/OTP input, bounded sanitized errors and no secret logging | Transient input exists in browser/native request memory |
| Unintended remembered sign-in | Memory-only new sign-ins; explicit opt-in; only Linux Secret Service, macOS Keychain or Windows Credential Manager; no plaintext/mock fallback | Vault availability, unlock policy and at-rest protection depend on the OS |
| Competing process or late vault write | Secret-free exclusive per-user lease; serialized store work; epoch checks and cleanup for late writes; finite async waits | Blocking OS calls cannot be forcibly interrupted; deletion failure must remain visible |
| Unrequested stream data or key-material leakage | Fixed WSS destination, eleven fixed read RVMs and native response gate before decode/retention | Reviewed topic/schema compatibility remains unverified against a real vehicle |
| Runaway stream or stale reconnect | Bounded frames/payloads/state, handshake/ack/write/heartbeat/idle waits, capped retries and epoch cancellation | Network failures still reduce availability |
| Misleading freshness or diagnostics | Nullable physical fields, per-field source times, newer-field merge, tire validity and age handling, associated charging sessions | Data can be incomplete/stale; readings are not health diagnoses |
| Replayed or changed vehicle rename | Native one-use two-minute ticket binds exact operation, variables, local session and account epoch; ownership recheck; no mutation retry | Remote outcome can be unknown after transport failure; owner must inspect it |
| Physical control or key misuse | Physical commands, enrollment/BLE pairing and signing remain blocked; no arbitrary signing endpoint | No desktop enrollment or physical-command acceptance is claimed |
| Malformed/oversized upstream input | Fixed verified TLS destinations, no redirects/system HTTP proxy, streamed HTTP bound, WebSocket bounds and strict envelope admission | An upstream schema change may make an admitted operation fail |
| Response injection | React text rendering, bounded JSON, constrained source links and restrictive CSP | Successful developer reads intentionally expose private owner data locally |
| Authentication abuse or limiting | Bounded login/MFA attempts, challenge expiry, resend cooldown and limited read renewal | Rivian may impose additional controls; no bypass is attempted |
| Dependency/binary tampering | Locked dependencies, pinned CI actions, clean-revision rebuild, file/source/lock hashes and SBOMs; optional signature gates | Unsigned hashes do not authenticate a publisher; signing success needs real evidence |

## Account, storage and telemetry lifecycle

Account change, disconnect, local expiry and revocation invalidate native epochs
before awaiting queued work. Older HTTP reads, login/MFA replies, confirmations
and subscription credentials cannot acquire a newer account implicitly. Native
stream failures retain their originating epoch, so an older authentication failure
cannot sign out a newer account. UI account/vehicle generations and closed-poller
guards provide an additional presentation boundary.

The saved native bundle excludes passwords and MFA challenges. Restoration bounds
its shape, enforces a 30-day local retention ceiling and rechecks account identity
and current owned vehicles with Rivian. `--session-only` prevents any saved-entry
read/write, including deletion of an earlier entry. Production explicitly selects
the OS credential backend; test stores are confined to tests.

An exclusive lease protects the fixed entry throughout native-store lifetime. Its
lock file contains no credential. Vault I/O runs off the async executor behind a
serialized gate. A timed-out OS call may continue, so invalidation and same-gate
cleanup prevent late persistence from silently resurrecting revoked authority.
Disconnect/End session clear memory and attempt deletion. A deletion failure is
reported separately from local sign-out and remote logout. Close app/process exit
preserves earlier explicit opt-in storage.

Native telemetry connects only for an owned selected vehicle. Every incoming RVM
must be one of the eleven requested read topics before decoding or retention.
Unknown and known-but-unrequested payloads, including key material, are dropped
before either owner cards or developer snapshots. Portable unknown-topic decoding
is not network authorization. Stop, vehicle replacement and revocation cancel
both socket work and reconnect backoff, then clear native telemetry state.

## Confirmed write boundary

The sole executable mutation is `rename-vehicle`. Preparation validates the exact
name and vehicle, checks ownership under the captured account epoch and stores a
random native confirmation. The store admits at most eight pending tickets.
Execution accepts only the ticket ID, consumes it before network dispatch and
uses its stored operation/variables. Wrong-epoch, expired and replayed tickets fail
closed. Ordinary read execution refuses the mutation.

No automatic retry follows a write, including an authentication rejection. A
transport failure can leave the remote outcome unknown; the UI tells the owner
to inspect the vehicle name before preparing another change. This scope does not
authorize physical vehicle actions or enroll a signing key.

## October 14 verification and remediation

Independent-context same-provider reviews reproduced and corrected account epoch
races, MFA challenge expiry, stream source-age conflicts, unrequested key-topic
retention and misleading persistence/packaging status. Native authorization uses
the original request/ticket epoch through asynchronous work; CAS transitions and
post-lock checks prevent old continuations clearing or adopting a new account.
Authentication-failure telemetry metadata remains native and is omitted from
browser JSON. Delayed start/stop cannot cancel a newer account worker.

The final Linux verification passed 127 Rust, 44 UI and 14 delivery tests plus
compiled parity and both Chromium suites. These tests use synthetic contracts,
isolated memory stores and no real Rivian account. The advisory scan initially
found RUSTSEC-2026-0346/0347/0348/0349 in Rustls 0.23.45; the lock was updated to
[patched 0.23.46](https://github.com/rustls/rustls/releases/tag/v/0.23.46).
The repeated Cargo audit reported zero vulnerabilities and warnings; npm audit
reported zero vulnerabilities, without ignore exceptions. Database and lock
digests are retained in [the scan evidence](evidence/2026-10-14/advisory-scans.json).
Scans cover the recorded dependency snapshot, not unknown flaws or the OS/browser.

Independent Anthropic/provider review and re-review of frozen commits remains an
external gate. Same-provider agents and passing tests do not satisfy that gate.

## Historical review evidence

The earlier 2026-10-09 read-only milestone had a separate-agent, same-provider
adversarial review. Its recorded reviewer runs were 17 host security tests,
14 native API tests, 18 core tests and 28 UI tests. That review found and addressed
cookie-only session recovery, delayed-body/login revocation, stale UI results,
malformed GraphQL envelopes, account/vehicle mismatches, charging attribution,
per-field freshness, response bounds and package/source provenance.

Those results describe the earlier revision. They do not independently accept the
new vault, telemetry, expanded catalog, confirmed write or native installers.
Current same-provider reviews and actual verification outputs must be recorded
against the current source in [status](STATUS.md). No high/critical-clearance claim
is inherited from the smaller milestone.

The earlier milestone also recorded `npm audit` with zero reported advisories and
`cargo-audit 0.22.2` with zero vulnerabilities/no warnings against 1,296 RustSec
advisories at revision `7eebec69c352c7191b1f13eb95dd510eeca5d1de`, updated
2026-10-09. New dependencies require a new scan. Historical advisory results are
not current dependency clearance or certification.

## Regression requirements

1. Reject wrong/ambiguous Host, Origin, fetch metadata, capability, cookie and
   CSRF; retain cookie-only session recovery and bootstrap replay/expiry races.
2. Revoke during delayed bodies, queued reads, login, OTP/resend, remembered restore
   and store writes; older work must neither dispatch under nor erase newer authority.
3. Check account pinning, vehicle/order/wallbox ownership and contradictory
   response identifiers independently of browser/WASM validation.
4. Verify unavailable/locked stores, explicit opt-in, lease contention, bounded
   store I/O and visible delete failures without plaintext or mock fallback.
5. Exercise native stream handshake/ack/heartbeat/idle bounds, capped retries,
   stop/replacement/revocation and unrequested-RVM drop before retention.
6. Preserve wire presence, encoded zero/false, partial fields, per-field source
   ages, stale messages, tire validity and charging-state power suppression.
7. Reject arbitrary document triples and variables; preparation/confirmation must
   bind exact changes, expire, consume once and never automatically retry writes.
8. Retain JSON/HTTP/WASM/stream limits, safe React rendering, strict GraphQL errors,
   secret redaction and redirect refusal.
9. Package only clean, rebuilt source; verify installed payload hashes, source/lock
   identity, SBOMs, update/rollback isolation and fail-closed signing gates.

## External gates

Real owner-account/firmware acceptance, legitimate key enrollment, actual platform
signing/notarization, clean-machine OS lifecycle acceptance and the independent
Anthropic adversarial review remain outstanding. They require the actual account,
platforms, credentials or independent reviewer, with results and any remediation
recorded before release. Phase II Tauri/mobile implementation and device/store
security work are also pending.

See [architecture](ARCHITECTURE.md), [release](RELEASE.md),
[acceptance](ACCEPTANCE.md) and [status](STATUS.md).
