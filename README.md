<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# QG DX Cluster

Watch DX spots from several nodes in one terminal, read typed events, or replay a recorded session offline.

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
qg-dxc tui --call Q1ABC --node example.invalid:7300/auto
qg-dxc tui --replay session.ndjson --replay second-session.ndjson
```

For operators watching several feeds, `qg-dxc tui` combines up to eight sources
in a newest-first spot list with a status line for each source. Display filters
select bands, modes, callsigns, and human or skimmer spots. Press `?` for keys;
`p` pauses scrolling while reception continues. Use repeated `--node` options
with one `--call`, or repeated `--replay` options for paced offline playback.
The interface requires terminal input and output. See [Annex F](spec/annex-f-tui.md).

Recording is opt-in; exact recording also requires the explicit retention flag.
Replay opens only the specified local file. The same retention flag also applies
to replay; its output follows `QGDXC-SEC-0003` and `QGDXC-SEC-0005`. Ctrl-C closes the active connection.
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
