# Architecture and design

[Owner guide](OWNER-GUIDE.md) · [Developer setup and API explorer](DEVELOPER-GUIDE.md)

Phase 1 is a standalone native Rust/Axum service with an embedded React interface
and shared Rust/WASM validation. In its default live mode, native Rust signs in to
the unofficial Rivian API and performs admitted read operations. `--demo` selects
credential-free synthetic data through the same presentation and validation
boundaries. These modes cannot be switched by an API request.

## System context

The owner runs one executable and uses an existing browser. Only the native
transport contacts Rivian. No project-operated cloud backend, database, browser
extension or external map service is required.

![System context: owner browser, local native app and outbound Rivian API](diagrams/system-context.png)

## Source boundaries

| Component | Responsibility |
| --- | --- |
| `rivian-core` | Portable operation definitions, bounded validation, live response normalization and deterministic demo fixtures; no network I/O or credentials |
| `rivian-wasm` | Non-secret bindings around shared input validation; its decision is advisory |
| `rivian-host` | IPv4 loopback listener, embedded UI, launcher, local session enforcement, account-owned vehicle authorization and native request dispatch |
| `rivian-api` | Fixed HTTPS destination, sign-in/MFA state, native memory-only session tokens, bounded replies and sanitized errors |
| `ui` | Owner views, separate API explorer, local sign-in form, shared validation comparison and bounded response rendering |
| `fixtures` | Synthetic conformance inputs used by native and compiled-WASM tests |

The executable embeds `ui/dist`. Node, Vite, Cargo and wasm-bindgen are build tools,
not runtime subprocesses. Later Tauri applications can reuse the portable core,
transport and React components with a native invocation adapter; no Axum server is
intended to run on Android or iOS.

## Presentation and data mapping

| Owner view | Local operation or route | Presentation boundary |
| --- | --- | --- |
| Account sign-in | `/api/account/login`, `/api/account/otp` | Credentials and verification codes are transient input; upstream tokens are never returned |
| Vehicle selection | `/api/vehicles` | Vehicles obtained from the signed-in account; native authorization retains the allowed IDs |
| Overview | `vehicle-state` | Available battery, range, charging and lock readings |
| Charging | `charging-status`, `charging-history` | Reported status and available session history; no setting changes |
| Vehicle health | `vehicle-state` | Available mileage, temperature and lock status; no invented diagnostics |
| Location | `vehicle-location` | Reported coordinates and accuracy; no external map or tracking |
| API explorer | `/api/catalog`, `/api/validate`, `/api/execute` | Admitted operations and request/response details; no arbitrary URL or GraphQL submission |

The API explorer uses local adapter IDs such as `charging-history`. Exact upstream
operation names, source documents and support status are documented separately in
[API coverage](API-COVERAGE.md). It is a Swagger-like interface, not a generated
OpenAPI client or a complete Rivian schema.

Owner normalization preserves missing fields as unavailable. Receipt time must
not silently replace a vehicle's observation time. Authentication errors and
unknown source envelopes stop rendering instead of producing demo fallbacks.
View/vehicle changes invalidate older responses; ending a session removes the
active data views. Changing presentations never changes backend privileges.

## Local HTTP contract

Every request must use the exact `127.0.0.1:PORT` Host. A supplied Origin must match
the exact application origin; unsafe requests require it. Cross-site and same-site
cross-origin fetch metadata are rejected. There is no permissive CORS policy.

After bootstrap, **every protected request**, including session recovery, requires
both the local HttpOnly cookie and `x-app-capability`. Unsafe requests also require
the separate CSRF value. A host-scoped cookie alone cannot recover or use a session.

| Route | Method | Result |
| --- | --- | --- |
| `/api/health` | GET | Nonsensitive build/mode information; no account access |
| `/api/bootstrap` | POST | Consume one-use launcher capability and issue local session material |
| `/api/session` | GET | Recover local CSRF/session state only with cookie and application capability |
| `/api/account` | GET | Sanitized account authentication state |
| `/api/account/login` | POST | Start native live sign-in; rejected in demo mode |
| `/api/account/otp` | POST | Complete pending verification without exposing upstream tokens |
| `/api/account/logout` | POST | Discard the current native account session |
| `/api/catalog` | GET | Admitted operation definitions and support metadata |
| `/api/vehicles` | GET | Account vehicles in live mode; synthetic vehicles in demo mode |
| `/api/validate` | POST | Bounded native validation |
| `/api/execute` | POST | Reauthorize, revalidate and execute an admitted read |
| `/api/logout` | POST | Revoke the local session, discard account state and expire its cookie |

Only health and bootstrap are exempt from local session requirements. There are
no shell, arbitrary filesystem, generic proxy, signing or token-read routes.

## Session sequence

![Launch, sign in, validate a read, authorize the selected vehicle and query Rivian](diagrams/data-flow.png)

The launcher creates a random 256-bit capability with a five-minute lifetime and
opens a browser URL containing it in a fragment. The UI scrubs the fragment before
rendering and exchanges it in a same-origin POST. Bootstrap is consumed atomically.
The returned local session cookie is HttpOnly, host-only and SameSite=Strict.

A separate random application capability is stored in the tab's origin-scoped
`sessionStorage` and sent explicitly on every protected request. Cookies have no
port isolation, but this capability is not automatically sent to another local
port. `/api/session` requires it as well; possession of a cookie alone does not
reissue it. CSRF state stays in memory. Local logout revokes server state and clears
the tab capability. Session restore is a browser behavior, not a secure erasure
guarantee; the user should end the session explicitly.

Local sessions have one-hour idle and eight-hour absolute limits. POST handlers
recheck authorization after body extraction. Account transport and vehicle
ownership are tied to the local session so stale requests cannot acquire a new
account implicitly. Backend execution remains authoritative even if browser/WASM
validation is bypassed.

## Native account and outbound boundary

The password and verification code are sent transiently from the local form to
native Rust. Native Rust makes fixed-destination TLS requests to Rivian. Passwords
are not saved; Rivian tokens remain in native process memory and are excluded from
all browser responses, fixture files and diagnostic logs. There is no persistence
or OS vault adapter in this release. Remembered sessions require a separate,
reviewed secure-storage implementation.

Authentication implements the community-documented CSRF/app-session exchange,
password login and OTP flow. Session renewal follows observed upstream behavior;
the presence of a field named `refreshToken` does not justify inventing a token
exchange. If a user session is rejected, the owner must sign in again. See
[coverage and provenance](API-COVERAGE.md) for the exact evidence and limitations.

The client admits only fixed operation documents and destinations. Vehicle reads
must target an identifier returned for the authenticated account, independently
of WASM checks. Redirects and transport errors must not expose a token, password,
upstream body or arbitrary URL. Read responses are size-bounded. Authentication
and failed operations are not blindly retried.

`--demo` performs no external requests and accepts no account credentials. Failed
live reads never fall back to synthetic data.

## Bounds and defense in depth

- HTTP JSON request bodies: 32 KiB. Variables: 16 KiB, depth 8, at most 256 values.
- WASM linear memory: maximum 64 MiB; this does not limit browser process memory.
- Upstream reply: at most 1 MiB. Browser adapter envelope: at most 3 MiB; request cancellation has a finite timeout.
- Exact operation admission, explicit variable schemas and native vehicle ownership checks.
- Restrictive CSP with bundled code and narrow WASM evaluation; no remote scripts,
  unsafe HTML rendering or general JavaScript `unsafe-eval`.
- `no-store`, `nosniff`, no-referrer, frame prevention and cross-origin isolation headers.
- Static traversal, encoded path aliases and dotfiles rejected before asset lookup.
- No active signed vehicle commands, enrollment or WebSocket subscriptions.

These controls assume a trusted OS and browser. A compromised same-origin script,
extension with page access, debugger or OS account can steal transient inputs and
local capabilities. Memory-only storage is not protection against process memory,
swap or crash-dump inspection. See the [security review](SECURITY-REVIEW.md).

## Delivery and later phases

The build pipeline bundles Rust/WASM, React and the native host into an OS-specific
executable. Packages include owner documentation, license notices, SHA-256
checksums and corresponding source. The checksums detect a changed archive; they
are not a digital signature. Three-OS CI and local mock tests do not establish
real-account compatibility or a supported desktop OS baseline.

Remaining release work includes owner-run live acceptance, clean-machine startup
on each target, measured resource budgets, signed installers/updates, independent
provider adversarial review and remediation. Extended API coverage needs reviewed
operation documents and tests. Signed commands, enrollment, subscriptions and
background collection are separate features. Phase 2 adds Tauri mobile hosts,
secure storage, app lifecycle handling and device/store validation.

## References and licensing

The executable API contracts are traced in [API coverage](API-COVERAGE.md).
Framework references: [Axum](https://docs.rs/axum/latest/axum/),
[wasm-bindgen](https://wasm-bindgen.github.io/wasm-bindgen/),
[Vite](https://vite.dev/guide/) and
[rust-embed](https://docs.rs/rust-embed/latest/rust_embed/).
Original project code is GPL-3.0-only; dependencies retain their own licenses and
[notices](THIRD-PARTY-NOTICES.md).
