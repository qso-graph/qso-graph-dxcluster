<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex A: event model

**Status: draft 0.1.0, 2026-11-16.** Part of [QGDXC-SPEC-001](QGDXC-SPEC-001.md).

## A.1 Envelope and values

The tables in this annex define the abstract event shape under
`QGDXC-EVT-0001` and `QGDXC-EVT-0003`. JSON is used for vector oracles, not as a
new server wire format.

| Member | Type and meaning |
|---|---|
| `spec` | Version string, `0.1.0` |
| `server_type` | `unknown`, `dxspider`, `arcluster`, `cccluster`, or `rbn` |
| `kind` | `spot`, `announcement`, `talk`, `wwv`, `wcy`, `prompt_status`, `connection_state`, or `unrecognized_line` |
| `fields` | Kind-specific object from A.2 |
| `raw_line` | Application content bytes after `QGDXC-FRM-0008`, excluding the line ending; canonical padded base64 in vectors |
| `raw_ending` | `crlf`, `lf`, `cr`, or `none` |
| `raw_status` | `exact`, `redacted`, `withheld`, `truncated`, or `synthetic` |
| `encoding` | `utf8`, `latin1`, or `none` |
| `received_at` | Optional caller-supplied UTC RFC 3339 receipt time; independent of the reported spot time |

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-EVT-0001` | Each event MUST contain the required envelope members above and a `fields` object for its kind; `received_at` is optional. Every received event MUST carry its own frame bytes, with evidence exceptions and statuses governed by `QGDXC-FRM-0003` and `QGDXC-SEC-0005`. | Test |
| `QGDXC-EVT-0002` | A locally caused connection transition MUST use empty `raw_line`, `raw_ending: none`, `raw_status: synthetic`, and `encoding: none`. A transition caused by received content MUST carry that content's evidence. Reusing evidence for a prompt event and its resulting transition is permitted; inventing a received line is not. | Test |
| `QGDXC-EVT-0003` | Events MUST use A.2's field names, types, units, and presence rules. Absent optional data MUST be omitted, not replaced with a guessed value, null, or zero; an observed empty text field MUST remain an empty string where its mapping defines that field. | Test |
| `QGDXC-EVT-0004` | Events MUST preserve input order. A framed line MUST produce one content event; a session transition MAY additionally produce a connection event immediately after its triggering content event. A parser MUST NOT silently deduplicate spots or merge separate observations. | Test |
| `QGDXC-EVT-0005` | Integers MUST be exact in −9007199254740991 through 9007199254740991; frequency uses the tighter core range. Text MUST follow `QGDXC-FRM-0005`. A mapped UTC date MUST be a valid Gregorian `YYYY-MM-DD` in years 0001–9999; `time_of_day` MUST be `HH:MM` in UTC without invented seconds. | Test |

## A.2 Event fields

### Spot

| Field | Type | Presence and meaning |
|---|---|---|
| `spot_kind` | Enum: `human`, `skimmer` | Required; classification rule A.3 |
| `source` | Enum: `cluster`, `rbn` | Required; delivery service, rule A.3 |
| `format` | Enum: `dx_de`, `cc11_dxspider`, `cc11_cccluster` | Required |
| `spotter_call` | Text | Required; exact received token, subject to `QGDXC-SEC-0005` |
| `call` | Text | Required; spotted callsign token, subject to `QGDXC-SEC-0005` |
| `frequency_hz` | Integer | Required; exact reported frequency |
| `time_of_day` | UTC minute | Required; reported UTC time |
| `date` | UTC date | Optional; only when the spot record itself supplies it |
| `comment` | Text | Required; empty is allowed |
| `mode` | Text | Optional; explicit annex-defined structured mode |
| `snr_db` | Integer | Optional; reported signal-to-noise ratio in dB |
| `speed` | Integer | Optional; reported nonnegative speed |
| `speed_unit` | Enum: `wpm`, `bps` | Present exactly when `speed` is present |
| `grid` | Text | Optional; explicit valid locator of the spotted station |
| `spot_type` | Text | Optional; e.g., RBN `CQ`, `BEACON`, `NCDXF B`, or `DX` |
| `native` | Object | Optional; server-annex fields and opaque slots, subject to privacy |

### Other received content

| Kind | Required fields | Optional fields and interpretation |
|---|---|---|
| `announcement` | `text` | `from_call`, `target`, `via_call`, `category` (`all` or `weather`); only from a supported annex layout |
| `talk` | `from_call`, `to_call`, `text` | No command or reply action implied |
| `wwv` | `from_call`, `hour`, `text` | `sfi`, `a`, `k` integers and `forecast` text when explicitly parsed |
| `wcy` | `from_call`, `hour`, `text` | `k`, `expected_k`, `a`, `r`, `sfi` integers; `sa`, `gmf`, `au` text when explicitly parsed |
| `prompt_status` | `status`, `text` | `status`: `login_prompt`, `server_identity`, `ready_prompt`, `setup_reply`, `notice`, or `password_prompt`; optional `native` object |
| `unrecognized_line` | `reason`, `text` | Core framing reasons, `before_identification`, `unsupported_layout`, or `privacy_withheld` under `QGDXC-SEC-0005`; dispatch follows `QGDXC-EVT-0010`; `text` is empty when evidence is withheld |

`hour` is an integer 0–23. Weather and propagation values report what the node
said; this draft makes no forecast accuracy claim. The received `text` is the
decoded content after permitted privacy transformations, not terminal-safe output.

### Connection state

| Field | Type and meaning |
|---|---|
| `state` | State name from core §3.1, also `backoff` and `blocked` |
| `reason` | Machine-readable local reason: `started`, `connected`, `login_sent`, `identified`, `setup_complete`, `eof`, `transport_error`, `cancelled`, `timeout`, `profile_unknown`, `profile_mismatch`, `password_unsupported`, `setup_failed`, `refused`, `displaced`, `silence_timeout`, `resource_limit`, `unsupported_transport`, `retry_scheduled`, or `retry_exhausted` |
| `retry_in_ms` | Optional nonnegative integer; supplied only when scheduling backoff |

## A.3 Mapping rules

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-EVT-0006` | `source` MUST be `rbn` for a direct Annex E session and `cluster` for Annexes B–D, including a cluster-relayed skimmer spot. A caller-selected profile MUST supply this delivery-service context even before identity evidence arrives. `spot_kind` MUST be `skimmer` for direct RBN or a spotter token ending in `-#`; otherwise it MUST be `human` as a protocol classification, without claiming a human actually authored it. An unconfirmed numeric spot-mode value MUST NOT override this rule. | Test |
| `QGDXC-EVT-0007` | Native fields with unknown meanings MUST remain opaque text under the annex's names; they MUST NOT populate common fields by analogy to another dialect. Empty native slots MUST retain their positions. Unsupported content MUST remain observable as `unrecognized_line`, and known session notices as `prompt_status`. | Test |
| `QGDXC-EVT-0008` | For supported non-spot layouts, a parser MUST extract only the fields established by the annex, validate required calls and hour, and classify incomplete required syntax as `unrecognized_line` with reason `malformed`. Optional numeric failures MUST omit that numeric member while preserving the text. Recognition order follows `QGDXC-EVT-0010`. | Test |
| `QGDXC-EVT-0009` | A common `grid` MUST contain exactly two ASCII letters A–R followed by two digits, matching letters case-insensitively and preserving received case. Invalid or empty candidates MUST leave the common grid absent while remaining in any annex-defined native slot. This draft admits four-character locators only. | Test |
| `QGDXC-EVT-0010` | Classification MUST follow the ordered dispatch and fallback rules in A.4. Each frame MUST have a single deterministic content kind and status/reason; a reserved prefix that fails its parser MUST NOT fall through to talk. | Test |

## A.4 Recognition order and fallback

This table is incorporated by `QGDXC-EVT-0010`. Recognizers use their core or
annex-defined state and profile contexts. A recognition does not broaden its
session-action permissions. Incremental prompt consumption still follows
`QGDXC-FRM-0004`; the dispatch table resolves competing interpretations.
Results remain subject to `QGDXC-SEC-0005`.

| Order | Recognition | Content result |
|---|---|---|
| 1 | Framer-designated blank, partial, or overlong frame (except an annex-defined terminal notice discovered during end-of-input finalization under `QGDXC-SES-0014`) | `unrecognized_line` with the framing reason; no semantic parsing |
| 2 | Applicable password/login prompt, then annex-defined refusal/displacement notice | `prompt_status` with `password_prompt`, `login_prompt`, or `notice`, respectively |
| 3 | Complete annex identity signature, full ready prompt, or current expected setup acknowledgment/token-only reply | `prompt_status` with `server_identity`, `ready_prompt`, or `setup_reply`, respectively; a prompt that also identifies the profile uses `ready_prompt` |
| 4 | In `auto` with no identified profile, literal prefix `DX de ` or `CC11^` at frame start | `unrecognized_line`/`before_identification`; evidence follows `QGDXC-EVT-0001`; no delivery service is inferred from the comment or spotter suffix |
| 5 | Literal prefix `DX de ` at frame start | Core/annex spot parse; failure is `unrecognized_line`/`malformed` |
| 6 | Literal prefix `CC11^` | Selected annex's enhanced-layout parse or its specified failure |
| 7 | Selected annex's specific announcement, weather, or propagation prefix: `To ALL de `, `WX de `, `WWV de `, `WCY de ` | Annex mapping; failed required syntax is `unrecognized_line`/`malformed`. An unsupported layout is `unrecognized_line`/`unsupported_layout` |
| 8 | Selected annex's generic talk layout, if supported | `talk` on success; failed required syntax is `unrecognized_line`/`malformed` |
| 9 | Selected annex's unsupported-layout recognizers, including AR-Cluster history rows under `QGDXC-ARC-0005` | Annex-defined result under the cited requirement |
| 10 | Any remaining frame | `unrecognized_line`/`unknown` |

Only specified identity signatures are `server_identity`. Unmatched banners,
MOTD, version text, name/location requests, paging text, and unsolicited or
out-of-step setup acknowledgments take the final fallback. For example, the
DXSpider `running DXSpider` version banner is `unrecognized_line`/`unknown`;
`Capabilities:` in its applicable context uses row 3. A ready prompt remains
`ready_prompt` on repetition without restarting setup. An acknowledgment/token
is `setup_reply` only while that reply is awaited. If a full ready prompt also
completes a setup step, it keeps `ready_prompt`. A malformed reserved prefix
never acquires another event kind through generic talk matching. For line-only
vectors, recognition context is defined in core §7.2.

Evidence-byte fidelity and third-party privacy are reconciled only in
`QGDXC-SEC-0005`; this annex does not create a second raw-data retention rule.
