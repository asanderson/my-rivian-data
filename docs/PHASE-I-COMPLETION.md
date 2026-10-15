# Phase I completion ledger

Source baseline: `563eac3080ad42066be9ae3f1cfb3ba20e11dfb3` (merged PR #5).
Authorization: the original approved October 7 plan, version 4, and October 14 instruction to finish Phase I.
Design and task details are under `docs/superpowers/`.

## Decisions

- Ruling: preserve the approved architecture and proceed without repeating design approval — the owner explicitly instructed execution of the existing Phase I plan.
- Ruling: enrollment-dependent physical commands remain cataloged and blocked until desktop enrollment is proven — this is the original plan's stated boundary, not a reduction introduced by implementation.
- Ruling: opt-in OS protected credential storage supersedes the earlier memory-only milestone rule; memory-only remains default and plaintext fallback remains prohibited.
- Ruling: separate implementation work by file ownership, using the available scoped-agent capacity; root integrates native authority and runs full verification.

- Ruling: tire validity uses documented proto3 control default zero (valid); physical readings still require wire presence — independent reviewer confirmed pinned schema and withdrew the generic missing-tag concern.

## Implementation and review status

- [x] Clean baseline verified on untouched main: release build, strict checks, 51 Rust tests, 28 UI tests, 37 native/WASM parity fixtures.
- [x] Portable current-schema Parallax decoding and partial-state merge.
- [x] Native authenticated stream lifecycle and owner/developer integration.
- [x] Comprehensive versioned known-operation inventory and guarded writes.
- [x] Protected remembered sessions and bounded MFA lifecycle.
- [x] Desktop installers, SBOM, signing/rollback pipeline and acceptance tools.
- [x] Resource measurements and exact workload evidence.
- [x] Source/security review findings fixed and verification complete.
- [x] Completion PR published: [PR #6](https://github.com/asanderson/my-rivian-data/pull/6).

## Review findings and evidence

Independent-context same-provider review identified account-generation races during delayed request bodies, remembered-store I/O, invalidation and confirmed mutation dispatch; all are bound to the original admission/ticket epoch, with compare-and-swap transitions and post-lock checks. Native telemetry retains its authentication-failure source epoch internally, drops stale snapshots, rejects unrequested topics before retention, and guards delayed stream stop/start against newer account workers. Browser merges compare per-field source ages. Persistence deletion failures remain visible without restoring prior data. Intrinsic MFA expiry and finite retries/resends are enforced. Reproductions failed before the fixes and passed after them.

The October 14 advisory scan found four Rustls advisories affecting the initial 0.23.45 lock. The lock was updated to patched 0.23.46; the repeated scan reported zero vulnerabilities and warnings, with no ignore exceptions. npm reported zero vulnerabilities. Exact database/lock digests and results are in [advisory evidence](evidence/2026-10-14/advisory-scans.json). These results describe the scanned snapshot only.

Ruling: the branch-completion skill menu does not require a new integration decision. The owner already authorized GitHub pull requests in this session; publish the reviewed branch against `main`, preserve the worktree, and do not merge or publish a release.

A fresh final review additionally reproduced delayed disconnect cleanup affecting a newer login, vault write timeout invalidating otherwise valid memory sessions, and missing command ambiguity after dispatch cancellation. Logout now returns its exact CAS epoch; the host cleans only the source epoch and preserves newer tickets/workers. Vault writes use an acknowledgment/cancellation handoff under their own serialized gate, preserving account authority while deleting unaccepted writes. Canceled dispatched commands report unknown outcome and are never replayed. All reproductions passed after failing first; focused re-review found no remaining P1/P2.

Final Linux verification: 139 Rust tests (71 API, 37 core, 3 confirmation, 21 HTTP security, 7 WASM), 44 UI tests, 14 delivery tests, strict checks, compiled parity and both browser suites. Native platform CI exposed an isolated-RPM-database dependency verification mistake and a Windows mandatory-lock test assumption; these were corrected without weakening payload checks or writer exclusion. Actual owner/OS-vault acceptance remains separate.

Final implementation commit `2240fec52f0018ff6f7f8b7f08cbad3d1f9174a8` passed all four native CI jobs: Linux x64, Windows x64, macOS Intel and macOS ARM. Each completed application verification, packaging, package checks and a synthetic backend benchmark; Linux also completed the browser suites. Exact job links, completion times and scope are retained in [platform CI evidence](evidence/2026-10-15/platform-ci.json).

Windows review confirmed that scripted installer error dialogs lacked silent defaults; these now exit with distinct nonzero codes and preserve verifier diagnostics outside the payload. Lifecycle checks have stage logs and bounded native-process/payload-removal waits. A diagnostic rerun exposed incompatible PowerShell module discovery after `pwsh` launches Node and Node launches Windows PowerShell. The checker and verifier now load only their matching host's bundled modules. The passing Windows-only regressions cover a native failure/timeout and real NSIS install, reinstall rejection, tamper rejection and uninstall; they run before the Rust build. The full application installer lifecycle also passed after these fixes.

The NSIS regression also forces a Restricted client process policy: intact payloads must install and modified payloads must exit with integrity error 11. The bundled verifier uses a process-only policy as documented for application-embedded scripts; machine/user policy is not changed and Group Policy remains authoritative. Signing subprocesses discard inherited PowerShell module paths so matching defaults, including certificate modules, are reconstructed. Real-certificate signing acceptance remains open.

Packaged runtime revision: `752677633c37710ee9766187414a1788545c99dc`. Its clean-source portable package passed extracted payload/embedded UI/WASM/protected-demo checks. The 120-second workload completed 122,338 requests with zero failures; [exact measurements, raw samples and provenance](evidence/2026-10-15/README.md) are retained. All implementable Phase I scope is delivered in PR #6; the release acceptance items below remain open.

## External release acceptance

- [ ] Owner-run real Rivian authentication/read validation and representative vehicle/firmware matrix.
- [ ] Legitimate desktop key enrollment and physical-command completion evidence, where supported.
- [ ] Actual platform signing/notarization using authorized owner-provided certificates.
- [ ] Clean-machine installer/update/uninstall acceptance on the supported desktop matrix.
- [ ] Independent-provider adversarial review and re-review of frozen commits.
- [ ] Owner release decision after evidence and residual-risk disposition.

These gates cannot be satisfied by mock tests, installer source files, CI builds or same-provider agent review alone.
