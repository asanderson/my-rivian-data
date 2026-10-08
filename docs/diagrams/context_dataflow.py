"""System context and actual offline request flow, derived from the source."""

from drawing import Diagram, GREEN, BLUE, AMBER, MUTED


def context():
    d = Diagram("system-context.png", "System context", "Your vehicles. Your data.  |  What runs today, and where it runs.", height=1120)
    d.box(405, 175, 390, 105, "Rivian owner", "Launch the app and explore the demo.", fill=GREEN)
    d.rect(40, 345, 1120, 480, fill="#edf2e9")
    d.text(600, 365, "OWNER’S COMPUTER  ·  Linux / macOS / Windows", size=17, bold=True, center=True)
    d.arrow([(405, 225), (275, 225), (275, 425)])
    d.text(140, 278, "Explore", size=16)
    d.arrow([(795, 225), (910, 225), (910, 425)])
    d.text(930, 278, "Launch", size=16)
    d.box(75, 425, 425, 310, "Existing web browser", "React interface and API explorer\n\nRust/WASM validator\n• Same portable validation core\n• Bounded, non-secret inputs\n• Browser check is advisory", fill=BLUE)
    d.box(700, 425, 425, 310, "my-rivian-data executable", "Native Rust / Axum host\n• Serves embedded UI + WASM\n• Local session and CSRF checks\n• Authoritative core validation\n• Synthetic vehicle/query fixtures\n• No live Rivian transport", fill=GREEN)
    d.arrow([(500, 510), (700, 510)])
    d.text(600, 459, "Local requests", size=15, center=True)
    d.text(600, 484, "127.0.0.1:PORT", size=14, center=True)
    d.arrow([(700, 650), (500, 650)])
    d.text(600, 598, "Bundled assets +", size=15, center=True)
    d.text(600, 621, "synthetic replies", size=15, center=True)
    d.text(65, 763, "No account credentials, live vehicle data, or commands in this milestone.", size=18, color=MUTED)
    d.box(40, 875, 1120, 155, "Planned integrations  ·  NOT ACTIVE", "Owner-authorized Rivian access + native transport + OS credential vaults.\nLater: Android / iOS Tauri apps reuse the Rust core and React UI.\nThese future components have no data path in the current offline prototype.", fill=AMBER, dashed=True)
    d.footer("Independent community project. No affiliation with Rivian. Diagram describes the current source, not a live API.")
    return d.save()


def dataflow():
    d = Diagram("data-flow.png", "Launch, validate, and read demo data", "Native Rust remains the authority; the browser never holds Rivian credentials.", height=1470)
    d.text(40, 164, "1  ESTABLISH A LOCAL SESSION", size=16, bold=True)
    d.box(40, 205, 335, 170, "Native launcher", "Create one-use capability.\nOpen browser URL fragment.\nExpiry: five minutes.", fill=GREEN)
    d.box(435, 205, 330, 170, "Browser startup", "Remove fragment immediately.\nPOST /api/bootstrap with\ncapability in JSON body.", fill=BLUE)
    d.box(825, 205, 335, 170, "Native bootstrap", "Consume capability atomically.\nReturn HttpOnly session cookie\nand in-memory CSRF token.", fill=GREEN)
    d.arrow([(375, 290), (435, 290)])
    d.arrow([(765, 290), (825, 290)])
    d.arrow([(992, 375), (992, 405), (600, 405), (600, 375)])
    d.text(745, 417, "Session established in browser", size=15, center=True)

    d.text(40, 475, "2  RUN A DEMO QUERY", size=16, bold=True)
    d.rect(40, 512, 525, 665, fill="#edf3f9")
    d.rect(635, 512, 525, 665, fill="#edf2e9")
    d.text(62, 528, "BROWSER  /  React + Rust/WASM", size=16, bold=True)
    d.text(657, 528, "NATIVE HOST  /  Rust + Axum", size=16, bold=True)
    d.box(65, 570, 475, 140, "A  Parse and validate locally", "Choose operation + JSON variables.\nRun shared validator in WASM.\nThen request native validation.", fill=BLUE)
    d.box(660, 570, 475, 140, "B  POST /api/validate", "Check local boundary, session + CSRF.\nRecheck session after body extraction.\nRun bounded core validation.", fill=GREEN)
    d.arrow([(540, 638), (660, 638)])
    d.box(65, 775, 475, 140, "C  Compare results", "Require native/WASM agreement,\nvalid inputs, and a demo operation.\nThen POST /api/execute.", fill=BLUE)
    d.arrow([(898, 710), (898, 742), (302, 742), (302, 775)])
    d.text(582, 714, "validation result", size=14, center=True)
    d.box(660, 775, 475, 140, "D  Reauthorize and revalidate", "Check session + CSRF after extraction.\nNative core validates again.\nOnly admitted demo queries can run.", fill=GREEN)
    d.arrow([(540, 842), (660, 842)])
    d.box(660, 985, 475, 140, "E  Read synthetic fixtures", "Generate the bounded demo response.\nNo Rivian request or vehicle command.\nUnavailable operations are rejected.", fill=GREEN)
    d.arrow([(898, 915), (898, 985)])
    d.box(65, 985, 475, 140, "F  Render the response", "Display JSON labeled synthetic.\nBound response read to 1 MiB.\nNo localStorage / sessionStorage.", fill=BLUE)
    d.arrow([(660, 1055), (540, 1055)])
    d.text(40, 1198, "Invalid, expired, or blocked requests stop; UI agreement never grants authority.", size=16)
    d.box(40, 1242, 1120, 130, "Session and data limits", "One-hour idle / eight-hour absolute session expiry. Logout revokes the session and clears its cookie.\n32 KiB HTTP JSON bodies; 16 KiB variables; depth 8; 256 values; 64 MiB WASM linear memory.\nSame-host cookies are not isolated by port: live credential support remains a separate design gate.", fill=AMBER)
    d.footer("Scope: current offline implementation. Session secrets shown as concepts only; no real tokens or account data.")
    return d.save()


def render():
    return [context(), dataflow()]


if __name__ == "__main__":
    for path in render():
        print(path.name)
