<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Security

Report vulnerabilities through [private vulnerability reporting](https://github.com/qso-graph/qso-graph-dxcluster/security/advisories/new).
Keep private recordings and credentials out of public issues.

The security requirements are [QGDXC-SEC-0001 through QGDXC-SEC-0006](spec/QGDXC-SPEC-001.md).
The library forbids unsafe Rust and has no runtime dependencies. Its public input
boundary admits one byte and returns a bounded batch; callers consume that batch
before admitting another byte. Framing and Telnet state have fixed limits.
Source scans include inactive source and test files, with planted violations for
I/O aliases and unsafe code. These conservative scans are not general Rust name
resolution or a proof about arbitrary future source changes.

Only local configuration and fixed startup literals produce output. Received
callsigns, signatures, signal reports, and coordinates are untrusted assertions.
Terminal escaping is separate from stored text. The CLI's default privacy option
is implemented before an event leaves the parser. Exact wire recording requires
the explicit retention flag; storage permissions, retention, and backups remain
the operator's responsibility. Recording errors are reported, including partial
captures. Replay validates record shapes, byte encoding, time ordering, and
connection boundaries; observer notes are separate from received content.

The CLI uses plain TCP. Encryption and remote identity verification are outside
this version's transport adapter. There is no credential or password support.
No tests or verification commands connect to a cluster node.

Dependency review status, imported audits, and exact-version exemptions are in
[docs/DEPENDENCIES.md](docs/DEPENDENCIES.md). No complete third-party source audit
is claimed by an exemption. Live interoperability beyond the pinned evidence in
the specification remains unconfirmed; source-backed synthetic startup exchanges
are tested offline.
