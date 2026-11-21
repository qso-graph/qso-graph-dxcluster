<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Dependencies

The library has zero normal, build, and development dependencies.
The policy gate reads `cargo metadata --locked --offline --no-deps` package
declarations for every target (QGDXC-IO-0001). It rejects normal and build
dependencies, lists development dependencies, and checks them against the
explicit, currently empty `LIBRARY_DEV_ALLOWLIST` in `xtask/src/checks.rs`.
Planted manifest tests cover individual dependency tables, optional aliases,
and target-specific normal, build, and development declarations. CLI argument handling,
base64, event serialization, framing, and protocol parsing use first-party Rust.
The CLI and repository tooling share serde_json. The CLI also names serde_core, already in the graph through serde_json at the same locked version, so the `qg-dxc tui` sources file reader can refuse duplicate JSON members (QGDXC-TUI-0007); it uses no derive. Repository tooling also imports the first-party CLI recording adapter to check application recording/replay vectors without sockets. The CLI uses ctrlc for portable
OS signal registration, avoiding first-party unsafe signal handlers. The CLI uses
getrandom for per-attempt OS randomness (QGDXC-DSP-0004 and QGDXC-SES-0010),
avoiding first-party unsafe platform bindings. Its r-efi dependency supplies Rust
UEFI bindings on that target; the shipped desktop targets do not use it.

The terminal interface pins ratatui 0.30.2 (published June 19, 2026) and
crossterm 0.29.0 (published April 5, 2025). The crates.io registry index
retrieved by `cargo fetch` lists these as the newest stable releases before
the November 4, 2026 cutoff. Ratatui defaults are disabled; only its crossterm
backend is enabled. The CLI requests Crossterm event support with defaults disabled; the Ratatui
backend also enables Crossterm defaults transitively. The interface does not
enable bracketed paste (QGDXC-TUI-0041). Other additions are transitive terminal dependencies.

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
| getrandom | 0.4.3 | OS randomness for live readiness tokens and retry jitter (CLI only) | Exact-version exemption; not audited yet |
| itoa | 1.0.19 | JSON numeric formatting and bounded input scanning | Exact-version exemption; not audited yet |
| libc | 0.2.191 | Portable Ctrl-C handling and Rust OS bindings (CLI only) | Exact-version exemption; not audited yet |
| memchr | 2.8.3 | JSON numeric formatting and bounded input scanning | Exact-version exemption; not audited yet |
| nix | 0.30.1 | Portable Ctrl-C handling and Rust OS bindings (CLI only) | Exact-version exemption; not audited yet |
| objc2 | 0.6.5 | macOS Ctrl-C dispatch and Rust Objective-C bindings (CLI only) | Exact-version exemption; not audited yet |
| objc2-encode | 4.1.0 | macOS Ctrl-C dispatch and Rust Objective-C bindings (CLI only) | Exact-version exemption; not audited yet |
| proc-macro2 | 1.0.107 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| quote | 1.0.47 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| r-efi | 6.0.0 | Target-specific UEFI bindings for getrandom | Exact-version exemption; not audited yet |
| ryu | 1.0.23 | JSON numeric formatting and bounded input scanning | Exact-version exemption; not audited yet |
| serde | 1.0.229 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| serde_core | 1.0.229 | JSON value support; sources-file reader without derive (CLI) | Exact-version exemption; not audited yet |
| serde_derive | 1.0.229 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| serde_json | 1.0.145 | JSON reader/writer for CLI recording and repository vector tooling | Exact-version exemption; not audited yet |
| syn | 2.0.119, 3.0.10 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| unicode-ident | 1.0.27 | JSON value support and optional derive/build-time support in resolved graph | Exact-version exemption; not audited yet |
| windows-link | 0.2.1 | Windows Ctrl-C OS bindings (CLI only) | Exact-version exemption; not audited yet |
| windows-sys | 0.61.2 | Windows Ctrl-C OS bindings (CLI only) | Exact-version exemption; not audited yet |
| allocator-api2 | 0.2.21 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| approx | 0.5.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| autocfg | 1.5.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| by_address | 1.2.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| castaway | 0.2.4 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| compact_str | 0.9.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| convert_case | 0.10.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| crossterm | 0.29.0 | Portable terminal input, raw mode, and screen control (CLI only) | Exact-version exemption; not audited yet |
| crossterm_winapi | 0.9.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| darling | 0.24.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| darling_core | 0.24.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| darling_macro | 0.24.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| deranged | 0.5.9 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| derive_more | 2.1.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| derive_more-impl | 2.1.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| document-features | 0.2.12 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| either | 1.19.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| equivalent | 1.0.3 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| errno | 0.3.15 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| foldhash | 0.2.0 | Rust-only hashing for terminal layout; exact Zlib license exception in deny.toml | Exact-version exemption; not audited yet |
| hashbrown | 0.17.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| heck | 0.5.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| ident_case | 1.0.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| indoc | 2.0.8 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| instability | 0.3.14 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| itertools | 0.14.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| kasuari | 0.4.13 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| libm | 0.2.16 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| line-clipping | 0.3.8 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| linux-raw-sys | 0.12.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| litrs | 1.0.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| lock_api | 0.4.14 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| log | 0.4.35 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| lru | 0.18.5 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| mio | 1.2.5 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| num-conv | 0.2.2 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| num-traits | 0.2.19 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| num_threads | 0.1.8 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| palette | 0.7.7 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| palette_derive | 0.7.7 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| palette_math | 0.7.7 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| parking_lot | 0.12.5 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| parking_lot_core | 0.9.12 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| portable-atomic | 1.15.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| powerfmt | 0.2.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| ratatui | 0.30.2 | Terminal cell layout and rendering (CLI only) | Exact-version exemption; not audited yet |
| ratatui-core | 0.1.2 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| ratatui-crossterm | 0.1.2 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| ratatui-widgets | 0.3.2 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| redox_syscall | 0.5.18 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| rustc_version | 0.4.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| rustix | 1.1.6 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| rustversion | 1.0.23 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| scopeguard | 1.2.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| semver | 1.0.28 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| signal-hook | 0.3.18 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| signal-hook-mio | 0.2.5 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| signal-hook-registry | 1.4.8 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| smallvec | 1.16.5 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| static_assertions | 1.1.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| strsim | 0.11.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| strum | 0.28.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| strum_macros | 0.28.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| thiserror | 2.0.21 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| thiserror-impl | 2.0.21 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| time | 0.3.55 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| time-core | 0.1.9 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| unicode-segmentation | 1.13.3 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| unicode-truncate | 2.0.1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| unicode-width | 0.2.3 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| wasi | 0.11.1+wasi-snapshot-preview1 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| winapi | 0.3.9 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| winapi-i686-pc-windows-gnu | 0.4.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | Rust-only terminal rendering or platform support (CLI dependency graph) | Exact-version exemption; not audited yet |

The excluded fuzz workspace is developer tooling and never enters the shipped
dependency graph. Its runtime decision is documented in fuzz/README.md.
Cargo-deny checks crates.io sources, license compatibility, and native build
helper bans. Verification runs cargo-vet with --locked --frozen, using only the
committed import store; no audit server is contacted after the fetch row. Cargo-audit 0.22.2 checks
both lockfiles with --no-fetch --deny warnings against the advisory database
prepared by that row. A temporary vulnerable lockfile proves advisory rejection
without adding a project dependency.

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
