# Packaged Phase I evidence — October 15, 2026

Benchmarked runtime source: [`752677633c37710ee9766187414a1788545c99dc`](https://github.com/asanderson/my-rivian-data/commit/752677633c37710ee9766187414a1788545c99dc). Later Windows delivery fixes and evidence commits do not change the native application or UI code measured here. This is an unsigned Linux development build and synthetic workload, not real-account or full desktop release acceptance.

The clean-source package rebuilt the embedded executable. Outside-checkout extraction verified all payload hashes, the version, embedded JavaScript/WASM and protected demo execution. Its exact source archive and executable digests are recorded in [verification.json](verification.json); the benchmark independently matched that package's `BUILD.json` and source-archive digest.

| Measurement | Result |
| --- | --- |
| Workload | 120 seconds; two workers; fixed synthetic vehicle-state request |
| Requests | 122,338; 0 failed |
| Latency p50 / p95 / p99 | 1.679 / 2.542 / 4.339 ms |
| Sampled native peak RSS | 5.07 MiB |
| Mean native CPU, one core = 100% | 14.09% |
| Native executable | 7.41 MiB |
| Compressed portable archive | 8.80 MiB |
| Extracted payload, including source/docs/notices | 18.72 MiB |

The native process is measured separately from Chromium and the Node driver. RSS is sampled; percentiles use the bounded sample stride recorded in [benchmark-summary.json](benchmark-summary.json). Browser memory, account/network workloads, energy/battery use, sustained multi-day behavior and other operating systems were not measured. This local binary's observed GNU libc floor is 2.39; native release packages are built separately on the documented 2.35 baseline.

[benchmark-raw.json.gz](benchmark-raw.json.gz) retains the exact numeric `raw.json` observations compressed with gzip (uncompressed SHA256 `39d8f164b6e6de3a4a75755961a66fbe22b87350d28247a75ff10d1c4e08c7fe`). It contains no request/response bodies, credentials, capabilities or private owner data. Decompress it with a gzip-compatible tool to inspect the native process and latency samples.

Automated verification passed 139 Rust, 44 UI and 14 delivery tests, compiled native/WASM parity, strict checks and both Chromium suites. The live browser suite uses fake replies and tests the native process shutdown. See [STATUS](../../STATUS.md), the [completion ledger](../../PHASE-I-COMPLETION.md) and [acceptance process](../../ACCEPTANCE.md) for the still-open real-account, OS-vault, clean-machine, signing and independent-provider gates.

Final delivery source [`2240fec52f0018ff6f7f8b7f08cbad3d1f9174a8`](https://github.com/asanderson/my-rivian-data/commit/2240fec52f0018ff6f7f8b7f08cbad3d1f9174a8) passed Linux x64, Windows x64 and both macOS architecture jobs, including native package checks and synthetic benchmarks. Windows additionally passed actual NSIS install/reinstall/tamper/uninstall fixtures under Restricted client process policy and bounded native-process termination. [platform-ci.json](platform-ci.json) records the exact run, jobs, completion times and limits. These unsigned CI checks are distinct from owner-device and signed-release acceptance.
