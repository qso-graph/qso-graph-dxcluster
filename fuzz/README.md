<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Fuzzing

Targets cover value validation, live spots, enhanced spots, non-spot layouts,
RBN comments, incremental framing, passive Telnet handling, and session control
under QGDXC-SEC-0001, and the `qg-dxc tui` sources file under QGDXC-TUI-0009.
Targets use arbitrary bytes, bounded admission, fragmentation,
EOF, and state resets. They do not open sockets or read private captures.

This is an excluded cargo-fuzz workspace. Its developer-only runtime is
libfuzzer-sys 0.4.10, which compiles the C++ LLVM libFuzzer runtime with its
cc build dependency. That is the same tooling-only separation used by the
reference workspace. No bundled native runtime or native build helper enters the shipped dependency graph.
This is an explicit tooling exception to the native-dependency policy; applying
the ban to developer runtimes too blocks conventional cargo-fuzz.

The library itself retains zero dependencies. Fuzzing never enables first-party
unsafe code. The runtime and build helpers are not claimed as source-audited.
Install the pinned tooling before verification:

```sh
cargo install cargo-fuzz --version 0.13.2 --locked
rustup toolchain install nightly-2026-10-30 --profile minimal --component rustfmt --component clippy
cd fuzz
cargo fmt --check
cargo fuzz run session -- -max_total_time=60 -max_len=70000 -timeout=10 -rss_limit_mb=2048
```

Use any other target name in the same command. Verify runs a one-second smoke per
target on Linux. Missing or incorrectly versioned prerequisites fail on Linux;
other platforms report SKIP with the platform reason. The first verify row
fetches this locked workspace; compilation and all smokes then run offline. Longer fuzz campaigns remain useful after smoke passes. Store
failure inputs outside published vectors until their privacy review is complete.
