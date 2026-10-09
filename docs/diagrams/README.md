# My Rivian Data diagrams

These standalone PNGs explain Phase 1 live account reads and explicit demo mode. Green identifies
native application/build components, blue identifies browser/source components,
and amber calls out scope or release limitations. Labels carry the meaning as well
as color. Mobile apps are explicitly marked as later work. Real-account acceptance and signed
consumer delivery are not implied by these diagrams.

| Diagram | What it explains | Embedded in |
| --- | --- | --- |
| [System context](system-context.png) | Owner views and developer API explorer, computer, native host, direct Rivian access and later mobile reuse | [Architecture](../ARCHITECTURE.md#system-context) |
| [Data flow](data-flow.png) | Bootstrap, account sign-in, native authorization, bounded live reads and owner/developer presentations | [Architecture](../ARCHITECTURE.md#session-sequence) |
| [Build pipeline](build-pipeline.png) | Locked setup, WASM bindings, UI bundle, embedded native executable, and verification | [Developer guide](../DEVELOPER-GUIDE.md#build-from-source) |
| [Installation and startup](installation-startup.png) | Source/package paths, platform matching, local sign-in and owner views | [Owner guide](../OWNER-GUIDE.md#get-started), [README](../../README.md#start-the-app) |

## Edit and regenerate

The editable sources are `context_dataflow.py` and `build_installation.py`;
`drawing.py` supplies the shared styles and layout primitives. PNGs are rendered
at twice the layout resolution for standalone viewing. Commit the generators and
their rendered PNGs together when the implementation changes.

Rendering requires Python 3.10 or newer, Pillow, and DejaVu Sans or Arial. These are
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
