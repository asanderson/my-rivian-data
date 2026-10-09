"""System context and live read flow, derived from the implemented boundaries."""

from drawing import Diagram, GREEN, BLUE, AMBER, MUTED


def context():
    d = Diagram("system-context.png", "System context", "Your vehicles. Your data.  |  One local application; direct Rivian account access.", height=1250)
    d.box(405, 170, 390, 105, "Rivian owner", "Launch, sign in and choose a vehicle.", fill=GREEN)
    d.rect(40, 325, 1120, 455, fill="#edf2e9")
    d.text(600, 347, "OWNER’S COMPUTER  ·  Linux / macOS / Windows", size=17, bold=True, center=True)
    d.arrow([(405, 220), (265, 220), (265, 405)])
    d.text(135, 275, "Use browser", size=16)
    d.arrow([(795, 220), (915, 220), (915, 405)])
    d.text(935, 275, "Launch", size=16)
    d.box(70, 405, 430, 295, "Existing web browser", "Owner views: everyday vehicle data\nDeveloper explorer: requests + JSON\n\nReact + shared Rust/WASM validator\nLocal capability: tab session storage\nRivian tokens never returned here", fill=BLUE)
    d.box(700, 405, 430, 295, "my-rivian-data executable", "Native Rust / Axum host\nEmbedded UI + WASM assets\nSession and vehicle authorization\nBounded, allowlisted API reads\nRivian tokens: native memory only\nNo saved password or history", fill=GREEN)
    d.arrow([(500, 490), (700, 490)])
    d.text(600, 443, "Local requests", size=15, center=True)
    d.text(600, 466, "127.0.0.1:PORT", size=14, center=True)
    d.arrow([(700, 625), (500, 625)])
    d.text(600, 578, "Assets +", size=15, center=True)
    d.text(600, 601, "vehicle data", size=15, center=True)
    d.text(65, 729, "Cookie + per-instance capability on every protected request; CSRF on writes.", size=18, color=MUTED)
    d.box(700, 855, 430, 180, "Rivian API", "Fixed HTTPS destinations\nSign-in + verification + session state\nAccount and vehicle read operations\nUndocumented community contracts", fill=BLUE)
    d.arrow([(820, 700), (820, 855)])
    d.text(860, 810, "Verified TLS", size=16)
    d.arrow([(1050, 855), (1050, 700)])
    d.box(70, 855, 560, 180, "Explicit --demo mode", "Fictional vehicles and fixed sample readings.\nNo credential entry or outbound Rivian traffic.\nFailed live requests never become demo data.\nNo signed controls or continuous subscriptions.", fill=AMBER)
    d.box(40, 1080, 1120, 90, "Later: Tauri on Android / iOS", "Reuse Rust + React through native invocations; no Axum server on the phone.", fill=AMBER, dashed=True)
    d.footer("Independent community project. Real-account acceptance and signed consumer packages remain unverified.")
    return d.save()


def dataflow():
    d = Diagram("data-flow.png", "Launch, sign in, and read vehicle data", "Native Rust owns account authority; WASM validates non-secret inputs only.", height=1660)
    d.text(40, 162, "1  ESTABLISH A LOCAL SESSION", size=16, bold=True)
    d.box(40, 205, 335, 185, "Native launcher", "Create one-use capability.\nOpen browser URL fragment.\nExpiry: five minutes.\nNever print it by default.", fill=GREEN)
    d.box(435, 205, 330, 185, "Browser startup", "Scrub fragment immediately.\nPOST /api/bootstrap.\nRetain local app capability\nin tab session storage.", fill=BLUE)
    d.box(825, 205, 335, 185, "Native bootstrap", "Consume capability once.\nReturn HttpOnly cookie,\napp capability and CSRF.\nNo Rivian tokens in reply.", fill=GREEN)
    d.arrow([(375, 292), (435, 292)])
    d.arrow([(765, 292), (825, 292)])
    d.text(40, 419, "Protected calls require cookie + app capability; unsafe calls also require CSRF.", size=17)

    d.text(40, 481, "2  SIGN IN TO RIVIAN", size=16, bold=True)
    d.box(40, 525, 335, 190, "Local sign-in form", "Enter email and password.\nSubmit to native host.\nComplete verification if asked.\nNever store account secrets.", fill=BLUE)
    d.box(435, 525, 330, 190, "Native authentication", "Check local authority.\nCreate app / user session.\nHandle pending verification.\nKeep tokens in native memory.", fill=GREEN)
    d.box(825, 525, 335, 190, "Rivian account service", "Fixed HTTPS GraphQL routes.\nPassword / OTP validation.\nAccount vehicles and roles.\nFailure returns a safe error.", fill=BLUE)
    d.arrow([(375, 615), (435, 615)])
    d.arrow([(765, 615), (825, 615)])
    d.text(40, 744, "No remembered sign-in. A rejected user session requires the owner to sign in again.", size=17)

    d.text(40, 811, "3  AUTHORIZE AND RUN AN ADMITTED READ", size=16, bold=True)
    d.rect(40, 850, 525, 505, fill="#edf3f9")
    d.rect(635, 850, 525, 505, fill="#edf2e9")
    d.text(62, 866, "BROWSER  /  React + Rust/WASM", size=16, bold=True)
    d.text(657, 866, "NATIVE HOST  /  Rust + Axum", size=16, bold=True)
    d.box(65, 910, 475, 145, "A  Validate the requested read", "Owner feature or developer operation.\nBound inputs; validate in WASM.\nCompare with native /api/validate result.", fill=BLUE)
    d.box(660, 910, 475, 145, "B  Reauthorize before execution", "Check local session after body extraction.\nRevalidate the exact operation and variables.\nConfirm vehicle belongs to this account.", fill=GREEN)
    d.arrow([(540, 987), (660, 987)])
    d.box(660, 1140, 475, 165, "C  Query Rivian and normalize", "Use fixed operation document + HTTPS route.\nBound replies and sanitize errors.\nKeep missing fields null and source times.\nOnly explicit --demo reads use fixtures.", fill=GREEN)
    d.arrow([(898, 1055), (898, 1140)])
    d.box(65, 1140, 475, 165, "D  Show the requested data", "Owner cards or developer response JSON.\nDiscard results from older vehicle/view.\nBound local replies to 3 MiB.\nDo not cache personal readings to disk.", fill=BLUE)
    d.arrow([(660, 1220), (540, 1220)])
    d.text(40, 1380, "Invalid sessions, unowned vehicles and blocked operations stop before upstream dispatch.", size=17)
    d.box(40, 1425, 1120, 135, "End the local session", "Logout revokes local authority, clears the tab capability and discards native account tokens.\nRemote logout is attempted; offline conditions can prevent Rivian-side revocation.\nStopping the native process ends access. Browser/OS compromise is outside this boundary.", fill=AMBER)
    d.footer("Scope: implemented live reads and explicit demo mode. Diagrams contain no real accounts, credentials or vehicle data.")
    return d.save()


def render():
    return [context(), dataflow()]


if __name__ == "__main__":
    for path in render():
        print(path.name)
