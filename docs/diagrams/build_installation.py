"""Render build and installation diagrams from the standalone application workflow."""

from drawing import AMBER, BLUE, GREEN, Diagram


def build_pipeline():
    diagram = Diagram(
        "build-pipeline.png",
        "Build pipeline",
        "Native Rust host + embedded React interface and shared Rust/WASM validation.",
        height=1190,
    )
    diagram.box(
        40, 165, 530, 145,
        "Build prerequisites",
        "Rust 1.99.0 and the WASM target\n"
        "Node.js 24+ with npm\n"
        "Native compiler/linker for the target OS",
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
            "Cargo builds rivian-wasm and its rivian-core dependency.\n"
            "Release target: wasm32-unknown-unknown",
        ),
        (
            500, "2  Generate browser bindings",
            "wasm-bindgen --target web\n"
            "WASM module and JavaScript bindings → ui/public/wasm",
        ),
        (
            635, "3  Bundle the React interface",
            "npm run build --prefix ui runs TypeScript and Vite.\n"
            "Output: ui/dist, including the WASM module and bindings",
        ),
        (
            770, "4  Build the native host",
            "Cargo builds rivian-host in release mode.\n"
            "rust-embed includes ui/dist inside the executable.",
        ),
        (
            905, "5  Produce an OS-specific executable",
            "target/release/my-rivian-data (Windows: .exe)\n"
            "Runtime: native OS libraries and an existing browser",
        ),
    ]
    for y, title, body in steps:
        diagram.box(40, y, 705, 105, title, body, fill=GREEN)
    for start, end in zip(steps, steps[1:]):
        diagram.arrow([(392, start[0] + 105), (392, end[0])])

    diagram.box(
        795, 365, 365, 300,
        "Verification wrapper",
        "node scripts/verify.mjs\n"
        "Formatting check\n"
        "Full build sequence at left\n"
        "Clippy and Rust tests\n"
        "TypeScript and UI tests\n"
        "Native/WASM fixture parity\n"
        "WASM memory-limit check",
        fill=BLUE,
    )
    diagram.box(
        795, 705, 365, 305,
        "GitHub Actions",
        "Linux / Windows / macOS\n"
        "Each job runs setup + verify.\n"
        "Linux adds Chromium smoke.\n"
        "node scripts/package.mjs\n\n"
        "Successful jobs upload\n"
        "unsigned app archives + hashes.\n"
        "Retention: 7 days",
        fill=AMBER,
    )
    diagram.text(
        40, 1040,
        "Build separately for each OS and architecture. Rebuild the host after UI or WASM changes.\n"
        "Consumers do not need Node.js, Rust, npm or a separate web server.",
        size=18,
    )
    diagram.footer("Source: scripts/setup.mjs, scripts/build.mjs, scripts/verify.mjs and .github/workflows/verify.yml")
    return diagram.save()


def installation_startup():
    diagram = Diagram(
        "installation-startup.png",
        "Installation and startup",
        "Current delivery: unsigned, self-contained development packages for desktop computers.",
        height=1220,
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
        "Option B  Successful CI artifact",
        "Open the intended GitHub Actions run.\n"
        "Check the target job completed successfully.\n"
        "Download its unsigned application artifact.\n"
        "Extract artifact, then its .tar.gz package.\n"
        "Artifacts expire after 7 days.",
        fill=BLUE,
    )
    diagram.arrow([(305, 380), (305, 413), (580, 413), (580, 450)])
    diagram.arrow([(895, 380), (895, 413), (620, 413), (620, 450)])
    diagram.box(
        175, 450, 850, 130,
        "Match your operating system and architecture",
        "Use a compatible executable and the OS libraries it requires.\n"
        "These are unsigned developer builds; signed consumer installers are pending.\n"
        "Android and iOS applications are planned for a later phase.",
        fill=AMBER,
    )
    diagram.arrow([(600, 580), (600, 620)])
    diagram.box(
        175, 620, 850, 105,
        "Run the executable from a terminal",
        "Linux / macOS: chmod u+x my-rivian-data, then ./my-rivian-data\n"
        "Windows PowerShell: .\\my-rivian-data.exe   |   Keep the terminal open.",
        fill=GREEN,
    )
    diagram.arrow([(600, 725), (600, 765)])
    diagram.box(
        175, 765, 850, 130,
        "Local host starts and opens your default browser",
        "The host binds to 127.0.0.1 on a free port by default.\n"
        "A private, single-use bootstrap link establishes the local browser session.\n"
        "The link expires after five minutes; keep it out of shared logs and screenshots.",
        fill=GREEN,
    )
    diagram.arrow([(600, 895), (600, 935)])
    diagram.box(
        175, 935, 850, 105,
        "Sign in locally, then choose your vehicle",
        "Enter your Rivian account details; complete verification if requested.\n"
        "Overview · Charging · Vehicle health · Location · Separate API explorer",
        fill=GREEN,
    )
    diagram.text(
        175, 1070,
        "Your vehicles. Your data.\n"
        "For fictional data use --demo. Choose End session, then Ctrl+C to stop the app.",
        size=18,
    )
    diagram.footer("Source: README.md, crates/rivian-host/src/main.rs and .github/workflows/verify.yml")
    return diagram.save()


def render():
    return [build_pipeline(), installation_startup()]


if __name__ == "__main__":
    for output in render():
        print(output)
