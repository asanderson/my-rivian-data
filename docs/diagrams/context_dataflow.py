"""System context and account data flow, derived from native Phase 1 boundaries."""

from drawing import Diagram, GREEN, BLUE, AMBER, MUTED


def context():
    d = Diagram("system-context.png", "System context", "Your vehicles. Your data.  |  One local application; direct Rivian account access.", height=1470)
    d.box(405, 170, 390, 105, "Rivian owner", "Launch, connect and choose a vehicle.", fill=GREEN)
    d.rect(40, 325, 1120, 705, fill="#edf2e9")
    d.text(600, 347, "OWNER’S COMPUTER  ·  Linux / macOS / Windows", size=17, bold=True, center=True)
    d.arrow([(405, 220), (265, 220), (265, 405)])
    d.text(135, 275, "Use browser", size=16)
    d.arrow([(795, 220), (915, 220), (915, 405)])
    d.text(935, 275, "Launch", size=16)
    d.box(70, 405, 430, 330, "Existing web browser", "Owner views: everyday vehicle data\nOverview · Charging · Health · Location\nSource freshness + valid tire readings\nSeparate developer explorer + JSON\nReact + shared Rust/WASM validation\nLocal capability in tab session storage\nRivian tokens never enter the browser", fill=BLUE)
    d.box(700, 405, 430, 330, "my-rivian-data executable", "Native Rust / Axum + embedded assets\nNative identity + vehicle ownership\nFixed reads + live WebSocket telemetry\n11 admitted RVMs; drop unrequested\nAccount epoch + worker cancellation\nTokens: memory-only by default\nExplicit OS-vault opt-in; no passwords", fill=GREEN)
    d.arrow([(500, 490), (700, 490)])
    d.text(600, 443, "Local requests", size=15, center=True)
    d.text(600, 466, "127.0.0.1:PORT", size=14, center=True)
    d.arrow([(700, 665), (500, 665)])
    d.text(600, 618, "Assets +", size=15, center=True)
    d.text(600, 641, "vehicle data", size=15, center=True)
    d.text(65, 765, "Host + Origin checks; cookie + app capability; CSRF on unsafe requests.", size=18, color=MUTED)

    d.box(70, 815, 560, 165, "Optional remembered session · native OS vault", "Linux Secret Service · macOS Keychain\nWindows Credential Manager\nOnly after explicit owner opt-in; no passwords.\nRestoration rechecks session, identity and ownership.", fill=GREEN)
    d.arrow([(730, 735), (730, 795), (530, 795), (530, 815)])
    d.box(700, 1100, 430, 165, "Rivian services", "Fixed HTTPS + WSS destinations\nSign-in, verification and session state\nAccount / vehicle reads + Parallax\nConfirmed vehicle nickname change", fill=BLUE)
    d.arrow([(820, 735), (820, 1100)])
    d.text(850, 1044, "Verified TLS", size=16)
    d.arrow([(1050, 1100), (1050, 735)])
    d.box(40, 1100, 530, 165, "Explicit --demo mode", "Fictional vehicles and fixed sample readings.\nNo credential entry or outbound Rivian traffic.\nFailed live requests never become demo data.\nPhysical controls and enrollment remain blocked.", fill=AMBER)
    d.box(40, 1310, 1120, 90, "Phase 2: Tauri on Android / iOS · not implemented", "Reuse native Rust + React through native invocations, without an Axum server on the phone.", fill=AMBER, dashed=True)
    d.footer("Unverified: real accounts, cross-OS acceptance, release signing and independent Anthropic acceptance.")
    return d.save()


def dataflow():
    d = Diagram("data-flow.png", "Connect, read, and receive live updates", "Native Rust owns account authority; WASM validates bounded, non-secret inputs only.", height=2050)
    d.text(40, 162, "1  ESTABLISH A LOCAL SESSION", size=16, bold=True)
    d.box(40, 205, 335, 185, "Native launcher", "Create one-use capability.\nOpen browser URL fragment.\nExpiry: five minutes.\nNever print it by default.", fill=GREEN)
    d.box(435, 205, 330, 185, "Browser startup", "Scrub fragment immediately.\nPOST /api/bootstrap.\nKeep local app capability\nin tab session storage.", fill=BLUE)
    d.box(825, 205, 335, 185, "Native bootstrap", "Consume capability once.\nReturn HttpOnly cookie,\napp capability and CSRF.\nNo Rivian tokens in reply.", fill=GREEN)
    d.arrow([(375, 292), (435, 292)])
    d.arrow([(765, 292), (825, 292)])
    d.text(40, 419, "Protected calls require cookie + app capability; unsafe calls also require CSRF.", size=17)

    d.text(40, 481, "2  CONNECT YOUR RIVIAN ACCOUNT", size=16, bold=True)
    d.box(40, 525, 335, 225, "Local sign-in form", "Send email / password to host.\nComplete verification if asked.\nMemory-only is the default.\nOptional: Remember sign-in.\nNo password is saved.", fill=BLUE)
    d.box(435, 525, 330, 225, "Native authentication", "Authenticate / verify account.\nTokens remain native.\nExplicit opt-in uses OS vault.\nRestore opted-in bundle:\nvalidate / refresh session,\nrecheck identity + ownership.", fill=GREEN)
    d.box(825, 525, 335, 225, "Rivian account service", "Fixed HTTPS destinations.\nSign-in / verification / session.\nIdentity and owned vehicles.\nFailure returns a safe error.\nNo invented refresh exchange.", fill=BLUE)
    d.arrow([(375, 615), (435, 615)])
    d.arrow([(765, 615), (825, 615)])
    d.text(40, 779, "--session-only skips remembered-entry reads and writes; restore failure allows local sign-in.", size=17)

    d.text(40, 839, "3  AUTHORIZE AND RUN AN ADMITTED OPERATION", size=16, bold=True)
    d.rect(40, 880, 525, 390, fill="#edf3f9")
    d.rect(635, 880, 525, 390, fill="#edf2e9")
    d.text(62, 897, "BROWSER  /  React + Rust/WASM", size=16, bold=True)
    d.text(657, 897, "NATIVE HOST  /  Rust + Axum", size=16, bold=True)
    d.box(65, 940, 475, 180, "A  Submit a bounded request", "Owner feature or developer fixed operation.\nWASM validates non-secret inputs only.\nExact-document import asks native admission.\nBrowser validation grants no authority.", fill=BLUE)
    d.box(660, 940, 475, 180, "B  Reauthorize before execution", "Recheck local session + native account epoch.\nValidate fixed operation + bounded variables.\nExact documents resolve to existing local IDs.\nCheck ownership; never proxy imported queries.\n33 fixed reads / confirmed rename only.", fill=GREEN)
    d.arrow([(540, 1017), (660, 1017)])
    d.box(660, 1150, 475, 105, "C  Dispatch fixed request", "Fixed document + HTTPS route; bounded replies.\nPreserve missing values + source times.", fill=GREEN)
    d.arrow([(898, 1120), (898, 1150)])
    d.box(65, 1150, 475, 105, "D  Show the requested data", "Owner cards or developer response JSON.\nDiscard older vehicle / view results.", fill=BLUE)
    d.arrow([(660, 1202), (540, 1202)])
    d.text(40, 1294, "244 fixed catalog entries / 234 source surfaces; catalog breadth is not dispatch authority.", size=17)

    d.text(40, 1350, "4  RECEIVE LIVE VEHICLE UPDATES", size=16, bold=True)
    d.box(40, 1390, 530, 190, "Native WebSocket worker", "Fixed WSS endpoint; verified TLS.\nRequest and admit only 11 reviewed RVMs.\nDrop unrequested RVMs before retention.\nCancel on selected vehicle / account change.\nEpoch-check before publishing; bounded retries.", fill=GREEN)
    d.box(630, 1390, 530, 190, "Owner readings + developer topics", "Poll protected native snapshots.\nMerge only newer source times per field.\nShow valid tire readings; missing stays unavailable.\nEach reading keeps its own reported age.\nNo personal-reading cache on disk.", fill=BLUE)
    d.arrow([(570, 1485), (630, 1485)])
    d.box(40, 1620, 1120, 135, "Vehicle nickname · explicit confirmation", "Review exact vehicle and name; one-use native ticket expires after two minutes.\nTicket binds variables + account epoch; ownership is rechecked; no automatic write retry.\nPhysical controls, phone/key enrollment and signed commands remain blocked.", fill=AMBER)
    d.box(40, 1795, 1120, 155, "Finish or disconnect", "End session / Disconnect account revokes authority, cancels telemetry and attempts vault deletion.\nVault deletion failures are reported separately; remote revocation may fail when offline.\nClose app / ordinary process exit clears native memory and preserves opted-in remembered sessions.", fill=AMBER)
    d.footer("Unverified: real-account reads / telemetry / rename, cross-OS delivery, signing and independent Anthropic acceptance.")
    return d.save()


def render():
    return [context(), dataflow()]


if __name__ == "__main__":
    for path in render():
        print(path.name)
