<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Conformance

Target: QGDXC-SPEC-001 version 0.1.0, Event parser, Session engine, and
Application classes; dxspider, arcluster, cccluster, rbn, and auto detection.
Optional features: caller-selected silence timeout, literal DXSpider prompt, caller-configured supported setup-error literals,
third-party data retention, recording, and replay. Passwords are unsupported.
The conformance claim is **draft review only** until the versioned suite and
inspection review cover every applicable rule (QGDXC-VEC-0004).

The normative rules live only in spec/. This document records implementation
locations, verification evidence, and limitations; it does not redefine them.

| Area | Evidence |
|---|---|
| IO, FRM | Library framing.rs, session.rs, and tests/protocol.rs |
| DX, EVT, server mappings | Library parser.rs and tests/protocol.rs |
| SES, startup profiles | Library session.rs and tests/session.rs; vectors/sessions.ndjson |
| Privacy | Library privacy.rs; protocol tests; sanitization planted-case check |
| REC | CLI recording.rs tests; offline Records and Replay adapters; the application-recording-replay-multiple-connections vector |
| VEC | Independent xtask oracle.rs, generate.rs, scenarios.rs, runner.rs |
| Security policy | xtask checks.rs and its planted cases; excluded fuzz targets |

Inspection: QGDXC-GEN-0001 — The specification uses RFC 2119 keywords; implementation documentation cites its IDs.

Inspection: QGDXC-GEN-0002 — Normative tables define each ID once. Coverage rejects duplicate definitions; documentation links to definitions.

Inspection: QGDXC-GEN-0003 — The target version, classes, profiles, and optional features are identified above.

Inspection: QGDXC-GEN-0004 — Server semantics are limited to the pinned source/capture evidence in Annexes B–E; their UNCONFIRMED items are not promoted to commands or typed mappings.

Inspection: QGDXC-GEN-0005 — Session output has one construction path: configured login, fixed startup literals, and caller-generated readiness token. No posting or arbitrary command API is exposed.

Inspection: QGDXC-IO-0001 — The library manifest has no dependencies. Source policy scans every Rust file in the library, including tests and inactive source; all time and transport outcomes enter through caller methods.

Inspection: QGDXC-SEC-0001 — One-byte admission bounds each returned batch, line storage is bounded, and numeric conversions are checked. Fuzz targets cover values, spots, enhanced input, non-spot input, RBN comments, framing, Telnet, and sessions. Fuzz execution status is reported separately by verify.

Inspection: QGDXC-SEC-0002 — Network input has no path to process, filename, or output-command construction. Session constants and Config are the output sources.

Inspection: QGDXC-SES-0009 — A Session owns one endpoint/identity; the CLI instantiates one Session and completes socket shutdown before replacement. Integrators provide equivalent ownership for multiple sessions.

Inspection: QGDXC-VEC-0001 — Oracle code imports no implementation symbols; captured vectors identify capture basenames and one-based physical record spans, and synthetic cases identify their derivation.

Inspection: QGDXC-VEC-0004 — Claims remain draft review only until all applicable evidence is complete; example success alone does not establish conformance.

The generator's independent oracle was hand-checked against these synthetic
samples: frequency-0 converts 0.001 kHz to 1 Hz; lexical-0 preserves punctuation
calls without assigning license validity; rbn-comment-2 maps FT8, signed SNR, and
the explicit four-character grid; spider-suffix-0 preserves the full padded
single-digit suffix; startup-cccluster-cccluster admits setup at 1000 ms, arms
its reply deadline at 6000 ms, and accepts completion at 5500 ms. These values
were checked from the specification, not copied from library output.

Four sanitized captured samples were also checked directly: captured-0000
(AR-Cluster) reports 10105.0 kHz as 10105000 Hz at 18:56 without a date;
captured-0049 (CC Cluster) preserves comment `up 1.4` and maps 14027000 Hz;
captured-0067 (DXSpider) retains the paired BEL bytes in evidence while omitting
them from the comment; captured-0106 (RBN) maps CW, 5 dB, 22 wpm, CQ, and
3504000 Hz, preserving the full skimmer suffix. Only sanitized vector IDs are
used here; source record references remain in each vector's provenance.

Captured spot samples are selected by layout and RBN comment variant, with a
finite count per category. The generator sorts identities before assigning Q
aliases and applies privacy transformations before writing inputs. The mapping
is not published, and raw recordings are never fixtures. Re-running generation
against the same captures produces identical bytes.

Caller integration: consume each returned Batch before admitting another byte;
complete Close before a later Connect; dispatch timers at their logical times,
with completion and cancellation before equal-time expiry except stable_ready;
and preserve shared pacing history when replacing a Session for the same
endpoint and canonical identity. The supplied CLI implements one active session.
The vector harness follows the abstract ordering in QGDXC-VEC-0006. Its Application vector also writes conforming in-memory recording boundaries and checks replayed content against the same independent complete event oracles. Local configuration-rejection vectors request no connection or output.

Live checks are intentionally absent. The evidence limitations marked
UNCONFIRMED in Annexes B–E remain: charset guarantees, additional or translated
notices, live custom prompts and complete DXSpider startup, additional AR-Cluster
layouts, opaque CC Cluster slot meanings, and additional RBN layouts. No live
network probing was used to resolve them.
