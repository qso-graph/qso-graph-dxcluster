<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Conformance

Target: QGDXC-SPEC-001 version 0.1.0, Event parser, Session engine, and
Application classes; dxspider, arcluster, cccluster, rbn, and auto detection.
Optional features: caller-selected silence timeout, literal DXSpider prompt, caller-configured supported setup-error literals,
third-party data retention, recording, and replay. Passwords are unsupported.
The conformance claim is **draft review only** until the versioned suite and
inspection review cover every applicable rule (QGDXC-VEC-0004).

The [coverage manifest](coverage.json) maps requirements to existing named tests
and vector IDs. Its scenario groups pin the committed suite; coverage rejects
a missing case or a Test requirement supported only by inspection prose.

The normative rules live only in spec/. This document records implementation
locations, verification evidence, and limitations; it does not redefine them.

| Area | Evidence |
|---|---|
| IO, FRM | Library framing.rs, session.rs, and tests/protocol.rs |
| DX, EVT, server mappings | Library parser.rs and tests/protocol.rs |
| SES, startup profiles | Library session.rs and tests/session.rs; vectors/sessions.ndjson |
| Privacy | Library privacy.rs; protocol and CLI rendering tests; sanitization planted-case check |
| REC | CLI recording.rs tests; offline Records and Replay adapters; the application-recording-replay-multiple-connections vector |
| VEC | Independent xtask oracle.rs, generate.rs, scenarios.rs, runner.rs |
| Security policy | xtask checks.rs and its planted cases; excluded fuzz targets |

Inspection: QGDXC-GEN-0001 — The specification uses RFC 2119 keywords; implementation documentation cites its IDs.

Inspection: QGDXC-GEN-0002 — Normative tables define each ID once. Coverage rejects duplicate definitions; documentation links to definitions.

Inspection: QGDXC-GEN-0003 — The target version, classes, profiles, and optional features are identified above.

Inspection: QGDXC-GEN-0004 — Server semantics are limited to the pinned source/capture evidence in Annexes B–E; their UNCONFIRMED items are not promoted to commands or typed mappings.

Inspection: QGDXC-GEN-0005 — Session output has one construction path: configured login, fixed startup literals, and caller-generated readiness token. No posting or arbitrary command API is exposed.

Inspection: QGDXC-IO-0001 — Cargo metadata checks library dependency declarations on every target, including optional and renamed packages. The library has no normal, build, or development dependencies. Source policy scans every Rust file in the library, including tests and inactive source. It conservatively reserves the `path` token to reject plain and nested conditional module redirection. An offline Cargo check supplies compiler dep-info for both production and unit-test library artifacts; canonical source paths are confined to `crates/qso-graph-dxcluster/src`. The compiled external-module and symlink planted tests check that confinement independently of attribute detection. All time and transport outcomes enter through caller methods.

Inspection: QGDXC-SEC-0001 — One-byte admission bounds each returned batch, line storage is bounded, and numeric conversions are checked. Fuzz targets cover values, spots, enhanced input, non-spot input, RBN comments, framing, Telnet, and sessions. Fuzz execution status is reported separately by verify.

Inspection: QGDXC-SEC-0002 — Network input has no path to process, filename, or output-command construction. Session constants and Config are the output sources.

Inspection: QGDXC-SES-0009 — A Session owns one endpoint/identity; the CLI instantiates one Session and completes socket shutdown before replacement. Integrators provide equivalent ownership for multiple sessions.

Inspection: QGDXC-VEC-0001 — Oracle code imports no implementation symbols; captured vectors identify capture basenames and one-based contributing physical record numbers, and synthetic cases identify their derivation.

Inspection: QGDXC-VEC-0004 — Claims remain draft review only until all applicable evidence is complete; example success alone does not establish conformance.

The generator's independent oracle was hand-checked against these synthetic
samples: frequency-0 converts 0.001 kHz to 1 Hz; lexical-0 preserves punctuation
calls without assigning license validity; rbn-comment-2 maps FT8, signed SNR, and
the explicit four-character grid; spider-suffix-0 preserves the full padded
single-digit suffix; startup-cccluster-cccluster admits setup at 1000 ms, arms
its reply deadline at 6000 ms, and accepts completion at 5500 ms. These values
were checked from the specification, not copied from library output.

Additional independent session oracles cover QGDXC-SES-0011:
`stricter-spacing` admits setup at 2000 ms; `stricter-window-lines` uses a
one-line window and admits at 20000 ms; `stricter-window-duration` extends that
window to 30000 ms. Each rejects earlier admission and starts the reply deadline
at admission. `overlong-ar-banner-no-profile-evidence` checks QGDXC-EVT-0010
and QGDXC-ARC-0001: the truncated banner leaves the extended prompt unrecognized
and the session reaches `profile_unknown`.

The CLI rendering tests cover QGDXC-SEC-0003 and QGDXC-SEC-0005 for observer
notes, errors, diagnostics, and events. Human and JSON replay cases check both
retention settings, control escaping, IP replacement, sensitive CC11 slots,
and ambiguous-layout withholding. The render-path inspection found only static
help/version text outside the shared privacy and escaping paths.

The recording tests cover QGDXC-REC-0004 with fragmented, delayed outbound
completion, a reply at 33 seconds after setup admission at 5 seconds, delayed
close boundaries, and fragmented token observations confined to the current
connection. The QGDXC-REC-0004 stream tests check lazy token lookup, byte-budget
failure, missing-token errors, preserved note numbers, and absence of lookahead
for profiles that do not request tokens.
The CLI's controlled connector tests cover QGDXC-SES-0009 without network I/O:
a nonresponding first address consumes its share before a working alternative
connects. Additional cases cover the minimum address budget, remaining deadline,
redistribution after an immediate error, and cancellation before another attempt.

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

Caller integration: use the chunk admission and draining contract in
QGDXC-IO-0005; consume each returned Batch before continuing;
complete Close before a later Connect; deliver due live timers using current
monotonic time and report actual output admission with the Session `output_admitted` method;
handle completion and cancellation before equal-time expiry except stable_ready;
and preserve shared pacing history when replacing a Session for the same
endpoint and canonical identity. The supplied CLI implements one active session.
The CLI's live renderer uses a dedicated writer with a 64-line channel, a
64-line pending reserve for batch and control events, and one line held by the
writer (QGDXC-IO-0005). The session loop pauses socket reads when output is full;
timers, cancellation, transport actions, and recording continue independently.
The chunk tests compare event and action order across every split and enforce
the admission limits. Linux memory tests drain a 1 MiB LF chunk through the
library, live adapter, and replay engine under a 64 MiB ceiling. The replay
stream test also covers a coalesced record containing 750,000 LF bytes within
the physical record cap (QGDXC-REC-0004).
The queue test checks bounds and complete ordered delivery after resumption. A
controlled deadline test checks transport closure and recording under stalled
rendering. On Linux, the stalled-pipe integration test confirms a blocked pipe
write, paused input admission, recording finalization and socket closure before
the writer drain, and exit status 130 with stdout still blocked. A separate test
covers cancellation arriving during an existing drain (QGDXC-IO-0002).

The vector harness follows the abstract ordering in QGDXC-VEC-0006. Its Application vector also writes conforming in-memory recording boundaries and checks replayed content against the same independent complete event oracles. Local configuration-rejection vectors request no connection or output.

Live checks are intentionally absent. The evidence limitations marked
UNCONFIRMED in Annexes B–E remain: charset guarantees, additional or translated
notices, live custom prompts and complete DXSpider startup, additional AR-Cluster
layouts, opaque CC Cluster slot meanings, and additional RBN layouts. No live
network probing was used to resolve them.

The QGDXC-SES-0010 and QGDXC-DSP-0004 reconnect tests run 4,096 stable-ready
connections for each profile, including auto detection and the DXSpider echo
sequence. The engine storage test runs 8,192 attempts and checks unchanged
retained token/rate capacities and reset parser state. Caller-owned token sets
check uniqueness separately. The CLI entropy test exercises fresh OS randomness,
repeated random values, the maximum attempt number, jitter bounds, rejection
sampling, and source errors. On Linux, the replay memory regression streams
500,000 RBN open/close pairs through the CLI under a 64 MiB address-space ceiling;
the fixture is written incrementally and output is discarded. It failed with an
allocation error before the bounded replay change and passes after it.
The vector runner resolves resource requests from the explicit per-attempt
inputs before comparing the abstract action trace (QGDXC-VEC-0006).

The QGDXC-SEC-0001 input audit also covers the CLI and verification tools.
Capture import streams physical records and protocol lines, retaining only a
bounded sample and identity map. Vector validation, coverage, and sanitization
stream NDJSON. Other repository text and captured command output use fixed byte
caps; diagnostic display seeks to a bounded log tail. Tests reject oversized
physical records, protocol lines, text files, indexes, and command output, and
check fragmented capture provenance.

The capture provenance regressions cover QGDXC-SEC-0001 and QGDXC-VEC-0001.
On Linux, the importer processes two million intervening note records under a
64 MiB address-space ceiling and publishes only the two contributing record
numbers. The same case failed with an allocation error before the change.
Unit tests exercise the provenance cap, ignored notes and empty input records,
and contributors to LF, CR, CRLF, and unterminated frames. Retained provenance
is charged against the capture sample budget.

The QGDXC-SEC-0006 scanner tests plant punctuated IPv4, IPv6, mapped IPv4,
and private DNS names in README prose, decoded vector byte members, and inline
encoded fixtures. They cover absolute DNS spelling, case variations, endpoint
ports, quotes, brackets, and sentence punctuation while retaining permitted
documentation addresses and example names.
