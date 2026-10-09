# Implementation status

Application: My Rivian Data. Date: 2026-10-09.
Milestone: Phase 1 native live account reads, with explicit offline demo mode.

## Implemented

- Default live sign-in with Rivian email/password, pending verification and OTP.
  Passwords are not saved; native account tokens stay in process memory.
- Fixed-destination HTTPS transport with TLS verification, no redirects or automatic
  system proxy, finite timeouts, bounded replies, sanitized errors and rate limits.
- Application-session rotation and one bounded read retry on an authentication
  failure. Invalid user sessions require a new sign-in; no unverified refresh-token
  exchange or stored-password re-login is used.
- Eighteen admitted read operations across account, vehicle telemetry, charging,
  schedules, wall chargers, software metadata, image metadata, drivers and orders.
  See [coverage and provenance](API-COVERAGE.md) for exact mappings and gaps.
- Native account/vehicle authorization, including strict vehicle association for
  charging history and rejection of contradictory response identifiers.
- Owner overview, charging, vehicle health readings and location, with missing
  values retained as unavailable and request time distinct from observation time.
- Separate developer API explorer with admitted operations, variables, local
  requests, upstream operation details and response JSON.
- Explicit `--demo` mode with fictional vehicles, fixed timestamps, no Rivian
  traffic and no credential entry. Live failures never substitute sample data.
- Single-use bootstrap, local session cookie plus per-instance capability on every
  protected request, CSRF, Host/Origin enforcement, bounded input and output,
  revocation/cancellation, guarded static assets and restrictive response headers.
- Owner-first documentation, live sign-in screenshot, standalone PNG architecture,
  data-flow, build and installation diagrams with editable sources.
- Portable setup/build/verification and unsigned packaging with documentation,
  notices, checksums and corresponding source; three-OS CI configuration.

## Verification evidence

Linux implementation checks use Rust 1.99.0, Node 24 and the matching locked
wasm-bindgen CLI. Deterministic API tests use synthetic server replies and do not
require credentials, contact an owner's vehicle or establish upstream acceptance.

| Check | Recorded evidence |
| --- | --- |
| Release/quality checks | `scripts/verify.mjs` passed on the final source: formatting, release build, clippy, all 51 Rust tests and the UI/parity gates |
| Rust advisory scan | `cargo-audit 0.22.2`: 0 vulnerabilities, no warnings; 1,296 RustSec advisories at database revision `7eebec69c352c7191b1f13eb95dd510eeca5d1de` |
| Packaged Linux executable | Built from clean committed source, extracted outside the repository and launched successfully; embedded HTML/WASM, 18-read catalog, live signed-out status, binary checksum and corresponding source verified |
| npm advisory scan | `npm audit`: 0 reported advisories across 70 packages, including development/optional dependencies |
| Rust core | 18 tests passed, including bounded live schemas, ownership, normalization and missing fields |
| WASM wrapper | 2 tests passed |
| Native host security | 17 tests passed, including cookie-only recovery rejection, capability enforcement and logout lifecycle |
| Native API transport | 14 tests passed, independently rerun with core and host security suites |
| UI unit tests | 28 passed, including live envelopes, null readings, timestamp distinctions and session material handling |
| Actual compiled-WASM parity | 22 demo fixtures plus 15 live fixtures and both catalogs match native results; 64 MiB memory ceiling checked |
| Chromium demo smoke | Passed owner views, vehicle switching, stale-response suppression, errors/retry, developer response display and logout |
| Chromium live smoke | Passed real native sign-in page, synthetic login/MFA replies, live owner presentation, developer details, reload and disconnect/logout boundaries |
| Browser screenshot | Blank live sign-in captured from the real native app; no credentials or bootstrap link shown |
| Documentation diagrams | Four PNGs regenerated and visually inspected |

The RustSec database was last updated on 2026-10-09 at 10:12:02 +02:00.
Advisory scans report known database matches at that snapshot; they are not a
certification or proof that every dependency is free of vulnerabilities.

The browser suites exercise a real locally built native host. Their Rivian
responses are deliberately synthetic; they are not live-account tests. The
Playwright browser download failed in this environment, so tests used an external
test-only Chromium 153.0.8010.0 installation via
`PLAYWRIGHT_CHROMIUM_EXECUTABLE`. The product does not bundle that test browser.

The preceding main revision `9bd07945e2362f7c7eadc45e58da8f0bc7985431` passed
[Linux, Windows and macOS CI](https://github.com/asanderson/my-rivian-data/actions/runs/37820327124).
That is baseline evidence, not proof the live implementation has passed the same
jobs. Current PR checks report its own cross-platform results. Formatting, clippy,
release build and final test totals should be checked against that PR's results.

A no-credential attempt to reach Rivian's CSRF service from the implementation
host failed at the network layer. No account password or Rivian token was used or
logged. **Actual Rivian connectivity and owner-account acceptance are unverified.**

## Remaining release work and limitations

- Owner-run sign-in/MFA and read acceptance against a real account, with credentials
  entered only into the local app. Verify per-vehicle generation, region and account role.
- Signed vehicle controls, key enrollment/BLE pairing, command signing,
  WebSocket/Parallax streaming and complete unofficial API coverage.
- Remembered sessions, OS credential vaults, persistent history, collection,
  retention settings and export. Current reads remain session-only snapshots.
- Clean-machine Windows/macOS/Linux startup, supported OS baselines, signed
  consumer installers, macOS notarization, signed updates and a durable release channel.
- Independent-provider adversarial review and remediation/re-review. The current
  review uses a separate agent from the implementation provider; it is not the
  requested independent Anthropic review.
- Phase 2 Tauri Android/iOS hosts, platform secure storage, lifecycle handling,
  device testing and application-store packaging.
- CPU, RAM, battery and full runtime-resource benchmarks. See the artifact-only
  measurements below; they do not establish runtime memory budgets.
- Sustained 24x7 agent execution and a measured multi-day subscription throughput
  pilot. No such runner or validated token-consumption budget is claimed.


## Artifact measurements

Latest Linux x86_64 release measurements from this implementation:

| Artifact | Bytes | Interpretation |
| --- | --- | --- |
| Linux portable archive | 6,487,341 | Measured package with executable, documentation, notices and corresponding source; archive size changes with source/documentation metadata |
| Native executable | 4,865,744 | Includes the built browser interface and WASM |
| Compiled WASM | 101,711 | Subset of the embedded interface, not additional runtime installation space |
| UI asset files including WASM | 691,654 | Already embedded in the native executable |

Packages additionally contain documentation, license notices and source. Build
caches are not runtime installation requirements. Reliable native/browser RSS,
CPU and battery measurements were unavailable in this environment, so no such
measurements are asserted. The build host does not establish minimum supported
Linux distribution or desktop hardware requirements.

## Decisions and threat-driven changes

- Cookie host scope is not treated as origin isolation. Every protected endpoint,
  including session recovery, also requires a random per-instance capability in
  origin-scoped tab storage. Rivian tokens are never stored there.
- Revocation cancels in-flight native account work; requests are bound to their
  original local/account lifecycle. UI epochs prevent old login or garage results
  from restoring a disconnected account.
- Malformed GraphQL errors, mismatched identifiers and incomplete readings fail
  safely. Request receipt is not relabeled as fresh vehicle observation.
- Unknown charging sessions are not assigned by guess, even to a single vehicle.
  The account-wide developer read preserves access to those sessions.
- Four passenger-door locks are not described as comprehensive vehicle security.
  Battery charge is not battery health; missing fields do not imply zero or false.
- Memory-only secret storage reduces persistence. It does not defeat a compromised
  OS, browser extension, debugger, crash dump or same-origin script.
- Ordinary release optimization remains enabled with LTO disabled. The preceding
  environment's thin-LTO failure is not a reason to skip the ordinary release build.

See [security review](SECURITY-REVIEW.md) for the review scope, findings and limits.
