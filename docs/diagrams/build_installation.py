"""Render native desktop build, packaging and owner startup workflows."""

from drawing import AMBER, BLUE, GREEN, Diagram


def build_pipeline():
    diagram = Diagram(
        "build-pipeline.png",
        "Build and desktop packaging",
        "Rust / Axum + embedded React interface and shared Rust/WASM validation.",
        height=1430,
    )
    diagram.box(
        40, 165, 530, 145,
        "Build prerequisites",
        "Repository-pinned Rust toolchain + WASM target\n"
        "Node.js / npm and native compiler / linker\n"
        "Target OS packaging tools for native installers",
        fill=BLUE,
    )
    diagram.box(
        630, 165, 530, 145,
        "node scripts/setup.mjs",
        "Fetch locked Cargo dependencies\n"
        "Install the matching wasm-bindgen CLI\n"
        "Install UI dependencies with npm ci",
        fill=BLUE,
    )
    diagram.arrow([(570, 237), (630, 237)])
    diagram.arrow([(680, 310), (680, 365)])
    diagram.text(40, 330, "node scripts/build.mjs", size=19, bold=True)

    steps = [
        (
            365, "1  Compile shared Rust to WASM",
            "Cargo builds rivian-wasm + its rivian-core dependency.\n"
            "Release target: wasm32-unknown-unknown",
        ),
        (
            500, "2  Generate browser bindings",
            "wasm-bindgen --target web\n"
            "WASM module + JavaScript bindings → ui/public/wasm",
        ),
        (
            635, "3  Bundle the React interface",
            "npm run build --prefix ui runs TypeScript + Vite.\n"
            "Output: ui/dist, including WASM module + bindings",
        ),
        (
            770, "4  Build the native host",
            "Cargo builds rivian-host in release mode.\n"
            "rust-embed includes ui/dist inside the executable.",
        ),
        (
            905, "5  Produce an OS-specific executable",
            "target/release/my-rivian-data (Windows: .exe)\n"
            "Runtime: native OS libraries + an existing browser",
        ),
    ]
    for y, title, body in steps:
        diagram.box(40, y, 705, 105, title, body, fill=GREEN)
    for start, end in zip(steps, steps[1:]):
        diagram.arrow([(392, start[0] + 105), (392, end[0])])

    diagram.box(
        795, 365, 365, 280,
        "Verification wrapper",
        "node scripts/verify.mjs\n"
        "Formatting check + full build\n"
        "Clippy and Rust tests\n"
        "TypeScript and UI tests\n"
        "Native/WASM fixture parity\n"
        "WASM memory-limit check",
        fill=BLUE,
    )
    diagram.box(
        795, 685, 365, 325,
        "CI workflow",
        "Linux / Windows / macOS\n"
        "setup + verify per target\n"
        "Packaging + lifecycle checks\n"
        "Linux adds Chromium smoke\n"
        "Unsigned artifacts + hashes\n\n"
        "Runner execution and cross-OS\n"
        "acceptance remain unverified.",
        fill=AMBER,
    )
    diagram.arrow([(392, 1010), (392, 1050)])
    diagram.box(
        40, 1050, 1120, 210,
        "6  Package portable archives and native desktop installers",
        "node scripts/package.mjs rebuilds from clean source with locked inputs.\n"
        "Linux DEB/RPM · macOS app/DMG · Windows NSIS installer · portable archive\n"
        "Includes corresponding source, GPL notices, SBOMs, hashes and build provenance.\n"
        "Unsigned development packaging is implemented; signing needs external credentials.\n"
        "Signature / notarization checks and a signed manifest gate release candidates.",
        fill=GREEN,
    )
    diagram.text(
        40, 1290,
        "Build separately for each OS / architecture; rebuild the host after UI or WASM changes.\n"
        "Signing and independent Anthropic acceptance remain unverified; mobile is Phase 2 work.",
        size=18,
    )
    diagram.footer("Source: scripts/setup.mjs, build.mjs, verify.mjs, package.mjs; packaging/; CI workflows.")
    return diagram.save()


def installation_startup():
    diagram = Diagram(
        "installation-startup.png",
        "Installation and startup",
        "Desktop packaging is implemented; current development delivery is unsigned.",
        height=1560,
    )
    diagram.box(
        40, 170, 530, 210,
        "Option A  Build from source",
        "Install the documented build prerequisites.\n"
        "node scripts/setup.mjs\n"
        "node scripts/build.mjs\n"
        "node scripts/verify.mjs\n"
        "Use the executable in target/release/.",
        fill=BLUE,
    )
    diagram.box(
        630, 170, 530, 210,
        "Option B  Available desktop package",
        "Check the intended artifact and target evidence.\n"
        "Download a portable archive or native package.\n"
        "Inspect corresponding source + notices + hashes.\n"
        "Hashes detect changes, not publisher identity.\n"
        "Successful CI artifacts have limited retention.",
        fill=BLUE,
    )
    diagram.arrow([(305, 380), (305, 413), (580, 413), (580, 450)])
    diagram.arrow([(895, 380), (895, 413), (620, 413), (620, 450)])
    diagram.box(
        175, 450, 850, 160,
        "Match your operating system and architecture",
        "Linux DEB/RPM · macOS app/DMG · Windows NSIS installer\n"
        "Native packages are implemented; cross-OS owner acceptance is unverified.\n"
        "Signed delivery requires external signing / notarization evidence.\n"
        "Android / iOS Tauri applications are Phase 2 and not implemented.",
        fill=AMBER,
    )
    diagram.arrow([(600, 610), (600, 650)])
    diagram.box(
        175, 650, 850, 130,
        "Launch installed app or portable executable",
        "Start menu / Applications / Linux desktop launcher.\n"
        "Portable: run my-rivian-data (.exe on Windows) from a terminal.\n"
        "The executable embeds UI + WASM; an existing browser is required.",
        fill=GREEN,
    )
    diagram.arrow([(600, 780), (600, 820)])
    diagram.box(
        175, 820, 850, 140,
        "Native host opens your default browser",
        "Bind to 127.0.0.1 on a free port by default.\n"
        "Single-use bootstrap establishes cookie + app capability + CSRF.\n"
        "Private link expires after five minutes; keep it out of shared logs.",
        fill=GREEN,
    )
    diagram.arrow([(600, 960), (600, 1000)])
    diagram.box(
        175, 1000, 850, 210,
        "Sign in locally or restore an opted-in session",
        "Password / verification inputs go only to the native host.\n"
        "Memory-only default; optional Remember sign-in uses the OS vault.\n"
        "Restore validates / refreshes session and rechecks identity + ownership.\n"
        "--session-only skips reading and writing remembered account entries.\n"
        "Choose vehicle: Overview · Charging · Health · Location · Live updates.",
        fill=GREEN,
    )
    diagram.arrow([(600, 1210), (600, 1250)])
    diagram.box(
        175, 1250, 850, 185,
        "Choose how to finish",
        "Close app stops the local host; an opted-in remembered session remains.\n"
        "End session / Disconnect account cancels access and attempts vault deletion.\n"
        "Vault deletion failures are reported separately; passwords are never saved.\n"
        "Use --demo for offline fictional data with no credential entry or live fallback.",
        fill=GREEN,
    )
    diagram.footer("Unverified: real-account acceptance, cross-OS delivery, signing and independent Anthropic acceptance.")
    return diagram.save()


def render():
    return [build_pipeline(), installation_startup()]


if __name__ == "__main__":
    for output in render():
        print(output)
