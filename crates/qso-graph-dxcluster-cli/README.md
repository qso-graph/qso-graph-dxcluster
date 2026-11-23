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

Repeat `--node` to combine feeds, or use `--replay session.ndjson` for offline
playback. Press `?` for keys and source details, `p` to pause, and `t` to cycle
themes. Use `--theme color|daylight|phosphor|amber|mono` at startup. Layout,
`NO_COLOR`, and color-depth behavior follow Annex F (`QGDXC-TUI-0070` through
`QGDXC-TUI-0073`). Filters select bands,
modes, callsigns, and human or skimmer spots. The `connect` command emits events;
`replay` reads a local recording. Recording is opt-in.

Use `--grid <locator>` (or the sources file's `grid`) to show the next sunrise and
sunset in UTC, plus the moon's illumination and elevation, below the sources on
wide screens. Your locator stays on your computer and is never sent. The world
map shows the grey line and marks the sun and moon. When you supply a locator,
it also marks your position. The calculations follow NOAA's solar method and
Jean Meeus's *Astronomical Algorithms* (QGDXC-TUI-0140–0143, Annex L).

Add `--node pota` for Parks on the Air activator spots, or select a source with
`"node": "pota"` from your sources file. The feed has a one-minute floor between
requests ([Annex F.14](https://github.com/qso-graph/qso-graph-dxcluster/blob/main/spec/annex-f-tui.md#f14-pota-spots)).

See the [project documentation](https://github.com/qso-graph/qso-graph-dxcluster)
for the specification and privacy behavior.

GPL-3.0-or-later. See LICENSE.
