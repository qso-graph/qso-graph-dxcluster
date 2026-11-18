<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex F: `qg-dxc tui`, the terminal interface

**Status: draft 0.1.0, 2026-11-18.** Application annex of
[QGDXC-SPEC-001](QGDXC-SPEC-001.md). It applies to the `Application` class only.
The library is unchanged by this annex: it passes the full received stream, and
every filter here is a display filter in `qg-dxc`.

`qg-dxc tui` watches one or more nodes at once and shows their spots in one list,
newest first, with a status line per node. It is receive-only under
`QGDXC-GEN-0005`: it sends nothing beyond each session's login and setup.

## F.1 Sources

```
qg-dxc tui --call <CALL> --node <host>:<port>[/<profile>] [--node …]
qg-dxc tui --replay <file> [--replay …]
```

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0001` | `qg-dxc tui` MUST accept 1–8 sources: repeated `--node <host>:<port>[/<profile>]` with one `--call`, or repeated `--replay <file>`, not both. `<profile>` MUST be one of `auto`, `dxspider`, `arcluster`, `cccluster`, or `rbn`; omission selects `auto`. A bracketed IPv6 literal MUST be accepted as `[<addr>]:<port>`. The call, every endpoint, and every profile MUST be validated before any connection opens, under `QGDXC-IO-0003`; zero sources, more than eight, a duplicate endpoint (host compared ASCII case-insensitively, same port), or a port outside 1–65535 MUST be rejected with a configuration error naming the offending argument. | Test |
| `QGDXC-TUI-0002` | Each `--node` MUST run its own session under §3, with its own retry state, timers, and transport. One source's failure, retry, exhaustion, or blocking outcome MUST NOT close, delay, or restart another source. | Test |
| `QGDXC-TUI-0003` | Each `--replay` source MUST use the replay adapter of `QGDXC-REC-0004`, opening no socket. All sources MUST share one interface-start monotonic origin; a record with timestamp `t` becomes due `t` seconds after that origin, without subtracting the first record's timestamp. Due records MUST be delivered in ascending `t`, then source argument order, then physical record order; all continuation batches from a record retain its position in that order. Overdue records MUST be delivered without dropping them. This aligns recording-relative time, not original UTC chronology. At valid end of file, the adapter MUST be finalized once, the source MUST be marked `COMPLETE`, and its retained spots and final connection state MUST remain visible without scheduling another connection. A replay error MUST stop only that source and remain visible on its status line. | Test |
| `QGDXC-TUI-0004` | Each source MUST receive a unique label of at most 16 display columns. Its base is the `--node` host or `--replay` file stem, sanitized under `QGDXC-TUI-0013`; an empty stem uses `source`. Process sources in argument order. First try the base clipped to 16 columns. If already used, try suffixes `#2`, `#3`, … in increasing order, clipping the base to leave room for the suffix, until the complete rendered label is unused. The label is local configuration, not network text. | Test |
| `QGDXC-TUI-0005` | When standard input or standard output is not a terminal, `qg-dxc tui` MUST exit with an error before opening any source, and MUST NOT write terminal control sequences. | Test |

`qg-dxc tui` records nothing in this draft. `qg-dxc connect --record` remains the
way to capture a session.

## F.2 Merged spot list

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0010` | Every `spot` event from every source MUST enter one list in arrival order at the application, newest at the top. Spots MUST NOT be merged, deduplicated, or reordered by reported spot time; the same DX station spotted by two sources appears twice, each with its own source label. | Test |
| `QGDXC-TUI-0011` | Each row MUST allocate separate, nonoverlapping columns for reported UTC time `HHMM`, frequency, DX call, spotter, band under F.3, display mode under F.4 including `unknown`, kind (`human` or `skimmer`), source label, and comment. The source column MUST fit the complete source label; every other non-comment column MUST have positive width. Fields MUST be sanitized under `QGDXC-TUI-0013` before measurement and clipped to their allocated columns; a truncated non-comment field MUST end with `~`. The comment receives the remaining width, which MAY be zero. Frequency MUST use decimal kHz with exactly one fractional digit, rounded to the nearest 100 Hz with ties upward using integer arithmetic: let `n = (frequency_hz + 50) / 100`, using integer division, and display `n / 10`, `.`, and `n % 10`. Derived band and mode MUST use the original event values, not clipped or rounded display text. A missing optional field other than mode MUST show as blank, never as a guessed value. | Test |
| `QGDXC-TUI-0012` | The list MUST retain at most 10,000 spots, discarding the oldest when full. Filters select from retained spots; changing a filter MUST NOT discard or reorder retained spots. The status line MUST show retained and total-received counts. | Test |
| `QGDXC-TUI-0013` | Every text value displayed by the interface, including received fields, source labels, filter input, replay notes, errors, and diagnostics, MUST be sanitized before measurement, clipping, or output. Only Unicode scalars U+0020–U+007E MAY appear literally; every other scalar, including C0, DEL, C1, bidirectional formatting, zero-width, and combining characters, MUST appear as the ASCII escape `\u{hhhh}`, using lowercase hexadecimal with at least four digits. Each resulting ASCII character occupies one display column. Fields MUST be confined to their allocated cell rectangles, without wrapping into another field or row. Terminal control sequences MUST originate only from the renderer, never from displayed text. This satisfies and is stricter than `QGDXC-SEC-0003`. | Test |
| `QGDXC-TUI-0014` | Every live and replay session MUST set `retain_third_party_data` to false, and the interface MUST NOT expose an option to enable it. Network-derived display values MUST be computed only from event fields after `QGDXC-SEC-0005` has been applied. This guarantees the transformations defined by that requirement, not detection of arbitrary personal data or coordinates embedded in free text. | Test |

## F.3 Band

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0020` | Band MUST be derived from `frequency_hz` using the ADIF 3.1.7 `Band` enumeration's lower and upper limits, both inclusive. A frequency outside every ADIF band MUST be band `other`. Band MUST NOT be read from the comment. | Test |

The ADIF table is used because it is the logging standard the rest of QSO Graph
follows. Band edges are not a licence statement; ITU regions and national band
plans differ, and the interface makes no claim about where a station may transmit.

## F.4 Mode

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0030` | Mode MUST be, in order: the event's structured `mode` field, when present; otherwise the first comment word, compared ASCII case-insensitively and delimited by spaces or the comment's ends, that is one of `CW`, `SSB`, `USB`, `LSB`, `AM`, `FM`, `FT8`, `FT4`, `RTTY`, `PSK31`, `PSK63`, `JT65`, `JS8`, `MSK144`, `Q65`, `SSTV`; otherwise `unknown`. `USB` and `LSB` MUST display and filter as `SSB`. | Test |
| `QGDXC-TUI-0031` | Mode MUST NOT be inferred from frequency. | Inspection |

Frequency-segment inference is excluded on purpose: segment conventions differ by
IARU region and by band, change over time, and would label spots with a mode
nobody reported. A spot that states no mode is `unknown`, and the mode filter can
show or hide `unknown` like any other value.

## F.5 Filters

Filters are display filters only. They never send a command, never change what a
session receives, and never discard a retained spot.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0040` | The interface MUST provide four filters, all off at start, combined with AND: **band** (any subset of the bands of F.3, including `other`); **mode** (any subset of `CW`, `SSB`, `AM`, `FM`, `FT8`, `FT4`, `RTTY`, `other`, `unknown`, where `other` is every recognized F.4 mode not listed); **callsign** (F.5.1); **kind** (`all`, `human`, or `skimmer`, from `spot_kind`). An inactive filter MUST accept every spot. An active band or mode filter MUST accept exactly the selected values; an empty selected set MUST accept no spots. | Test |
| `QGDXC-TUI-0041` | **Callsign** MUST accept up to 16 comma-separated patterns, each 1–20 bytes of ASCII letters, digits, `/`, `-`, `*` (any run, including empty), and `?` (one character). Matching MUST be ASCII case-insensitive against the whole DX call. Matching MUST run in time linear in pattern length times call length; regular expressions MUST NOT be accepted. A callsign filter MUST accept a spot when at least one pattern matches its whole DX call. An empty entry MUST disable the callsign filter. Nonempty entries MUST contain 1–16 nonempty patterns; empty components, excess patterns, and invalid characters or lengths MUST reject the entire entry with a message and leave the previous filter in force. The callsign editor MUST retain at most 335 input bytes; an input or paste exceeding that bound MUST reject the edit without applying a truncated prefix, leaving the previous filter in force. | Test |
| `QGDXC-TUI-0042` | Each source's spots MUST be hideable by toggling that source, independent of the four filters. A hidden source keeps its session and keeps counting spots. | Test |
| `QGDXC-TUI-0043` | Whenever the normal interface is visible, one bounded line MUST identify each active filter. Band, mode, and callsign filters MAY be summarized by selected-value or pattern counts; their complete values MUST be available in the corresponding editor or dialog. The line MUST be clipped to the terminal width. The clear key defined by `QGDXC-TUI-0051` MUST disable all four filters and unhide every source. | Test |

### F.5.1 Why wildcards and not regular expressions

A callsign watch list needs prefix and suffix matching (`VK0*`, `*/P`, `3Y?X`).
Wildcards cover that with matching that cannot run away on a hostile call token;
a user-typed regular expression can.

## F.6 Status and keys

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0050` | Each source MUST have a status line with: label; connection state and `reason` from its latest `connection_state` event; server type; spots received; and seconds since that source's last spot. A source whose state is `disconnected` or `blocked` with retries exhausted MUST be shown so on its line and MUST NOT be removed. Before the first connection event, state and reason MUST be blank and server type MUST be `unknown`. Spot counts MUST start at zero and include spots hidden by filters or source visibility. Before the first spot, age MUST be blank; afterward it MUST be the whole seconds elapsed on the interface's monotonic clock since that source's most recent spot entered the merged list, regardless of its reported UTC time. | Test |
| `QGDXC-TUI-0051` | Ctrl-C MUST quit from every input mode. The following shortcuts apply only outside an editor or dialog: `q` quit; `b` band filter; `m` mode filter; `/` callsign filter; `k` cycle kind `all` → `human` → `skimmer`; `1`–`8` toggle that source; `c` clear filters; `p` pause and resume list scrolling; Up, Down, PageUp, PageDown, Home scroll; `?` help. In the callsign editor, printable characters MUST be treated as entry data rather than global shortcuts; Backspace deletes the preceding character, Enter validates and commits the whole entry, and Escape cancels without changing the filter. Band and mode dialogs MUST use Up and Down to move, Space to toggle a selection, Enter to commit, and Escape to cancel. While paused, new spots MUST still enter retention and counts under `QGDXC-TUI-0012`, without shifting the viewport's top spot solely because newer spots arrive; manual scrolling MUST remain available. If the top spot is evicted, the viewport MUST clamp to the oldest matching retained spot. A filter or visibility change MUST reset the viewport to the newest matching retained spot. Pausing MUST NOT retain a separate spot snapshot. The status line MUST show `PAUSED` and the count of all spots received since the most recent transition into pause, starting at zero. Resuming MUST return to the newest matching retained spot. | Test |
| `QGDXC-TUI-0052` | Quit, Ctrl-C, a fatal interface error, or an unwinding panic MUST cancel every active or retrying session under `QGDXC-SES-0013`, applying end-of-input finalization under `QGDXC-SES-0014`, and close all source resources before process exit. An individual source failure MUST remain isolated under `QGDXC-TUI-0002`. After any terminal-state change, cleanup MUST restore the terminal state saved before entry, including screen, cursor visibility, and input mode, before exit diagnostics are printed. Exit paths that have not changed terminal state MUST emit no restoration sequences. | Test |
| `QGDXC-TUI-0053` | The interface MUST NOT block a session's I/O on drawing. Each source MUST pass events to the interface through a queue of at most 1,024 events; when it is full, the source MUST stop reading its socket until there is room, under `QGDXC-IO-0005`, rather than dropping events. Redraws MUST happen at most 20 times per second and at least once per second while any source is connected. | Test |
| `QGDXC-TUI-0054` | The normal layout MUST fit terminals at least 80 columns wide and 24 rows high. At smaller dimensions, the interface MUST display only a larger-window message clipped to the available cells; this takes precedence over other display requirements. Sources MUST continue processing under their normal bounded admission rules, and quit and Ctrl-C MUST remain available. The normal layout MUST resume when both minimum dimensions are met. | Test |

## F.7 Verification

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0060` | Interface tests MUST render to an in-memory terminal backend and assert on the resulting cell grid, driving input through replay sources and synthetic key events, so that no test needs a real terminal or a network. | Test |
