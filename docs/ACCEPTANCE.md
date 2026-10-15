# Desktop and owner acceptance

Automated tests and synthetic measurements do not establish compatibility with an
owner's real account, model, firmware or every desktop environment. Record actual
run results in [implementation status](STATUS.md); leave unrun checks explicit.

## Automated artifact checks

After a clean-source package build, `node scripts/check-packages.mjs` verifies
every artifact checksum and extracted portable payload and launches the packaged
executable with `--version`. On Windows it installs silently into the current
user's profile, verifies the full installed payload and Start Menu shortcut target,
launches that executable and uninstalls the test build. On macOS it mounts the DMG
read-only, checks the app plist/resources and executes its bundled binary; signed
builds also verify bundle signatures. On Linux it validates the desktop entry,
extracts the DEB and tests its executable/payload, then installs, verifies and
removes the RPM in an isolated package root/database. These checks do not simulate
a downloaded-file security prompt, interactive drag/install or browser behavior.

Before marking a target accepted, manually verify: installer/DMG opens; expected
OS trust prompt identifies the actual publisher; mouse launch opens the local
app; demo owner views render; real login stays in the local app; Close app stops the
backend; update preserves a verified previous build; rollback opens it; uninstall
removes only the selected build; and a modified download/payload is rejected.
Exercise fresh Windows 10/11 profiles, Intel and Apple Silicon Macs, and Linux
on the documented GNU libc baseline. Record an actual result for each target.

## Synthetic backend measurements

```text
node scripts/benchmark.mjs --demo --duration-seconds 120 --concurrency 2 --sample-interval-ms 500
node scripts/benchmark.mjs --demo --duration-seconds 3600 --concurrency 2 --sample-interval-ms 1000
```

The runner launches a child backend in explicit demo mode, captures the one-use
launch capability only in memory, establishes its protected local session, and
repeats a fixed synthetic vehicle-state request. It never signs in or contacts
Rivian. Options allow 1–86,400 seconds, 1–32 workers and 100–60,000 ms sampling.
`--binary PATH` selects a locally built executable. To bind the run to a packaged source revision, select the executable inside the verified package directory (beside its `BUILD.json` and `source.tar.gz`), rather than the build-cache copy in `target/release`. A long run still needs an
actual uninterrupted run; a configured one-hour command is not evidence of it.

Reports under `artifacts/benchmarks/` include `raw.json` and `summary.json`:
executable SHA256 and application version, package source revision only when a
matching `BUILD.json` is present (otherwise explicitly unknown), archive/payload
bytes only when package provenance matches the selected executable, health/session startup time,
request status counts/rate and round-trip latency, backend RSS and CPU samples.
Linux reads the verified backend's `/proc` identity (including PID namespaces),
macOS uses `ps`, and Windows uses `Get-Process`. Browser/driver memory and CPU are
excluded. Samples are bounded to 10,000 deterministic observations; request
counts/minimum/maximum/mean include all requests, while reported percentile stride
explains any downsampling. CPU uses one core as 100%; RSS is sampled, not a proven
maximum. Energy/battery impact is unmeasured.

Reports contain numeric statuses and timings, not launch URLs, capabilities,
cookies, request/response bodies, credentials or native diagnostics. Compare
identical workloads/builds/hardware and keep raw reports with the summary. No
performance threshold or cross-platform resource claim is inferred from one host.

## Real owner account acceptance

Run only when the owner intends to test their own account:

```text
node scripts/acceptance.mjs
```

The helper launches the ordinary local application and opens the owner's default
browser. The owner manually signs in and completes verification **in that app**.
Use the app's owner views and developer explorer to check the admitted reads.
Verify ownership rejection, MFA/recovery, logout/forget and telemetry availability
without recording account identifiers, VINs, locations or responses. Unavailable
or blocked operations remain explicit; never execute a vehicle command to test
read-only acceptance. Subscription connection success does not prove every field.

The interactive helper asks only for an allowlisted model, numeric dotted firmware
version and each catalog operation's `passed`, `failed`, `blocked` or `not-tested`
outcome. It displays the complete sanitized record and saves only after the owner
types `yes`. Records are written under `artifacts/acceptance/` and contain those
approved fields plus executable SHA256/version, matching package source revision
or explicit unknown provenance, and a fingerprint of that executable's
credential-free catalog export. No credentials, MFA codes, vehicle IDs, network transcripts,
free-text notes or screenshots are collected. The helper never logs in or queries
the live account itself. Its child output is discarded and its backend is stopped
when the helper exits.

`passed` means the owner actually observed that operation work on the stated
model/firmware. `blocked` is a capability restriction, and `not-tested` is no
evidence. Review the sanitized record before including it in any public artifact;
a test outcome must never be fabricated to close an acceptance gap.
