# My Rivian Data diagrams

These standalone PNGs explain how an owner connects an account, reads vehicle data,
receives live updates and starts the desktop app. Green identifies native application
and build components, blue identifies browser and source components, and amber
identifies scope or acceptance limits. Labels carry the meaning as well as color.

| Diagram | What it explains | Embedded in |
| --- | --- | --- |
| [System context](system-context.png) | Owner views, separate developer explorer, native authority, optional OS vault and direct Rivian access | [Architecture](../ARCHITECTURE.md#system-context) |
| [Data flow](data-flow.png) | Protected bootstrap, sign-in or restoration, fixed reads, native WebSocket updates and confirmed nickname change | [Architecture](../ARCHITECTURE.md#session-sequence) |
| [Build pipeline](build-pipeline.png) | Locked setup, shared Rust/WASM, React bundle, native executable, verification and desktop packaging | [Developer guide](../DEVELOPER-GUIDE.md#build-from-source) |
| [Installation and startup](installation-startup.png) | Source/package paths, platform matching, local connection, owner views and session controls | [Owner guide](../OWNER-GUIDE.md#get-started), [README](../../README.md#start-the-app) |

The Phase 1 architecture remains Rust/Axum + React + shared Rust/WASM. Native Rust
owns account identity, resource ownership, account epochs and cancellation. The
fixed catalog has 244 entries drawn from 234 source surfaces; 33 fixed reads and one
explicitly confirmed vehicle rename are admitted for live dispatch. Exact-document
admission resolves only an existing fixed operation; imported GraphQL is never
proxied. Rename uses a one-use, two-minute ticket bound to the exact variables and
native account epoch. Physical controls, phone/key enrollment and signed commands
remain blocked.

Native WebSocket telemetry requests and admits only 11 reviewed RVMs, dropping
unrequested RVMs before retention or browser serialization. Owner views preserve
per-field source freshness and show tire pressure only with a valid reading.
Missing data stays unavailable; failed live requests never become demo samples.

Account tokens stay native, in memory by default. Remember sign-in requires explicit
owner opt-in and uses Linux Secret Service, macOS Keychain or Windows Credential
Manager; passwords are never saved. Restored sessions recheck identity and ownership.
`--session-only` disables remembered-entry reads and writes. Close app or ordinary
process exit preserves prior opted-in sessions; End session or Disconnect account
attempts vault deletion and reports deletion failures separately.

Portable archives and native desktop packaging are implemented: Linux DEB/RPM,
macOS app/DMG and Windows NSIS installer. Development delivery is unsigned; release
signing and notarization require external credentials and verified evidence.
Real-account acceptance, cross-OS owner acceptance, signing and independent
Anthropic acceptance remain unverified. Android/iOS Tauri apps are Phase 2 work and
are not implemented; they will invoke native Rust without an Axum server on the phone.

## Edit and regenerate

The editable sources are `context_dataflow.py` and `build_installation.py`;
`drawing.py` supplies the shared styles and layout primitives. PNGs are rendered
at twice the layout resolution for standalone viewing. Commit the generators and
their rendered PNGs together when the implementation changes.

Rendering requires Python, Pillow, and DejaVu Sans or Arial. These are
optional documentation tools; application users and normal application builds do
not need them. From the repository root, in a Python environment with Pillow installed:

```text
python docs/diagrams/render.py
```

Install Pillow in a documentation-only virtual environment if needed:

```text
python -m venv .venv
```

Then use `.venv/bin/python -m pip install Pillow` on Linux/macOS, or
`.venv\Scripts\python.exe -m pip install Pillow` on Windows, and run the rendering
command with that same Python executable. The diagrams use only synthetic concepts;
never add passwords, session capabilities, account identifiers, or real vehicle data.
