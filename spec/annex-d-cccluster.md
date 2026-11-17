<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex D: CC Cluster

**Status: draft 0.1.0, 2026-11-16.** Profile `cccluster` of
[QGDXC-SPEC-001](QGDXC-SPEC-001.md).

## D.0 Evidence

All observed behavior in this annex comes from capture
`cccluster-n8ux.ndjson`, software banner version 3.397. Its SHA-256 is
`56aee307451b41b67025f4090083b5cf93a858e7faed769b1f2d01fa494e5342`.

| Records | Evidence |
|---|---|
| 2–4 | Welcome banner, `Running CC Cluster software version 3.397`, login request |
| 6–8 | Callsign output, subsequent `login: `, echoed callsign, notices, WWV history table, duplicated ready prompt |
| 9, 11–14, 16–17 | Live `DX de` lines |
| 10, 15 | Announcements with sender and relay in angle brackets |
| 18–20 | `set/ve7cc`, acknowledgment and prompt, first enhanced live spot |
| 21–22 | History request and enhanced history reply |
| 23–25 | Further live enhanced spots |

## D.1 Login and startup

The login request is `Please enter your call:` with CRLF. After the callsign
is sent, another `login: ` fragment arrives, then the callsign echo and welcome
content (capture `cccluster-n8ux.ndjson`, records 4, 6–8). This makes the core
send-once login rule significant: the later fragment is not a second request
to transmit. The post-login prompt has the shape
`<call> de <node> <d-Mon-yyyy> <HHMMZ>   CCC >` and appears twice in record 8.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-CCC-0001` | The profile MUST identify CC Cluster from the line beginning `Running CC Cluster software version ` in banner context or the full post-login `<configured-call> de <node> <d-Mon-yyyy> <HHMMZ> CCC >` prompt with flexible padding and valid date/time. Setup MUST wait for the full post-login prompt even if identification preceded login. Evidence: capture `cccluster-n8ux.ndjson`, records 3, 8. | Test |
| `QGDXC-CCC-0002` | After the ready prompt, the startup sequence MUST send `set/ve7cc` once and await `Enhanced Spots Enabled` followed by the full prompt before entering `ready`. Duplicate prompts MUST NOT repeat setup. No echo, paging, width, resolution, or history command is part of startup. Evidence: capture `cccluster-n8ux.ndjson`, records 18–20. | Test |

The `<configured-call>` comparison follows `QGDXC-IO-0006`; prompt, banner,
and acknowledgment classification follows `QGDXC-EVT-0010`.

## D.2 Live spots and enhanced layout

The two CC11 dialects use different counting conventions in the research.
Here “24 fields” means 24 payload fields after the marker: splitting a captured
CC Cluster line produces **25 tokens**, indexed 0–24, even when token 24 is
empty. DXSpider's “20 fields” includes its marker. These are not the same layout.
Evidence: capture `cccluster-n8ux.ndjson`, records 20, 22–25.

The following table is incorporated by `QGDXC-CCC-0004`. Field positions and
lexical forms are observed; labels in the final column are hypotheses unless
the table explicitly maps them. Opaque storage allows later evidence to assign
meanings without inventing data now.

| Index | Observed form | Mapping or candidate meaning |
|---|---|---|
| 0 | `CC11` | `format: cc11_cccluster` |
| 1 | Decimal frequency | `frequency_hz`, decimal kHz conversion |
| 2 | Callsign | `call` |
| 3 | `d-Mon-yyyy` | `date` |
| 4 | `HHMM` or `HHMMZ` | `time_of_day`; the same date/time position in live and history replies |
| 5 | Text, possibly empty | `comment` |
| 6 | Callsign | `spotter_call` |
| 7 | Integer-looking text | `native.slot_07`; candidate hops, **UNCONFIRMED D-U1** |
| 8–9 | Callsign-like tokens | `native.slot_08`, `native.slot_09`; candidate origin/relay nodes, **UNCONFIRMED D-U1** |
| 10–13 | Integer-looking text | `native.slot_10` through `native.slot_13`; candidate zones, **UNCONFIRMED D-U1** |
| 14–15 | Empty or state-like text | `native.slot_14`, `native.slot_15`; candidate DX/spotter states, **UNCONFIRMED D-U1** |
| 16–17 | Prefix-like text | `native.slot_16`, `native.slot_17`; candidate DX/spotter prefixes, **UNCONFIRMED D-U1** |
| 18–19 | Empty or locator-like text | `native.slot_18`, `native.slot_19`; candidate grids, **UNCONFIRMED D-U1** |
| 20 | Empty in this sample | `native.slot_20`; reserved meaning **UNCONFIRMED D-U1** |
| 21 | `0` or `32` | `native.slot_21`; candidate spot-mode flags, bit meanings **UNCONFIRMED D-U2** |
| 22 | Signed decimal pair separated by `/` | `native.slot_22` only on retention opt-in; candidate latitude/longitude, **UNCONFIRMED D-U3** |
| 23 | IP literal | `native.spotter_ip` only on retention opt-in; privacy-sensitive address slot; identity attribution **UNCONFIRMED D-U3** |
| 24 | Empty or integer-looking text | `native.slot_24`; candidate sequence value, **UNCONFIRMED D-U1** |

No external country-code conversion, mode-bit interpretation, or coordinate
conversion is justified by these observations. In particular, the values `0`
and `32` do not prove whether a spot originated from a human or a skimmer.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-CCC-0003` | Live `DX de` lines MUST use the core grammar with no nonspace post-time suffix in this observed profile. Evidence: capture `cccluster-n8ux.ndjson`, records 9, 11–14, 16–17. | Test |
| `QGDXC-CCC-0004` | Enhanced input MUST have exactly 25 caret-separated tokens, preserving empty final tokens, and use D.2's layout; index 4 MUST accept valid `HHMM` with or without `Z`. Required frequency/calls/date/time MUST validate under the common value rules. Other counts MUST yield `unrecognized_line` with reason `unsupported_layout`; comment-caret recovery MUST NOT be borrowed from DXSpider. Slots 22–23 MUST be designated sensitive for `QGDXC-SEC-0005`, including when the line is malformed. | Test |
| `QGDXC-CCC-0005` | The profile MUST recognize `To ALL de <from> <via> : <text>` with a literal angle-bracketed relay token as `announcement`, mapping `from_call`, `via_call`, target `ALL`, category `all`, and the remaining text. It MUST retain any embedded date/time as text. Other talk or propagation-table layouts remain unsupported content. Evidence: capture `cccluster-n8ux.ndjson`, records 10, 15; the history table in record 8 is not a live WWV record. | Test |

## D.3 Unconfirmed items and research corrections

- **UNCONFIRMED D-U1:** Semantic definitions of opaque slots 7–20 and 24,
  including hops versus a country code, origin versus relay, zone/state/prefix/grid
  assignments, the empty slot, and sequence lifetime or ordering. Captures prove
  positions and text, not all meanings asserted in the research.
- **UNCONFIRMED D-U2:** Spot-mode flag meanings, including `0` and `32`, and
  other possible values. No numeric value is mapped to mode or spot kind.
- **UNCONFIRMED D-U3:** Which party slot 22 locates, coordinate order and
  longitude sign convention, and whether slot 23 always identifies the original
  spotter rather than another participant. Both slots remain privacy-sensitive.
- **UNCONFIRMED D-U4:** Caret escaping in comments and other fields. The observed
  count is accepted; a different count has no speculative recovery algorithm.
- **UNCONFIRMED D-U5:** Password policy, additional setup commands and echo
  readiness, idle/keepalive policy, and non-ASCII charset beyond this session.

The capture closes the research's earlier unknown login/banner/prompt spelling.
Its later “24 fields” description omits the marker when counting. The stronger
semantic claims about those fields remain unproven without a CC11 source table
or independently discriminating capture; a CC61 table is not a CC11 definition.

Server-side filter syntax and extra output settings are future annex material.
