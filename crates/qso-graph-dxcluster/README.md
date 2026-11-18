<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# QG DX Cluster library

A receive-only sans-I/O DX cluster and Reverse Beacon Network session engine
for Rust applications. The library has no runtime dependencies and opens no
sockets, files, or processes. The caller supplies bytes, time, readiness tokens,
and retry jitter, and receives typed events and connection actions.

Supports DXSpider, AR-Cluster, CC Cluster, and direct Reverse Beacon Network
feeds, with automatic profile detection and bounded parsing of untrusted input.

See the [project documentation](https://github.com/qso-graph/qso-graph-dxcluster)
for the specification, examples, conformance evidence, and caller responsibilities.
For the terminal application, install `qso-graph-dxcluster-cli`.

GPL-3.0-or-later. See LICENSE.
