# Implementation status

Application: My Rivian Data. Updated: 2026-10-14.
Milestone: Phase I desktop implementation, preserving Rust/Axum + React/Rust-WASM.

Implemented source, automated synthetic evidence and external release acceptance
are separate. Catalog `live` means a fixed contract is admitted by the code; it
does not mean a real account or firmware has accepted it. The remaining external
gates below are not silently treated as complete.

## Implemented scope

- Default live sign-in with Rivian email/password, bounded OTP verification,
  ten-minute MFA expiry and rate-limited resend. Passwords and MFA challenges are
  never persisted, and no password replay or invented refresh-token grant is used.
- Memory-only new sign-ins with explicit remembered-session opt-in through Linux
  Secret Service, macOS Keychain or Windows Credential Manager. Fixed-entry lease,
  bounded serialized I/O, epoch-aware late-write cleanup, identity/ownership
  revalidation on restore, visible deletion failure and `--session-only` isolation.
  No plaintext or production mock-store fallback.
- Fixed-destination HTTPS transport with verified TLS, no redirects/system HTTP
  proxy, finite timeouts, bounded replies, strict envelopes and sanitized errors.
  Read renewal is limited; writes are never automatically retried.
- Versioned 234-surface known-source inventory and fixed 244-row browser catalog,
  including implemented, documented and blocked entries with provenance and
  verification metadata. There are 33 fixed reads, one explicitly confirmed
  vehicle rename and a separately controlled native stream. See [API coverage](API-COVERAGE.md).
- Native account/resource authorization, including current owned vehicles and
  fresh order/wallbox checks. Exact-document import resolves only to existing
  fixed contracts; it cannot create a proxy or grant execution.
- Native selected-vehicle WebSocket lifecycle with eleven reviewed read RVMs,
  bounded handshake/ack/heartbeat/idle waits and reconnects, epoch cancellation,
  vehicle replacement and unrequested-topic rejection before decode/retention.
- Portable bounded Parallax decoder and timestamp-aware partial merge. Missing
  physical tags remain missing, encoded zero/false is retained, source age stays
  per-field, tire pressure follows valid measurement age and stopped charging
  cannot retain apparently active power.
- Owner overview, charging, vehicle health and location; refresh and live-update
  controls; separate developer explorer with reviewed contracts and bounded JSON.
  The sole write reviews exact rename variables and consumes a native one-use
  two-minute ticket bound to the account epoch.
- Explicit offline `--demo` with fictional vehicles, fixed times, no credential
  entry, remembered-store access or Rivian traffic. Live failures never become
  sample readings.
- One-use local bootstrap, cookie plus per-instance capability on every protected
  route, CSRF and Host/Origin checks, post-body reauthorization, native epoch capture,
  cancellation, guarded assets and restrictive response headers.
- Desktop portable archives, Windows per-user installer, Intel/Apple Silicon
  macOS app/DMG and Linux DEB/RPM packaging paths. Clean-revision rebuilds, complete
  file/source/lock hashes, both SBOMs, local update/rollback tools, signed-candidate
  gates, package checks and owner acceptance helpers. Ordinary CI artifacts are
  unsigned; workflows do not publish a release.
- Owner-first documentation and four embedded standalone PNG diagrams with their
  editable generation sources. Screenshots were refreshed from the October 14 automated sample/startup runs; they contain fictional data or the signed-out form.

## Verification evidence

Linux checks use the pinned Rust 1.99.0 toolchain, Node 24 and matching locked
wasm-bindgen CLI. Synthetic server fixtures and browser mocks do not contact an
owner's vehicle or establish real Rivian acceptance. Current verification results
are recorded after the final implementation run; no cross-platform result is
inferred from this host.

The final patched Linux run of `node scripts/verify.mjs` passed: 127 Rust tests
(60 native API, 37 core, 2 host confirmation, 21 HTTP security, 7 WASM),
44 UI tests, 14 delivery/measurement/release tests, strict clippy, formatting,
TypeScript and the embedded release build. Compiled native/WASM parity covered
22 demo, 46 live-validation and 25 telemetry/error fixtures, both inventories
(234 source entries / 244 catalog rows), and the enforced 64 MiB memory ceiling.

Both Chromium smoke suites passed on the native release executable. Demo checks
cover owner/developer views, vehicle switching, stale replies, reload/logout,
CSP and desktop/mobile-width layout. The live-mode suite uses fake local replies
for remembered sign-in/MFA resend, nullable readings, stream zero/age/vehicle
guards, exact document admission, confirmed rename and account/session lifecycle;
Close app also stopped the real native process. No real Rivian login was used.

Dependency scans reported zero Rust vulnerabilities/warnings and zero npm
vulnerabilities after updating Rustls 0.23.45 to 0.23.46 for four advisories
published on October 14. No ignore exception was used. See the exact
[advisory snapshot](evidence/2026-10-14/advisory-scans.json).

The earlier untouched read-only baseline passed 51 Rust tests, 28 UI tests and
37 native/WASM parity fixtures. Its historical package/advisory measurements are
not reused as acceptance for newly added dependencies, telemetry, storage, writes
or installers.

## Resource measurement scope

The implemented benchmark runner measures a fixed synthetic vehicle-state
workload in explicit demo mode. It records startup, bounded latency observations,
request counts, native backend RSS/CPU and artifact provenance when matched to a
package. It excludes browser/driver resources and battery/power. Raw reports and
summaries are produced under `artifacts/benchmarks/`.

An earlier ten-second Linux run preceded the final Phase I rebuild and lacked a
matched source/executable digest record. Its measurements do not establish the
current build's performance. No multi-day throughput, full browser memory budget,
battery impact or cross-platform performance is claimed. See [measurement and
acceptance guidance](ACCEPTANCE.md).

## External release acceptance

| Gate | Current status |
| --- | --- |
| Real Rivian sign-in, OTP/resend, reads, remembered restore and telemetry | Unverified with an owner's account and vehicle; requires owner-run results |
| Vehicle/model/region/firmware and account-role matrix | Unverified; source or fixture review does not establish compatibility |
| Legitimate desktop key enrollment/BLE pairing and physical commands | Not implemented/accepted; enrollment-dependent controls remain explicitly blocked |
| Actual Windows/macOS signing, Apple notarization and signed release manifest | Workflows implemented; no successful execution with authorized signing credentials recorded |
| Clean-machine install/launch/update/rollback/uninstall | Platform checks implemented; actual supported Windows/macOS/Linux acceptance remains open |
| Independent Anthropic adversarial review and remediation/re-review | Outstanding; same-provider agents do not satisfy it |
| Durable signed public release and owner release decision | Not performed; workflows do not publish |
| Full runtime, battery and sustained multi-day throughput evidence | Not established by a short native synthetic workload |

Real-account acceptance must keep credentials and codes in the local app. The
interactive helper saves only sanitized, owner-approved model/firmware and
operation outcomes. A `blocked` or `not-tested` result is not a passing test.
The full process is in [acceptance](ACCEPTANCE.md); delivery details are in
[release](RELEASE.md).

## Phase II and other future work

Android/iOS Tauri hosts, mobile lifecycle, device tests and app-store packaging
are not implemented. Mobile will invoke the reused native Rust core, not host an
Axum server. Persistent vehicle history, collection/retention controls, exports
and a remote auto-updater are not implemented.

The local app assumes a trusted OS/browser. Memory-only and protected storage do
not defeat a compromised OS, extension, debugger, crash dump or same-origin script.
A remote change or logout can remain unconfirmed when the network fails; local
revocation, vault deletion status and remote outcome are reported separately.
See [security boundary and review](SECURITY-REVIEW.md).
