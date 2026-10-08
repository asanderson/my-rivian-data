# My Rivian Data

**Your vehicles. Your data.** My Rivian Data aims to give Rivian owners easy access
to the data associated with the vehicles they own, organized around everyday
questions about charging, vehicle health and location.

**The current app is an offline demo.** It shows two fictional vehicles and sample
data. It does not ask for a Rivian password, connect to your account or vehicle,
or change vehicle settings. This is an independent community project, not
affiliated with Rivian.

## For vehicle owners

Choose a demo vehicle, then explore a familiar area of ownership. No API or JSON
knowledge is needed for these views.

| View | What you can explore in the demo |
| --- | --- |
| **Overview** | Battery level, estimated range, charging state and lock status at a glance |
| **Charging** | Sample charging status, charge limit and power, plus a history of charging sessions with energy added and duration |
| **Vehicle health** | Mileage, temperature and lock status; battery health, tire pressure and service records are unavailable |
| **Location** | Fictional coordinates and sample location accuracy; no map or tracking |

All values and timestamps are fixed examples, not live vehicle readings.
Switching vehicles updates the selected vehicle across the owner views.

![My Rivian Data owner overview with fictional vehicle data](docs/screenshots/desktop.png)

Read the [owner guide](docs/OWNER-GUIDE.md) for a walkthrough, screenshots and help.

## Start the demo

1. Obtain a development executable matching your operating system and computer
   architecture. Successful [GitHub Actions](https://github.com/asanderson/my-rivian-data/actions) jobs provide unsigned
   executable artifacts; extract the downloaded archive. A developer can also
   [build from source](docs/DEVELOPER-GUIDE.md#build-from-source).
2. Run it from a terminal: `./my-rivian-data` on Linux/macOS, or
   `.\my-rivian-data.exe` in Windows PowerShell. On Linux/macOS, run
   `chmod u+x my-rivian-data` first if needed.
3. Your default browser opens the local demo. Leave the terminal open while using
   the app; press Ctrl+C in that terminal to stop it.

A built executable contains the whole interface. You need a compatible desktop OS
and a current browser; you do not need Rust, Node.js, npm or a separate web server.
These are unsigned development builds. Signed installers, supported-OS
certification and Android/iOS apps are future milestones. See
[current validation and limitations](docs/STATUS.md) before choosing a build.

![Installation and startup: obtain a matching build, run it, then explore the local demo](docs/diagrams/installation-startup.png)

## Developers

Under **Developer tools**, the separate **API explorer** view exposes the local operation catalog, editable
request variables and raw requests/responses in a Swagger-like layout. It uses the
same validation and execution checks as the owner views. It describes this demo's
local API, not a verified upstream Rivian schema.

Phase 1 uses a native Rust/Axum host with an embedded React interface and a shared
Rust/WASM validator. Phase 2 plans to reuse the Rust core and React interface in
Android/iOS Tauri apps, with a native invocation adapter instead of a phone web server.

- [Developer guide: API explorer, setup, build, verification and CI](docs/DEVELOPER-GUIDE.md)
- [Architecture and design: system context, API boundary and data flow](docs/ARCHITECTURE.md)
- [Standalone PNG diagrams and editable sources](docs/diagrams/README.md)
- [Implementation status and next milestones](docs/STATUS.md)
- [Security review](docs/SECURITY-REVIEW.md)

The original project code uses the [GNU GPLv3 license](LICENSE)
(`GPL-3.0-only`). Dependencies retain their own [licenses and notices](docs/THIRD-PARTY-NOTICES.md).
