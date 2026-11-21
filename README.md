<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# QG DX Cluster

An old-school DX cluster terminal built in modern Rust, for radio operators who
want several cluster nodes and the Reverse Beacon Network in one live,
filterable spot list. `qg-dxc` brings the feeds together in your terminal. It is
receive-only and never sends a password.

Download a single self-contained program for macOS, Windows, or Linux from
[GitHub Releases](https://github.com/qso-graph/qso-graph-dxcluster/releases).
Unpack it and run `qg-dxc --help`. Or install from crates.io:

```sh
cargo install qso-graph-dxcluster-cli
```

For developers, `qso-graph-dxcluster` is a sans-I/O Rust library with no runtime
dependencies. It handles framing, profile recognition, startup, and retry policy;
the application owns I/O. It supports DXSpider, AR-Cluster, CC Cluster, and direct
Reverse Beacon Network feeds, with automatic profile detection.

Events preserve received evidence and distinguish spots, announcements, talk,
WWV/WCY reports, prompts, and connection changes. Default privacy handling and
terminal escaping follow [QGDXC-SEC-0003 and QGDXC-SEC-0005](spec/QGDXC-SPEC-001.md).

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
select bands, modes, callsigns, and human or skimmer spots. Retention defaults
to 100 human and 100 skimmer spots; `--keep-human N` and `--keep-skimmer N`
set each cap independently (10–10,000 each), so a skimmer flood keeps human
spots intact (`QGDXC-TUI-0012`). Press `l` to edit both caps while the TUI runs.
Lowering a cap immediately removes that kind’s oldest spots, including while paused.
When two nodes relay the same spot, the list shows it once, and its source reads
`node+1` when one more node carried it; press `d` to show every copy (`QGDXC-TUI-0015`).
Press `s` to hide self-spots, where a station spots itself (`QGDXC-TUI-0045`).
Press `?` for keys;
Scrolling down holds your place while reception continues, and Home returns to the newest spots; `p` pauses and resumes too, and `t` cycles the five themes.
Use `--theme color|daylight|phosphor|amber|mono` to choose a theme at startup.
The responsive layout, activity charts, `NO_COLOR` behavior, and terminal color
selection follow `QGDXC-TUI-0070`–`QGDXC-TUI-0073` in Annex F. Use repeated `--node` options
with one `--call`, or repeated `--replay` options for paced offline playback.
Or list your own nodes in a sources file and choose them when `qg-dxc tui` starts;
see [Choose your nodes when the TUI starts](docs/user/sources.md).
Press `M` (capital) to switch the spot list to a world map: each new spot draws a
great-circle path from the spotter to the DX station in its band's color, and the
path fades out within a minute. Skimmer paths stay dim and brief so RBN never covers
the map. Ends come from the spot's grid square when it has one, otherwise from the
callsign's prefix ([Annex F, F.9](spec/annex-f-tui.md)). The prefix table is the
country file from country-files.com; see [NOTICE.md](NOTICE.md).
The interface requires terminal input and output. See [Annex F](spec/annex-f-tui.md).

Recording is opt-in; exact recording also requires the explicit retention flag.
Replay opens only the specified local file. The same retention flag also applies
to replay; its output follows `QGDXC-SEC-0003` and `QGDXC-SEC-0005`. Ctrl-C closes the active connection.
A node that asks for a password stops the session with `password_unsupported`.
qg-dxc never sends a password over plain Telnet: anyone on the network path can read
it, and passwords are often reused. Password login may come later, only over verified
TLS. Human output has one escaped
line per event; `--json` produces Annex A NDJSON events.

For library integration, start with [the API documentation](crates/qso-graph-dxcluster/src/lib.rs)
and [conformance and caller responsibilities](docs/CONFORMANCE.md).
Run the complete offline check suite with `cargo xtask verify`. See
[CONTRIBUTING.md](CONTRIBUTING.md) for dependencies and vector regeneration.

Status: implementation of specification 0.1.0 under conformance review; no release
or publication automation is included. [CHANGELOG.md](CHANGELOG.md) tracks changes.

GPL-3.0-or-later. See [LICENSE](LICENSE).
