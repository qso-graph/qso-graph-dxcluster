<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# QGDXC-SPEC-001: QG DX Cluster specification

**Status: draft 0.1.0, 2026-11-16.** This first draft specifies a receive-only
client. It defines requirements, event fields, server profiles, and a vector
format. It does not claim an implementation or a completed vector suite.

The library/crate is `qso-graph-dxcluster`; the command-line application is
`qg-dxc`. The common core has a session layer and an event layer. Server-specific
behavior belongs in Annexes B–E; [Annex A](annex-a-events.md) defines event fields.
This document is independent of a particular Rust API or terminal interface.

## 0. Reading this specification

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-GEN-0001` | The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT, RECOMMENDED, MAY, and OPTIONAL have the meanings in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) when uppercase. | Inspection |
| `QGDXC-GEN-0002` | Each normative rule MUST have one defining `QGDXC-<AREA>-NNNN` ID; other locations MUST cite that ID instead of restating its obligation. A requirement's explicitly incorporated field or layout table is part of that requirement. | Inspection |
| `QGDXC-GEN-0003` | A conformance claim MUST identify specification version, conformance class, supported server profiles, and optional features. The requirements in incorporated Annexes A–E apply to the declared classes and profiles. | Inspection |
| `QGDXC-GEN-0004` | Server-specific interpretations MUST be supported by a pinned source location or a named capture and one-based record number. Unsupported server claims MUST be marked **UNCONFIRMED**, informative, and excluded from automatic session actions and typed semantic mappings. | Inspection |
| `QGDXC-GEN-0005` | This profile MUST be receive-only: application output is limited to caller-configured login, the annex's session setup and readiness exchanges, and disconnect. Spot posting, arbitrary command construction, event encoding, and re-encoding are outside this profile. | Inspection |

`Test` means a reproducible automated check or vector; `Inspection` means review
of the implementation or document against the rule. These labels specify how to
verify a future implementation, not checks already passed.

| Area | Definition location |
|---|---|
| GEN | Document conventions, scope, and claims: §0 |
| IO | Application and library boundary: §1 |
| FRM | Byte framing and decoding: §2 |
| SES | Session lifecycle and actions: §3 |
| DX | Common live spot grammar: §4 |
| REC | Session recording: §5 |
| SEC | Untrusted input and privacy: §6 |
| VEC | Vectors and conformance: §7 |
| EVT | Event model: Annex A |
| DSP, ARC, CCC, RBN | Server profiles: Annexes B, C, D, E |

An *input chunk* is an arbitrary slice of received bytes, not a line. A *line*
is a framed content byte sequence. A *prompt fragment* is recognized before a
line delimiter. An *event* describes received content or a local connection
transition. A *session action* requests transport work from the caller; an event
does not authorize that work. *Ready* means the profile's startup exchange has
completed, not that the server authenticated the operator or delivers all spots.

Server-side filter languages are future annex material. Client display filters,
deduplication, node directories, historical query interfaces, and spot aggregation
policy are outside this draft. Saved node filters can limit the received feed.

## 1. Sans-I/O boundary

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-IO-0001` | The library MUST perform no socket, file, process, clock, environment, credential-store, or logging I/O. It MUST accept caller-supplied bytes, transport outcomes, configuration, and time values, and return events, bounded pending output, and transport/timer actions. | Inspection |
| `QGDXC-IO-0002` | `qg-dxc` MUST own its socket, connection establishment and closure, timer delivery, OS credential-store integration, terminal rendering, and optional recording. Other applications using the library assume the same caller responsibilities. | Inspection |
| `QGDXC-IO-0003` | A connection configuration MUST specify the endpoint host and port, the operator's own callsign, and a server profile or `auto`; no default public node or shipped node directory is part of this profile. Login input MUST be a single caller-supplied ASCII token of 1–64 bytes, containing only letters, digits, `/`, and `-`, with at least one letter and one digit. A trailing hyphen-SSID, if present, MUST be decimal 1–99 with no leading zero; violations MUST be rejected locally before connecting with a clear configuration error. Rationale: Annex B, S1 login aliases. This is injection prevention, not license validation. | Test |
| `QGDXC-IO-0004` | The caller MUST supply nondecreasing monotonic time for timers and optional UTC receipt time separately. Socket chunk boundaries, wall-clock changes, and timer polling frequency MUST NOT change the resulting protocol interpretation. | Test |
| `QGDXC-IO-0005` | The library MUST expose backpressure and bounded output admission; it MUST NOT silently discard admitted input or grow a queue without a declared finite bound. Each session input or drain call MUST return at most eight events. A chunk MUST admit at most 1 MiB of bytes; unconsumed bytes MUST remain pending inside the engine within that bound. The caller MUST consume each batch and complete its actions, then drain pending input before offering more input. Admission while input is pending or above the byte bound MUST fail without changing pending input. Live and replay adapters MUST use this admission and draining path and retain at most one returned batch. The caller MUST consume events or pause input, or explicitly close with `resource_limit`. | Test |
| `QGDXC-IO-0006` | Login-identity comparisons, including each annex's `<configured-call>` placeholder, MUST compare ASCII letters case-insensitively. The canonical identity for session uniqueness and endpoint/identity rate accounting MUST be the configured token with ASCII `a`–`z` changed to `A`–`Z`, preserving every other byte and suffix. Transmission follows `QGDXC-SES-0002`; received callsign evidence MUST retain its original case. | Test |

Transport choice and any TLS implementation belong to the caller. An endpoint
label or advertised software version is configuration or evidence, not proof of
server identity. Passive Telnet command handling is defined by
`QGDXC-FRM-0008`; it does not negotiate options.

## 2. Framing, bounds, and text

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-FRM-0001` | The framer MUST accept CRLF, lone LF, and lone CR, including delimiters split across chunks. It MUST treat CRLF as one delimiter and retain its exact spelling in `raw_ending`. | Test |
| `QGDXC-FRM-0002` | To disambiguate CRLF, a trailing CR MUST remain pending until the next byte or caller-signaled EOF. A following LF completes CRLF; another byte completes lone CR and begins the next frame. EOF MUST flush a pending CR as a delimiter and any other unterminated content as a partial frame. | Test |
| `QGDXC-FRM-0003` | The content limit MUST default to 4,096 bytes and be configurable only in 256–65,536 bytes. On the first excess byte, the framer MUST emit one `unrecognized_line` with reason `line_too_long`, at most the admitted prefix, and `raw_ending: none`; `raw_status` MUST be `truncated` unless the privacy rule supersedes it. The framer MUST discard through the next delimiter before accepting another frame. No discarded suffix becomes a spot or prompt. | Test |
| `QGDXC-FRM-0004` | Prompt matching MUST work incrementally across chunks within the content limit and only in the applicable session state. Recognition MUST be evaluated after each byte and consume through the earliest complete match, ending at the prompt's colon or `>` without absorbing subsequent padding. A matched fragment MUST be emitted once as `prompt_status`, with `raw_ending: none`; consumed bytes MUST NOT be emitted again when a delimiter arrives. Remaining padding and its delimiter form a separate blank frame, classified as `blank` even if it contains only spaces. | Test |
| `QGDXC-FRM-0005` | A complete line or prompt fragment MUST decode as UTF-8 if valid, otherwise as ISO-8859-1 (Latin-1), independently for each frame. The event MUST identify `encoding` as `utf8`, `latin1`, or `none` for a local event. No replacement-character decoding is permitted. | Test |
| `QGDXC-FRM-0006` | Raw application content and delimiter bytes MUST remain distinct from decoded and parsed text, subject only to `QGDXC-SEC-0005` and the oversize rule. A parsing view MAY remove outer ASCII spaces and annex-defined suffix bytes; it MUST NOT alter retained evidence or normalize embedded whitespace. | Test |
| `QGDXC-FRM-0007` | Except for an annex-defined terminal notice recognized during end-of-input finalization, empty, malformed, unknown, and unterminated non-prompt content MUST produce `unrecognized_line` with the applicable reason (`blank`, `malformed`, `unknown`, or `partial`). A malformed line MUST NOT terminate parsing of subsequent bounded frames. | Test |
| `QGDXC-FRM-0008` | Before line framing and decoding, the engine MUST apply the passive Telnet stripper in §2.1, retaining its state across chunks and generating no negotiation reply. Only an overlong or unterminated subnegotiation MUST end the session as `resource_limit`; other IAC commands MUST NOT cause closure. Event frame evidence MUST contain the resulting application bytes; exact wire recording, including stripped bytes, follows `QGDXC-REC-0001`. | Test |

The limit and recovery policy are QG DX Cluster choices, not claims about a
server's maximum output. ASCII-only captures do not establish a server charset;
`QGDXC-FRM-0005` is the common decoding policy.

### 2.1 Passive Telnet stripper

This algorithm is incorporated by `QGDXC-FRM-0008`. Byte values are hexadecimal;
IAC is `ff`, SB is `fa`, and SE is `f0`. This is defensive client policy, not a
claim that any supported server negotiates Telnet. Live behavior is
**UNCONFIRMED B-U3** in Annex B.

| Input outside subnegotiation | Result |
|---|---|
| Any byte other than IAC | Pass the byte to the application framer |
| `IAC IAC` | Pass one data byte `ff` |
| `IAC WILL option`, `IAC WONT option`, `IAC DO option`, `IAC DONT option` (`fb`, `fc`, `fd`, `fe`) | Drop all three bytes, including any option byte |
| `IAC SB` | Enter subnegotiation; drop through its terminating `IAC SE` |
| Any other `IAC x` | Drop the two bytes |

Inside subnegotiation, all bytes are discarded, including CR/LF. `IAC IAC`
is escaped payload and does not terminate it; only an unescaped `IAC SE`
terminates it. Other `IAC x` pairs are discarded without nesting or restarting
the bound. The maximum is 4,096 wire bytes from the opening IAC through the
terminating SE, inclusive. The first excess byte, or end of input while still
inside subnegotiation, produces `resource_limit` under `QGDXC-SES-0013`. Buffering the
payload is unnecessary. End of input after an incomplete non-SB command discards
that command without a transport-specific failure. End-of-input processing
follows `QGDXC-SES-0014`. Stripped commands alone produce no
content event and do not count as application-data liveness activity.

## 3. Session layer

### 3.1 State and login

The state names and transitions in this table are incorporated by
`QGDXC-SES-0001`. Unexpected input remains observable under Annex A.

| State | Trigger | Next state or action |
|---|---|---|
| `disconnected` or `blocked` | Caller starts | `connecting`; request one transport connection |
| `connecting` | Connection succeeds | `await_login` |
| `await_login` | Recognized login fragment | Send configured callsign; `await_identity` |
| `await_identity` | Profile evidence and post-login ready marker | `setup`; run profile startup sequence |
| `setup` | Required replies complete | `ready` |
| Any active state, including `connecting` | Failure or cancellation | Outcome from `QGDXC-SES-0013` |
| `backoff` | Deadline reached | `connecting` for the already-budgeted retry |
| Any state | Caller explicitly restarts | Outcome from `QGDXC-SES-0013`; retry reset from `QGDXC-SES-0010` |

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-SES-0001` | The session engine MUST follow the state table, emit a `connection_state` event for every transition, and discard login, detection, pending command, prompt, framing, and Telnet-stripper state when a transport connection ends, after `QGDXC-SES-0014`. Events received during startup MUST remain observable before `ready`. | Test |
| `QGDXC-SES-0002` | In `await_login`, the engine MUST recognize a fragment ending in ASCII `login:` or `call:`, case-insensitively; the token starts at the frame start or after whitespace. Following spaces are handled by `QGDXC-FRM-0004`. This covers prefixed requests such as `Please enter your call:`. It MUST send the configured callsign plus CRLF exactly once per connection, and MUST ignore later login-looking text for transmission. | Test |
| `QGDXC-SES-0003` | Version 0.1.0 MUST NOT send a node password or continue a password-required login. In `await_login` or `await_identity`, a case-insensitive `password:` request at a prompt boundary MUST produce `password_unsupported` under `QGDXC-SES-0013`. Any credential integration reserved for later versions MUST obtain credentials only through the caller's OS credential store; credentials MUST NOT appear in code, configuration, examples, logs, events, or recordings. | Test, Inspection |
| `QGDXC-SES-0004` | Auto detection MUST use only the signatures and contexts in the server annexes, never an endpoint name or a `DX de`/`CC11` line alone. Conflicting profile evidence, or evidence contradicting a caller-selected profile, MUST produce `profile_mismatch`; missing evidence or its required ready marker at the identity deadline MUST produce `profile_unknown`. Outcomes follow `QGDXC-SES-0013`. | Test |
| `QGDXC-SES-0005` | Readiness MUST follow the selected annex. A banner, an echoed callsign, a generic `>` character, or the receipt of spots alone MUST NOT establish readiness. Unsupported readiness commands MUST NOT be probed on other profiles. | Test |
| `QGDXC-SES-0006` | Setup output MUST come only from the annex's fixed startup sequence and caller-generated readiness token. Commands MUST be sent individually with CRLF, with at most one awaiting completion. An explicit supported error or a missing required completion at its deadline MUST produce `setup_failed` under `QGDXC-SES-0013`, without repeatedly trying the command. | Test |
| `QGDXC-SES-0007` | Connection, login, identity, and each setup-reply deadline MUST default to 30 seconds each, configurable to finite values from 5–120 seconds. Each deadline MUST start at its corresponding event: connection request; connection success; admission of the login line for transmission; or admission of that setup command for transmission after pacing. Each MUST be canceled by its corresponding completion: connection success; login-fragment recognition; receipt of required profile evidence and post-login ready marker; or the command's required completion. Time spent waiting for pacing admission MUST count against no reply deadline. Repeated prompt fragments or unsolicited spots MUST NOT extend a startup deadline. | Test |

Annex B defines the echo readiness check. Annexes C–E use observed post-login
prompts and do not assert a verified echo exchange. The fixed session literals
are implementation constants, not an application command-building interface.

### 3.2 Liveness

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-SES-0008` | In `ready`, the default liveness policy MUST have no application-data silence timeout and send no periodic command or blank-line keepalive. EOF or a caller-reported transport failure ends the session. A caller MAY explicitly configure a finite silence timeout of at least 900 seconds; expiry MUST be reported as `silence_timeout`, not proof of server failure, before applying §3.3. | Test |

Nodes need not send keepalives. Annex B's source-backed idle prompt is a default
server behavior, not a cross-server heartbeat guarantee. The optional timer
measures time since any received application bytes, not since the last spot.

### 3.3 Reconnect and etiquette

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-SES-0009` | A caller MUST permit at most one active or connecting session for the same endpoint and canonical login identity (`QGDXC-IO-0006`). It MUST finish closing an old transport before opening its replacement. Retry eligibility follows `QGDXC-SES-0013`. | Test |
| `QGDXC-SES-0010` | Retryable failures MUST use exponential backoff: base delays 5, 10, 20, 40, 80, 160, then 300 seconds, capped at 300; caller-supplied jitter MAY add up to 20% of the base delay. At most eight consecutive automatic retries are permitted between resets; the explicit initial connection is not a retry. Scheduling a retry MUST consume one retry and advance the backoff index. The counter and backoff index MUST both reset to zero on explicit start/restart and after ten continuous minutes in `ready`, and MUST NOT reset on a shorter successful connection. Cancellation follows `QGDXC-SES-0013`. The engine MUST request caller-supplied jitter for each scheduled retry, with no lifetime pool or attempt-count resource limit. Live adapters MUST obtain fresh jitter from the OS random source for each request; deterministic harnesses MUST supply each value explicitly. | Test |
| `QGDXC-SES-0011` | The caller MUST enforce at least one second between session output lines and at most 20 lines in any rolling 20 seconds for an endpoint/identity, including login and setup across reconnects. It MUST honor a stricter configured node limit and any recognized refusal or retry interval by stopping or delaying, never by cycling identities or endpoints. | Test |
| `QGDXC-SES-0012` | Node notices and limits MUST be made available as events for the operator. Free-text suggestions MUST NOT be executed or automatically parsed into a command; only an annex-defined notice can change session behavior. Displacement, refusal, password, and setup-error outcomes MUST apply only to complete, non-truncated `prompt_status` events with the matching status in Annex A.2; other content kinds MUST NOT trigger these outcomes. | Test |
| `QGDXC-SES-0013` | The engine MUST apply the failure outcomes and precedence in §3.4, cancel obsolete timers and pending output, and request closure of any live transport or pending connection attempt. Terminal outcomes MUST remain stopped until explicit caller start/restart. A specific failure MUST remain latched through subsequent EOF, transport-error, or close notifications; those notifications MUST NOT reclassify it or schedule another retry. | Test |
| `QGDXC-SES-0014` | Every transport termination (EOF, error, reset, cancellation, or timeout) MUST use one end-of-input procedure: finalize pending Telnet state under `QGDXC-FRM-0008`, flush pending CR and partial content exactly as `QGDXC-FRM-0002` does for EOF, and emit the resulting events before applying `QGDXC-SES-0013` and clearing connection state under `QGDXC-SES-0001`. | Test |

The conservative rate policy has room below the DXSpider threshold documented
in B.1. The finite retry budget addresses repeated connection loss as well as
peers that repeatedly displace an older session.

### 3.4 Failure outcomes

This table is incorporated by `QGDXC-SES-0013`. These outcomes apply in every
active state where the trigger is possible, including before profile
identification. Annexes restrict which received notices can produce a trigger.
Termination first follows `QGDXC-SES-0014`. A terminal outcome found during that
flush, including `refused`, `displaced`, or `password_unsupported`, takes precedence
over `transport_error` or `eof`. There is no inference of refusal from a silent
disconnect.

| Reason and trigger | Resulting state | Automatic retry |
|---|---|---|
| `eof`: peer closure without a more specific cause | `disconnected` with this reason, then `backoff` with `retry_scheduled` | Yes, within `QGDXC-SES-0010` |
| `transport_error`: connection establishment or connected transport fails | Same sequence as `eof`, retaining `transport_error` | Yes, within `QGDXC-SES-0010` |
| `timeout`: connection or login deadline expires | Same sequence as `eof`, retaining `timeout` | Yes, within `QGDXC-SES-0010` |
| `silence_timeout` from `QGDXC-SES-0008` | Same sequence as `eof`, retaining `silence_timeout` | Yes, within `QGDXC-SES-0010` |
| `profile_unknown` from `QGDXC-SES-0004` | `blocked` | No |
| `profile_mismatch` from `QGDXC-SES-0004` | `blocked` | No |
| `password_unsupported` from `QGDXC-SES-0003` | `blocked` | No |
| `setup_failed` from `QGDXC-SES-0006` | `blocked` | No |
| `refused` from `QGDXC-DSP-0009` | `blocked` | No |
| `displaced` from `QGDXC-DSP-0003` | `blocked` | No |
| `resource_limit` from `QGDXC-IO-0005` or `QGDXC-FRM-0008` | `blocked` | No |
| `unsupported_transport`: caller reports an incompatible transport/adapter; not an IAC-byte outcome | `blocked` | No |
| `retry_exhausted`: a retryable failure occurs after the eighth retry has been consumed | `disconnected` with the original failure reason, then `blocked` with `retry_exhausted` | No; no ninth retry or timer |
| `cancelled`: explicit caller cancellation, including during `backoff` | `disconnected`; cancel all pending retries | No |

An explicit cancellation ends any pending recovery even if a cause was already
latched. Otherwise, the first specific terminal cause wins; a generic closure
cannot override it. On a retryable outcome, `retry_in_ms` belongs to the
`backoff` transition only. An already stopped engine ignores later closure
notifications. Start eligibility follows §3.1; in other states the caller uses
restart. Restart emits `disconnected` with
`cancelled` for an active attempt before the new `connecting`/`started`
transition and obeys `QGDXC-SES-0009`. A start while already active has no effect.

## 4. Common live `DX de` grammar

The following layout notation is incorporated by `QGDXC-DX-0001`. `SP+` means
one or more ASCII spaces, not a column number. Annexes define allowed suffixes
and additional formats; they do not redefine this grammar.

```text
DX de SP+ spotter : SP+ frequency_khz SP+ spotted_call SP+ comment SP+ HHMMZ suffix
```

An empty comment is allowed. The spotter token immediately precedes the colon;
spaces between that colon and frequency are padding. The final time is the
rightmost valid time token followed only by an admitted annex suffix or spaces.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-DX-0001` | A live spot parser MUST anchor on literal ASCII `DX de` at the start, then parse the spotter's colon, frequency, spotted callsign, and trailing `HHMMZ` field in that order using the layout above. It MUST NOT slice by fixed character columns or search arbitrary text for an embedded spot. | Test |
| `QGDXC-DX-0002` | Frequency MUST be an unsigned decimal kHz token with 1–12 integer digits and zero to three fractional digits, at most 16 bytes including the optional dot. It MUST convert exactly to integer `frequency_hz` in 1–999999999999999 without binary floating-point rounding; exponents, signs, commas, empty fractions, overflow, and sub-hertz precision MUST be rejected as malformed. This bound is client policy. | Test |
| `QGDXC-DX-0003` | `HHMMZ` MUST have hours 00–23 and minutes 00–59 and map to UTC `time_of_day` at minute precision. A live line without a date MUST NOT acquire a date from receipt time or a preceding prompt. | Test |
| `QGDXC-DX-0004` | Received callsign tokens (spotter and spotted station) MUST contain 1–64 bytes of visible ASCII (`0x21`–`0x7E`), excluding `:` and `^`, with no letter or digit requirement. Each token MUST be preserved exactly, including case and full suffixes, subject to `QGDXC-SEC-0005`. This is lexical recognition only, never license validation. Outbound login validation remains `QGDXC-IO-0003`. | Test |
| `QGDXC-DX-0005` | A successful live parse MUST map the fields in Annex A, preserving the comment's internal spacing and removing only outer ASCII padding. It MUST NOT infer a mode, grid, SNR, or speed from an ordinary cluster comment; only an annex-defined structured comment permits that mapping. | Test |
| `QGDXC-DX-0006` | An invalid required field, ambiguous admitted time boundary, or unsupported suffix MUST yield `unrecognized_line` with reason `malformed`, never a partial spot. An invalid optional structured-comment mapping MAY leave those optional fields absent while preserving the core spot and full comment, as the annex specifies. | Test |

Sanitized illustrative content, without a delimiter:

```text
DX de Q1ABC: 14025.0 Q2XYZ CQ 1832Z
DX de Q9XYZ-2-#: 14074.00 Q3ABC/P FT8 -12 dB AA00 CQ 1832Z
```

These are examples, not captured vectors. The second comment follows Annex E.

## 5. Session recording

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-REC-0001` | Recording MUST be off by default and enabled only by an explicit caller request naming a destination. The caller MUST write ordered NDJSON records, each with `t` (finite nonnegative numeric elapsed seconds from recording start, nondecreasing in record order). Wire-byte records MUST additionally carry direction `dir` (`in` or `out`) and canonical padded RFC 4648 base64 `b64` for the exact corresponding wire bytes, including framing. Inbound and outbound chunk sizes MAY differ from socket reads/writes but byte order within each direction MUST be unchanged. | Test |
| `QGDXC-REC-0002` | A note record MUST have `t` and `note` text, with no `dir`, `b64`, `conn`, or `reason`; a note is observer metadata, never received protocol evidence or a connection boundary. Record numbering MUST count every physical NDJSON line, including notes and boundaries, starting at one. | Test |
| `QGDXC-REC-0003` | Enabling exact raw recording MUST explicitly request retention of third-party data under `QGDXC-SEC-0005`. Recording MUST capture only bytes actually received or accepted for transmission, and MUST report a recording failure to the caller rather than silently claiming a complete capture. | Test |
| `QGDXC-REC-0004` | A replay adapter MUST deliver decoded inbound bytes through the same framing and parsing path as live input, use `t` as elapsed monotonic time, treat outbound records as observations rather than instructions to send, and deliver observer notes separately. Only a recorded `conn: open` MUST start an attempt, resetting framing and Telnet-stripper state. Replay MUST stream physical NDJSON records with a 1 MiB per-record byte cap, without a whole-recording prepass or per-recording resource pools. Configuration discovery and readiness-token lookup MUST each use at most 1 MiB of lookahead, charging encoded record bytes plus 256 bytes per buffered record; discovery MUST stop within the first connection and token lookup MUST stop within the current connection. Only when the session requests a readiness token MUST replay obtain it from that connection’s recorded outbound `echo <token>` line, including fragments; if none is found before the boundary or byte limit, replay MUST fail that connection with an explicit error. Profiles that never request tokens MUST allocate no tokens. Retained replay state MUST be independent of recording length. Termination MUST occur at the recorded `conn: close`, applying `QGDXC-SES-0014` using the recorded reason before resetting those states. Output admission and reply deadlines MUST use the recorded completion time of the outbound line (its last fragment). Replay MUST NOT synthesize timers that would terminate or restart a connection before its next recorded boundary. It MUST support fragmented and coalesced chunks; it MUST NOT treat an NDJSON record as a protocol line. | Test |
| `QGDXC-REC-0005` | Each recorded connection MUST have a boundary record `{"t": <elapsed>, "conn": "open"}` before its wire-byte records and `{"t": <elapsed>, "conn": "close", "reason": <A.2 connection-state reason>}` after them. Boundary records MUST be distinct from notes and wire-byte records. Open/close pairs MUST NOT overlap; wire-byte records MUST occur within a pair. One recording MAY span several connections. | Test |

Runtime captures can contain personal information and are distinct from published
vectors. Storage permissions, rotation, and retention lifetime belong to the
recording application. This draft does not require copying original captures
into the repository.

The numeric `t` convention matches all seven supplied 2026-11-16 captures.
They predate connection-boundary records and mark connections with notes only
(each capture's record 1; basenames and pins in Annexes B–E). They are evidence,
not conforming recordings under `QGDXC-REC-0005`. Elapsed time does not supply
UTC receipt time or a spot date.

## 6. Security and privacy

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-SEC-0001` | Every parser and session recognizer MUST be safe on every byte sequence, with bounded memory, bounded per-byte processing, checked numeric conversion, and no panic or process execution. Each parser, incremental framer, and session state machine MUST have a fuzz target, including fragmentation and limit recovery. | Test, Inspection |
| `QGDXC-SEC-0002` | Network content MUST NOT become executable code, a process argument, a filename, or outbound command text. Only the configured identity and local startup literals/token admitted by `QGDXC-SES-0006` can produce protocol output. | Test, Inspection |
| `QGDXC-SEC-0003` | Terminal output MUST escape C0, DEL, and C1 controls, including ESC, BEL, CR, and LF contained in received text, before display. Layout newlines MUST be generated by the renderer. Raw evidence MUST NOT be printed directly to a terminal, including in errors and diagnostics. | Test |
| `QGDXC-SEC-0004` | The Rust library MUST forbid unsafe code. Reported callsigns, identities, modes, coordinates, signal reports, and software signatures MUST be treated as untrusted assertions, not authentication or authorization to act on a station. | Inspection |
| `QGDXC-SEC-0005` | By default, third-party IP addresses and precise coordinate fields MUST be discarded from structured fields, text views, diagnostic copies, and retained raw evidence. An explicit per-session caller option `retain_third_party_data` MAY retain them. With retention disabled, privacy MUST override token preservation. If a required structured field (`spotter_call`, `call`, `from_call`, or `to_call`) would be an IP literal, the line MUST produce `unrecognized_line` with reason `privacy_withheld`, empty `text` and `raw_line`, and `raw_status: withheld`. Otherwise, sensitive CC11 slots MUST be replaced by empty byte spans before an event leaves the parser, with `raw_status: redacted`, regardless of the selected profile or semantic acceptance. A recognizable CC11 marker followed by caret fields is subject to this rule even in unsupported or malformed content. If field boundaries cannot be identified safely, including an ambiguous variable-length layout, the whole content MUST be withheld (`raw_line` empty, `raw_status: withheld`). Outside those CC11 slots, each identifiable IPv4/IPv6 literal in a retained view or raw evidence MUST be replaced by the four ASCII bytes `<ip>`, with `raw_status: redacted`. Literal extraction MUST scan maximal runs of ASCII hexadecimal digits, colons, periods, and percent signs, testing at the start of each run and after each colon or period. At each boundary, it MUST test candidates of at most 45 bytes, longest first, and ignore matches contained within a previously recognized literal. This includes literals followed by sentence punctuation or preceded by labels. Original sensitive bytes MUST NOT remain in queued events or logs. | Test |
| `QGDXC-SEC-0006` | Published examples and vectors MUST use invented Q-prefix callsigns, exclude real personal addresses and private endpoint names, and use only removed IP fields, `<ip>`, or documentation addresses where an address is necessary. Capture-derived vectors MUST remove IPs entirely; synthetic privacy-test input MAY use documentation addresses. Expected events MUST use the exact privacy transformations in `QGDXC-SEC-0005`, and mark them in provenance. Capture export MUST use the literal extraction in `QGDXC-SEC-0005` before identity substitution. The generator MUST validate every generated public file with the public-file sanitization check before writing any of them; if validation fails, it MUST write nothing. Raw captures MUST NOT be published as fixtures. | Inspection |

`QGDXC-SEC-0005` resolves the tension between exact evidence and default privacy:
every event has byte evidence, but redacted evidence is explicitly distinguishable
from exact bytes. Full raw fidelity, including sensitive fields, is available
only on the caller's explicit retention request. This is a storage policy, not a
claim that a free-text protocol can reliably detect every form of personal data.
Password handling is defined only by `QGDXC-SES-0003`.

## 7. Vectors and conformance

### 7.1 Classes

| Class | Applicable requirements |
|---|---|
| Event parser | GEN, IO library rules, FRM, DX, EVT, SEC, VEC, and declared server line mappings |
| Session engine | Event parser plus SES and declared server startup profiles |
| Application | Caller-side IO, SES, SEC, EVT consumption, and REC when recording/replay is claimed |

### 7.2 Vector format

This format is incorporated by `QGDXC-VEC-0001`. A vector is one UTF-8 JSON
object; an optional `SPDX-License-Identifier` metadata member identifies its license and has no protocol meaning; a suite is NDJSON with one object per physical line. Published vectors live in `vectors/`; implementation conformance is reported separately.

| Member | Type and meaning |
|---|---|
| `id` | Unique descriptive string |
| `spec` | `0.1.0` |
| `class` | One of the class names in §7.1 |
| `profile` | `dxspider`, `arcluster`, `cccluster`, `rbn`, or `auto` for detection cases |
| `provenance` | Object: `capture` basename, `records` array of one-based record numbers, and `sanitization` array describing substitutions; synthetic negative/mutation cases additionally name `derived_from` |
| `config` | Explicit parser/session settings, including line limit, selected profile, privacy option, and timer settings; identity is required for session cases and for line-only cases matching a configured-call prompt |
| `tokens` | Array of caller-generated readiness tokens in attempt order for `dxspider`/`auto`; one reserved on each connection attempt, even if it fails before setup. Other profiles use an empty array. Token syntax follows `QGDXC-DSP-0004` |
| `jitter_ms` | Array of nonnegative integer additions, one consumed on each scheduled retry; each value is bounded by `QGDXC-SES-0010`. Supply explicit zeros when jitter is disabled |
| `steps` | Ordered stimulus objects from the schema below |
| `expected_events` | Ordered objects `{step, elapsed_ms, event}`; `event` is a complete Annex A oracle using base64 for byte members; `step` is the zero-based triggering step index |
| `expected_actions` | Ordered objects `{step, elapsed_ms, action}`; `action` follows the schema below |
| `expected_state` | Final session state for session cases; absent for line-only cases |
| `expected_config_error` | Optional text for a local configuration-rejection case; when present, `steps`, `expected_events`, and `expected_actions` are empty and `expected_state` is absent. No connection or output is requested. Other invalid vector shapes remain invalid. |

An expected event is an independent oracle, not a copy of the implementation's
output. A line vector can contain one captured line split across several input
steps; a session vector can interleave local transport stimuli with capture data.

Every stimulus has `type` and nonnegative integer `elapsed_ms`, with times
nondecreasing in array order. All milliseconds, indices, and attempt numbers
are exact integers within `QGDXC-EVT-0005`. Fields in the last column below are
additional required members unless marked optional; other members are invalid.
Optional `received_at` on `input` is caller-supplied UTC as in Annex A, never
derived from elapsed time.

| Stimulus `type` | Additional members and meaning |
|---|---|
| `start` | None; explicit start from a stopped state |
| `restart` | None; explicit replacement of the current attempt or start from a stopped state |
| `cancel` | None; explicit cancellation, including during backoff |
| `input` | `b64`: canonical padded base64 of inbound wire bytes; `attempt`: connection-attempt number for session cases, absent for line-only cases; optional `received_at` |
| `tick` | None; delivers due timers according to §7.3 |
| `transport` | `attempt`: attempt number; `outcome`: `connected`, `eof`, `error`, `closed`, `unsupported_transport`, or `resource_limit`. Optional `detail` is local diagnostic text with no behavioral meaning |

`connected` completes establishment; `error` maps to `transport_error` and
means the transport/attempt has ended, including on reset. Transport termination
follows `QGDXC-SES-0014`. `eof` ends the peer transport. `closed` acknowledges
requested closure; without a pending close it is generic EOF. The last two
outcomes are caller/adapter failures from
§3.4, not server text. Refusal, displacement, password requests, and profile
failures are driven by input/deadlines, not injected transport outcomes.
Line-only cases use only `input` and `tick`, plus `transport`/`eof` without
`attempt` to flush the framer. Session bytes for a stale attempt or one that is
not connected are ignored; they cannot affect another attempt.

Every action is an object with `type` and exactly the additional members below.
Attempt numbers start at 1 and increase on each `connect`, including explicit
restarts. The endpoint and identity come from `config`; no action obtains them
from received text. Actions describe the abstract harness boundary, not a Rust
API or a new command-construction interface.

| Action `type` | Additional members and meaning |
|---|---|
| `connect` | `attempt`; request one connection to the configured endpoint |
| `close` | `attempt`; close a live transport or cancel a pending connection |
| `output` | `attempt`, `b64`; one admitted login/setup line including CRLF, emitted only when pacing permits |
| `arm_timer` | `timer`, `deadline_ms`; register or replace the named absolute monotonic deadline |
| `cancel_timer` | `timer`; cancel a registered deadline |

Timer names, also their tie-breaking order, are `connect`, `login`, `identity`,
`setup`, `stable_ready`, `silence`, `retry`, and `pacing`. They correspond to
the deadlines/reset/pacing rules in §3. A `pacing` deadline is the earliest time
the next pending output satisfies `QGDXC-SES-0011`. An unchanged deadline emits
no timer action; replacement emits `cancel_timer` then `arm_timer`. Firing
removes a timer without emitting `cancel_timer`. Missing token/jitter entries,
an impossible transport success, or an invalid stimulus/action shape invalidate
the vector instead of selecting a default nondeterministically.

### 7.3 Session harness and ordering

The following execution rules are incorporated by `QGDXC-VEC-0006`.

The initial session state is `disconnected` at elapsed time zero, with no
transport, timers, admitted output, framing/stripper state, profile evidence,
retry history, or rate history. Configuration is applied before the first step
without emitting events. Every case supplies all settings that affect its
oracle; omitted settings use this draft's defaults. A session case explicitly
starts using a `start` or `restart` step. A line-only case instead initializes
an empty framer and the declared profile in post-login `ready` recognition
context, with no setup reply outstanding and no session actions. Its `config`
supplies an identity if a prompt mapping requires one; `auto` has no identified
profile until evidence is received.

The harness accepts each `output` action fully and immediately at its recorded
elapsed time, updating shared endpoint/canonical-identity pacing history. A
`close` action synchronously finishes closure/cancellation in this abstract
harness before any later `connect`; an optional later `closed` notification is
idempotent. Real callers remain bound by `QGDXC-SES-0009`. Output transport
failures are delivered as a subsequent `transport`/`error`. The harness drains
all produced events/actions after each stimulus and timer firing; it does not
introduce queue backpressure. Cancellation/restart discards pending output;
restart preserves rate history as required by `QGDXC-SES-0011`.

Before each step at time T, timers with deadlines strictly less than T fire in
deadline order at their logical deadline times, with the table's timer order
breaking ties. A `stable_ready` timer due exactly at T also fires before that
step: the continuous ready duration has already been achieved, even if a
failure arrives at T. All other deadlines equal to T wait for the tick.
At time T, non-tick steps run in array order; each group of
equal-time steps ends with exactly one `tick`. That tick fires all still-due
timers with deadlines at T in timer order, including any newly armed due timer
after the current firing. Thus cancellation or a completion arriving at its
deadline is processed before expiry. Cancellation during backoff removes the
retry timer, so a later tick cannot connect. Late ticks do not move deadlines or
inflate a backoff delay. Automatic firings before a step use that step's index
in oracle wrappers, with their own logical `elapsed_ms`. No timer advances
beyond the final step.

For each processed stimulus or timer firing, events retain their causal order
under `QGDXC-EVT-0004`. Actions retain causal order across transitions and are
never sorted or normalized. All produced actions appear in `expected_actions`,
including registration and cancellation of a transient timer within one
stimulus. For example, entering `ready` and then receiving a displacement
notice in the same input stimulus emits `arm_timer` for `stable_ready` before
its later `cancel_timer`.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-VEC-0001` | A published vector suite MUST follow §7.2, identify its specification version, and pair sanitized captured input with independently specified expected events and actions. Synthetic cases MUST be labeled as mutations or synthetic cases, never claimed as captured behavior. Frame provenance MUST list only physical records that contribute content or delimiter bytes. Capture import MUST reject a frame with more than 256 contributing records before allocating retained provenance. | Test, Inspection |
| `QGDXC-VEC-0002` | Event-parser coverage MUST include every supported mapping, empty and malformed fields, numeric and line bounds, received-call lexical boundaries under `QGDXC-DX-0004` (including tokens with neither letters nor digits), non-UTF-8 bytes, control characters, fragmented/coalesced input, all delimiters, EOF, unknown lines, and both privacy settings, including each required-call fallback and non-CC11 IPv4/IPv6 replacement under `QGDXC-SEC-0005`. CC11 cases MUST include empty trailing fields, dialect mismatch, and DXSpider carets inside comments. Telnet cases MUST cover every §2.1 branch split at each byte boundary, escaped IAC in data and SB, exact/excess SB bounds, incomplete-command EOF, and unterminated SB. Overlap cases MUST include malformed `DX de ` versus talk, announcement/weather prefixes, annex-defined unsupported layouts including `QGDXC-ARC-0005`, and deterministic banner/setup fallback. DXSpider suffix cases MUST include single-digit zone padding and exact `native.trailing_extra`. | Test |
| `QGDXC-VEC-0003` | Session coverage MUST include each observed login and ready signature, duplicate login-looking text, password blocking, setup replies and errors, deadlines, silence without a keepalive, retry exhaustion/cancellation, displacement, command pacing across reconnects, and recording/replay when claimed, including multi-connection boundaries under `QGDXC-REC-0004` and `QGDXC-REC-0005`. Cases MUST exercise every termination in `QGDXC-SES-0014` with pending CR and partial content, flush-discovered terminal precedence under `QGDXC-SES-0013`, pre-identification refusal with following EOF, pre-identification `DX de` and CC11 input with `auto` and caller-selected profiles, custom-prompt configuration acceptance/rejection and consumption under `QGDXC-DSP-0002`, a ready transition followed by displacement in coalesced and equal-time split input, ASCII identity-case variants, local rejection of `Q1ABC-0`, `Q1ABC-00`, and `Q1ABC-01` before any connection request under `QGDXC-IO-0003`, every §3.4 outcome, eight consecutive retries, stable-ready resets of both retry counter and backoff, explicit restart, cancellation at a retry deadline, and absence of output/connection after cancellation. A synthetic CC Cluster case MUST use login output at 0 ms, the post-login prompt at 100 ms, paced `set/ve7cc` output at 1,000 ms, a 5,000 ms setup-reply timeout, and acknowledgment plus prompt at 5,500 ms, expecting a 6,000 ms setup deadline canceled on completion and `ready` under `QGDXC-SES-0007`. | Test |
| `QGDXC-VEC-0004` | A conformance claim MUST satisfy every applicable requirement and pass the versioned suite for its declared classes/profiles. Before that suite exists, a claim MUST be labeled `draft review only`; passing examples or schema validation alone is insufficient. | Inspection |
| `QGDXC-VEC-0005` | A runner MUST compare event kind, all defined fields, raw evidence and its status, event order, actions, and final state as parsed values. It MUST NOT compare by re-encoding server messages. Chunk-split invariance follows `QGDXC-VEC-0006`. | Test |
| `QGDXC-VEC-0006` | A vector runner MUST initialize and execute the abstract harness in §7.3, consume caller token/jitter inputs deterministically, and compare the complete timestamped event/action traces. For identical input bytes repartitioned into chunks with equal elapsed time, it MUST verify chunk-split invariance by comparing the concatenated ordered action trace and event sequence, ignoring only wrapper step indices changed by splitting and preserving all semantic values, timestamps, and ordering. | Test |

## Incorporated annexes

- [Annex A: event model](annex-a-events.md)
- [Annex B: DXSpider](annex-b-dxspider.md)
- [Annex C: AR-Cluster](annex-c-arcluster.md)
- [Annex D: CC Cluster](annex-d-cccluster.md)
- [Annex E: Reverse Beacon Network](annex-e-rbn.md)

Capture citations refer to the supplied 2026-11-16 evidence set by basename and
physical record number, including notes. A frame split across records cites all
contributing records. The annex source tables identify the evidence; no local
capture storage path is part of the specification. Capture observations establish
those sessions, not every installation or software release.
