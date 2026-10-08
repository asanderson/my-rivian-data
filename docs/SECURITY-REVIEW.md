# Phase 1 prototype security review

Review date: 2026-10-08. Scope: the offline Rust/Axum host, portable Rust
validation core, Rust/WASM wrapper, and React session and request interface.

This is a separate-agent, same-provider code and design review. It is not an
Anthropic review, an external penetration test, a security certification, or
evidence of live Rivian compatibility. The requested independent-provider
adversarial review remains pending. The application is an offline prototype;
this review does not approve adding credentials or vehicle command authority.

## Trust boundary and conclusions

The protected boundary is a hostile web page attempting to reach a service on
the user's loopback interface. The application assumes a trusted operating
system, browser, installed browser extensions, same-user processes, and other
services on the same loopback host. The latter assumption matters because
cookies are scoped to a host, not to a port.

No critical or high-severity issue was identified in the reviewed offline
implementation. The delayed-body lifecycle finding was fixed and its regression
test passed in a separate reviewer-run execution. This is a bounded source-review
and test conclusion, not a guarantee that vulnerabilities are absent. The
prototype is suitable for delivery as an offline, credential-free demonstration;
live account access and command execution remain outside this conclusion.

## Findings and decisions

| ID | Severity in this prototype | Finding | Remediation and status | Retest |
| --- | --- | --- | --- | --- |
| SEC-01 | Low; more significant with real credentials | A session cookie for `127.0.0.1` can be sent to another service on that host regardless of port. A random cookie name does not provide port isolation. | Accepted only for the offline prototype's explicitly trusted same-user environment. Before credential support, reevaluate an in-memory per-instance capability required by every protected request, including session recovery, or another origin-isolation design. | Source inspection confirms the cookie is host-scoped and has no `Domain` attribute. Cross-port exposure is a documented browser-cookie property, not a fixed issue. |
| SEC-02 | Low for synthetic read-only data; resolved | Initial middleware authorization occurred before JSON body extraction. A request admitted before logout or expiry could finish uploading and execute afterward. | Both validation and execution handlers now recheck the session and CSRF token after complete body extraction. Future external command submission also needs authorization at the actual dispatch boundary. | Reviewer independently ran the host security suite: the delayed-body regression admits a request, blocks its body, logs out, then releases the body and receives HTTP 401. Passed. Separate zero-lifetime tests verify idle and absolute expiration rejection. |
| SEC-03 | Informational | `--print-launch-url` deliberately prints a bearer bootstrap secret to the terminal. Terminal capture or copied logs can expose it during its validity window. | Explicit opt-in only; ordinary startup omits the secret. The link is single-use and expires after five minutes. Keep the help text and operator instructions explicit; do not include such output in support bundles. | Source inspection confirms ordinary logs contain only the nonsensitive origin and that URL printing requires this flag. |

## Controls verified by source inspection

- The listener binds to IPv4 loopback. Its default port is selected by the OS.
  No CLI option exposes a non-loopback bind address.
- Middleware requires one exact `Host` value. Foreign or ambiguous `Origin`
  headers are rejected, unsafe HTTP methods require the exact origin, and
  cross-site or same-site-but-cross-origin fetch metadata is rejected.
- Only health and bootstrap are exempt from API session authentication. Unsafe
  authenticated requests also require a separate CSRF token. The prototype has
  no permissive CORS middleware.
- A 256-bit random bootstrap secret has a five-minute lifetime. A mutex protects
  both its successful single-use consumption and session creation. Session and
  CSRF tokens use separate random values; secret comparisons are constant-time
  for values of equal length.
- Session cookies are `HttpOnly`, `SameSite=Strict`, host-only, and session-only.
  Server-side limits are one hour idle and eight hours absolute. Logout revokes
  the server session and expires its cookie. Plain HTTP is intentional for this
  loopback-only prototype; this cookie is not described as a secure HTTPS cookie.
- All host responses, including middleware errors, receive `Cache-Control:
  no-store`, content-type sniffing prevention, a restrictive CSP, referrer
  suppression, and cross-origin isolation-related response policies.
- The interface removes the launch fragment on startup, keeps CSRF state in
  memory, has no credential input, and does not use browser persistent storage.
  Browser fragment removal is not a guarantee against extensions or previously
  captured launch URLs, which are outside the stated trust boundary.
- The API body limit is 32 KiB. Core variables have a 16 KiB serialized limit,
  depth limit of eight, and node-count limit of 256. Operation schemas reject
  unknown fields and unauthorized vehicle identifiers.
- Native execution validates independently of WASM. Only explicitly allowlisted
  demo queries execute. Mutations and subscriptions cannot run even when called
  directly without the UI. Responses and timestamps identify synthetic fixtures.
- WASM exports only non-secret catalog and validation functions. The core and
  WASM wrapper do not implement network, credential, token, key, or filesystem
  access. The host has no generic proxy, arbitrary signing, shell, or filesystem
  API.
- React renders variable-validation messages and JSON responses as text. Source
  links are constrained separately and open with `noopener`/`noreferrer`.

## Verification evidence

The reviewer independently ran `cargo test -p rivian-host --test security
--locked` on Linux using the project's scoped Rust toolchain. Result: **11
passed, 0 failed**. The executed tests cover authentication and error headers,
single-use concurrent bootstrap, invalid and expired bootstrap, idle and absolute
session expiration, Host/Origin/fetch-metadata checks, CSRF requirements,
logout revocation, blocked/unknown/unauthorized operations, oversized bodies,
duplicate cookies, static-method restrictions, traversal rejection, and
authorization after delayed body extraction.

The traversal test passed in this debug build for `..`, repeated parent segments,
percent-encoded parent segments, encoded backslashes, and dotfiles. Source review
confirms the rejection occurs before asset lookup and is unconditional in debug
and release builds. This reviewer did not separately run that test in the release
profile.

The implementation agent additionally reported passing formatting, clippy,
release build, all 22 Rust tests, 10 UI tests, 22 actual compiled-WASM parity
fixtures, catalog equality and the configured WASM memory limit. The UI agent
reported successful Chromium desktop/mobile-width smoke tests without fetch/CSP
errors. Those broader results are implementation-team evidence; they were not
independently rerun by this reviewer. Consult `STATUS.md` for the final recorded
build and browser evidence.

## Regression coverage to retain and extend

Retain the following threat-driven checklist as the application evolves. The
observed suite above covers its principal offline boundaries; this list also
includes extensions, such as duplicate protocol headers and real-browser
network observation, and does not assert every permutation has been tested:

1. Reject wrong, duplicate, suffix-confusable, and alternate-port Host values;
   reject foreign/null/missing unsafe-request Origin and cross-site metadata.
2. Reject replayed, expired, invalid, and concurrently reused bootstrap tokens.
3. Reject missing, duplicate, incorrect, logged-out, idle-expired, and
   absolute-expired sessions; reject missing or incorrect CSRF on unsafe methods.
4. Recheck authorization after delayed body extraction and before execution.
5. Reject oversized or malformed bodies, unknown operation IDs, extra fields,
   unauthorized vehicle IDs, deep/large variables, and every blocked command.
6. Confirm restrictive headers on successful, error, and missing routes. Probe
   traversal paths and encoded traversal; no file outside bundled assets should
   be returned in debug or release builds.
7. Execute the same sanitized fixtures in compiled WASM and native Rust and
   compare results. A native build of the wrapper alone is not a WASM parity test.
8. Observe a browser session through startup, validation, execution and logout,
   recording whether any automatic requests leave loopback. External community
   reference links require an explicit user click and are not background traffic.

## Required before live account access

Revisit the cookie trust boundary; implement and review native secret storage;
keep long-lived tokens and signing keys out of JavaScript/WASM; implement refresh,
MFA and revocation behavior; and test account/vehicle authorization independently
of UI validation. Add outbound destination controls, timeouts, rate limiting,
retry limits, stale-data indicators, sensitive-data redaction and fixture hygiene.
Command enablement requires fresh native authorization, replay protection,
explicit user intent, and the independent-provider adversarial review with
finding remediation and re-review.

Platform packaging, signed updates, dependency-advisory scanning, macOS/Windows
behavior, real-device testing, and live Rivian API behavior are separate release
gates. This review makes no claim that those gates have passed.
