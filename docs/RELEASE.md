# Desktop packaging and release

These scripts produce reviewable desktop artifacts. Configuration is not evidence
that a Windows/macOS installation, signing, notarization or real owner account
has passed. See [acceptance](ACCEPTANCE.md) and [implementation status](STATUS.md).
The ordinary verification workflow produces explicitly unsigned development
artifacts. No workflow creates tags, creates releases or publishes downloads.

## Supported build targets and artifacts

| Target | CI build image | Output | Runtime baseline to validate |
| --- | --- | --- | --- |
| Windows x64 | `windows-2025` | Per-user NSIS `-setup.exe`, portable `.tar.gz` | Windows 10/11 x64, default desktop browser |
| macOS Intel x64 | `macos-15-intel` | `My Rivian Data.app` inside `.dmg`, portable `.tar.gz` | macOS 13 or newer, default desktop browser |
| macOS Apple Silicon arm64 | `macos-15` | Native arm64 `.app` inside `.dmg`, portable `.tar.gz` | macOS 13 or newer, default desktop browser |
| Linux x64 | `ubuntu-22.04` | `.deb`, `.rpm`, portable `.tar.gz` | GNU libc 2.35 or newer, `xdg-utils`, desktop browser |

The Linux packager inspects required GNU libc symbol versions and rejects native
packages above the 2.35 floor. Portable development archives record their observed
floor, which can be newer when built locally. Linux ARM and universal macOS
binaries are outside this matrix. Browser/device/interactive OS compatibility
requires the acceptance checklist; successful compilation does not establish it.

## Build from the exact source revision

Install the repository's pinned Rust toolchain, Node 24+, matching WASM CLI and
locked dependencies with `node scripts/setup.mjs`. Native packaging additionally
requires NSIS 3.12 on Windows, Xcode Command Line Tools on macOS, and
`dpkg-deb`, `rpmbuild`, `readelf` and `desktop-file-validate` on Linux. CI installs
the missing NSIS/RPM/desktop-file tools. All subprocesses use separate arguments.

```text
node scripts/verify.mjs
node --test scripts/tests/delivery.test.mjs scripts/tests/measurement.test.mjs scripts/tests/release.test.mjs
node scripts/package.mjs
node scripts/check-packages.mjs
```

Commit the intended source first. Packaging rejects a dirty tree, rebuilds the
native executable and embedded UI/WASM from clean `HEAD` with the locks, and
checks that the revision/tree stayed unchanged. Existing cached executables are
never accepted as proof of source identity. `--portable-only` produces an unsigned
development archive when native packaging tools are unavailable; it cannot be
combined with `--signed`. For that explicit development mode, run
`node scripts/check-packages.mjs --portable-only` to verify hashes, the extracted
payload, embedded UI/WASM and protected demo execution while skipping native
installer checks.

Each payload contains the GPLv3 license, notices/license files, complete project
source as `source.tar.gz`, build scripts and locks, docs, `BUILD.json`, both SBOMs
and a `FILES.json` SHA256 inventory. Each external artifact has a `.sha256` file;
the platform `.SHA256SUMS` binds the artifact filenames and digests. `BUILD.json`
records source revision, source/lock/executable digests, target, tool versions and
CI run provenance when available. Byte-for-byte reproducibility is not claimed.

`SBOM.cyclonedx.json` (CycloneDX 1.6) and `SBOM.spdx.json` (SPDX 2.3) inventory
every locked Cargo/npm package, including build/test, optional and other-platform
dependencies. Cargo SHA256 and npm SRI archive digests are copied exactly from
the locks. npm build dependencies are explicitly excluded from runtime scope;
the comprehensive inventory is not a deployed-only dependency graph. License
expressions copied from notices/npm metadata remain metadata, not a legal audit.

## Install, update, rollback and remove

Quit the native app before switching versions or removing an installation. Use
the application's **Close app** control when available; closing a browser tab
alone does not stop its backend. This project has no remote auto-updater and
never downloads arbitrary update URLs or replaces its own running executable.

- **Windows:** open the setup executable. It requests ordinary user privileges,
  installs under `%LOCALAPPDATA%\My Rivian Data\versions\VERSION-REVISION`, checks
  every payload digest, and registers per-user Start Menu shortcuts and uninstall
  entries. Open the current shortcut to launch the browser. New builds use new
  directories and preserve old builds. For rollback, quit and open the prior
  version's named Start Menu shortcut. Uninstall a specific build through Windows
  Apps or its named uninstaller; other build directories stay intact. Exact-build
  reinstall is rejected until that build has been uninstalled.
- **macOS:** open the DMG and drag the app to Applications (or `~/Applications`
  for a per-user installation); open the app. Keep a verified copy of the previous
  app/DMG before updating. Quit, move the current app aside, and copy the verified
  replacement. To roll back, quit and restore that prior app. To uninstall, quit
  and move that app to Trash. Unsigned development artifacts can be blocked by
  Gatekeeper; they are not production signed releases.
- **Linux:** verify the package digest first, then use `sudo apt install ./FILE.deb`
  or `sudo dnf install ./FILE.rpm`. Launch My Rivian Data from the desktop menu or
  `my-rivian-data` in a terminal. Retain the previous verified package; after quit,
  use apt to install that exact local version with `--allow-downgrades`, or dnf
  `downgrade ./PREVIOUS.rpm`. Remove with `sudo apt remove my-rivian-data` or
  `sudo dnf remove my-rivian-data`. Releases must increase the workspace version;
  Git suffixes identify development builds and do not define chronological order.

Portable packages can be extracted and launched directly. Contributors with
Node can also use the verified local-only installer below; obtain the expected
digest through an authenticated release channel, not just the neighboring
unsigned checksum. Installation checks archive paths/types and full payload
integrity before selecting a new version. No prior version files are overwritten.

```text
node scripts/install-portable.mjs install DIRECTORY LOCAL_ARCHIVE EXPECTED_SHA256
node scripts/install-portable.mjs launch DIRECTORY
node scripts/install-portable.mjs rollback DIRECTORY
node scripts/install-portable.mjs uninstall DIRECTORY
```

## Optional signed release candidates

`release.yml` is manual-only and uses the `desktop-release` environment. Before
building, it requires **all** certificate, notarization and GPG secrets. Missing
secrets, failed signatures, rejected notarization, a missing target or differing
source revisions block the final signed candidate. The unsigned PR workflow
continues to work without those credentials. Do not describe its output as signed.

| CI secret | Purpose |
| --- | --- |
| `RIVIAN_WINDOWS_PFX_BASE64`, `RIVIAN_WINDOWS_PFX_PASSWORD` | Existing code-signing certificate/private key; no certificate purchase is performed |
| `RIVIAN_TIMESTAMP_URL` | HTTPS RFC3161 timestamp service selected by the maintainer |
| `RIVIAN_MAC_P12_BASE64`, `RIVIAN_MAC_P12_PASSWORD`, `RIVIAN_MAC_IDENTITY` | Existing Developer ID Application identity |
| `RIVIAN_APPLE_ID`, `RIVIAN_APPLE_TEAM_ID`, `RIVIAN_APPLE_APP_PASSWORD` | Existing Apple notarization account credentials |
| `RIVIAN_RELEASE_GPG_KEY`, `RIVIAN_RELEASE_GPG_PASSPHRASE` | One armored release private key and its passphrase |

Secrets stay in CI environment variables and temporary private certificate/key
files. Commands containing secret inputs and native error output are not logged;
temporary keys are removed after packaging. Windows verifies SHA256 Authenticode
signatures for the executable, generated uninstaller and final installer. macOS
uses Developer ID, hardened runtime and timestamps, then waits for accepted
`notarytool` submissions, staples/validates the app and DMG, and checks Gatekeeper.
No code-signing or notarization success can be established without actual keys.

The aggregate job requires matching Windows/macOS native-signature metadata and
all four target revisions, checks artifact hashes, and signs
`RELEASE-SHA256SUMS` with GPG. Linux authenticity comes from this detached manifest
signature. The exported public key is a convenience; verify its fingerprint via
an independently trusted channel. A public key bundled with an untrusted download
does not establish publisher identity.

```text
gpg --verify RELEASE-SHA256SUMS.asc RELEASE-SHA256SUMS
sha256sum --check RELEASE-SHA256SUMS
```

Only a future explicit maintainer decision may promote a verified candidate to a
stable public release, after the acceptance evidence is reviewed. The workflows
configured here have no publication step or `contents: write` permission.

## Primary packaging references

- [NSIS scripting reference](https://nsis.sourceforge.io/Docs/Chapter4.html) and [CreateShortCut](https://nsis.sourceforge.io/Reference/CreateShortCut).
- [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution) and [custom workflows](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow).
- [Microsoft SignTool](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool).
- [Debian package control fields](https://www.debian.org/doc/debian-policy/ch-controlfields) and [RPM spec format](https://rpm.org/docs/4.20.x/manual/spec.html).
- [GitHub-hosted runner labels/architectures](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
- [CycloneDX JSON schema](https://github.com/CycloneDX/specification/blob/master/schema/bom-1.6.schema.json) and [SPDX package verification](https://spdx.github.io/spdx-spec/v2.3.1/how-to-use/).
