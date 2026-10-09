# Phase 1 security review

Review date: 2026-10-09. Scope: the native Rust/Axum local service, native Rivian
transport, fixed read-operation catalog, Rust/WASM validation, React sign-in and
owner/developer views, and unsigned package provenance.

This is a separate-agent, same-provider adversarial source review with independent
reviewer-run regression checks. It is **not** the requested independent Anthropic
review, an external penetration test, a certification or proof of live Rivian
compatibility. No real credentials, vehicles or mobile devices were used.

## Conclusion and trust boundary

The review identified lifecycle, capability, response validation and presentation
issues; the implementation was changed and targeted tests were rerun. No remaining
high/critical source-review blocker was identified within the fixed, read-only
scope. This is a bounded finding, not a guarantee that vulnerabilities are absent.
Independent-provider review, real-account acceptance and consumer-release checks
remain outstanding.

The app protects a loopback service from unrelated web pages and rejects cookie-only
replay from another local port. It assumes a trusted OS, browser and browser
extensions. Same-origin script execution, malicious extensions, process memory
access, debuggers, crash dumps and OS compromise can defeat that assumption.
Memory-only tokens reduce deliberate persistence; they are not a guarantee that
no copy reaches swap, diagnostic memory or an HTTP library allocation.

## Threat vectors and controls

| Threat | Implemented boundary | Residual risk / limit |
| --- | --- | --- |
| Malicious web page or DNS rebinding | IPv4 loopback binding, exact Host and Origin, fetch-metadata checks, no permissive CORS | Does not protect a compromised browser or OS |
| Cookie leakage to another loopback port | Separate random app capability required on every protected call, including `/api/session`; stored in origin-scoped tab storage | Same-origin XSS or a privileged extension can read that local capability |
| CSRF or capability replay | One-use expiring bootstrap, HttpOnly SameSite cookie, independent capability, CSRF on unsafe calls and server expiry | Private launch links are temporary credentials; intentional printing can disclose them |
| Logout/expiry race or stale account work | Authorization after body extraction, account lifecycle/epoch checks, revocation cancellation and session-owned native client | Network cancellation cannot undo a request already received remotely; only admitted reads run |
| Cross-account or cross-vehicle disclosure | Natively established account identity and owned-vehicle set, fixed documents, returned-ID checks when schema supports them | Rivian remains the data authority; legacy state query has no ID echo |
| Credential/token leakage | Native-only Rivian tokens, no persistence or token endpoints, sanitized errors, no secret logging | Transient password input is present in the browser form and native request memory |
| Malformed or oversized upstream data | Verified TLS, fixed destinations, no redirects/system proxy, streamed 1 MiB limit, finite timeouts, strict GraphQL envelope checks | An upstream schema change can make a supported read fail |
| Injection through developer response | JSON rendered as React text, no HTML execution, constrained source links, CSP | Successful raw reads intentionally expose owner data; users must redact before sharing |
| Misleading readings or wrong charging attribution | Nullable missing values, per-field observation times, strict vehicle-ID association, unknown sessions withheld from vehicle history | Data may be incomplete or stale; the app does not diagnose vehicle health |
| Authentication abuse or upstream limiting | Bounded attempts, MFA expiry, cooldowns, one limited application-session retry | Rivian can impose additional controls; no bypass is attempted |
| Dependency or binary tampering | Locked dependencies, pinned CI actions, checksums, included notices/source, package rebuild from clean revision | Artifacts are unsigned; hashes alone do not authenticate the publisher |
| Vehicle commands or key misuse | Read-only admission, blocked controls/subscriptions, no enrollment or arbitrary signing interface | Additional command authority requires a separate design and review |

## Findings and remediation

| ID | Finding | Resolution and verification |
| --- | --- | --- |
| SEC-01 | Host-scoped cookies leak across ports; the offline predecessor could recover CSRF from a cookie alone | Added an independently random, per-instance app capability to every protected route, including session recovery. Host tests reject missing/wrong capabilities and cookie-only recovery. This replaces the prior offline-only acceptance. |
| SEC-02 | A delayed body can outlive logout or expiry | Retained native post-body reauthorization and extended lifecycle checks into account operations. Delayed-body and delayed-login/logout regressions pass. |
| SEC-03 | Printed bootstrap URLs can leak a short-lived bearer secret | Printing remains explicit `--print-launch-url` opt-in; ordinary logs omit it. Single use and five-minute expiry remain enforced. |
| SEC-04 | Dropped/revoked local sessions could leave account work alive | Session ownership, revocation epochs and cancellation now invalidate pending native operations. Pending-read revocation, reusable account logout and MFA cancellation tests pass. |
| SEC-05 | Late browser login/vehicle responses could repopulate a disconnected account | Mounted/epoch guards reject stale authentication and garage results. UI tests and browser scenarios cover lifecycle behavior. |
| SEC-06 | Malformed GraphQL error shapes might be treated as success; secret JSON cleanup was incomplete | Strict error-envelope admission and secret-value cleanup were corrected. Redaction, malformed-response and native cleanup regressions pass. Cleanup does not claim control over all third-party allocations. |
| SEC-07 | An unexpected account identity or vehicle response could be associated with the wrong owner | Account identity is pinned; explicit contradictory vehicle IDs are rejected. Schema-supported `getVehicle.id` must match. Legacy VehicleState relies on the authorized request ID because the schema has no ID echo. |
| SEC-08 | Sessions with no vehicle association and mixed observation times could imply false attribution/freshness | Unknown charging sessions are withheld from vehicle history even for single-vehicle accounts. Individual source times are preserved; retrieval time is displayed separately. Core and UI regression checks pass. |
| SEC-09 | A 1 MiB browser cap could reject a legitimate 1 MiB upstream reply once raw and normalized representations were combined | Upstream remains capped at 1 MiB; local envelopes allow up to 3 MiB. UI tests admit the expanded envelope and reject over-limit content. |
| SEC-10 | A clean source tree could still package an old executable with a newer source archive | Packaging now checks a clean revision and runs the build before copying the executable and corresponding source. Source inspection verified the corrected order; actual package execution is recorded separately. |

## Reviewer-run evidence

The reviewer independently ran:

- `cargo test --locked -p rivian-host --test security`: **17 passed**.
- `cargo test --locked --lib -p rivian-api -p rivian-core`: **14 native API and
  18 core tests passed**.
- `npm test --prefix ui`: **28 passed**.

These checks include local capability enforcement, delayed-body/login revocation,
MFA lifecycle, pending-read cancellation, account pinning, malformed GraphQL
responses, sanitized errors, size limits, redirect refusal, vehicle ownership,
charging association and data freshness. The reviewer also inspected the UI
response-bound and package-provenance fixes. They did not run a live account or
independently execute the completed unsigned package.

The implementation team ran formatting, clippy, release builds, the two WASM
wrapper tests, actual compiled native/WASM fixture parity and real-browser demo/live
smoke suites. Those are broader implementation evidence; see [status](STATUS.md)
for the final counts, platform evidence and caveats. The implementation team
also ran `npm audit` (0 reported advisories) and `cargo-audit 0.22.2` (0
vulnerabilities, no warnings against 1,296 RustSec advisories at database revision
`7eebec69c352c7191b1f13eb95dd510eeca5d1de`, updated 2026-10-09). These are known
advisory matches at a recorded snapshot, not a certification or a substitute for
source review.

## Regression requirements

1. Retain wrong/ambiguous Host, Origin and fetch-metadata rejection tests. Unsafe
   methods must require exact Origin and CSRF in addition to cookie/capability.
2. Reject replayed, expired and concurrently reused bootstrap links; reject
   missing/duplicate/incorrect session material and cookie-only session recovery.
3. Exercise logout and expiry while a JSON body, login, MFA or read is pending.
   Old work must not restore a revoked session or a newly selected account.
4. Reject unknown operations, extra variables, unowned vehicle IDs, contradictory
   response IDs, oversized/deep bodies, malformed upstream errors and every blocked command.
5. Check response-size/time limits, redirect refusal, safe error messages, rate
   limits, MFA expiry and bounded retry behavior without real secrets.
6. Preserve real compiled-WASM/native parity; browser agreement never replaces
   native authorization. Verify guarded asset lookup in debug and release builds.
7. Observe sign-in, reload, vehicle switching, disconnect and logout in a real
   browser. No Rivian tokens should appear in browser storage, requests or responses.
8. Keep fixtures synthetic and review logs/screenshots before publishing them.
   Raw successful account/vehicle responses are personal data even without tokens.

## Outstanding gates

The project still needs the requested independent-provider adversarial review,
remediation and re-review, real-account acceptance, clean-machine OS/browser
checks, signed release/update delivery, and resource measurements. Remembered
sessions require reviewed native secure storage. Signed vehicle controls require
fresh native authorization, reviewed enrollment/signing, explicit owner intent
and separate adversarial testing before admission. Tauri mobile delivery has its
own storage, lifecycle, permissions and device validation work.
