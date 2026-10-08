# Implementation status

Application: My Rivian Data. Date: 2026-10-08.
Milestone: first offline Rust/Axum + React/Rust-WASM prototype.

## Implemented

- Rust workspace with a portable core, WASM bindings and native Axum host.
- Embedded React dashboard and searchable operation explorer.
- Six synthetic queries, one planned subscription and three blocked mutations.
- Shared bounded validator, native execution revalidation, and 22 conformance fixtures.
- One-use local bootstrap, session/CSRF controls, strict host/origin checks,
  fixed local binding, logout/expiry, CSP and guarded asset lookup.
- Cross-platform source setup/build/verification scripts and a three-OS CI definition.

## Evidence

Verified on Linux x86_64, kernel 6.18.44 and glibc 2.39, with Rust 1.99.0,
Node 24.19.0, wasm-bindgen 0.2.129 and Chromium 153.0.8010.0:

| Check | Result |
| --- | --- |
| Default release build through `node scripts/build.mjs` | Passed; embedded UI and WASM, no LTO override |
| Rust formatting and clippy, warnings rejected | Passed |
| Rust tests | 22 passed: 10 core, 11 host security, 1 wrapper |
| TypeScript checking and UI unit tests | Passed; 10 unit tests |
| Real compiled native/WASM comparison | 22 fixtures and catalog match |
| WASM memory ceiling | Growth to 64 MiB succeeds; one more page is rejected |
| Desktop and mobile-width Chromium smoke | Passed at 1440 px and 390 px, no horizontal overflow |
| Browser behaviors | Real WASM load, query/validation, blocked controls, invalid vehicle, session reload, logout/cookie removal, no runtime/CSP errors |
| Separate review context | Independently reran all 11 host security tests; delayed-body issue fixed and retested |

Screenshots are included in `screenshots/` (tracked copies; browser tests regenerate `artifacts/screenshots/`). No bootstrap capability or
real account information appears in them. The usual Playwright Chromium CDN
download failed in this environment, so browser verification used Chromium 153
from an external test-only installation via `PLAYWRIGHT_CHROMIUM_EXECUTABLE`.
The product does not bundle or depend on that test browser.

Measured artifact sizes for this limited offline workload:

- Linux native executable with embedded assets: 2,408,352 bytes (about 2.30 MiB).
- Compiled WASM module: 89,433 bytes (about 87.3 KiB).
- Built UI asset files, including WASM and bindings: 365,535 bytes (about 357 KiB),
  already embedded in the executable; do not add them again to its size.

The executable dynamically uses the OS's libc, libm and libgcc_s. This build host
does not establish a minimum supported Linux distribution. Windows/macOS CI is
configured but has not run. CPU, RAM, battery, account coverage and multi-day agent
throughput have not been benchmarked; source/compiler caches are not runtime size.

## Not implemented or validated yet

- Real Rivian login/password/MFA, persistent credentials, vault adapters or account access.
- Real Rivian HTTP/WebSocket/Parallax, enrollment, signing or vehicle commands.
- Complete API coverage, live vehicle tests, media, background collection or history.
- Windows/macOS runtime tests, signed installers, updates, Android or iOS binaries.
- Independent Anthropic review. Current review is a separate context using the
  implementation platform; it is not the planned independent-vendor gate.
- A sustained 24x7 agent runner or a measured multi-day subscription throughput pilot.

## Findings and decisions

- Delayed POST bodies must be authenticated again after extraction; fixed and covered
  by a deterministic logout-race regression test.
- Same-host cookies are not isolated by port. This limitation is explicit and accepted
  only for the credential-free offline milestone; live-mode policy remains a gate.
- Rust 1.99.0 thin-LTO release linking failed in this environment with unresolved core
  symbols. The release profile disables LTO; ordinary release optimization remains.
- Language/runtime/project token and resource budgets in the plan remain unmeasured
  planning estimates. This prototype's artifact sizes are a narrower workload, not a
  replacement for the full product budgets.

## Next task

Implement the native Rivian authentication/transport state machine behind deterministic
HTTP mocks, build the versioned operation inventory, and establish OS vault adapters.
Do not request credentials through chat or add real credentials to fixtures.
