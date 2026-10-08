# my-rivian-data

**Rivian Local** is the working application name for this project. The planned product retrieves a registered user's Rivian data using native API transport and local credential handling. The current milestone is an offline prototype.

Phase 1 starts with a self-contained Rust/Axum host, a React interface and a real Rust/WASM validation module. This milestone is an **offline feasibility prototype**: it uses synthetic vehicle data, accepts no Rivian credentials, makes no Rivian API requests and cannot send vehicle commands. It is not affiliated with Rivian.

The native and WASM builds share the same catalog and validation code. The host revalidates every request and owns the local session. The browser module handles bounded, non-secret data. Phase 2 will reuse the native Rust core and React interface in independent Android/iOS Tauri applications, with a native invocation adapter instead of an Axum server on the phone.

## Run a built executable

Build from source using the commands below. The GitHub Actions workflow also creates
unsigned executable artifacts for successful jobs; a workflow definition is not
evidence that a platform build has passed. Compiled binaries are not committed.

A release build embeds the React assets and WASM module. Consumers need a supported desktop OS and a current browser; they do not need Rust, Node.js, npm or a separate web server. Executables are OS/architecture specific, use normal OS libraries, and are currently unsigned development builds. Installers, signing, supported-OS certification and mobile packages are later milestones.

On Linux/macOS, run `./rivian-local`. On Windows, run `rivian-local.exe`. The app binds only to `127.0.0.1`, chooses a free port and opens the default browser. Leave its terminal running; press Ctrl+C to stop it.

```text
rivian-local --help
rivian-local --port 43127
rivian-local --no-open --print-launch-url
```

`--port 0` chooses a free port (the default). `--no-open` suppresses browser launch. `--print-launch-url` explicitly prints a sensitive, single-use bootstrap link that expires after five minutes; open it locally and keep it out of screenshots, tickets and shared logs. Restart the application to obtain a new link. The normal status URL alone does not authorize a browser session.

## Build from source

Install [Rust through rustup](https://rustup.rs/) and [Node.js 24 or newer with npm](https://nodejs.org/). Rust is pinned to 1.99.0, with rustfmt, clippy and the `wasm32-unknown-unknown` target in `rust-toolchain.toml`.

- **Windows:** use the MSVC Rust toolchain and Visual Studio Build Tools with the C++ desktop workload and Windows SDK. PowerShell works for the commands below.
- **macOS:** install Xcode Command Line Tools (`xcode-select --install`).
- **Linux:** install the distribution's native C/C++ compiler and linker (for example, the `build-essential` package on Ubuntu/Debian).

From the project directory, run these same commands on each desktop OS:

```text
node scripts/setup.mjs
node scripts/build.mjs
node scripts/verify.mjs
```

Setup downloads Cargo dependencies, installs the exact `wasm-bindgen-cli` version recorded in `Cargo.lock` when necessary, and runs `npm ci` against the UI lockfile. It needs internet access and can take several minutes on a fresh machine. Build and verification use the locked dependencies; tools and dependencies still need to be downloaded before an air-gapped build.

The build compiles Rust to WASM, generates its web bindings, bundles the React interface, and then embeds that bundle in the native release binary. The executable appears at `target/release/rivian-local` or `target/release/rivian-local.exe`. Build after every UI or WASM change so the embedded assets stay current. Build separately on each target operating system/architecture; a Linux executable is not a Windows or macOS package.

The scripts invoke npm's JavaScript entry point through Node without a shell. If an unusual Node installation prevents discovery, set `RIVIAN_NPM_CLI` to the absolute path of its `npm-cli.js`. Cargo's default `target` directory is assumed by the build scripts; do not override `CARGO_TARGET_DIR` for this prototype.

## Verify

`node scripts/verify.mjs` runs formatting checks, the release build, clippy with warnings rejected, workspace tests, TypeScript checking, UI unit tests and native/WASM parity. The parity check runs the **compiled WASM** in Node, compares every checked-in fixture and the catalog with native Rust output, and checks that a 64 MiB memory ceiling is enforced. It can also run alone after a build:

```text
node scripts/check-parity.mjs
```

For the real-browser smoke test, install Chromium once and run the UI test script after building the host:

```text
node ui/node_modules/playwright/cli.js install chromium
npm run test:browser --prefix ui
```

On Linux, Playwright may also require distribution packages; `node ui/node_modules/playwright/cli.js install --with-deps chromium` installs those when run with suitable system privileges. The test can use an existing compatible Chromium by setting `PLAYWRIGHT_CHROMIUM_EXECUTABLE` to its executable path. Browser downloads are development/test dependencies, separate from the self-contained application.

The GitHub Actions workflow is configured to run the core checks on Linux, Windows and macOS, run the Chromium smoke test on Linux, and retain unsigned executable artifacts. Configuration is not evidence that those CI jobs have run. Actual results, browser checks, limitations and remaining work belong in [docs/STATUS.md](docs/STATUS.md).

## Project layout

| Path | Responsibility |
| --- | --- |
| `crates/rivian-core` | Portable catalog, bounded input validation and synthetic data |
| `crates/rivian-wasm` | Non-secret WASM exports of the shared core |
| `crates/rivian-host` | Axum loopback service, local session boundary and embedded assets |
| `ui` | React/TypeScript interface and native-host HTTP adapter |
| `fixtures` | Synthetic fixtures consumed by native/WASM verification |
| `scripts` | Portable dependency setup, build and verification |
| `docs` | Implementation status and review evidence |

## Security and next milestones

The local host checks Host/Origin, uses a single-use bootstrap capability and session/CSRF protections, and exposes a bounded operation catalog. WASM linear memory is capped at 64 MiB; this does not cap the browser's total memory. The module does not export credential storage, networking, signing or command execution. These controls reduce specific attack paths; an offline prototype is not evidence of production security. See the review and status records in `docs` for the tested boundary and outstanding findings.

Next milestones are a native credential-vault interface and authentication state machine, owner-authorized live read-only API validation, catalog expansion, reviewed command authority, packaging and independent adversarial review. Actual credential entry and live vehicle operations require those implementations and their gates; the demo UI must never solicit account passwords.

This original project code uses the repository's [GNU GPLv3 license](LICENSE) (`GPL-3.0-only`). Third-party dependencies retain their own licenses and [notices](docs/THIRD-PARTY-NOTICES.md). No assertion of a completed independent Anthropic review, live Rivian compatibility, signed release or store acceptance is made by this prototype.

## Prototype screenshots

All values below are synthetic.

![Desktop prototype](docs/screenshots/desktop.png)

[Mobile-width screenshot](docs/screenshots/mobile.png)
