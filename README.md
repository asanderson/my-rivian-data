# My Rivian Data

**Your vehicles. Your data.** My Rivian Data gives Rivian owners a local interface
for information associated with the vehicles they own, organized around charging,
vehicle readings and location. The desktop app connects directly to the unofficial
Rivian API through native Rust; your browser is the interface.

The default mode now supports Rivian account sign-in and live read requests.
This is an independent community project, not affiliated with Rivian. The live
implementation has deterministic tests, but **has not yet been verified with an
owner's real account**. Rivian can change or restrict its undocumented API. See
[validation and limitations](docs/STATUS.md) before using a development build.

## For vehicle owners

| View | Information shown when Rivian supplies it |
| --- | --- |
| **Overview** | Battery level, estimated range, charging state and lock status |
| **Charging** | Current charging status, charge limit and power, plus available charging sessions |
| **Vehicle health** | Reported mileage, temperature and lock status; unavailable diagnostics stay clearly marked |
| **Location** | Reported coordinates and location accuracy when available |

Choose an account vehicle once and switch between these areas. Missing data is
shown as unavailable; battery charge is never presented as battery health. The
app does not change vehicle settings or collect a background location history.

![Local Rivian sign-in page; blank fields contain no account information](docs/screenshots/live-sign-in.png)

Read the [owner guide](docs/OWNER-GUIDE.md) for sign-in, everyday features and help.

## Start the app

1. Download an unsigned package for your operating system and architecture from a
   successful [GitHub Actions](https://github.com/asanderson/my-rivian-data/actions)
   run. Extract the downloaded artifact, then extract the `.tar.gz` package inside
   it. A developer can also [build from source](docs/DEVELOPER-GUIDE.md#build-from-source).
2. Open a terminal in the extracted folder. Run `./my-rivian-data` on Linux/macOS,
   or `.\my-rivian-data.exe` in Windows PowerShell. On Linux/macOS, run
   `chmod u+x my-rivian-data` first if needed.
3. Your default browser opens the local app. Sign in with the email address and
   password you use for Rivian. Complete account verification if requested.
4. Leave the terminal open while using the app. Choose **End session** when done;
   press Ctrl+C in the terminal to stop it.

Enter credentials only in the local app. Passwords are not saved; Rivian session
tokens stay in native process memory and are not returned to the browser. There
is no remembered sign-in or cloud service run by this project.

For a credential-free preview, run `./my-rivian-data --demo` (Windows:
`.\my-rivian-data.exe --demo`). Demo mode uses two fictional vehicles and clearly
labeled sample readings, and does not contact Rivian.

A built executable contains the interface and WASM module. You need a compatible
desktop OS and a current browser; Rust, Node.js, npm and a separate web server are
not runtime requirements. Signed installers and Android/iOS apps are later work.

![Installation and startup: extract a matching package, launch the local app and sign in](docs/diagrams/installation-startup.png)

## Developers

Under **Developer tools**, **API explorer** presents the admitted operation catalog,
editable variables and request/response JSON in a Swagger-like layout. Both owner
views and the explorer use native authorization and validation. The explorer is
an adapter interface; it does not provide arbitrary GraphQL or unrestricted proxy
access. Signed controls and subscriptions remain unavailable.

Phase 1 uses a native Rust/Axum host with embedded React and shared Rust/WASM
validation. Phase 2 will reuse Rust and React through Tauri on Android/iOS,
with a native invocation adapter in place of the desktop loopback server.

- [Developer guide: explorer, setup, build, verification and packaging](docs/DEVELOPER-GUIDE.md)
- [Architecture, security boundary and data flow](docs/ARCHITECTURE.md)
- [API coverage and primary-source provenance](docs/API-COVERAGE.md)
- [Standalone PNG diagrams and editable sources](docs/diagrams/README.md)
- [Implementation status and remaining release work](docs/STATUS.md)
- [Security review and threat model](docs/SECURITY-REVIEW.md)

Original project code uses the [GNU GPLv3 license](LICENSE) (`GPL-3.0-only`).
Dependencies retain their own [licenses and notices](docs/THIRD-PARTY-NOTICES.md).
