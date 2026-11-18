<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# qg-dxc

An old-school DX cluster terminal built in modern Rust. Several cluster nodes
and the Reverse Beacon Network merge into one live, filterable spot list in
your terminal. The client is receive-only and never sends a password.

Download a self-contained program for macOS, Windows, or Linux from
[GitHub Releases](https://github.com/qso-graph/qso-graph-dxcluster/releases),
or install with Cargo:

```sh
cargo install qso-graph-dxcluster-cli
qg-dxc --help
```

Use a node you are authorized to access and your own operator identity.
This example uses an invented identity and a nonresolving documentation endpoint:

```sh
qg-dxc tui --call Q1ABC --node example.invalid:7300/auto
```

Repeat `--node` to combine feeds. Press `?` for keys. Filters select bands,
modes, callsigns, and human or skimmer spots. The `connect` command emits events;
`replay` reads a local recording. Recording is opt-in.

See the [project documentation](https://github.com/qso-graph/qso-graph-dxcluster)
for the specification and privacy behavior.

GPL-3.0-or-later. See LICENSE.
