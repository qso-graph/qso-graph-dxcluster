<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex F: `qg-dxc tui`, the terminal interface

**Status: draft 0.1.0, 2026-11-19.** Application annex of
[QGDXC-SPEC-001](QGDXC-SPEC-001.md). It applies to the `Application` class only.
The library is unchanged by this annex: it passes the full received stream, and
every filter here is a display filter in `qg-dxc`.

`qg-dxc tui` watches one or more nodes at once and shows their spots in one list,
newest first, with a status line per node. It is receive-only under
`QGDXC-GEN-0005`: it sends nothing beyond each session's login and setup.

## F.1 Sources

```
qg-dxc tui --call <CALL> --node <host>:<port>[/<profile>] [--node …]
           [--keep-human N] [--keep-skimmer N]
qg-dxc tui --replay <file> [--replay …] [--keep-human N] [--keep-skimmer N]
```

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0001` | `qg-dxc tui` MUST accept 1–8 sources: repeated `--node <host>:<port>[/<profile>]` with one `--call`, or repeated `--replay <file>`, not both. `<profile>` MUST be one of `auto`, `dxspider`, `arcluster`, `cccluster`, or `rbn`; omission selects `auto`. A bracketed IPv6 literal MUST be accepted as `[<addr>]:<port>`. The call, every endpoint, and every profile MUST be validated before any connection opens, under `QGDXC-IO-0003`; with neither, F.1.1 applies; more than eight, a duplicate endpoint (host compared ASCII case-insensitively, same port), or a port outside 1–65535 MUST be rejected with a configuration error naming the offending argument. | Test |
| `QGDXC-TUI-0002` | Each `--node` MUST run its own session under §3, with its own retry state, timers, and transport. One source's failure, retry, exhaustion, or blocking outcome MUST NOT close, delay, or restart another source. | Test |
| `QGDXC-TUI-0003` | Each `--replay` path MUST name a regular file, checked before it is opened; any other file type MUST be rejected with a configuration error. Each `--replay` source MUST use the replay adapter of `QGDXC-REC-0004`, opening no socket. All sources MUST share one interface-start monotonic origin; a record with timestamp `t` becomes due `t` seconds after that origin, without subtracting the first record's timestamp. Due records MUST be delivered in ascending `t`, then source argument order, then physical record order; all continuation batches from a record retain its position in that order. Overdue records MUST be delivered without dropping them. This aligns recording-relative time, not original UTC chronology. At valid end of file, the adapter MUST be finalized once, the source MUST be marked as finished, and its retained spots and final connection state MUST remain visible without scheduling another connection. A replay error MUST stop only that source and remain visible on its status line. | Test |
| `QGDXC-TUI-0004` | Each source MUST receive a unique label of at most 16 display columns. Its base is the `--node` host or `--replay` file stem, sanitized under `QGDXC-TUI-0013`; an empty stem uses `source`. Process sources in argument order. First try the base clipped to 16 columns. If already used, try suffixes `#2`, `#3`, … in increasing order, clipping the base to leave room for the suffix, until the complete rendered label is unused. The label is local configuration, not network text. | Test |
| `QGDXC-TUI-0005` | When standard input or standard output is not a terminal, `qg-dxc tui` MUST exit with an error before opening any source, and MUST NOT write terminal control sequences. | Test |

`qg-dxc tui` records nothing in this draft. `qg-dxc connect --record` remains the
way to capture a session.

### F.1.1 Saved sources

```
qg-dxc tui [--sources <file>] [--keep-human N] [--keep-skimmer N] [--theme <name>] [--grid <locator>]
```

An operator can keep their own nodes in a JSON file and choose among them when the
interface starts. The project ships no such file and no node list
(`QGDXC-IO-0003`); the user guide shows how to write one.

```json
{
  "version": 1,
  "call": "Q1ABC",
  "sources": [
    { "name": "Home node", "node": "dx.example.invalid:7300", "selected": true },
    { "name": "RBN CW", "node": "rbn.example.invalid:7000", "profile": "rbn", "call": "Q1ABC-2" }
  ]
}
```

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0006` | Without `--node` or `--replay`, `qg-dxc tui` MUST read a sources file: the `--sources <file>` path when given, otherwise the default path. The default path MUST be `qso-graph/dxcluster-sources.json` under `%APPDATA%` on Windows; elsewhere under `$XDG_CONFIG_HOME` when it is set to an absolute path, otherwise under `$HOME/.config`. If the needed variable is unset, empty, or not absolute, the interface MUST exit with a configuration error naming it. `--sources` MUST NOT be combined with `--node`, `--replay`, or `--call` (it MAY be combined with `--grid`), and MUST appear at most once. The path MUST name a regular file, checked before it is opened, and the interface MUST read at most 65,536 bytes; a missing file, another file type, or a longer file MUST be rejected with a configuration error naming the path. When the default file is missing, the error MUST also say how to create it, with a link to the user guide, and show the `--node` form; those lines MUST be the program's own text, with only the escaped path taken from the environment. Every rejection under F.1.1 MUST happen before the interface changes terminal state or opens any source. | Test |
| `QGDXC-TUI-0007` | The file MUST be UTF-8 JSON (RFC 8259) whose top level is an object with the members `version` and `sources`, MAY have `call` and `grid` (F.15), and no other member. `version` MUST be the integer 1. `sources` MUST be an array of 1–64 objects. Each source object MUST have `name` and `node`, and MAY have `profile`, `selected`, and `call`, and no other member. Every `call`, at either level, MUST satisfy `QGDXC-IO-0003`; a source logs in with its own `call` when present, otherwise the top-level `call`, and a source with neither MUST be rejected. `name` MUST be 1–32 bytes of ASCII U+0020–U+007E, not beginning or ending with a space, and unique within the file under ASCII case-insensitive comparison. `node` MUST be a string in the `--node` endpoint form of `QGDXC-TUI-0001`, without a `/<profile>` suffix; endpoints MUST be unique under that requirement's comparison. `profile` MUST be one of the `QGDXC-TUI-0001` profile names, defaulting to `auto`. `selected` MUST be a boolean, defaulting to false. Invalid JSON or a duplicate member name in any object MUST reject the whole file with a configuration error giving the line and column. A missing or unexpected member, a wrong JSON type, or an invalid value MUST reject the whole file with a configuration error naming the JSON path of the first offending value (for example `sources[2].node`). Errors MUST be sanitized under `QGDXC-TUI-0013`. No part of a rejected file is used. | Test |
| `QGDXC-TUI-0008` | After a valid file is read, the interface MUST show a Sources screen listing every saved source in file order with its name, endpoint, and profile, with the sources marked `selected` already chosen. Up and Down MUST move, Space MUST toggle the source under the cursor, Enter MUST start the chosen sources, and `q`, Escape, or Ctrl-C MUST exit without opening any source, restoring the terminal under `QGDXC-TUI-0052`. Enter with no source chosen or more than eight chosen MUST keep the screen open with `Choose 1 to 8 sources` and open nothing. The chosen sources MUST then run exactly as the equivalent `--call` and `--node` arguments in file order would, under `QGDXC-TUI-0001` through `QGDXC-TUI-0004`, except that each label's base MUST be the source's `name` rather than its host. The screen MUST follow `QGDXC-TUI-0013`, `QGDXC-TUI-0054`, and the themes of F.8. | Test |
| `QGDXC-TUI-0009` | The interface MUST NOT write, create, or rename the sources file or its directory. The file reader MUST be covered by a fuzz target that accepts arbitrary bytes and either returns a validated source list or a configuration error, never panicking or exceeding the bounds of `QGDXC-TUI-0006` and `QGDXC-TUI-0007`. | Test |

## F.2 Merged spot list

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0010` | Every `spot` event from every source MUST enter one list in arrival order at the application, newest at the top. Spots MUST NOT be merged or reordered by reported spot time, and every copy of a report MUST be retained; the display MAY collapse copies only as `QGDXC-TUI-0015` defines. | Test |
| `QGDXC-TUI-0011` | Each spot row MUST allocate separate, nonoverlapping columns for a kind marker, reported UTC time `HHMM`, frequency, DX call, spotter, band under F.3, display mode under F.4, and comment. Wide rows under `QGDXC-TUI-0070` MUST also allocate SNR, WPM, and the source's short label (the portion before its first period), in a column sized to the longest short label between 3 and 8 characters; the complete source label MUST be available in the sources panel or help screen. The wide frequency column MUST be 10 characters, so a 10 GHz frequency such as `10489520.0` is shown whole. A human spot MUST carry the interface-owned `▎` marker; a skimmer marker MUST be blank. Fields MUST be sanitized under `QGDXC-TUI-0013` before measurement and clipped to their allocated columns; a truncated non-comment field MUST end with `~`. Every non-comment column MUST have positive width. The comment receives the remaining width, which MAY be zero. Frequency MUST use decimal kHz with exactly one fractional digit, rounded to the nearest 100 Hz with ties upward using integer arithmetic: let `n = (frequency_hz + 50) / 100`, using integer division, and display `n / 10`, `.`, and `n % 10`. Derived band and mode MUST use the original event values, not clipped or rounded display text. A missing optional field MUST show as blank, never as a guessed value; mode `unknown` MUST display as a blank Mode column while still filtering as `unknown`. | Test |
| `QGDXC-TUI-0015` | Two arrivals MUST be copies of one report when both carry a nonempty `spotter_call` and `call` that match ignoring ASCII case, both carry a nonzero `frequency_hz` within 1,000 Hz of each other, and the later arrives no more than 120 seconds after the earlier. An arrival MUST be compared only with retained first copies, so frequencies cannot chain; any other arrival is the first copy of its own report. Copies from the same source and from different sources MUST be treated alike. By default the list and the map MUST show, for each report, only the oldest copy that passes the filters and source visibility of F.5, so hiding one source shows another source's copy in its place. In wide rows the source column MUST then read `<label>+<n>`, where `<n>` counts the report's other copies that pass the same filters and visibility, and the column MUST widen by two characters when more than one source is configured. Outside an editor or dialog, `d` MUST switch between collapsed copies and every copy; while every copy is shown, the filter summary MUST include `dupes`, no `+<n>` MUST be shown, and the column MUST keep the width of `QGDXC-TUI-0011`. `c` MUST NOT change this setting. Collapsing MUST NOT change retention, counts, activity statistics, or recordings. | Test |
| `QGDXC-TUI-0012` | `--keep-human N` and `--keep-skimmer N` MUST set independent retention caps, each defaulting to 100 spots. Each value MUST be an integer in 10..=10000, inclusive; any other value MUST be rejected before opening sources with a usage error naming the option and allowed range. Outside an editor or dialog, `l` MUST open a Limits editor with Human and Skimmer numeric fields prefilled with the current caps. Up and Down MUST move between fields, and Tab MUST cycle between them. ASCII digits and Backspace MUST edit the focused field, retaining at most five digits. Enter MUST commit both caps together only when each is in the allowed range; otherwise the editor MUST stay open, show `Limits must be 10 to 10,000`, and leave both caps unchanged. Escape MUST cancel without changes. Lowering a cap MUST immediately evict the oldest spots of that kind, including spots held while paused, until its retained count fits; raising a cap MUST preserve all retained spots. Runtime changes MUST follow the viewport eviction rules of `QGDXC-TUI-0051`, without changing activity charts or rate counters. The footer and help screen MUST list `l limits`. An arriving human spot MUST evict only the oldest human spot when its cap is full; an arriving skimmer spot MUST evict only the oldest skimmer spot when its cap is full. For retention, a spot whose `spot_kind` is neither `human` nor `skimmer`, including a missing kind, MUST count as skimmer. Retained spots MUST preserve the order of `QGDXC-TUI-0010`. These caps MUST apply while paused, including newly received spots under `QGDXC-TUI-0051`. Filters select from retained spots; changing a filter MUST NOT discard or reorder retained spots. The status line MUST show retained and total-received counts. | Test |
| `QGDXC-TUI-0013` | Every external text value displayed by the interface, including received fields, source labels, filter input, replay notes, errors, and diagnostics, MUST be sanitized before measurement, clipping, or output. Only Unicode scalars U+0020–U+007E MAY appear literally; every other scalar, including C0, DEL, C1, bidirectional formatting, zero-width, and combining characters, MUST appear as the ASCII escape `\u{hhhh}`, using lowercase hexadecimal with at least four digits. Each resulting ASCII character occupies one display column. Fields MUST be confined to their allocated cell rectangles, without wrapping into another field or row. Fixed interface-owned glyphs for borders, markers, state dots, superscripts, separators, and block charts MAY use Unicode. External values MUST NOT use this exception. Terminal control sequences MUST originate only from the renderer, never from displayed text. This satisfies and is stricter than `QGDXC-SEC-0003`. | Test |
| `QGDXC-TUI-0014` | Every live and replay session MUST set `retain_third_party_data` to false, and the interface MUST NOT expose an option to enable it. Network-derived display values MUST be computed only from event fields after `QGDXC-SEC-0005` has been applied. This guarantees the transformations defined by that requirement, not detection of arbitrary personal data or coordinates embedded in free text. | Test |

## F.3 Band

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0020` | Band MUST be derived from `frequency_hz` using the ADIF 3.1.8 `Band` enumeration's lower and upper limits, both inclusive. A frequency outside every ADIF band MUST be band `other`. Band MUST NOT be read from the comment. | Test |

The ADIF table is used because it is the logging standard the rest of QSO Graph
follows. Band edges are not a licence statement; ITU regions and national band
plans differ, and the interface makes no claim about where a station may transmit.

## F.4 Mode

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0030` | Mode MUST be, in order: the event's structured `mode` field, when present; otherwise the first comment word, compared ASCII case-insensitively and delimited by spaces or the comment's ends, that is an ADIF 3.1.8 Mode or Submode name consisting of a single word, excluding import-only names; otherwise `unknown`. Mode MUST NOT be inferred from frequency. `USB` and `LSB` MUST display and filter as `SSB`. | Test |
| `QGDXC-TUI-0031` | Mode MUST NOT be inferred from frequency. | Inspection |

Frequency-segment inference is excluded on purpose: segment conventions differ by
IARU region and by band, change over time, and would label spots with a mode
nobody reported. A spot that states no mode is `unknown`, and the mode filter can
show or hide `unknown` like any other value.

## F.5 Filters

Filters are display filters only. They never send a command, never change what a
session receives, and never discard a retained spot.

The filter set, its keys, and its matching rules are provisional in this draft and
are expected to change with use. The guarantees in the paragraph above are not.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0040` | The interface MUST provide five filters, all off at start, combined with AND: **self-spots** (`QGDXC-TUI-0045`); **band** (any subset of the bands of F.3, including `other`); **mode** (any subset of `CW`, `SSB`, `AM`, `FM`, `FT8`, `FT4`, `RTTY`, `JTTY`, `other`, `unknown`, where `other` is every recognized F.4 mode not listed); **callsign** (F.5.1); **kind** (`all`, `human`, or `skimmer`, from `spot_kind`). An inactive filter MUST accept every spot. An active band or mode filter MUST accept exactly the selected values; an empty selected set MUST accept no spots. | Test |
| `QGDXC-TUI-0041` | **Callsign** MUST accept up to 16 comma-separated patterns, each 1–20 bytes of ASCII letters, digits, `/`, `-`, `*` (any run, including empty), and `?` (one character). Matching MUST be ASCII case-insensitive against the whole DX call. Matching MUST run in time linear in pattern length times call length; regular expressions MUST NOT be accepted. A callsign filter MUST accept a spot when at least one pattern matches its whole DX call. An empty entry MUST disable the callsign filter. Nonempty entries MUST contain 1–16 nonempty patterns; empty components, excess patterns, and invalid characters or lengths MUST reject the entire entry with a message and leave the previous filter in force. The callsign editor MUST retain at most 335 input bytes; input exceeding that bound MUST reject the edit without applying a truncated prefix, leaving the previous filter in force. The interface MUST NOT enable bracketed paste, so pasted text arrives as individual key events and no input decoder buffers an unbounded paste. | Test |
| `QGDXC-TUI-0042` | Each source's spots MUST be hideable by toggling that source, independent of the five filters. A hidden source keeps its session and keeps counting spots. | Test |
| `QGDXC-TUI-0043` | Whenever the normal interface is visible, one bounded line MUST identify each active filter. Band, mode, and callsign filters MAY be summarized by selected-value or pattern counts; their complete values MUST be available in the corresponding editor or dialog. The line MUST be clipped to the terminal width. The clear key defined by `QGDXC-TUI-0051` MUST disable all five filters and unhide every source. | Test |
| `QGDXC-TUI-0044` | The band and mode dialogs MUST list `All` as their first row, above the values of `QGDXC-TUI-0040`. A dialog opened while its filter is inactive MUST show every value selected. `All` MUST show `[x]` when every value is selected, `[ ]` when none is, and `[-]` otherwise. Space on `All` MUST select every value unless every value is already selected, in which case it MUST clear every value. Enter with every value selected MUST leave that filter inactive; Enter with any other selection, including none, MUST apply it as `QGDXC-TUI-0040` defines. Escape MUST leave the filter unchanged. | Test |
| `QGDXC-TUI-0045` | A spot MUST be a self-spot when its `spotter_call`, less everything from its first `-` (as in `-#` or `-2`), is nonempty and equals its `call`, ignoring ASCII case; a portable or other suffix on the DX call (`/P`) MUST make it a different call. Outside an editor or dialog, `s` MUST toggle a filter that hides self-spots; while it is on, the filter summary MUST include `self:hidden`. The clear key MUST turn it off. A spot without a spotter or call MUST NOT be a self-spot. | Test |

### F.5.1 Why wildcards and not regular expressions

A callsign watch list needs prefix and suffix matching (`Q0*`, `*/P`, `Q9?X`).
Wildcards cover that with matching that cannot run away on a hostile call token;
a user-typed regular expression can.

## F.6 Status and keys

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0050` | Each source MUST have status details with: label; connection state and `reason` from its latest `connection_state` event; server type; spots received; and seconds since that source's last spot. Both layouts MUST make the full details available on the help screen; the dashboard MUST show source summaries and exceptional status details under `QGDXC-TUI-0070`. A source whose state is `disconnected` or `blocked` with retries exhausted MUST be shown so on its line and MUST NOT be removed. Detail text MAY be clipped to the available width under `QGDXC-TUI-0013`. Before the first connection event, state and reason MUST be blank and server type MUST be `unknown`. Spot counts MUST start at zero and include spots hidden by filters or source visibility. Before the first spot, the age field MUST be omitted; afterward it MUST be the whole seconds elapsed on the interface's monotonic clock since that source's most recent spot entered the merged list, regardless of its reported UTC time. | Test |
| `QGDXC-TUI-0051` | Ctrl-C MUST quit from every input mode. The following shortcuts apply only outside an editor or dialog: `q` quit; `b` band filter; `m` mode filter; `/` callsign filter; `k` cycle kind `all` → `human` → `skimmer`; `d` switch between collapsed and all copies under `QGDXC-TUI-0015`; `s` hide or show self-spots under `QGDXC-TUI-0045`; `1`–`8` toggle that source; `c` clear filters; `p` pause and resume list scrolling; Up, Down, PageUp, PageDown, Home scroll, where scrolling away from the newest matching spot MUST pause as `p` does and Home MUST resume and show the newest spot; `t` cycle themes under `QGDXC-TUI-0071`; `M` (capital) switch between the spot list and the map under F.9; `B` (capital) switch between the spot list and the beacon view under F.11; `W` (capital) switch between the spot list and the space-weather view under F.12; `?` help. In the callsign editor, printable characters MUST be treated as entry data rather than global shortcuts; Backspace deletes the preceding character, Enter validates and commits the whole entry, and Escape cancels without changing the filter. Band and mode dialogs MUST use Up and Down to move, Space to toggle a selection, Enter to commit, and Escape to cancel. While paused, new spots MUST still enter retention and counts under `QGDXC-TUI-0012`, without shifting the viewport's top spot solely because newer spots arrive; manual scrolling MUST remain available. If the top spot is evicted, the viewport MUST clamp to the oldest matching retained spot. A filter or visibility change MUST reset the viewport to the newest matching retained spot. Pausing MUST NOT retain a separate spot snapshot. The status line MUST show `PAUSED` and the count of all spots received since the most recent transition into pause, starting at zero. Resuming MUST return to the newest matching retained spot. | Test |
| `QGDXC-TUI-0052` | Quit, Ctrl-C, a fatal interface error, or an unwinding panic MUST cancel every active or retrying session under `QGDXC-SES-0013`, applying end-of-input finalization under `QGDXC-SES-0014`, and close all source resources before process exit. An individual source failure MUST remain isolated under `QGDXC-TUI-0002`. After any terminal-state change, cleanup MUST restore the terminal state saved before entry, including screen and input mode, before exit diagnostics are printed. The cursor MUST be hidden while the interface owns the screen and MUST be shown on exit, because a terminal cannot report its earlier cursor visibility. Exit paths that have not changed terminal state MUST emit no restoration sequences. Terminal restoration MUST NOT wait for any source. A source blocked in an operation that cannot be cancelled, such as host-name resolution, MUST NOT delay process exit by more than 2 seconds after restoration; the operating system then closes its resources. | Test |
| `QGDXC-TUI-0053` | The interface MUST NOT block a session's I/O on drawing. Each source MUST pass events to the interface through a queue of at most 1,024 events; when it is full, the source MUST stop reading its socket until there is room, under `QGDXC-IO-0005`, rather than dropping events. Redraws MUST happen at most 20 times per second and at least once per second while any source is connected. | Test |
| `QGDXC-TUI-0054` | The normal layout MUST fit terminals at least 80 columns wide and 24 rows high. At smaller dimensions, the interface MUST display only a larger-window message clipped to the available cells; this takes precedence over other display requirements. Sources MUST continue processing under their normal bounded admission rules, and quit and Ctrl-C MUST remain available. The normal layout MUST resume when both minimum dimensions are met. | Test |

## F.7 Verification

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0060` | Interface tests MUST render to an in-memory terminal backend and assert on the resulting cell grid, driving input through replay sources and synthetic key events, so that no test needs a real terminal or a network. | Test |

## F.8 Layout and appearance

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0070` | In both layouts the inverse ` QG DX Cluster ` logo MUST be followed by the running version as ` v<version>` in the dim role. At widths of at least 100 columns, the interface MUST show the logo, version, live-source count (`1 source live`, otherwise `<n> sources live`), retained-spot count, spots per minute, a 12-cell minute sparkline, and a right-aligned UTC date and time. Below it, two side-by-side rounded boxes MUST show sources and band activity. Each source row MUST show a key superscript `¹`–`⁸`, state dot (`●` ready, `◐` connecting or retrying, `○` hidden or inactive), label, server profile, state, received count, and an eight-cell minute sparkline. Additional detail rows MUST appear only for sources that are not ready, including connecting, retrying, failed, replay-finished, or error states. Each MUST use its key superscript followed by one plain-English status line; ready sources MUST have no detail row. Replay completion MUST read `replay finished`. Full status details remain available under `QGDXC-TUI-0050`. A known retry deadline MUST show a countdown in seconds rounded upward, never negative. The bands box MUST show three rows of vertical block bars, a label row, and a count row, for 160, 80, 60, 40, 30, 20, 17, 15, 12, 10, and 6 meters in that order. Below the panels, a rounded spots box MUST show `newest first`, active filters including hidden-source count, and the columns of `QGDXC-TUI-0011`. While paused it MUST show an inverse `PAUSED` chip and the new-arrival count, with filter information still visible. The bands box's bottom border MUST show solar figures under F.10. The spots box's bottom border MUST show total received and `<h> human · <s> skimmer`, where `<h>` and `<s>` are the retained counts under `QGDXC-TUI-0012`, in both layouts. Counts MUST include spots hidden by filters or source visibility. The footer MUST show key hints with emphasized keys and the theme name at the right; hints MUST be separated by two spaces from 150 columns and by one space below that; below 120 columns the `d`, `s` and `t` hints MUST show the key alone, below 130 columns the `B` and `W` hints MUST show the key alone and the source hint MUST read `1-8 src`, and below 100 columns the source hint MUST show `1-8` alone, so every hint fits at 80, 100, 120, 130, 140 and 150 columns with the longest theme name while paused. At widths below 100 columns, the header MUST omit the spot count and the spots-per-minute figure (the sparkline still shows the rate), the panels MUST fold into one bounded sources line, preserving each full short label (the portion before its first period) when its allocated share permits, and spot rows MUST omit SNR, WPM, and source columns. The minimum dimensions of `QGDXC-TUI-0054` take precedence. | Test |
| `QGDXC-TUI-0071` | `--theme` MUST accept exactly `color`, `daylight`, `phosphor`, `amber`, or `mono`, rejecting invalid or repeated values before opening sources. With no explicit theme, a set, nonempty `NO_COLOR` MUST select `mono`; otherwise the initial theme MUST be `color`. An explicit theme MUST override `NO_COLOR`. Outside editors and dialogs, `t` MUST cycle in the order listed, wrapping to `color`. Color roles MUST use the F.8.1 palette, subject to `QGDXC-TUI-0072`. Human DX calls MUST be bold and bright; skimmer calls and comments MUST use quieter roles. Band and mode fields MUST use their palette roles; unlisted bands or modes MUST use the text role. SNR MUST use the good role at 20 dB or higher, text at 8–19 dB, and dim below 8 dB. | Test |
| `QGDXC-TUI-0072` | The renderer MUST leave the terminal background unchanged except for the logo, chips, and new-spot highlights. `mono` MUST use terminal-default foreground and background without RGB, indexed, or ANSI palette colors; bold, underline, and inverse attributes MAY preserve emphasis. If `COLORTERM`, compared ASCII case-insensitively, is `truecolor` or `24bit`, color themes MUST emit RGB colors. Otherwise a `TERM` containing `256color` MUST select the nearest xterm-256 palette entry by squared RGB distance, choosing the lowest index on ties. Other terminals MUST use the nearest standard ANSI 16-color entry by the same rule, with black dim, border, spotter, time, and heading roles promoted to bright black for readability. On 16-color terminals, new-spot emphasis MUST use bold instead of a background color. | Test |
| `QGDXC-TUI-0073` | Activity statistics MUST count arrivals across all sources independently of filters, visibility, pause, and retained-spot eviction. A bounded history of at most 900 one-second counter buckets MUST supply rolling 60-second rates and 15-minute band counts; minute sparklines MUST include the current partial minute, with missing minutes zero-filled and their maximum scaled to the highest block. Zero-valued sparkline cells MUST display `▁` in the dim role, preserving the full extent defined by `QGDXC-TUI-0070`. Band bars MUST scale against the largest displayed count. New spots MUST carry their monotonic arrival time. New human spots MUST receive a highlight for approximately three seconds, fading in three one-second steps from the new-highlight role toward the palette background before returning to the terminal background. Skimmer spots, and spots counted as skimmer under `QGDXC-TUI-0012`, MUST NOT be highlighted, so a busy skimmer feed does not keep the list flashing. The 16-color exception of `QGDXC-TUI-0072` applies. The wide SNR column MUST use only integer `snr_db`; WPM MUST use only integer `speed` with `speed_unit` equal to `wpm`. The interface MUST NOT parse additional metrics from comments or guess missing values. As a display-only derivation, the Comment column MUST omit a leading mode/SNR/WPM prefix only when it exactly matches the typed fields fully shown in that layout, preserving the remaining comment text. Otherwise, in wide rows of a spot with no integer `snr_db`, it MUST omit a leading comment word equal, ignoring ASCII case, to the displayed mode when that mode is not `unknown` and fits the Mode column; `USB` and `LSB` are not equal to `SSB` and remain. If an exact derivation is unavailable, it MUST show the original comment. This derivation MUST NOT change retained events or recordings. | Test |

### F.8.1 Palette

Decimal RGB triplets below define the color themes. The monochrome theme uses
terminal defaults and attributes under `QGDXC-TUI-0072`. Palette backgrounds are
used only inside chips and highlights, never as a screen fill.

| Role | Color | Daylight | Phosphor | Amber |
|---|---|---|---|---|
| Background | `15, 19, 22` | `247, 247, 242` | `4, 16, 7` | `18, 10, 0` |
| Text | `197, 206, 214` | `43, 48, 54` | `63, 227, 108` | `255, 176, 0` |
| Dim | `102, 114, 125` | `122, 131, 140` | `31, 122, 58` | `138, 93, 0` |
| Border | `52, 65, 76` | `185, 193, 200` | `23, 85, 42` | `92, 62, 0` |
| Title | `238, 243, 247` | `17, 20, 23` | `182, 255, 201` | `255, 212, 121` |
| Accent and keys | `255, 180, 84` | `179, 92, 0` | `182, 255, 201` | `255, 212, 121` |
| Good | `127, 217, 98` | `47, 138, 47` | `182, 255, 201` | `255, 212, 121` |
| Warning | `255, 180, 84` | `179, 92, 0` | `182, 255, 201` | `255, 212, 121` |
| Bad | `240, 113, 120` | `192, 57, 43` | `182, 255, 201` | `255, 212, 121` |
| Sparkline | `92, 207, 230` | `15, 127, 154` | `63, 227, 108` | `255, 176, 0` |
| Human DX | `255, 255, 255` | `0, 0, 0` | `182, 255, 201` | `255, 212, 121` |
| Skimmer DX | `197, 206, 214` | `43, 48, 54` | `63, 227, 108` | `255, 176, 0` |
| Spotter | `127, 139, 150` | `107, 116, 125` | `42, 166, 76` | `192, 132, 0` |
| Frequency | `230, 237, 243` | `17, 20, 23` | `63, 227, 108` | `255, 176, 0` |
| UTC time | `143, 155, 166` | `107, 116, 125` | `42, 166, 76` | `192, 132, 0` |
| Column heading | `143, 155, 166` | `107, 116, 125` | `42, 166, 76` | `192, 132, 0` |
| New highlight | `38, 68, 90` | `255, 226, 166` | `14, 61, 28` | `61, 39, 0` |
| Logo background | `255, 180, 84` | `179, 92, 0` | `63, 227, 108` | `255, 176, 0` |
| Logo foreground | `15, 19, 22` | `255, 255, 255` | `4, 16, 7` | `18, 10, 0` |
| CW | `92, 207, 230` | `15, 127, 154` | `63, 227, 108` | `255, 176, 0` |
| FT8 | `212, 166, 255` | `138, 63, 192` | `63, 227, 108` | `255, 176, 0` |
| SSB | `255, 213, 128` | `148, 101, 0` | `63, 227, 108` | `255, 176, 0` |
| RTTY | `149, 230, 203` | `19, 134, 106` | `63, 227, 108` | `255, 176, 0` |
| 160 m | `240, 113, 120` | `192, 57, 43` | `63, 227, 108` | `255, 176, 0` |
| 80 m | `242, 150, 104` | `200, 84, 31` | `63, 227, 108` | `255, 176, 0` |
| 60 m | `255, 180, 84` | `184, 115, 10` | `63, 227, 108` | `255, 176, 0` |
| 40 m | `255, 213, 128` | `154, 125, 0` | `63, 227, 108` | `255, 176, 0` |
| 30 m | `213, 227, 107` | `111, 138, 0` | `63, 227, 108` | `255, 176, 0` |
| 20 m | `145, 215, 106` | `47, 138, 47` | `63, 227, 108` | `255, 176, 0` |
| 17 m | `95, 212, 160` | `19, 134, 106` | `63, 227, 108` | `255, 176, 0` |
| 15 m | `92, 207, 230` | `15, 127, 154` | `63, 227, 108` | `255, 176, 0` |
| 12 m | `89, 168, 240` | `37, 99, 196` | `63, 227, 108` | `255, 176, 0` |
| 10 m | `143, 140, 245` | `91, 79, 208` | `63, 227, 108` | `255, 176, 0` |
| 6 m | `212, 166, 255` | `138, 63, 192` | `63, 227, 108` | `255, 176, 0` |
| Kp quiet | `127, 217, 98` | `47, 138, 47` | `63, 227, 108` | `255, 176, 0` |
| Kp G1 | `255, 225, 77` | `154, 125, 0` | `182, 255, 201` | `255, 212, 121` |
| Kp G2 | `255, 163, 77` | `200, 122, 0` | `182, 255, 201` | `255, 212, 121` |
| Kp G3 | `255, 127, 42` | `200, 84, 31` | `182, 255, 201` | `255, 212, 121` |
| Kp G4 | `240, 72, 72` | `192, 57, 43` | `182, 255, 201` | `255, 212, 121` |
| Kp G5 | `179, 32, 42` | `127, 16, 16` | `182, 255, 201` | `255, 212, 121` |

## F.9 Map

The map shows where spots are heard from and where the spotted stations are: each
new spot draws a path from its spotter to the DX station. It is a display only, like
the list.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0080` | Outside an editor or dialog, `M` (capital) MUST switch the spots box between the spot list and the map; lowercase `m` remains the mode filter. The header, panels, filters, retention, pause and every other key MUST be unchanged by the switch. The footer MUST show `M map` while the list is shown and `M list` while the map is shown, at widths of at least 100 columns, and `M` alone below that. The help screen MUST list `M: map`. | Test |
| `QGDXC-TUI-0081` | The map MUST fill the spots box with an equirectangular world map, longitude −180° to 180° left to right and latitude −90° to 90° bottom to top, with the coastline drawn in the border role. The bottom border MUST show the number of paths drawn and a legend distinguishing locator ends from prefix ends. | Test |
| `QGDXC-TUI-0082` | A spot's DX end MUST be the center of its four-character `grid` when present (an exact end), otherwise the place of its `call` under `QGDXC-TUI-0083` (an approximate end). Its spotter end MUST be the place of its `spotter_call` under `QGDXC-TUI-0083`. Ends MUST be determined once, when the spot is retained. A spot with either end unknown MUST NOT be drawn and MUST still be counted and listed. Exact ends MUST be drawn as points and approximate ends as small circles. | Test |
| `QGDXC-TUI-0083` | Callsign places MUST come from a country file in the `cty.dat` format: entity headers of eight colon-terminated fields with latitude and longitude, the longitude positive west, followed by a comma-separated list of prefixes ending in `;`, each optionally carrying `(cq)`, `[itu]`, `<lat/lon>`, `{continent}` or `~offset~` markers, of which only `<lat/lon>` changes the place. The reader MUST reject the whole file on any malformed header, coordinate outside ±90° or ±180°, prefix that is not capitals, digits and `/` of 1–32 bytes, or unknown marker, and MUST bound the file to 1 MiB, 1,000 entities and 100,000 prefixes. Lookup MUST ignore a `-#` or `-N` suffix, MUST use an exact-callsign entry when one matches, and otherwise the longest matching prefix; for a call with `/`, it MUST drop `P`, `M`, `MM`, `AM`, `QRP`, `A`, `R`, `LH` and single-digit parts and use the shorter remaining part. The shipped table MUST be generated from the published file by `scripts/strip-cty.sh`, which removes exact-callsign entries. The reader MUST be covered by a fuzz target. | Test |
| `QGDXC-TUI-0084` | A path MUST follow the short great-circle arc between its ends, drawn in segments of at most 2° of arc, and MUST be split where it crosses the 180th meridian so that no segment spans the map. Antipodal ends, whose great circle is undefined, MUST draw no path. | Test |
| `QGDXC-TUI-0085` | The map MUST draw paths only for spots that pass the filters and visibility of F.5, and for one copy of each report under `QGDXC-TUI-0015`. A human spot's path MUST use its band role for its first 20 seconds, the dim role until 40 seconds, the border role until 60 seconds, and then disappear. A skimmer spot's path, including a spot counted as skimmer under `QGDXC-TUI-0012`, MUST use the dim role and disappear after 20 seconds, so a busy skimmer feed does not cover the map. Newer paths MUST be drawn over older ones. | Test |
| `QGDXC-TUI-0086` | The map MUST draw at most 200 paths per frame, the newest first, so drawing work is bounded whatever the arrival rate. | Test |

## F.10 Solar figures

Cluster nodes broadcast the latest solar figures as WWV reports (typically every three hours)
and WCY reports (typically hourly). The interface shows the latest one it has received, so an
operator sees the conditions without leaving the spot list. It uses only the `wwv` and
`wcy` events of Annex A; it makes no other connection. WWV reports carry NOAA's geophysical alert as
broadcast by NIST's time station WWV in Fort Collins, Colorado; WCY reports carry the hourly geophysical
summary of DK0WCY, run by volunteers of the Deutscher Amateur-Radio-Club (DARC). Cluster nodes relay both.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0090` | The interface MUST keep the most recent `wwv` or `wcy` event received from any source, replacing it with each later one of either kind regardless of the report's `hour`, of filters and of source visibility. It MUST keep the event's kind, `hour`, source, monotonic arrival time, and the integer fields `sfi`, `a` and `k`, and for `wcy` also `expected_k` and `r`; text fields and absent or non-integer values MUST be ignored. Keeping it MUST NOT change spot retention, counts or activity statistics. | Test |
| `QGDXC-TUI-0091` | At widths of at least 100 columns, the bands box's bottom border MUST show the kept report as `<superscript><kind> <hour>:00 UTC · SFI <n> · A <n> · K <n>`, followed for `wcy` by ` · expK <n> · R <n>`, where `<superscript>` is the source's key superscript, `<kind>` is `WWV` or `WCY`, `<hour>` has two digits, and each field is shown only when kept. When that text, with ` · stale` under `QGDXC-TUI-0092`, does not fit inside the box's border, `expK` and `R` MUST be omitted, and if it still does not fit the hour MUST read `<hour>Z`. Below 100 columns, the spots box's bottom border MUST show `SFI <n> A <n> K <n>` with the same omissions, centered, and only when it fits with at least one border cell on each side of the labels of `QGDXC-TUI-0070`. Before any report is kept, nothing MUST be shown. Text MAY be clipped to the box under `QGDXC-TUI-0013`. | Test |
| `QGDXC-TUI-0092` | A kept `wwv` report MUST be shown as stale once more than 3 hours, and a `wcy` report once more than 1 hour, has passed on the monotonic clock since its arrival: the figures MUST then use the dim role and, in the wide layout, end with ` · stale`. Otherwise `K` MUST use the warning role at 4, the bad role at 5 through 9, and the text role below 4; a `K` outside 0–9 MUST be shown as received in the text role. Other figures MUST use the text role. | Test |

## F.11 Beacons

The beacon view shows the NCDXF/IARU International Beacon Project's beacons: which
is on each band now, from the library's schedule (Annex G), and which the Reverse
Beacon Network has reported recently. It is a display only, like the list and the map.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0100` | The beacon view and the map MUST take the beacons, frequencies, the beacon on air, the next beacon and the seconds left from the library under `QGDXC-BCN-0001`, with the interface's UTC clock of `QGDXC-TUI-0070` as `t`. No network time or beacon status MUST be fetched. | Test |
| `QGDXC-TUI-0101` | Outside an editor or dialog, `B` (capital) MUST switch the spots box between the beacon view and the spot list; lowercase `b` remains the band filter. Showing the beacon view MUST leave the map, and `M` MUST leave the beacon view for the map. The header, panels, filters, retention, pause and every other key MUST be unchanged by the switch. The footer MUST show `B beacons` while the beacon view is not shown and `B list` while it is, at widths of at least 130 columns, and `B` alone below that, within the fit of `QGDXC-TUI-0070`. The help screen MUST list `B: beacons`. | Test |
| `QGDXC-TUI-0102` | A `spot` event that the library identifies as a beacon report under `QGDXC-BCN-0002` MUST be kept as that beacon's latest report on that band, from any source and regardless of filters and source visibility: its monotonic arrival time, its source, its integer `snr_db` when present, and its `spotter_call`. At most one report per beacon and band MUST be kept. The spot MUST otherwise be handled as every other spot. | Test |
| `QGDXC-TUI-0103` | The beacon view MUST fill the spots box. Its top border MUST show `NCDXF/IARU beacons` and the whole seconds left in the current transmission, 1 to 10, and at its right `SNR dB · last 15 min · all sources`, saying what the view uses; it MUST NOT show the spot list's filter summary or hidden-source count, which do not apply to it. Then one row per frequency, lowest first, MUST show the frequency in MHz, its band, the call and location of the beacon on it now, the call of the next, and, when that beacon's report on that frequency arrived within the last 15 minutes, its SNR when kept, its age, its spotter and its source's short label as in the Src column of `QGDXC-TUI-0011`. Then a table MUST show each beacon in order, with its call, location, and one cell per frequency: the report's SNR in dB, or `+` without one, when a report arrived within the last 15 minutes, in the band's role; `·` in the dim role otherwise. Rows that do not fit MAY be clipped at the box's bottom. Its bottom border MUST show `B list` and credit the schedule to the NCDXF/IARU International Beacon Project. | Test |
| `QGDXC-TUI-0104` | While the map is shown, it MUST also mark each beacon transmitting now with `◆` in its band's role at the place of its call under `QGDXC-TUI-0083`, omitting a beacon whose place is unknown, and the map's bottom-border legend MUST include `beacon ◆`. | Test |

## F.12 Space weather

The space-weather view shows the NOAA SWPC products of Annex H in the spots box, like the
map and the beacon view. It is the interface's only use of the web, and it makes no
request until the operator asks for it.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0110` | Outside an editor or dialog, `W` (capital) MUST switch the spots box between the space-weather view and the spot list. Showing it MUST leave the map and the beacon view, and `M` and `B` MUST leave it. The header, panels, filters, retention, pause and every other key MUST be unchanged. The footer MUST show `W weather` while the view is not shown and `W list` while it is, at widths of at least 130 columns, and `W` alone below that, within the fit of `QGDXC-TUI-0070`. The help screen MUST list `W: space weather`. | Test |
| `QGDXC-TUI-0111` | The interface MUST make no web request until the operator first shows the space-weather view or starts with `--space-weather`. From then until exit it MUST fetch each product of H.1, except the solar flux history, which `QGDXC-TUI-0123` governs, when it has none and again when its floor has passed since the last request for it: 15 minutes for solar flux, planetary K and NOAA scales, and 5 minutes for X-rays, solar wind speed, solar wind field and alerts. SWPC states no request limit; these floors are QSO Graph's recorded judgement. Requests MUST run off the drawing thread, one at a time, and a request MUST never delay spot reception or drawing. | Test |
| `QGDXC-TUI-0112` | Requests MUST go only to the fixed `https://services.swpc.noaa.gov` URLs of H.1, through `qso-graph-http` (Annex I) with the webpki root set and a client allowing only that host. Each request MUST time out after 10 seconds to connect and 30 seconds in all and read at most the product's byte limit plus one byte. The `User-Agent` MUST be `qg-dxc/<version>` and MUST carry no callsign, e-mail address or other identity. | Test, Inspection |
| `QGDXC-TUI-0113` | SWPC's policy under `QGDXC-HTTP-0006` MUST apply: a `429`, or a `503` carrying `Retry-After`, MUST stop every request to SWPC until the stated wait, clamped to between 60 seconds and 1 hour (60 seconds when it gives none or one that does not parse), has passed, and the view MUST show the wait. Any other failure, or a product the library refuses, MUST keep the last good values, show the failure briefly in the view, and retry only after the floor. | Test |
| `QGDXC-TUI-0114` | The view MUST show, each with the time NOAA gives for it: SFI; Kp and A now with a sparkline of the last 24 hours of Kp; the R, S and G scales; the X-ray class now (`QGDXC-SWX-0005`) with a sparkline of the 6-hour flux on a logarithmic scale; solar wind speed, Bt and Bz, with Bz in the warning role below 0 nT and the bad role at −10 nT or lower; and alerts. Kp MUST use the Kp roles of `QGDXC-TUI-0122`. Values not yet fetched MUST read `waiting`. A product whose last success is older than three times its floor MUST be shown in the dim role and marked `stale`. Alert text MUST be escaped under `QGDXC-SEC-0005` before display. | Test |
| `QGDXC-TUI-0117` | Times of figures and alerts in the view MUST read `HH:MM UTC` when they fall on the current UTC date and `MM-DD HH:MM UTC` otherwise, without seconds. Chart axes follow `QGDXC-TUI-0121`. | Test |
| `QGDXC-TUI-0118` | The alert list is for current conditions, not history; trends are the charts of F.13. It MUST show only alerts issued within the last 5 days of the interface's UTC clock, and only the newest of each `product_id` among them, with `×<n>` after the ID when that ID was issued `<n>` times in those 5 days (`n` of 2 or more), newest first, clipped to the box. With none, it MUST read `no alerts in the last 5 days`. The library's output under `QGDXC-SWX-0006` is unchanged; this is display only. | Test |
| `QGDXC-TUI-0119` | At widths of at least 120 columns the view MUST use two columns: the figures and their sparklines on the left, and the alerts on the right, each column with its own heading; below 120 columns the alerts MUST follow the figures. Sparklines MUST widen to use the column's free width, up to one cell per data point. Rows that do not fit the box's height MAY be clipped at its bottom, figures before alerts, under `QGDXC-TUI-0013`. | Test |
| `QGDXC-TUI-0127` | Once both the solar flux and the planetary K product have been fetched, the view MUST show a **Band outlook** section in the figures column, after the figures and before the charts of F.13: a heading `Band outlook · SFI <n> · Kp <k> · rule of thumb` with the SFI used and the newest Kp to two decimals, then one row per band of `QGDXC-SWX-0012` with the band, its status and its comment. The status MUST use the good role for Excellent and Good, the warning role for Fair, the bad role for Poor and the dim role for Sporadic. Before both products have been fetched the section MUST read `waiting`; when either is stale under `QGDXC-TUI-0114` the whole section MUST be in the dim role with `stale` in its heading. Comments that do not fit MAY be clipped under `QGDXC-TUI-0013`. The outlook MUST cause no request of its own. | Test |
| `QGDXC-TUI-0128` | Tests MUST check the band outlook section with recorded products at 100, 120 and 150 columns: its rows and roles for a quiet and a storm set, `waiting` before either product, and the dim, `stale` form. | Test |
| `QGDXC-TUI-0115` | `deny.toml` MUST allow `cmake` and `pkg-config` only as dependencies of `aws-lc-sys`, and `cc` only as a dependency of `aws-lc-sys` or of that `cmake` (the chain AWS-LC's build uses, as in QG Trust). The TLS stack's exact pins live in `qso-graph-http` under `QGDXC-HTTP-0013`, which also keeps it out of the parsing libraries. | Test |
| `QGDXC-TUI-0116` | The fetching MUST sit behind an interface that tests replace with recorded product bytes and scripted failures, so that no test makes a network request. | Test, Inspection |

### F.12.1 Ribbon

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0125` | While space-weather fetching is running under `QGDXC-TUI-0111` and the library returns at least one item in force at the interface's UTC clock (`QGDXC-SWX-0011`), the interface MUST show a ribbon on every page: `▌<KIND> <headline text after its label>`, then ` · until HH:MM UTC` when the item has an end (`MM-DD HH:MM UTC` when not today), then `  +<n>` when `<n>` more items are in force. `<KIND>` is `ALERT`, `WARNING` or `WATCH` (`CONTINUED ALERT` and `EXTENDED WARNING` read `ALERT` and `WARNING`). The ribbon MUST use the bad role for alerts, the warning role for warnings and the accent role for watches, and its text MUST be redacted under `QGDXC-SEC-0005` and escaped under `QGDXC-TUI-0013`. With nothing in force, or before fetching has started, no ribbon MUST be shown, and the ribbon MUST NOT cause any request. | Test |
| `QGDXC-TUI-0126` | At widths of at least 100 columns the ribbon MUST occupy the bands box's first row below the band counts when the box has such a row, and otherwise the bands box's top border after its title. Below 100 columns it MUST show only `▌<KIND>` and `+<n>` on the spots box's top border after its left label,  It MAY be clipped to the space it occupies under `QGDXC-TUI-0013`. | Test |

## F.13 Space-weather history

The history charts show trends that current figures cannot: whether the sun is heading toward or away from
good conditions, when the last storm was, and when the last flare was. It is drawn with the terminal
interface's chart widgets and is a display only.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0120` | The history charts of this section MUST be part of the space-weather view, below its figures and alerts, not a separate page. | Test |
| `QGDXC-TUI-0121` | The figures and alerts of `QGDXC-TUI-0119` MUST take only the rows their content needs, and the four charts MUST fill the rest of the box: two-by-two at widths of at least 120 columns, otherwise stacked, each at least 4 rows high, as many as fit in the order of `QGDXC-TUI-0122`. Each chart MUST have a title naming the quantity, its unit and its span, and axis labels in UTC (`MM-DD` for spans of days, `HH:MM` for spans of hours). | Test |
| `QGDXC-TUI-0122` | The four charts MUST be: **solar flux, 30 days**: a line of the daily Noon flux under `QGDXC-SWX-0008` and a second, dim line of its 90-day mean; **Kp, 7 days**: one bar per 3-hour Kp value from the planetary K product, in NOAA's planetary K colours ([SWPC planetary K-index](https://www.spaceweather.gov/products/planetary-k-index)): the Kp quiet role below 5, and the G1, G2, G3, G4 and G5 roles at 5, 6, 7, 8 and 9, where a value counts at its nearest whole level except that only 9.00 (9o) is G5 and 8.67 (9−) is G4; in the phosphor, amber and mono themes, which have one hue, G3 to G5 MUST also be bold; against a vertical scale from 0 to 9 labeled at least at 0, 5 (where G1 storms begin) and 9; **X-rays, 6 hours**: a line of `log10` of the 0.1–0.8 nm flux, with the vertical axis labeled `A`, `B`, `C`, `M` and `X` at 1e-8 to 1e-4 W/m², or, when the chart is too short for five labels, with that class key in its title; **solar wind, since start**: lines of speed (km/s) and Bz (nT) from each successful fetch of the two summary products during this run, keeping at most 288 points each (24 hours at the 5-minute floor), oldest dropped first. | Test |
| `QGDXC-TUI-0123` | The solar flux history MUST NOT be requested until the space-weather view is first shown with room for its chart, and then at most once an hour, under the same rules as `QGDXC-TUI-0111`–`0113`. Showing the chart MUST NOT change the other products' floors. | Test |
| `QGDXC-TUI-0124` | A chart without data MUST show `waiting` inside its border, and a chart whose product is stale under `QGDXC-TUI-0114` MUST be drawn in the dim role with `stale` in its title. | Test |

## F.14 POTA spots

Parks On the Air activators are spotted on their own site, not on the clusters. A POTA source puts
those spots in the same list, with the same filters, collapse, map and highlighting as cluster and RBN
spots. It is the interface's second use of the web, and it makes no request unless the operator adds
the source.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0130` | A source written `pota` (in `--node` or as a sources-file `node`, with no port or profile) MUST add a POTA source labeled `pota`, with server type `pota`, counted among the at most eight sources and toggled with its key like any other. No request MUST be made unless such a source is configured. `--call` is required only when a cluster source is configured. | Test |
| `QGDXC-TUI-0131` | The POTA source MUST request only `https://api.pota.app/spot/activator`, through `qso-graph-http` with a client allowing only `api.pota.app`, the webpki roots, a byte limit of 1 MiB, and `User-Agent` `qg-dxc/<version>`. Its policy under Annex I MUST be: a floor of 60 seconds; waits apply to the whole provider; `Retry-After` clamped to between 60 seconds and 1 hour, and 60 seconds for a `429` whose `Retry-After` is absent or unreadable (the policy's choice under `QGDXC-HTTP-0002`); other failures back off from 1 minute, doubling to 15 minutes. POTA states no request limit; these numbers are QSO Graph's recorded judgement, as pota-mcp's (live spots are worthless stale; the floor keeps one program to one request a minute). | Test, Inspection |
| `QGDXC-TUI-0132` | Each POTA spot whose `spotId` was not seen before in this run MUST enter the list as a `spot` event with `call` the activator, `spotter_call` the spotter, `frequency_hz`, `mode` when present, `comment` the comments, `grid` the four-character locator when present, `time_of_day` from `spotTime`, `spot_kind` under `QGDXC-POTA-0004`, and `pota_reference` the park reference. A `spotId` already seen MUST NOT add a row. At most 10,000 seen IDs MUST be kept, oldest dropped first. The Comment column MUST begin with the park reference for POTA spots. Spots then follow F.2 to F.9 as any other spot, including `QGDXC-TUI-0014` privacy before retention and the collapse of `QGDXC-TUI-0015`. | Test |
| `QGDXC-TUI-0133` | The POTA source's status MUST show `connecting` before its first answer, `ready` after a usable answer, the wait while resting under `Retry-After` or backoff, and a short failure reason otherwise; entries skipped under `QGDXC-POTA-0003` MUST be counted in its detail line, which then appears under `QGDXC-TUI-0070` even while the source is ready. A failure MUST keep the spots already listed and MUST never delay other sources or drawing. | Test |
| `QGDXC-TUI-0134` | POTA spots MUST NOT be written to a recording under `--record`, which records cluster wire bytes only, and a POTA source MUST be refused with `--replay`. | Test |
| `QGDXC-TUI-0135` | The POTA fetching MUST sit behind the same fake-fetch interface as F.12, so no test makes a network request. | Test, Inspection |

## F.15 Sun, moon and grey line

The operator's own location makes the interface say when the sun rises and sets there and where the
moon is, and the map shows where the grey line is now. Everything is calculated on the computer by
`qso-graph-astro` (Annex L) from a locator the operator gives; nothing is fetched, and the location
never leaves the computer.

```
qg-dxc tui … [--grid <locator>]
```

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0140` | `--grid <locator>` MUST be accepted at most once, with `--node`, `--replay` or `--sources` or with none of them, and MUST be a 4, 6 or 8 character locator under `QGDXC-GRID-0002`; a 2-character field is refused as too coarse for a sunrise. A sources file MAY carry the same as a top-level `grid` member (`QGDXC-TUI-0007`); `--grid` MUST override it. An invalid locator MUST be rejected with a configuration error naming `--grid` or `grid` before the interface changes terminal state or opens any source. Without either, nothing in the interface is location-aware. | Test |
| `QGDXC-TUI-0141` | The locator and the position taken from it MUST be used only to draw this section's displays. They MUST NOT be sent to any source or provider, written to a recording, or included in any status text, error or log; a test MUST show that no byte the interface sends to a cluster, replay or HTTPS fake contains the locator in either case or its centre's coordinates written as decimal degrees. | Test, Inspection |
| `QGDXC-TUI-0142` | With a locator, at widths of at least 100 columns, the sources box's bottom border MUST show `Sun ↑HH:MM ↓HH:MM · Moon <p>% <e>° UTC` for the locator's centre: `↑` the next sunrise and `↓` the next sunset under `QGDXC-AST-0012` at −0.833° over the 1,440 minutes from the interface's current UTC minute; `<p>` the moon's illuminated fraction as a whole percentage; `<e>` its topocentric elevation rounded to a whole degree with a sign (`+12`, `-3`, `0`). A missing rise or set MUST read `—` after its arrow; with neither, the sun part MUST read `Sun up all day` or `Sun down all day`. When the text does not fit inside the border, ` · Moon …` MUST be dropped and the UTC kept, and if it still does not fit nothing MUST be shown. The labels MUST use the heading role and the figures the text role. Below 100 columns, and without a locator, nothing MUST be shown there. | Test |
| `QGDXC-TUI-0143` | The map MUST show the grey line at the interface's UTC clock, with or without a locator: the terminator at −0.833° in the warning role and at −10.1° in the dim role, each under `QGDXC-AST-0030` with a step of 2°; the subsolar point as `*` in the warning role; the sublunar point as `o` in the text role; and, with a locator, the operator's place as `+` in the accent role at its centre. These MUST be drawn over the coastline and beneath paths and beacon marks, MUST NOT count as paths, and MUST NOT change the bound of `QGDXC-TUI-0086`. The legend of `QGDXC-TUI-0081` MUST add `sun * · moon o`, and `you +` with a locator, and MAY be clipped under `QGDXC-TUI-0013`. | Test |
| `QGDXC-TUI-0144` | The sun, moon and terminator figures MUST be computed at most once per UTC minute and reused until the minute changes, so drawing work is bounded whatever the frame rate. | Test |
| `QGDXC-TUI-0145` | The DX end of `QGDXC-TUI-0082` MUST take a locator's centre from `qso-graph-grid`, so the map and this section share one implementation; the four-character rule of `QGDXC-EVT-0009` is unchanged. | Test, Inspection |
| `QGDXC-TUI-0146` | Tests MUST fix the UTC clock and check the border text and map marks for a place with both events, a polar day, a polar night, and a missing moon event, at 100, 120 and 150 columns, and the fit rule of `QGDXC-TUI-0142`. | Test |
