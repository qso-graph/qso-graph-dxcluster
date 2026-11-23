<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# QG DX Cluster

An old-school DX cluster terminal built in modern Rust, for radio operators who
want several cluster nodes, the Reverse Beacon Network and Parks on the Air spots
in one live, filterable spot list. Alongside the spots, `qg-dxc` shows:

- a world map with each spot's path and the grey line
- the NCDXF/IARU beacons on the air now
- NOAA space weather and a band outlook
- your sunrise, sunset and the moon, worked out on your own computer

It is receive-only and never sends a password.

Install it with your package manager:

```sh
brew install qso-graph/tap/qso-graph-dxcluster     # macOS (Apple Silicon), Linux
sudo apt install qso-graph-dxcluster               # Debian, Ubuntu
sudo dnf install qso-graph-dxcluster               # Fedora, RHEL, Rocky
```

apt and dnf need QSO Graph's signed package repository first; the
[packages page](https://github.com/qso-graph/qso-graph-packages) has the two-line setup.
Or download a single self-contained program for macOS (Apple Silicon, signed and
notarized), Windows (x86_64, signed), or Linux (x86_64 and arm64, including
Raspberry Pi) from [GitHub Releases](https://github.com/qso-graph/qso-graph-dxcluster/releases).
Unpack it and run `qg-dxc --help`. Every download carries a build attestation:
`gh attestation verify <file> -R qso-graph/qso-graph-dxcluster`. Or install from crates.io:

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
The latest solar figures from the nodes' WWV and WCY broadcasts (SFI, A, K, and
from WCY expK and R) show under the band chart, and go dim once out of date (F.10).
WWV figures are NOAA's geophysical alert as broadcast by NIST's station WWV; WCY figures
are the hourly summary of DK0WCY, run by volunteers of the DARC. Cluster nodes relay both.
Press `B` (capital) for the NCDXF/IARU beacons: which of the 18 beacons is on
14.100, 18.110, 21.150, 24.930 and 28.200 MHz right now and which is next, worked
out from the clock, with each beacon's recent reception reports from the Reverse
Beacon Network and the source that carried them. On the map, the beacons on air show
as `◆` (F.11). The schedule and the matching of RBN beacon reports are in the
library (its `beacons` module, Annex G), so other programs get the same
answers. The beacon schedule is the
[NCDXF/IARU International Beacon Project](https://www.ncdxf.org/beacon/)'s, published by
the Northern California DX Foundation; the beacons are built and run by NCDXF, the
IARU and their volunteer hosts.

Press `W` (capital) to switch between the spot list and space weather, with data from the
[NOAA Space Weather Prediction Center](https://www.swpc.noaa.gov/).
The weather page shows current figures and alerts, with charts of solar flux, Kp,
X-rays, and solar wind below as space allows (`QGDXC-TUI-0120`, `QGDXC-TUI-0121`).
The weather page's band outlook uses SFI and Kp as a rule of thumb for band conditions (`QGDXC-TUI-0127`).
Fetching starts when you first open the view or use `--space-weather`.
When an alert, warning, or watch is in force, a ribbon shows it on every dashboard page.
Solar flux, planetary K, and NOAA scales have a 15-minute minimum between
requests. X-rays, solar wind, and alerts have a five-minute minimum
(`QGDXC-TUI-0111`).

Use `--grid <locator>` (or the sources file's `grid`) to show the next sunrise and
sunset in UTC, plus the moon's illumination and elevation, below the sources on
wide screens. Your locator stays on your computer and is never sent. The world
map shows the grey line and marks the sun and moon. When you supply a locator,
it also marks your position. The calculations follow NOAA's solar method and
Jean Meeus's *Astronomical Algorithms* (QGDXC-TUI-0140–0143, Annex L).

Add `--node pota` to include activator spots from Parks on the Air, or set
`"node": "pota"` in your sources file and select it at startup. The feed has a
one-minute floor between requests. See [Annex F.14](spec/annex-f-tui.md#f14-pota-spots).

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
The [`qso-graph-http` crate](crates/qso-graph-http/README.md) provides synchronous HTTPS and request scheduling for QSO Graph applications.
Run the complete offline check suite with `cargo xtask verify`. See
[CONTRIBUTING.md](CONTRIBUTING.md) for dependencies and vector regeneration.

Releases are published to GitHub Releases, crates.io, Homebrew, apt and dnf.
[CHANGELOG.md](CHANGELOG.md) tracks changes.

GPL-3.0-or-later. See [LICENSE](LICENSE).
