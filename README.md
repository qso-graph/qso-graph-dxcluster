<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# QG DX Cluster

Read live DX spots as typed events, or replay a recorded session offline.

QG DX Cluster is for radio operators who want a terminal feed and developers who
need a receive-only Rust protocol engine. It supports DXSpider, AR-Cluster,
CC Cluster, and direct Reverse Beacon Network feeds, with automatic profile
detection. The library handles framing, profile recognition, startup, and retry
policy; the `qg-dxc` application owns TCP and recording.

Events preserve received evidence and distinguish spots, announcements, talk,
WWV/WCY reports, prompts, and connection changes. Default privacy handling and
terminal escaping follow [QGDXC-SEC-0003 and QGDXC-SEC-0005](spec/QGDXC-SPEC-001.md).
The library has no runtime dependencies and performs no I/O.

Install from source with Rust 1.99.1:

```sh
cargo install --locked --path crates/qso-graph-dxcluster-cli
qg-dxc --help
```

Supply a node you are authorized to use and your own operator identity. These
examples use an intentionally nonresolving documentation endpoint and invented
identities:

```sh
qg-dxc connect example.invalid 7300 --call Q1ABC --profile auto
qg-dxc connect example.invalid 7300 --call Q1ABC --profile cccluster --json
qg-dxc connect example.invalid 7300 --call Q1ABC --record session.ndjson --retain-third-party-data
qg-dxc replay session.ndjson --json
```

Recording is opt-in; exact recording also requires the explicit retention flag.
Replay opens only the specified local file. Ctrl-C closes the active connection.
A node that asks for a password stops the session with `password_unsupported`.
No password option is available in this version. Human output has one escaped
line per event; `--json` produces Annex A NDJSON events.

For library integration, start with [the API documentation](crates/qso-graph-dxcluster/src/lib.rs)
and [conformance and caller responsibilities](docs/CONFORMANCE.md).
Run the complete offline check suite with `cargo xtask verify`. See
[CONTRIBUTING.md](CONTRIBUTING.md) for dependencies and vector regeneration.

Status: implementation of specification 0.1.0 under conformance review; no release
or publication automation is included. [CHANGELOG.md](CHANGELOG.md) tracks changes.

GPL-3.0-or-later. See [LICENSE](LICENSE).
