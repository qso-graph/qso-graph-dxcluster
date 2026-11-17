<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Dependencies

The library has zero runtime and development dependencies. CLI argument handling,
base64, event serialization, framing, and protocol parsing use first-party Rust.
The CLI and repository tooling share serde_json. Repository tooling also imports the first-party CLI recording adapter to check application recording/replay vectors without sockets. The CLI uses ctrlc for portable
OS signal registration, avoiding first-party unsafe signal handlers.

Every registry package below is an explicit decision. Versions are locked in
Cargo.lock; exact-version cargo-vet exemptions mean **not audited yet**, not an
audit. The store follows the sibling project's four imported audit sources.
No C or C++ source is built or bundled in shipped dependencies. OS bindings call
platform system libraries; they are Rust packages, not bundled native libraries.

| Package | Locked version | Purpose | Review status |
|---|---|---|---|
| bitflags | 2.13.2 | Portable Ctrl-C handling and Rust OS bindings (CLI only) | Exact-version exemption; not audited yet |
| block2 | 0.6.2 | macOS Ctrl-C dispatch and Rust Objective-C bindings (CLI only) | Exact-version exemption; not audited yet |
| cfg-if | 1.0.5 | Portable Ctrl-C handling and Rust OS bindings (CLI only) | Exact-version exemption; not audited yet |
| cfg_aliases | 0.2.2 | Portable Ctrl-C handling and Rust OS bindings (CLI only) | Exact-version exemption; not audited yet |
| ctrlc | 3.5.1 | Portable Ctrl-C handling and Rust OS bindings (CLI only) | Exact-version exemption; not audited yet |
| dispatch2 | 0.3.1 | macOS Ctrl-C dispatch and Rust Objective-C bindings (CLI only) | Exact-version exemption; not audited yet |
| itoa | 1.0.19 | JSON numeric formatting and bounded input scanning | Exact-version exemption; not audited yet |
| libc | 0.2.191 | Portable Ctrl-C handling and Rust OS bindings (CLI only) | Exact-version exemption; not audited yet |
| memchr | 2.8.3 | JSON numeric formatting and bounded input scanning | Exact-version exemption; not audited yet |
| nix | 0.30.1 | Portable Ctrl-C handling and Rust OS bindings (CLI only) | Exact-version exemption; not audited yet |
| objc2 | 0.6.5 | macOS Ctrl-C dispatch and Rust Objective-C bindings (CLI only) | Exact-version exemption; not audited yet |
| objc2-encode | 4.1.0 | macOS Ctrl-C dispatch and Rust Objective-C bindings (CLI only) | Exact-version exemption; not audited yet |
| proc-macro2 | 1.0.107 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| quote | 1.0.47 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| ryu | 1.0.23 | JSON numeric formatting and bounded input scanning | Exact-version exemption; not audited yet |
| serde | 1.0.229 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| serde_core | 1.0.229 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| serde_derive | 1.0.229 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| serde_json | 1.0.145 | JSON reader/writer for CLI recording and repository vector tooling | Exact-version exemption; not audited yet |
| syn | 3.0.10 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| unicode-ident | 1.0.27 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| windows-link | 0.2.1 | Windows Ctrl-C OS bindings (CLI only) | Exact-version exemption; not audited yet |
| windows-sys | 0.61.2 | Windows Ctrl-C OS bindings (CLI only) | Exact-version exemption; not audited yet |

The excluded fuzz workspace is developer tooling and never enters the shipped
dependency graph. Its runtime decision is documented in fuzz/README.md.
Cargo-deny checks crates.io sources, license compatibility, and native build
helper bans. Verification runs cargo-vet with --locked --frozen, using only the
committed import store; no audit server is contacted during verification.

The tooling-only libFuzzer exception is isolated by the excluded workspace.
These exact additional registry packages are reviewed decisions, not completed audits:

| Fuzz package | Version | Purpose |
|---|---|---|
| arbitrary | 1.5.0 | Fuzzer input support |
| cc | 1.11.2 | Build-time runtime compilation and platform support (developer tooling only) |
| cfg-if | 1.0.5 | Build-time runtime compilation and platform support (developer tooling only) |
| find-msvc-tools | 0.1.15 | Build-time runtime compilation and platform support (developer tooling only) |
| getrandom | 0.4.3 | Build-time runtime compilation and platform support (developer tooling only) |
| jobserver | 0.1.35 | Build-time runtime compilation and platform support (developer tooling only) |
| libc | 0.2.191 | Build-time runtime compilation and platform support (developer tooling only) |
| libfuzzer-sys | 0.4.10 | C++ libFuzzer runtime (developer tooling only) |
| r-efi | 6.0.0 | Build-time runtime compilation and platform support (developer tooling only) |
| shlex | 2.0.1 | Build-time runtime compilation and platform support (developer tooling only) |
