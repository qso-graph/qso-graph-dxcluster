<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex E: Reverse Beacon Network

**Status: draft 0.1.0, 2026-11-16.** Profile `rbn` of
[QGDXC-SPEC-001](QGDXC-SPEC-001.md).

## E.0 Evidence

| Evidence | Location and scope |
|---|---|
| C1 | Capture `rbn-7000.ndjson`, records 2, 4–6: login and ready prompt; records 11, 37, 50, 67, 618: comment variants |
| C2 | Capture `rbn-7001.ndjson`, records 2, 4–6: login and ready prompt; records 6, 11, 14, 26, 40: grid-slot variants and FT4 |
| P1 | RBN's [Telnet servers page](https://www.reversebeacon.net/pages/Telnet+servers+30), [archived rendered-text excerpt](evidence/rbn-telnet-servers-2026-11-16.txt), retrieved 2026-11-16, original page lines 63–72 (archive lines 5–14); SHA-256 `842b0a2e6c5357512741e2a9d7a8d248542ed7aa319adc2bd14c1206908431d9` of the complete archive file including its SPDX header. Service purpose and ports; not a spot grammar source. |

| Capture | SHA-256 |
|---|---|
| `rbn-7000.ndjson` | `8787ceee423afc1883e7ba32a319d58704c5d16ff4b6847cf9bf66db9131bfb8` |
| `rbn-7001.ndjson` | `dc5473dfb3b5eba080fee018cd21583e3e8c6be123c32c0296b0613567a0a170` |

P1 describes throughput-oriented relays without server-side filtering and
prefers end users to use retail clusters. It lists
`telnet.reversebeacon.net` ports 7000 for CW/RTTY and 7001 for FT8; this hostname
is a source reference, not a default node. The captures establish actual CW,
RTTY, FT8, and FT4 line variants, not a permanent port-to-mode guarantee.

## E.1 Login and readiness

Both sessions show an unterminated `Please enter your call: ` (capture
`rbn-7000.ndjson`, record 2; capture `rbn-7001.ndjson`, record 2). After login,
each shows a greeting, local-user and spot-rate status, and a prompt shaped as
`<call> de RELAY <d-Mon-yyyy> <HH:MMZ> >` (record 5 of each capture). The colon
in the prompt's clock differs from live spots' `HHMMZ` time token.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-RBN-0001` | In post-login context, the profile MUST identify RBN and complete its ready check only from the full `<configured-call> de RELAY <d-Mon-yyyy> <HH:MMZ> >` prompt with valid date/time and flexible spaces. Its startup command sequence MUST be empty. It MUST NOT send cluster setup, echo, or history commands. Evidence: C1 records 4–6; C2 records 4–6; the callsign is the only outbound record in each session. | Test |

The `<configured-call>` comparison follows `QGDXC-IO-0006`; prompt and banner
classification follows `QGDXC-EVT-0010`.

## E.2 Core spot grammar and structured comments

The outer line uses core §4. The following comment alternatives are incorporated
by `QGDXC-RBN-0003`. `SP+` permits variable ASCII padding.

```text
CW   SP+ signed_snr SP+ dB SP+ speed SP+ WPM SP+ type
RTTY SP+ signed_snr SP+ dB SP+ speed SP+ BPS SP+ type
FT8  SP+ signed_snr SP+ dB SP+ [slot SP+] type
FT4  SP+ signed_snr SP+ dB SP+ [slot SP+] type
```

The optional `slot` is a single token in the position commonly used for a
four-character locator. `type` can contain a space, notably `NCDXF B`.

| Line property | Capture evidence |
|---|---|
| CW, WPM, `CQ` | Capture `rbn-7000.ndjson`, record 6 |
| `BEACON` | Capture `rbn-7000.ndjson`, record 11 |
| `NCDXF B` | Capture `rbn-7000.ndjson`, record 37 |
| RTTY, BPS, `BEACON` and `CQ` | Capture `rbn-7000.ndjson`, records 50, 67 |
| `DX` | Capture `rbn-7000.ndjson`, record 618 |
| FT8, signed SNR, valid grid and absent slot | Capture `rbn-7001.ndjson`, record 6 |
| Numeric non-grid slot | Capture `rbn-7001.ndjson`, record 11 |
| FT4 with grid, non-grid slot, and absent slot | Capture `rbn-7001.ndjson`, records 14, 26, 40 |
| Eight-byte spotted-station token containing an underscore and no digit; lexical handling follows `QGDXC-DX-0004` | Capture `rbn-7001.ndjson`, record 389, eighth line |

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-RBN-0002` | Direct RBN live lines MUST use the core grammar with no nonspace post-time suffix. Spotter token handling follows `QGDXC-DX-0004`, including SSID-bearing forms such as `Q9XYZ-2-#`. Classification follows `QGDXC-EVT-0006`. Evidence: C1 record 6; C2 records 6, 11. | Test |
| `QGDXC-RBN-0003` | Supported comments MUST map explicit mode, signed decimal `snr_db` in −32768–32767, and a nonnegative integer speed in 0–65535 with its `wpm` or `bps` unit using E.2. `spot_type` MUST accept observed `CQ`, `BEACON`, `NCDXF B`, and `DX`, matching the longest complete type suffix first. Numeric tokens MUST be at most six bytes. These numeric limits are client policy, not server claims. | Test |
| `QGDXC-RBN-0004` | For FT8/FT4, a nonempty slot valid under `QGDXC-EVT-0009` MUST map to `grid`. An absent slot MUST leave `grid` absent. Another single token MUST remain `native.rbn_slot`, without assigning a meaning or copying it to `grid`. Evidence: C2 records 6, 11, 14, 26, 40. | Test |
| `QGDXC-RBN-0005` | If mode, type, units, token count, or numeric values do not match a supported structured comment, the core spot MUST remain a spot with its full `comment` and no structured-comment fields. The parser MUST NOT guess a new mode, type, speed, or grid. A `CC11` line MUST remain `unrecognized_line` with reason `unsupported_layout`. | Test |

The captures contain 1,073 complete spot lines on the CW/RTTY connection and
6,718 on the FT connection after reassembling input chunks (capture
`rbn-7000.ndjson`, records 6–729; capture `rbn-7001.ndjson`, records 6–1004).
Of the latter, 6,157 are FT8 and 561 are FT4; 3,428 have no slot, 3,164 have a
valid grid slot, and 126 have another token. These are observed counts, not
throughput requirements or a deduplication policy.

## E.3 Unconfirmed items and research corrections

- **UNCONFIRMED E-U1:** Meaning of numeric slot tokens such as `1844`. They
  resemble times but are not assigned a time or locator interpretation.
- **UNCONFIRMED E-U2:** Additional modes, spot types, charset, exact width
  guarantees, and whether RBN ever provides an enhanced format. No CC11 behavior
  is established by the absence of such lines in these sessions.
- **UNCONFIRMED E-U3:** Password policy, echo-command support, and idle/keepalive
  guarantees outside these short successful sessions. The passive startup and
  core liveness rule do not rely on them.

The research's FT8 “grid in place of speed” description is incomplete: the slot
can be absent or not a grid. The captured service on port 7001 also carries FT4,
so “FT8” is not an exclusive observed mode. Its claimed generic capture sequence
does not describe RBN: neither RBN recording contains `set/ve7cc`, `sh/dx 5`, or
`bye`; only the callsign was sent. No such commands are inferred from notes.

Server-side filter languages are outside this draft; no filter commands are
specified for the direct relay profile.
