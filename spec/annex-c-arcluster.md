<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex C: AR-Cluster

**Status: draft 0.1.0, 2026-11-16.** Profile `arcluster` of
[QGDXC-SPEC-001](QGDXC-SPEC-001.md).

## C.0 Evidence

| Evidence | Location and scope |
|---|---|
| C1 | Capture `arcluster-k1sa.ndjson`, records 2, 4–7: login, version 6.1.5123, prompt, live spot; records 21–22: rejected enhanced-output command; records 23–25: history layout |
| C2 | Capture `arcluster-iw0erq-8.ndjson`, records 2–3, 5–8: login, version 6.6.247, prompt, live spot; records 22–23: rejected enhanced-output command; records 25–26: history layout |
| P1 | [AR-Cluster 6.6 user manual](https://arcluster.org/user-guide/), [archived rendered-text excerpt](evidence/arcluster-user-guide-2026-11-16.txt), retrieved 2026-11-16, original page lines 24–29 (archive lines 5–10); SHA-256 `c11a2bfd173f9e0a0f80b071c88f6ddaa70726ecb61b58d0942a69197022f474` of the complete archive file including its SPDX header. Original line 26 documents an optional listener password; no password-required session is supplied. |

| Capture | SHA-256 |
|---|---|
| `arcluster-k1sa.ndjson` | `17481b7b88f5ccf15d390448cbf513e74f86f7a9847b61e17f89144f56e25413` |
| `arcluster-iw0erq-8.ndjson` | `f4a8d114cea9283d83d5f6dfb84aa21b924bebe9da0d2bb3512ad3ce760a9a1e` |

## C.1 Login, detection, and readiness

The 6.1 session ends its login request with `Please enter your call:` followed
by padding and CRLF (capture `arcluster-k1sa.ndjson`, record 2). The 6.6 session
has `Enter your callsign`, blank lines, and an unterminated `login: ` (capture
`arcluster-iw0erq-8.ndjson`, records 2–3). Both then show a prompt ending in
`arc6>`; the date lacks a year, and the 6.6 month is lowercase (capture
`arcluster-k1sa.ndjson`, record 6; capture `arcluster-iw0erq-8.ndjson`, record 7).
Neither session asked for a password. Core login matching and password blocking
apply without assuming that every node has the same access policy.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-ARC-0001` | In post-login context, the profile MUST recognize a complete `<configured-call> de <node> <d-Mon> <HHMMZ> arcN>` prompt, where `N` is one or more decimal digits, with flexible spaces and case-insensitive month text. It MUST require a matching AR-Cluster banner before accepting a digit other than the observed `6`. Evidence: C1 records 5–6; C2 records 6–7. Other digits are an explicitly bounded recognition extension, not observed server versions. | Test |
| `QGDXC-ARC-0002` | The complete recognized post-login prompt MUST establish identity and complete this profile's ready check. The startup command sequence MUST be empty; no echo, paging, width, beep, or enhanced-output command is automatically sent. Evidence for the passive ready marker: C1 records 5–7; C2 records 6–8. | Test |

The `<configured-call>` comparison follows `QGDXC-IO-0006`; prompt and banner
classification follows `QGDXC-EVT-0010`.

## C.2 Line mappings

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-ARC-0003` | Live `DX de` lines MUST use the core grammar with no nonspace post-time suffix in this capture-backed profile. Ordinary comment text, including apparent mode or geographic text, MUST follow `QGDXC-DX-0005`. Evidence: C1 records 7–20; C2 records 8–21. | Test |
| `QGDXC-ARC-0004` | The profile MUST NOT send `set/ve7cc` or interpret a received `CC11` line using another profile. Such a line MUST produce `unrecognized_line` with reason `unsupported_layout`. Evidence: C1 records 21–22; C2 records 22–23 reject that command. | Test |
| `QGDXC-ARC-0005` | History rows shaped as `<frequency> <call> <date> <HHMMZ> <comment> <spotter-in-angle-brackets>` MUST remain `unrecognized_line` with reason `unsupported_layout` in this receive-live profile; they MUST NOT be mistaken for live `DX de` spots. Evidence: C1 record 25; C2 record 26. | Test |

Both captured versions reject `set/ve7cc` as an unknown command. This is the
evidence for “AR-Cluster has no CC11” in this draft; it is not proof about every
future release. The profile does not query history. It retains unrelated banner
and notice text under the common event rules, without borrowing DXSpider's
announcement, talk, WWV, or WCY grammar.

## C.3 Unconfirmed items and research corrections

- **UNCONFIRMED C-U1:** Exact interactive first-login requests for name and
  location, and how to complete them. No automatic replies are specified.
- **UNCONFIRMED C-U2:** Portable settings and acknowledgments for paging, width,
  echo, beep, and a command-based echo readiness check across the captured
  versions. The empty startup sequence avoids assuming DXSpider syntax.
- **UNCONFIRMED C-U3:** The older documented post-time state/country suffix and
  version-specific announcement, talk, WWV, and WCY layouts. These are outside
  the capture-backed mappings above.
- **UNCONFIRMED C-U4:** Idle-disconnect and keepalive guarantees, and non-ASCII
  server charset. Short ASCII sessions cannot establish either.

The research's early `arc>` shorthand omits the digit in the observed `arc6>`
prompt. Its later capture summary supplies the correct spelling. The statement
“Password: none for users” is broader than the captures support and contradicts
P1's optional listener password. The core blocks password requests regardless
of server profile. The 6.6 capture's banner cites the active AR-Cluster
documentation site (capture `arcluster-iw0erq-8.ndjson`, records 2, 6); the draft
does not rely on a defunct vendor site for a protocol rule.

Server-side filter languages are future annex material.
