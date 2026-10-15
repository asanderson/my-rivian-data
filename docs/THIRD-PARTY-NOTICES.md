# Third-party notices

This index covers every registry package in the locked Cargo dependency graph,
including target-specific, optional and development dependencies, and every
package in the npm lock. It is deliberately broader than the dependencies linked
into one desktop executable. Original published license, copyright, AUTHORS and
notice files are retained verbatim under `dependency-licenses/`; nested notice
paths are preserved. For Cargo packages that omit license files from their
published archive, pinned upstream or owning-project license files and provenance
are included. Declared terms below come from Cargo package metadata or the npm
lock, without inventing missing license grants.

The bundled React/react-dom/scheduler runtime notices are included. npm build
packages are not distributed as dependency binaries in the desktop payload;
their available installed notice files are included for source-build attribution.
Optional npm packages for other platforms that are not installed on this host
are indexed from the lock as metadata only. This does not claim that absent
optional build-package notice files were retrieved or that those binaries are
redistributed. Cargo package archive digests and npm integrity digests are recorded
in the accompanying locked-manifest SBOMs.

The Rust 1.99.0 library notices are in `dependency-licenses/rust-1.99.0/`.
Project source uses the repository's GNU GPLv3 license (`GPL-3.0-only`);
these notices preserve the separate terms of third-party dependencies. Retained
historical notice directories may exceed the active lock inventory; their
presence is not a claim that those versions are linked into the current build.

| Component | Version | Declared license | Included notices |
| --- | --- | --- | --- |
| aes | 0.8.4 | MIT OR Apache-2.0 | [files](dependency-licenses/aes-0.8.4/) |
| async-broadcast | 0.7.2 | MIT OR Apache-2.0 | [files](dependency-licenses/async-broadcast-0.7.2/) |
| async-channel | 2.5.0 | Apache-2.0 OR MIT | [files](dependency-licenses/async-channel-2.5.0/) |
| async-io | 2.6.0 | Apache-2.0 OR MIT | [files](dependency-licenses/async-io-2.6.0/) |
| async-lock | 3.4.2 | Apache-2.0 OR MIT | [files](dependency-licenses/async-lock-3.4.2/) |
| async-process | 2.5.0 | Apache-2.0 OR MIT | [files](dependency-licenses/async-process-2.5.0/) |
| async-recursion | 1.2.0 | MIT OR Apache-2.0 | [files](dependency-licenses/async-recursion-1.2.0/) |
| async-signal | 0.2.14 | Apache-2.0 OR MIT | [files](dependency-licenses/async-signal-0.2.14/) |
| async-task | 4.7.1 | Apache-2.0 OR MIT | [files](dependency-licenses/async-task-4.7.1/) |
| async-trait | 0.1.92 | MIT OR Apache-2.0 | [files](dependency-licenses/async-trait-0.1.92/) |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | [files](dependency-licenses/atomic-waker-1.1.2/) |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | [files](dependency-licenses/autocfg-1.5.1/) |
| axum | 0.8.9 | MIT | [files](dependency-licenses/axum-0.8.9/) |
| axum-core | 0.5.6 | MIT | [files](dependency-licenses/axum-core-0.5.6/) |
| base64 | 0.22.1 | MIT OR Apache-2.0 | [files](dependency-licenses/base64-0.22.1/) |
| base64 | 0.23.1 | MIT OR Apache-2.0 | [files](dependency-licenses/base64-0.23.1/) |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | [files](dependency-licenses/bitflags-2.13.2/) |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | [files](dependency-licenses/block-buffer-0.10.4/) |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 | [files](dependency-licenses/block-buffer-0.12.1/) |
| block-padding | 0.3.3 | MIT OR Apache-2.0 | [files](dependency-licenses/block-padding-0.3.3/) |
| blocking | 1.7.0 | Apache-2.0 OR MIT | [files](dependency-licenses/blocking-1.7.0/) |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 | [files](dependency-licenses/bumpalo-3.20.3/) |
| byteorder | 1.5.0 | Unlicense OR MIT | [files](dependency-licenses/byteorder-1.5.0/) |
| bytes | 1.12.1 | MIT | [files](dependency-licenses/bytes-1.12.1/) |
| cbc | 0.1.2 | MIT OR Apache-2.0 | [files](dependency-licenses/cbc-0.1.2/) |
| cc | 1.6.0 | MIT OR Apache-2.0 | [files](dependency-licenses/cc-1.6.0/) |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | [files](dependency-licenses/cfg-if-1.0.5/) |
| cfg_aliases | 0.2.2 | MIT | [files](dependency-licenses/cfg_aliases-0.2.2/) |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 | [files](dependency-licenses/chacha20-0.10.2/) |
| cipher | 0.4.4 | MIT OR Apache-2.0 | [files](dependency-licenses/cipher-0.4.4/) |
| combine | 4.6.8 | MIT | [files](dependency-licenses/combine-4.6.8/) |
| concurrent-queue | 2.5.0 | Apache-2.0 OR MIT | [files](dependency-licenses/concurrent-queue-2.5.0/) |
| const-oid | 0.10.2 | Apache-2.0 OR MIT | [files](dependency-licenses/const-oid-0.10.2/) |
| core-foundation | 0.9.4 | MIT OR Apache-2.0 | [files](dependency-licenses/core-foundation-0.9.4/) |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 | [files](dependency-licenses/core-foundation-0.10.1/) |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 | [files](dependency-licenses/core-foundation-sys-0.8.7/) |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | [files](dependency-licenses/cpufeatures-0.2.17/) |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | [files](dependency-licenses/cpufeatures-0.3.1/) |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | [files](dependency-licenses/crossbeam-utils-0.8.23/) |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | [files](dependency-licenses/crypto-common-0.1.7/) |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 | [files](dependency-licenses/crypto-common-0.2.2/) |
| data-encoding | 2.11.1 | MIT | [files](dependency-licenses/data-encoding-2.11.1/) |
| dbus | 0.9.12 | Apache-2.0/MIT | [files](dependency-licenses/dbus-0.9.12/) |
| dbus-secret-service | 4.1.0 | MIT OR Apache-2.0 | [files](dependency-licenses/dbus-secret-service-4.1.0/) |
| deranged | 0.5.8 | MIT OR Apache-2.0 | [files](dependency-licenses/deranged-0.5.8/) |
| digest | 0.10.7 | MIT OR Apache-2.0 | [files](dependency-licenses/digest-0.10.7/) |
| digest | 0.11.3 | MIT OR Apache-2.0 | [files](dependency-licenses/digest-0.11.3/) |
| directories | 6.0.0 | MIT OR Apache-2.0 | [files](dependency-licenses/directories-6.0.0/) |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 | [files](dependency-licenses/dirs-sys-0.5.0/) |
| dispatch2 | 0.3.1 | Zlib OR Apache-2.0 OR MIT | [files](dependency-licenses/dispatch2-0.3.1/) (upstream/owning-project license with pinned provenance) |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | [files](dependency-licenses/displaydoc-0.2.7/) |
| endi | 1.1.1 | MIT | [files](dependency-licenses/endi-1.1.1/) |
| enumflags2 | 0.7.12 | MIT OR Apache-2.0 | [files](dependency-licenses/enumflags2-0.7.12/) |
| enumflags2_derive | 0.7.12 | MIT OR Apache-2.0 | [files](dependency-licenses/enumflags2_derive-0.7.12/) |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | [files](dependency-licenses/equivalent-1.0.3/) |
| errno | 0.3.14 | MIT OR Apache-2.0 | [files](dependency-licenses/errno-0.3.14/) |
| event-listener | 5.4.2 | Apache-2.0 OR MIT | [files](dependency-licenses/event-listener-5.4.2/) |
| event-listener-strategy | 0.5.4 | Apache-2.0 OR MIT | [files](dependency-licenses/event-listener-strategy-0.5.4/) |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | [files](dependency-licenses/fastrand-2.5.0/) |
| find-msvc-tools | 0.1.14 | MIT OR Apache-2.0 | [files](dependency-licenses/find-msvc-tools-0.1.14/) |
| foreign-types | 0.3.2 | MIT/Apache-2.0 | [files](dependency-licenses/foreign-types-0.3.2/) |
| foreign-types-shared | 0.1.1 | MIT/Apache-2.0 | [files](dependency-licenses/foreign-types-shared-0.1.1/) |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | [files](dependency-licenses/form_urlencoded-1.2.2/) |
| fs2 | 0.4.3 | MIT/Apache-2.0 | [files](dependency-licenses/fs2-0.4.3/) |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 | [files](dependency-licenses/futures-channel-0.3.34/) |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | [files](dependency-licenses/futures-core-0.3.34/) |
| futures-io | 0.3.34 | MIT OR Apache-2.0 | [files](dependency-licenses/futures-io-0.3.34/) |
| futures-lite | 2.6.1 | Apache-2.0 OR MIT | [files](dependency-licenses/futures-lite-2.6.1/) |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 | [files](dependency-licenses/futures-macro-0.3.34/) |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 | [files](dependency-licenses/futures-sink-0.3.34/) |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | [files](dependency-licenses/futures-task-0.3.34/) |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | [files](dependency-licenses/futures-util-0.3.34/) |
| generic-array | 0.14.7 | MIT | [files](dependency-licenses/generic-array-0.14.7/) |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | [files](dependency-licenses/getrandom-0.2.17/) |
| getrandom | 0.3.4 | MIT OR Apache-2.0 | [files](dependency-licenses/getrandom-0.3.4/) |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | [files](dependency-licenses/getrandom-0.4.3/) |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | [files](dependency-licenses/hashbrown-0.17.1/) |
| hermit-abi | 0.5.3 | MIT OR Apache-2.0 | [files](dependency-licenses/hermit-abi-0.5.3/) |
| hex | 0.4.3 | MIT OR Apache-2.0 | [files](dependency-licenses/hex-0.4.3/) |
| hkdf | 0.12.4 | MIT OR Apache-2.0 | [files](dependency-licenses/hkdf-0.12.4/) |
| hmac | 0.12.1 | MIT OR Apache-2.0 | [files](dependency-licenses/hmac-0.12.1/) |
| http | 1.5.0 | MIT OR Apache-2.0 | [files](dependency-licenses/http-1.5.0/) |
| http-body | 1.1.0 | MIT | [files](dependency-licenses/http-body-1.1.0/) |
| http-body-util | 0.1.5 | MIT | [files](dependency-licenses/http-body-util-0.1.5/) |
| httparse | 1.10.1 | MIT OR Apache-2.0 | [files](dependency-licenses/httparse-1.10.1/) |
| httpdate | 1.0.3 | MIT OR Apache-2.0 | [files](dependency-licenses/httpdate-1.0.3/) |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 | [files](dependency-licenses/hybrid-array-0.4.15/) |
| hyper | 1.12.0 | MIT | [files](dependency-licenses/hyper-1.12.0/) |
| hyper-rustls | 0.27.10 | Apache-2.0 OR ISC OR MIT | [files](dependency-licenses/hyper-rustls-0.27.10/) |
| hyper-util | 0.1.21 | MIT | [files](dependency-licenses/hyper-util-0.1.21/) |
| icu_collections | 2.3.0 | Unicode-3.0 | [files](dependency-licenses/icu_collections-2.3.0/) |
| icu_locale_core | 2.3.0 | Unicode-3.0 | [files](dependency-licenses/icu_locale_core-2.3.0/) |
| icu_normalizer | 2.3.0 | Unicode-3.0 | [files](dependency-licenses/icu_normalizer-2.3.0/) |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 | [files](dependency-licenses/icu_normalizer_data-2.3.0/) |
| icu_properties | 2.3.0 | Unicode-3.0 | [files](dependency-licenses/icu_properties-2.3.0/) |
| icu_properties_data | 2.3.0 | Unicode-3.0 | [files](dependency-licenses/icu_properties_data-2.3.0/) |
| icu_provider | 2.3.1 | Unicode-3.0 | [files](dependency-licenses/icu_provider-2.3.1/) |
| idna | 1.1.0 | MIT OR Apache-2.0 | [files](dependency-licenses/idna-1.1.0/) |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT | [files](dependency-licenses/idna_adapter-1.2.2/) |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | [files](dependency-licenses/indexmap-2.14.2/) |
| inout | 0.1.4 | MIT OR Apache-2.0 | [files](dependency-licenses/inout-0.1.4/) |
| ipnet | 2.12.2 | MIT OR Apache-2.0 | [files](dependency-licenses/ipnet-2.12.2/) |
| itoa | 1.0.18 | MIT OR Apache-2.0 | [files](dependency-licenses/itoa-1.0.18/) |
| jni | 0.22.4 | MIT OR Apache-2.0 | [files](dependency-licenses/jni-0.22.4/) (upstream/owning-project license with pinned provenance) |
| jni-macros | 0.22.4 | MIT OR Apache-2.0 | [files](dependency-licenses/jni-macros-0.22.4/) (upstream/owning-project license with pinned provenance) |
| jni-sys | 0.4.1 | MIT OR Apache-2.0 | [files](dependency-licenses/jni-sys-0.4.1/) |
| jni-sys-macros | 0.4.1 | MIT OR Apache-2.0 | [files](dependency-licenses/jni-sys-macros-0.4.1/) (upstream/owning-project license with pinned provenance) |
| js-sys | 0.3.106 | MIT OR Apache-2.0 | [files](dependency-licenses/js-sys-0.3.106/) |
| keyring | 3.6.3 | MIT OR Apache-2.0 | [files](dependency-licenses/keyring-3.6.3/) |
| libc | 0.2.190 | MIT OR Apache-2.0 | [files](dependency-licenses/libc-0.2.190/) |
| libdbus-sys | 0.2.7 | Apache-2.0/MIT | [files](dependency-licenses/libdbus-sys-0.2.7/) |
| libredox | 0.1.25 | MIT | [files](dependency-licenses/libredox-0.1.25/) |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [files](dependency-licenses/linux-raw-sys-0.12.1/) |
| litemap | 0.8.3 | Unicode-3.0 | [files](dependency-licenses/litemap-0.8.3/) |
| log | 0.4.34 | MIT OR Apache-2.0 | [files](dependency-licenses/log-0.4.34/) |
| lru-slab | 0.1.3 | MIT OR Apache-2.0 OR Zlib | [files](dependency-licenses/lru-slab-0.1.3/) |
| matchit | 0.8.4 | MIT AND BSD-3-Clause | [files](dependency-licenses/matchit-0.8.4/) |
| memchr | 2.8.3 | Unlicense OR MIT | [files](dependency-licenses/memchr-2.8.3/) |
| memoffset | 0.9.1 | MIT | [files](dependency-licenses/memoffset-0.9.1/) |
| mime | 0.3.17 | MIT OR Apache-2.0 | [files](dependency-licenses/mime-0.3.17/) |
| mime_guess | 2.0.5 | MIT | [files](dependency-licenses/mime_guess-2.0.5/) |
| mio | 1.2.4 | MIT | [files](dependency-licenses/mio-1.2.4/) |
| ndk-context | 0.1.1 | MIT OR Apache-2.0 | [files](dependency-licenses/ndk-context-0.1.1/) (upstream/owning-project license with pinned provenance) |
| nix | 0.29.0 | MIT | [files](dependency-licenses/nix-0.29.0/) |
| num | 0.4.3 | MIT OR Apache-2.0 | [files](dependency-licenses/num-0.4.3/) |
| num-bigint | 0.4.8 | MIT OR Apache-2.0 | [files](dependency-licenses/num-bigint-0.4.8/) |
| num-complex | 0.4.6 | MIT OR Apache-2.0 | [files](dependency-licenses/num-complex-0.4.6/) |
| num-conv | 0.2.2 | MIT OR Apache-2.0 | [files](dependency-licenses/num-conv-0.2.2/) |
| num-integer | 0.1.47 | MIT OR Apache-2.0 | [files](dependency-licenses/num-integer-0.1.47/) |
| num-iter | 0.1.46 | MIT OR Apache-2.0 | [files](dependency-licenses/num-iter-0.1.46/) |
| num-rational | 0.4.2 | MIT OR Apache-2.0 | [files](dependency-licenses/num-rational-0.4.2/) |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | [files](dependency-licenses/num-traits-0.2.19/) |
| objc2 | 0.6.5 | MIT | [files](dependency-licenses/objc2-0.6.5/) (upstream/owning-project license with pinned provenance) |
| objc2-app-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | [files](dependency-licenses/objc2-app-kit-0.3.2/) (upstream/owning-project license with pinned provenance) |
| objc2-core-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT | [files](dependency-licenses/objc2-core-foundation-0.3.2/) (upstream/owning-project license with pinned provenance) |
| objc2-encode | 4.1.0 | MIT | [files](dependency-licenses/objc2-encode-4.1.0/) (upstream/owning-project license with pinned provenance) |
| objc2-foundation | 0.3.2 | MIT | [files](dependency-licenses/objc2-foundation-0.3.2/) |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | [files](dependency-licenses/once_cell-1.21.4/) |
| openssl | 0.10.81 | Apache-2.0 | [files](dependency-licenses/openssl-0.10.81/) |
| openssl-macros | 0.1.1 | MIT/Apache-2.0 | [files](dependency-licenses/openssl-macros-0.1.1/) |
| openssl-src | 300.6.1+3.6.3 | MIT/Apache-2.0 | [files](dependency-licenses/openssl-src-300.6.1+3.6.3/) |
| openssl-sys | 0.9.117 | MIT | [files](dependency-licenses/openssl-sys-0.9.117/) |
| option-ext | 0.2.0 | MPL-2.0 | [files](dependency-licenses/option-ext-0.2.0/) |
| ordered-stream | 0.2.0 | MIT OR Apache-2.0 | [files](dependency-licenses/ordered-stream-0.2.0/) |
| parking | 2.2.1 | Apache-2.0 OR MIT | [files](dependency-licenses/parking-2.2.1/) |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | [files](dependency-licenses/percent-encoding-2.3.2/) |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | [files](dependency-licenses/pin-project-lite-0.2.17/) |
| piper | 0.2.5 | MIT OR Apache-2.0 | [files](dependency-licenses/piper-0.2.5/) |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | [files](dependency-licenses/pkg-config-0.3.34/) |
| polling | 3.11.0 | Apache-2.0 OR MIT | [files](dependency-licenses/polling-3.11.0/) |
| potential_utf | 0.1.6 | Unicode-3.0 | [files](dependency-licenses/potential_utf-0.1.6/) |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 | [files](dependency-licenses/powerfmt-0.2.1/) |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 | [files](dependency-licenses/ppv-lite86-0.2.21/) |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 | [files](dependency-licenses/proc-macro-crate-3.5.0/) |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | [files](dependency-licenses/proc-macro2-1.0.107/) |
| quinn | 0.11.12 | MIT OR Apache-2.0 | [files](dependency-licenses/quinn-0.11.12/) |
| quinn-proto | 0.11.19 | MIT OR Apache-2.0 | [files](dependency-licenses/quinn-proto-0.11.19/) |
| quinn-udp | 0.5.16 | MIT OR Apache-2.0 | [files](dependency-licenses/quinn-udp-0.5.16/) |
| quote | 1.0.47 | MIT OR Apache-2.0 | [files](dependency-licenses/quote-1.0.47/) |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | [files](dependency-licenses/r-efi-5.3.0/) |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | [files](dependency-licenses/r-efi-6.0.0/) |
| rand | 0.8.8 | MIT OR Apache-2.0 | [files](dependency-licenses/rand-0.8.8/) |
| rand | 0.9.5 | MIT OR Apache-2.0 | [files](dependency-licenses/rand-0.9.5/) |
| rand | 0.10.3 | MIT OR Apache-2.0 | [files](dependency-licenses/rand-0.10.3/) |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 | [files](dependency-licenses/rand_chacha-0.3.1/) |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 | [files](dependency-licenses/rand_chacha-0.9.0/) |
| rand_core | 0.6.4 | MIT OR Apache-2.0 | [files](dependency-licenses/rand_core-0.6.4/) |
| rand_core | 0.9.5 | MIT OR Apache-2.0 | [files](dependency-licenses/rand_core-0.9.5/) |
| rand_core | 0.10.1 | MIT OR Apache-2.0 | [files](dependency-licenses/rand_core-0.10.1/) |
| rand_pcg | 0.10.2 | MIT OR Apache-2.0 | [files](dependency-licenses/rand_pcg-0.10.2/) |
| redox_users | 0.5.3 | MIT | [files](dependency-licenses/redox_users-0.5.3/) |
| reqwest | 0.12.28 | MIT OR Apache-2.0 | [files](dependency-licenses/reqwest-0.12.28/) |
| ring | 0.17.14 | Apache-2.0 AND ISC | [files](dependency-licenses/ring-0.17.14/) |
| rust-embed | 8.13.0 | MIT | [files](dependency-licenses/rust-embed-8.13.0/) |
| rust-embed-impl | 8.13.0 | MIT | [files](dependency-licenses/rust-embed-impl-8.13.0/) |
| rust-embed-utils | 8.13.0 | MIT | [files](dependency-licenses/rust-embed-utils-8.13.0/) |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | [files](dependency-licenses/rustc-hash-2.1.3/) |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | [files](dependency-licenses/rustc_version-0.4.1/) |
| rustix | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [files](dependency-licenses/rustix-1.1.5/) |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | [files](dependency-licenses/rustls-0.23.46/) |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | [files](dependency-licenses/rustls-pki-types-1.15.1/) |
| rustls-webpki | 0.103.15 | ISC | [files](dependency-licenses/rustls-webpki-0.103.15/) |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | [files](dependency-licenses/rustversion-1.0.23/) |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 | [files](dependency-licenses/ryu-1.0.23/) |
| same-file | 1.0.6 | Unlicense/MIT | [files](dependency-licenses/same-file-1.0.6/) |
| secret-service | 4.0.0 | MIT OR Apache-2.0 | [files](dependency-licenses/secret-service-4.0.0/) |
| security-framework | 2.11.1 | MIT OR Apache-2.0 | [files](dependency-licenses/security-framework-2.11.1/) |
| security-framework | 3.7.0 | MIT OR Apache-2.0 | [files](dependency-licenses/security-framework-3.7.0/) |
| security-framework-sys | 2.17.0 | MIT OR Apache-2.0 | [files](dependency-licenses/security-framework-sys-2.17.0/) |
| semver | 1.0.28 | MIT OR Apache-2.0 | [files](dependency-licenses/semver-1.0.28/) |
| serde | 1.0.229 | MIT OR Apache-2.0 | [files](dependency-licenses/serde-1.0.229/) |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | [files](dependency-licenses/serde_core-1.0.229/) |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | [files](dependency-licenses/serde_derive-1.0.229/) |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | [files](dependency-licenses/serde_json-1.0.151/) |
| serde_path_to_error | 0.1.20 | MIT OR Apache-2.0 | [files](dependency-licenses/serde_path_to_error-0.1.20/) |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 | [files](dependency-licenses/serde_repr-0.1.21/) |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 | [files](dependency-licenses/serde_urlencoded-0.7.1/) |
| sha1 | 0.10.7 | MIT OR Apache-2.0 | [files](dependency-licenses/sha1-0.10.7/) |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | [files](dependency-licenses/sha2-0.10.9/) |
| sha2 | 0.11.0 | MIT OR Apache-2.0 | [files](dependency-licenses/sha2-0.11.0/) |
| shlex | 2.0.1 | MIT OR Apache-2.0 | [files](dependency-licenses/shlex-2.0.1/) |
| signal-hook-registry | 1.4.8 | MIT OR Apache-2.0 | [files](dependency-licenses/signal-hook-registry-1.4.8/) |
| simd_cesu8 | 1.2.0 | Apache-2.0 OR MIT | [files](dependency-licenses/simd_cesu8-1.2.0/) |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 | [files](dependency-licenses/simdutf8-0.1.5/) |
| slab | 0.4.12 | MIT | [files](dependency-licenses/slab-0.4.12/) |
| smallvec | 1.16.2 | MIT OR Apache-2.0 | [files](dependency-licenses/smallvec-1.16.2/) |
| socket2 | 0.6.5 | MIT OR Apache-2.0 | [files](dependency-licenses/socket2-0.6.5/) |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | [files](dependency-licenses/stable_deref_trait-1.2.1/) |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 | [files](dependency-licenses/static_assertions-1.1.0/) |
| subtle | 2.6.1 | BSD-3-Clause | [files](dependency-licenses/subtle-2.6.1/) |
| syn | 2.0.119 | MIT OR Apache-2.0 | [files](dependency-licenses/syn-2.0.119/) |
| syn | 3.0.6 | MIT OR Apache-2.0 | [files](dependency-licenses/syn-3.0.6/) |
| sync_wrapper | 1.0.2 | Apache-2.0 | [files](dependency-licenses/sync_wrapper-1.0.2/) |
| synstructure | 0.14.0 | MIT | [files](dependency-licenses/synstructure-0.14.0/) |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | [files](dependency-licenses/tempfile-3.27.0/) |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | [files](dependency-licenses/thiserror-2.0.21/) |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | [files](dependency-licenses/thiserror-impl-2.0.21/) |
| time | 0.3.55 | MIT OR Apache-2.0 | [files](dependency-licenses/time-0.3.55/) |
| time-core | 0.1.9 | MIT OR Apache-2.0 | [files](dependency-licenses/time-core-0.1.9/) |
| time-macros | 0.2.32 | MIT OR Apache-2.0 | [files](dependency-licenses/time-macros-0.2.32/) |
| tinystr | 0.8.4 | Unicode-3.0 | [files](dependency-licenses/tinystr-0.8.4/) |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | [files](dependency-licenses/tinyvec-1.13.3/) |
| tokio | 1.53.2 | MIT | [files](dependency-licenses/tokio-1.53.2/) |
| tokio-macros | 2.7.2 | MIT | [files](dependency-licenses/tokio-macros-2.7.2/) |
| tokio-rustls | 0.26.6 | MIT OR Apache-2.0 | [files](dependency-licenses/tokio-rustls-0.26.6/) |
| tokio-tungstenite | 0.28.0 | MIT | [files](dependency-licenses/tokio-tungstenite-0.28.0/) |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | [files](dependency-licenses/toml_datetime-1.1.2+spec-1.1.0/) |
| toml_edit | 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 | [files](dependency-licenses/toml_edit-0.25.17+spec-1.1.0/) |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | [files](dependency-licenses/toml_parser-1.1.5+spec-1.1.0/) |
| tower | 0.5.3 | MIT | [files](dependency-licenses/tower-0.5.3/) |
| tower-http | 0.6.11 | MIT | [files](dependency-licenses/tower-http-0.6.11/) |
| tower-layer | 0.3.3 | MIT | [files](dependency-licenses/tower-layer-0.3.3/) |
| tower-service | 0.3.3 | MIT | [files](dependency-licenses/tower-service-0.3.3/) |
| tracing | 0.1.44 | MIT | [files](dependency-licenses/tracing-0.1.44/) |
| tracing-attributes | 0.1.31 | MIT | [files](dependency-licenses/tracing-attributes-0.1.31/) |
| tracing-core | 0.1.36 | MIT | [files](dependency-licenses/tracing-core-0.1.36/) |
| try-lock | 0.2.5 | MIT | [files](dependency-licenses/try-lock-0.2.5/) |
| tungstenite | 0.28.0 | MIT OR Apache-2.0 | [files](dependency-licenses/tungstenite-0.28.0/) |
| typenum | 1.20.1 | MIT OR Apache-2.0 | [files](dependency-licenses/typenum-1.20.1/) |
| uds_windows | 1.2.1 | MIT | [files](dependency-licenses/uds_windows-1.2.1/) |
| unicase | 2.10.0 | MIT OR Apache-2.0 | [files](dependency-licenses/unicase-2.10.0/) |
| unicode-ident | 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | [files](dependency-licenses/unicode-ident-1.0.26/) |
| untrusted | 0.9.0 | ISC | [files](dependency-licenses/untrusted-0.9.0/) |
| url | 2.5.8 | MIT OR Apache-2.0 | [files](dependency-licenses/url-2.5.8/) |
| utf-8 | 0.7.6 | MIT OR Apache-2.0 | [files](dependency-licenses/utf-8-0.7.6/) |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT | [files](dependency-licenses/utf8_iter-1.0.4/) |
| vcpkg | 0.2.15 | MIT/Apache-2.0 | [files](dependency-licenses/vcpkg-0.2.15/) |
| version_check | 0.9.5 | MIT/Apache-2.0 | [files](dependency-licenses/version_check-0.9.5/) |
| walkdir | 2.5.0 | Unlicense/MIT | [files](dependency-licenses/walkdir-2.5.0/) |
| want | 0.3.2 | MIT | [files](dependency-licenses/want-0.3.2/) |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [files](dependency-licenses/wasi-0.11.1+wasi-snapshot-preview1/) |
| wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [files](dependency-licenses/wasip2-1.0.4+wasi-0.2.12/) |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 | [files](dependency-licenses/wasm-bindgen-0.2.129/) |
| wasm-bindgen-futures | 0.4.79 | MIT OR Apache-2.0 | [files](dependency-licenses/wasm-bindgen-futures-0.4.79/) |
| wasm-bindgen-macro | 0.2.129 | MIT OR Apache-2.0 | [files](dependency-licenses/wasm-bindgen-macro-0.2.129/) |
| wasm-bindgen-macro-support | 0.2.129 | MIT OR Apache-2.0 | [files](dependency-licenses/wasm-bindgen-macro-support-0.2.129/) |
| wasm-bindgen-shared | 0.2.129 | MIT OR Apache-2.0 | [files](dependency-licenses/wasm-bindgen-shared-0.2.129/) |
| web-sys | 0.3.106 | MIT OR Apache-2.0 | [files](dependency-licenses/web-sys-0.3.106/) |
| web-time | 1.1.0 | MIT OR Apache-2.0 | [files](dependency-licenses/web-time-1.1.0/) |
| webbrowser | 1.2.4 | MIT OR Apache-2.0 | [files](dependency-licenses/webbrowser-1.2.4/) |
| webpki-roots | 0.26.11 | CDLA-Permissive-2.0 | [files](dependency-licenses/webpki-roots-0.26.11/) |
| webpki-roots | 1.0.9 | CDLA-Permissive-2.0 | [files](dependency-licenses/webpki-roots-1.0.9/) |
| winapi | 0.3.9 | MIT/Apache-2.0 | [files](dependency-licenses/winapi-0.3.9/) |
| winapi-i686-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 | [files](dependency-licenses/winapi-i686-pc-windows-gnu-0.4.0/) (upstream/owning-project license with pinned provenance) |
| winapi-util | 0.1.11 | Unlicense OR MIT | [files](dependency-licenses/winapi-util-0.1.11/) |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 | [files](dependency-licenses/winapi-x86_64-pc-windows-gnu-0.4.0/) (upstream/owning-project license with pinned provenance) |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | [files](dependency-licenses/windows-link-0.2.1/) |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 | [files](dependency-licenses/windows-sys-0.52.0/) |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 | [files](dependency-licenses/windows-sys-0.59.0/) |
| windows-sys | 0.60.2 | MIT OR Apache-2.0 | [files](dependency-licenses/windows-sys-0.60.2/) |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | [files](dependency-licenses/windows-sys-0.61.2/) |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 | [files](dependency-licenses/windows-targets-0.52.6/) |
| windows-targets | 0.53.5 | MIT OR Apache-2.0 | [files](dependency-licenses/windows-targets-0.53.5/) |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_aarch64_gnullvm-0.52.6/) |
| windows_aarch64_gnullvm | 0.53.1 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_aarch64_gnullvm-0.53.1/) |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_aarch64_msvc-0.52.6/) |
| windows_aarch64_msvc | 0.53.1 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_aarch64_msvc-0.53.1/) |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_i686_gnu-0.52.6/) |
| windows_i686_gnu | 0.53.1 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_i686_gnu-0.53.1/) |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_i686_gnullvm-0.52.6/) |
| windows_i686_gnullvm | 0.53.1 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_i686_gnullvm-0.53.1/) |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_i686_msvc-0.52.6/) |
| windows_i686_msvc | 0.53.1 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_i686_msvc-0.53.1/) |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_x86_64_gnu-0.52.6/) |
| windows_x86_64_gnu | 0.53.1 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_x86_64_gnu-0.53.1/) |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_x86_64_gnullvm-0.52.6/) |
| windows_x86_64_gnullvm | 0.53.1 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_x86_64_gnullvm-0.53.1/) |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_x86_64_msvc-0.52.6/) |
| windows_x86_64_msvc | 0.53.1 | MIT OR Apache-2.0 | [files](dependency-licenses/windows_x86_64_msvc-0.53.1/) |
| winnow | 1.0.4 | MIT | [files](dependency-licenses/winnow-1.0.4/) |
| wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [files](dependency-licenses/wit-bindgen-0.57.1/) |
| writeable | 0.6.4 | Unicode-3.0 | [files](dependency-licenses/writeable-0.6.4/) |
| xdg-home | 1.3.0 | MIT | [files](dependency-licenses/xdg-home-1.3.0/) |
| yoke | 0.8.3 | Unicode-3.0 | [files](dependency-licenses/yoke-0.8.3/) |
| yoke-derive | 0.8.4 | Unicode-3.0 | [files](dependency-licenses/yoke-derive-0.8.4/) |
| zbus | 4.4.0 | MIT | [files](dependency-licenses/zbus-4.4.0/) |
| zbus_macros | 4.4.0 | MIT | [files](dependency-licenses/zbus_macros-4.4.0/) |
| zbus_names | 3.0.0 | MIT | [files](dependency-licenses/zbus_names-3.0.0/) |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | [files](dependency-licenses/zerocopy-0.8.63/) |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | [files](dependency-licenses/zerocopy-derive-0.8.63/) |
| zerofrom | 0.1.8 | Unicode-3.0 | [files](dependency-licenses/zerofrom-0.1.8/) |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | [files](dependency-licenses/zerofrom-derive-0.1.8/) |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | [files](dependency-licenses/zeroize-1.9.1/) |
| zeroize_derive | 1.5.0 | Apache-2.0 OR MIT | [files](dependency-licenses/zeroize_derive-1.5.0/) |
| zerotrie | 0.2.5 | Unicode-3.0 | [files](dependency-licenses/zerotrie-0.2.5/) |
| zerovec | 0.11.8 | Unicode-3.0 | [files](dependency-licenses/zerovec-0.11.8/) |
| zerovec-derive | 0.11.6 | Unicode-3.0 | [files](dependency-licenses/zerovec-derive-0.11.6/) |
| zmij | 1.0.23 | MIT | [files](dependency-licenses/zmij-1.0.23/) |
| zvariant | 4.2.0 | MIT | [files](dependency-licenses/zvariant-4.2.0/) |
| zvariant_derive | 4.2.0 | MIT | [files](dependency-licenses/zvariant_derive-4.2.0/) |
| zvariant_utils | 2.1.0 | MIT | [files](dependency-licenses/zvariant_utils-2.1.0/) |
| npm: @oxc-project/types | 0.153.0 | MIT | [files](dependency-licenses/npm-oxc-project-types-0.153.0/) |
| npm: @rolldown/binding-android-arm-eabi | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-android-arm64 | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-darwin-arm64 | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-darwin-x64 | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-freebsd-x64 | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-linux-arm-gnueabihf | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-linux-arm64-gnu | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-linux-arm64-musl | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-linux-ppc64-gnu | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-linux-s390x-gnu | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-linux-x64-gnu | 1.2.13 | MIT | Metadata only; no license file in published build package |
| npm: @rolldown/binding-linux-x64-musl | 1.2.13 | MIT | Metadata only; no license file in published build package |
| npm: @rolldown/binding-openharmony-arm64 | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-win32-arm64-msvc | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/binding-win32-x64-msvc | 1.2.13 | MIT | Metadata only; optional build package not installed on this host |
| npm: @rolldown/pluginutils | 1.0.1 | MIT | [files](dependency-licenses/npm-rolldown-pluginutils-1.0.1/) |
| npm: @types/react | 19.3.0 | MIT | [files](dependency-licenses/npm-types-react-19.3.0/) |
| npm: @types/react-dom | 19.3.0 | MIT | [files](dependency-licenses/npm-types-react-dom-19.3.0/) |
| npm: @typescript/typescript-aix-ppc64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-darwin-arm64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-darwin-x64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-freebsd-arm64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-freebsd-x64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-linux-arm | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-linux-arm64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-linux-loong64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-linux-mips64el | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-linux-ppc64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-linux-riscv64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-linux-s390x | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-linux-x64 | 7.0.2 | Apache-2.0 | [files](dependency-licenses/npm-typescript-typescript-linux-x64-7.0.2/) |
| npm: @typescript/typescript-netbsd-arm64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-netbsd-x64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-openbsd-arm64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-openbsd-x64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-sunos-x64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-win32-arm64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @typescript/typescript-win32-x64 | 7.0.2 | Apache-2.0 | Metadata only; optional build package not installed on this host |
| npm: @vitejs/plugin-react | 6.1.2 | MIT | [files](dependency-licenses/npm-vitejs-plugin-react-6.1.2/) |
| npm: csstype | 3.2.3 | MIT | [files](dependency-licenses/npm-csstype-3.2.3/) |
| npm: detect-libc | 2.1.2 | Apache-2.0 | [files](dependency-licenses/npm-detect-libc-2.1.2/) |
| npm: fdir | 6.5.0 | MIT | [files](dependency-licenses/npm-fdir-6.5.0/) |
| npm: fsevents | 2.3.3 | MIT | Metadata only; optional build package not installed on this host |
| npm: lightningcss | 1.33.0 | MPL-2.0 | [files](dependency-licenses/npm-lightningcss-1.33.0/) |
| npm: lightningcss-android-arm64 | 1.33.0 | MPL-2.0 | Metadata only; optional build package not installed on this host |
| npm: lightningcss-darwin-arm64 | 1.33.0 | MPL-2.0 | Metadata only; optional build package not installed on this host |
| npm: lightningcss-darwin-x64 | 1.33.0 | MPL-2.0 | Metadata only; optional build package not installed on this host |
| npm: lightningcss-freebsd-x64 | 1.33.0 | MPL-2.0 | Metadata only; optional build package not installed on this host |
| npm: lightningcss-linux-arm-gnueabihf | 1.33.0 | MPL-2.0 | Metadata only; optional build package not installed on this host |
| npm: lightningcss-linux-arm64-gnu | 1.33.0 | MPL-2.0 | Metadata only; optional build package not installed on this host |
| npm: lightningcss-linux-arm64-musl | 1.33.0 | MPL-2.0 | Metadata only; optional build package not installed on this host |
| npm: lightningcss-linux-x64-gnu | 1.33.0 | MPL-2.0 | [files](dependency-licenses/npm-lightningcss-linux-x64-gnu-1.33.0/) |
| npm: lightningcss-linux-x64-musl | 1.33.0 | MPL-2.0 | [files](dependency-licenses/npm-lightningcss-linux-x64-musl-1.33.0/) |
| npm: lightningcss-win32-arm64-msvc | 1.33.0 | MPL-2.0 | Metadata only; optional build package not installed on this host |
| npm: lightningcss-win32-x64-msvc | 1.33.0 | MPL-2.0 | Metadata only; optional build package not installed on this host |
| npm: nanoid | 3.3.20 | MIT | [files](dependency-licenses/npm-nanoid-3.3.20/) |
| npm: picocolors | 1.1.1 | ISC | [files](dependency-licenses/npm-picocolors-1.1.1/) |
| npm: picomatch | 4.0.7 | MIT | [files](dependency-licenses/npm-picomatch-4.0.7/) |
| npm: playwright | 1.64.0 | Apache-2.0 | [files](dependency-licenses/npm-playwright-1.64.0/) |
| npm: playwright-core | 1.64.0 | Apache-2.0 | [files](dependency-licenses/npm-playwright-core-1.64.0/) |
| npm: postcss | 8.5.29 | MIT | [files](dependency-licenses/npm-postcss-8.5.29/) |
| npm: react | 19.3.0 | MIT | [files](dependency-licenses/npm-react-19.3.0/) |
| npm: react-dom | 19.3.0 | MIT | [files](dependency-licenses/npm-react-dom-19.3.0/) |
| npm: rolldown | 1.2.13 | MIT | [files](dependency-licenses/npm-rolldown-1.2.13/) |
| npm: scheduler | 0.28.0 | MIT | [files](dependency-licenses/npm-scheduler-0.28.0/) |
| npm: source-map-js | 1.2.2 | BSD-3-Clause | [files](dependency-licenses/npm-source-map-js-1.2.2/) |
| npm: tinyglobby | 0.2.17 | MIT | [files](dependency-licenses/npm-tinyglobby-0.2.17/) |
| npm: typescript | 7.0.2 | Apache-2.0 | [files](dependency-licenses/npm-typescript-7.0.2/) |
| npm: vite | 8.3.3 | MIT | [files](dependency-licenses/npm-vite-8.3.3/) |

## API research attribution

The original Rust client, reviewed operation inventory, fixed GraphQL requests
and Parallax decoder were informed by these community-maintained primary sources:

- [RivDocs at commit 2eb98b4b50b26d9f80a190f65107d4bd9d828cd9](https://github.com/kaedenbrinkman/rivian-api/tree/2eb98b4b50b26d9f80a190f65107d4bd9d828cd9), dated 2026-10-10, including the App API, operation descriptions and RVM/transport catalog.
- [rivian-python-client at commit 7d6028233752cc226faeae35e9d12ce387abb2fe](https://github.com/bretterer/rivian-python-client/tree/7d6028233752cc226faeae35e9d12ce387abb2fe), dated 2026-10-14, including GraphQL interface declarations and reverse-engineered Parallax protobuf field tags, types and units.
- The earlier API adapter review used [rivian-python-client commit 4d15dd88e74cf1a0be0bd23f46565fe89b48af44](https://github.com/bretterer/rivian-python-client/tree/4d15dd88e74cf1a0be0bd23f46565fe89b48af44); it remains part of the original contract provenance.

The current Python client's pinned [pyproject.toml](https://github.com/bretterer/rivian-python-client/blob/7d6028233752cc226faeae35e9d12ce387abb2fe/pyproject.toml)
declares MIT and lists Brian Retterer, J.R. Gutierrez, Nathan Spencer and Trevor
Mack. Neither reviewed pinned source tree contains a standalone license or
copyright/notice file. No replacement license text or copyright year is invented
for those repositories.

The repository uses interface facts such as endpoint addresses, operation and
field names, GraphQL selection structure, RVM names, protobuf wire tags and units.
Its Rust transport, bounded protobuf decoding/state merge and synthetic fixtures
are original implementations; no upstream Python runtime implementation,
generated protobuf runtime, actual owner capture or vehicle test fixture is
included. Attribution is to the reviewed contracts and schema facts, and does not
relicense upstream documentation prose or imply Rivian/community endorsement.
See [API coverage and provenance](API-COVERAGE.md) for the exact source snapshot,
known differences and execution restrictions.
