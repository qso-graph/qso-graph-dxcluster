<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# QG DX Cluster

Read live DX spots from DX cluster nodes and the Reverse Beacon Network as typed, validated events.

`qso-graph-dxcluster` is a Rust library that logs into DXSpider, AR-Cluster and CC Cluster nodes and the
Reverse Beacon Network telnet feeds, works out which server it is talking to, and turns each line into a
spot, announcement, WWV/WCY report or status event. `qg-dxc` is its command-line client.

- **Receive-only and safe by design.** Every byte from a node is treated as untrusted: bounded lines,
  fuzzed parsers, control characters never reach your terminal, nothing from the stream is ever sent back.
- **Private by default.** Third-party IP addresses and coordinates are dropped unless you ask to keep them.
- **Polite to node operators.** Conservative command pacing, backoff on reconnect, one session per call.
- **Specified first.** Behavior is defined in [QGDXC-SPEC-001](spec/QGDXC-SPEC-001.md) and checked
  against captured sessions from real nodes.

Status: specification draft 0.1.0. No release yet.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
