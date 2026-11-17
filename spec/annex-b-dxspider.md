<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex B: DXSpider

**Status: draft 0.1.0, 2026-11-16.** Profile `dxspider` of
[QGDXC-SPEC-001](QGDXC-SPEC-001.md).

## B.0 Evidence

Source pin S1 is the DXSpider repository at full commit
`9b55f03ff870d50a9d3e57090dc7408035a4a2b1`. Every source link below uses that
commit; line numbers refer to the source file, not a rendered web page.

| Evidence | Location and scope |
|---|---|
| S1 login | [perl/ExtMsg.pm:123, 136–158, 185–207](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/ExtMsg.pm#L123): uppercase login input, invalid-callsign refusal (including disallowed slash calls), conditional password, timer removal on connection, unterminated login prompt, 60-second login timer |
| S1 login aliases | [perl/cluster.pl:290–295](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/cluster.pl#L290); [perl/DXUtil.pm:689–696](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/DXUtil.pm#L689): login suffix canonicalization motivates the local restriction in `QGDXC-IO-0003` |
| S1 identity and prompt | [perl/DXCommandmode.pm:101–151, 243–245, 686–703](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/DXCommandmode.pm#L101); [perl/Messages:173, 274](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/Messages#L173): capabilities and configurable prompt |
| S1 limits | [perl/DXCommandmode.pm:68–70, 417–431, 614–618](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/DXCommandmode.pm#L68): command threshold and idle prompt |
| S1 displacement | [perl/cluster.pl:198–199, 334–375](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/cluster.pl#L334); [perl/Messages:47](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/Messages#L47): default older-session displacement |
| S1 spots | [perl/DXCommandmode.pm:1026–1096, 1152–1153](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/DXCommandmode.pm#L1026): widths, suffix slots, and BEL bytes |
| S1 CC11 | [perl/VE7CC.pm:22–53](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/VE7CC.pm#L22), [perl/Spot.pm:253–283](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/Spot.pm#L253), [perl/DXUtil.pm:93–112](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/DXUtil.pm#L93): field ordering, escaping, dates and times |
| C1 | Capture `dxspider-wa9pie-2.ndjson`, records 2, 4–6, 16–20: login, identity, live format, setup, and CC11 |
| C2 | Capture `dxspider-w3lpl.ndjson`, records 2–3, 5–8, 18–22: split login and prompt, identity, BEL suffix, and CC11 |

Original capture SHA-256 values:

| Capture | SHA-256 |
|---|---|
| `dxspider-wa9pie-2.ndjson` | `b663ab9e376a8bfc3df40f1bca4d8fd06d04e9465878dcbb0a476ae86b4b983b` |
| `dxspider-w3lpl.ndjson` | `8892d72e7777ee36c9aaa9abbfa57c308dea57d1b7e44922cd8f8ad194eb1961` |

## B.1 Session behavior

The observed login is `login: ` without a line ending (capture
`dxspider-wa9pie-2.ndjson`, record 2; capture `dxspider-w3lpl.ndjson`, records
2–3). Post-login content includes a version banner, `Capabilities: ve7cc rbn`,
and a prompt shaped as `<call> de <node> <date> <HHMMZ> dxspider >`
(capture `dxspider-wa9pie-2.ndjson`, record 5; capture `dxspider-w3lpl.ndjson`,
records 6–7). The prompt is user-configurable in S1 identity and prompt.

S1 limits sets a maximum of 27 commands per 20-second accounting interval and
disconnects on a greater count. Its idle-prompt fallback is 660 seconds, applies
in prompt states, and is configurable. S1 login removes the login timer when
connected; these sources do not impose a general application heartbeat.
S1 displacement defaults to dropping an existing matching session on a new
login. The applicable client policies are `QGDXC-SES-0008` through `0011`.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-DSP-0001` | In post-login identity context, the profile MUST recognize a line beginning `Capabilities:` with whitespace-separated capability tokens containing `ve7cc` and `rbn` as DXSpider evidence. Capability words quoted inside unrelated content MUST NOT identify the profile. Evidence: C1 record 5; C2 record 6; S1 identity. | Test |
| `QGDXC-DSP-0002` | The default ready-prompt recognizer MUST accept the full `<configured-call> de <node> <d-Mon-yyyy> <HHMMZ> dxspider >` layout, allowing padding and case-insensitive month names, with a valid date/time. A caller MAY supply a literal custom prompt configured independently of network text, bounded by `QGDXC-FRM-0003`. That literal MUST end in `>` or `:`; other configurations MUST be rejected. Matching MUST consume the entire literal through its last byte, using `QGDXC-FRM-0004`. After that prompt, setup MUST follow B.2 and readiness requires its token exchange. Evidence: C1 record 5; C2 records 6–7; S1 prompt. The custom-prompt restrictions are client policy. | Test |
| `QGDXC-DSP-0003` | In a post-login DXSpider session (`await_identity`, `setup`, or `ready`), a whole notice matching the S1 displacement template `Reconnected as <configured-call> at <address>, this instance is disconnected` MUST produce `prompt_status`/`notice` followed by the `displaced` outcome in `QGDXC-SES-0013`. The address follows the core privacy rule. Evidence: S1 displacement. | Test |
| `QGDXC-DSP-0009` | In `await_login` or `await_identity`, for selected `dxspider` or `auto` before profile identification, a whole line matching `Sorry <configured-call> is an invalid callsign` (allowing outer ASCII padding and using `QGDXC-IO-0006` for the call) MUST produce `prompt_status`/`notice` followed by the `refused` outcome in `QGDXC-SES-0013`. This pre-identification safety recognizer MUST NOT itself establish profile identity. Other text, including a bare `Sorry`, MUST NOT be treated as this refusal. Evidence: S1 login, `perl/ExtMsg.pm:123, 138, 152–157`. | Test |

Identity comparison in these layouts follows `QGDXC-IO-0006`. S1 login
uppercases input; that observation does not change the configured spelling sent
under `QGDXC-SES-0002`. The refusal recognizer covers the source's English
invalid-callsign response before a capabilities banner can arrive. Refusal
wording in other languages and other profiles is **UNCONFIRMED B-U4**.

## B.2 Startup sequence

This table is incorporated by `QGDXC-DSP-0004`; acknowledgments below are the
source's English text. Receipt of the next full prompt completes a step after
its acknowledgment. An error or another language requires an explicitly
configured supported recognizer or the core setup-failure outcome.

| Order | Literal output, before CRLF | Required acknowledgment | Source |
|---|---|---|---|
| 1 | `set/page 0` | `Page Length is now 0 lines` | [cmd/set/page.pl:8–13](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/cmd/set/page.pl#L8); [perl/Messages:260](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/Messages#L260) |
| 2 | `set/nobeep` | `Beeps are now off` | [cmd/Aliases:109](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/cmd/Aliases#L109), [cmd/unset/beep.pl:8–11](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/cmd/unset/beep.pl#L8), [perl/Messages:29](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/Messages#L29) |
| 3 | `unset/echo` | `Echoing disabled` | [cmd/unset/echo.pl:8–11](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/cmd/unset/echo.pl#L8); [perl/Messages:123](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/Messages#L123) |
| 4 | `set/ve7cc` | `Operation successful` | [cmd/set/ve7cc.pl:15–17](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/cmd/set/ve7cc.pl#L15); capture `dxspider-wa9pie-2.ndjson`, records 16–18 |
| 5 | `echo <token>` | A complete line equal to the token | [cmd/echo.pl:10–15](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/cmd/echo.pl#L10) |

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-DSP-0004` | Each new DXSpider login MUST execute the B.2 sequence once. The caller-generated token MUST consist of `QGDXC` plus 16 uppercase hexadecimal digits, be different for each attempt, and contain no escapes or whitespace. The engine MUST request a caller-supplied token when each attempt starts, retaining only its current token, with no preallocated pool. Live adapters MUST obtain fresh OS randomness for each token and ensure uniqueness across attempts without retaining token history; deterministic harnesses MUST supply each token explicitly. Readiness MUST require the exact token-only received line after the echo command, followed by the full ready prompt; an echoed `echo <token>` command line is insufficient. | Test |

The first three commands and echo are source-backed but were not exercised in
these captures. `set/ve7cc` succeeds in capture `dxspider-wa9pie-2.ndjson`, records
16–18, and capture `dxspider-w3lpl.ndjson`, records 18–20. Its flag is set on the
session object by S1's command, so this profile reapplies it after reconnect.
No history request is part of this startup sequence.

## B.3 Live spots and CC11

`QGDXC-DSP-0006` incorporates the following zero-based logical layout. The name
“20 fields” includes the `CC11` marker: there are 19 payload fields. The comment
can contain additional carets, so a simple split can produce more than 20 tokens.
Evidence: S1 CC11; capture `dxspider-wa9pie-2.ndjson`, records 18, 20–23;
capture `dxspider-w3lpl.ndjson`, records 20, 22–23, 25–26.

| Index | Content | Event field or native member |
|---|---|---|
| 0 | `CC11` | `format: cc11_dxspider` |
| 1 | Frequency, decimal kHz | `frequency_hz` |
| 2 | Spotted callsign | `call` |
| 3 | UTC `d-Mon-yyyy`; day may have a leading space | `date` |
| 4 | UTC `HHMMZ` | `time_of_day` |
| 5 | Unescaped comment, possibly containing carets | `comment` |
| 6 | Spotter callsign | `spotter_call` |
| 7–8 | DX and spotter country codes | `native.dx_country_code`, `native.spotter_country_code`, opaque decimal text; no ADIF equivalence asserted |
| 9 | Origin; carets replaced with `~` | `native.origin`; no reverse substitution |
| 10–13 | DX ITU, DX CQ, spotter ITU, spotter CQ | `native.dx_itu`, `native.dx_cq`, `native.spotter_itu`, `native.spotter_cq`, text |
| 14–15 | DX state, spotter state | `native.dx_state`, `native.spotter_state`, text |
| 16–17 | Country-description strings from prefix data | `native.dx_country_text`, `native.spotter_country_text`, opaque text |
| 18–19 | Four-character DX and spotter grids, possibly empty | `native.dx_grid`, `native.spotter_grid`; valid nonempty DX grid also maps to `grid` |

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-DSP-0005` | For live spots, the profile MUST use the core grammar and allow zero or two trailing BEL bytes and an optional post-time suffix `SP+ (grid / state / zone) SP*`: a four-character grid under `QGDXC-EVT-0009`, two ASCII letters, or one/two decimal digits. `SP+` and `SP*` mean one-or-more and zero-or-more ASCII spaces. With a nonspace extra, `native.trailing_extra` MUST contain the exact ASCII span after the final time's `Z` and before the optional BEL pair or line delimiter, including all leading and trailing spaces; with spaces alone it MUST be absent. The suffix MUST remain opaque without deciding which user setting produced it. Evidence: S1 spots, especially `perl/DXCommandmode.pm:1059–1075, 1094`; C1 record 6; C2 record 8. | Test |
| `QGDXC-DSP-0006` | The CC11 parser MUST take marker through time from the first five tokens and logical fields 6–19 from the last 14 tokens; everything between is the comment, joined by literal carets. It MUST preserve empty final tokens and apply the B.3 table, validate required calls/frequency/date/time, and reject fewer than 20 tokens. It MUST NOT apply Annex D's layout. | Test |
| `QGDXC-DSP-0007` | A DXSpider-relayed RBN comment MUST NOT be parsed using Annex E's direct-feed comment grammar. Its spotter `-#` classification follows `QGDXC-EVT-0006`; unassigned comment contents remain text. Evidence: [perl/RBN.pm:563, 598, 626–638](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/RBN.pm#L563), which constructs a different comment and spotter representation. | Test |

## B.4 Other received lines

These layouts are incorporated by `QGDXC-DSP-0008`. All are source-backed;
the supplied sessions do not contain every event kind.

| Layout | Kind and mapping | Evidence |
|---|---|---|
| `To ALL de <from>: <text>` | `announcement`, target `ALL`, category `all` | [perl/DXCommandmode.pm:924–957](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/DXCommandmode.pm#L924) |
| `WX de <from>: <text>` | `announcement`, target `WX`, category `weather` | Same source, target `WX` branch |
| `<to> de <from>: <text>` | `talk`, calls from the two tokens | [perl/DXCommandmode.pm:960–970](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/DXCommandmode.pm#L960) |
| `WWV de <from> <hour>: SFI=<n>, A=<n>, K=<n>, <forecast>` | `wwv`; angle brackets enclose the decimal hour | [perl/DXCommandmode.pm:1178–1179](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/DXCommandmode.pm#L1178) |
| `WCY de <from> <hour> : K=<n> expK=<n> A=<n> R=<n> SFI=<n> SA=<text> GMF=<text> Au=<text>` | `wcy`; angle brackets enclose the decimal hour | [perl/DXCommandmode.pm:1203–1204](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/DXCommandmode.pm#L1203) |

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-DSP-0008` | The profile MUST recognize B.4 layouts with flexible ASCII padding and optional paired trailing BEL bytes and map their explicit fields under Annex A. Recognition order and deterministic banner/setup fallback MUST follow `QGDXC-EVT-0010`. It MUST NOT turn paging text or requests for name/location into automatic replies. | Test |

## B.5 Unconfirmed items and evidence corrections

- **UNCONFIRMED B-U1:** The actual node character set for non-ASCII output. The
  core decoding policy does not claim a server encoding.
- **UNCONFIRMED B-U2:** Live behavior of custom prompts and translated setup
  acknowledgments beyond the source-backed defaults. The complete B.2 sequence
  has not been captured end to end; this is a verification gap, not a guessed
  server capability.
- **UNCONFIRMED B-U3:** Live-server behavior involving IAC bytes. No inbound
  `ff` byte occurs in the seven supplied captures: `dxspider-wa9pie-2.ndjson`,
  records 1–27; `dxspider-w3lpl.ndjson`, records 1–29;
  `arcluster-k1sa.ndjson`, records 1–34; `arcluster-iw0erq-8.ndjson`, records
  1–46; `cccluster-n8ux.ndjson`, records 1–29; `rbn-7000.ndjson`, records
  1–730; `rbn-7001.ndjson`, records 1–1005 (inbound byte records within those
  ranges). The passive stripper in `QGDXC-FRM-0008` is a defensive client
  measure, not a negotiated or capture-verified server capability.
- **UNCONFIRMED B-U4:** Other refusal text, including translated DXSpider
  responses and refusal notices on Annex C–E servers. No semantic refusal
  recognizer is inferred from generic error text or EOF. The bare `Sorry` at
  S1 `perl/ExtMsg.pm:159–171` is a password-failure response; this draft stops
  at the password prompt and never reaches that exchange.

The research described the paging buffer as holding up to 20 broadcasts. At the
pinned source, [perl/DXChannel.pm:518–527](https://github.com/EA3CV/dxspider/blob/9b55f03ff870d50a9d3e57090dc7408035a4a2b1/perl/DXChannel.pm#L518)
removes the oldest entry when the size reaches 20, leaving at most 19 after that
operation. Paging is disabled by B.2; no buffer-capacity promise is made here.
The research's two-angle-bracket notation for WWV/WCY was a placeholder around
an already bracketed field: the source emits a single literal pair.

Server-side filter syntax is reserved for future annex material.
