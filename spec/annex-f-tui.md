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
qg-dxc tui [--sources <file>] [--keep-human N] [--keep-skimmer N] [--theme <name>]
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
| `QGDXC-TUI-0006` | Without `--node` or `--replay`, `qg-dxc tui` MUST read a sources file: the `--sources <file>` path when given, otherwise the default path. The default path MUST be `qso-graph/dxcluster-sources.json` under `%APPDATA%` on Windows; elsewhere under `$XDG_CONFIG_HOME` when it is set to an absolute path, otherwise under `$HOME/.config`. If the needed variable is unset, empty, or not absolute, the interface MUST exit with a configuration error naming it. `--sources` MUST NOT be combined with `--node`, `--replay`, or `--call`, and MUST appear at most once. The path MUST name a regular file, checked before it is opened, and the interface MUST read at most 65,536 bytes; a missing file, another file type, or a longer file MUST be rejected with a configuration error naming the path. Every rejection under F.1.1 MUST happen before the interface changes terminal state or opens any source. | Test |
| `QGDXC-TUI-0007` | The file MUST be UTF-8 JSON (RFC 8259) whose top level is an object with the members `version` and `sources`, MAY have `call`, and no other member. `version` MUST be the integer 1. `sources` MUST be an array of 1–64 objects. Each source object MUST have `name` and `node`, and MAY have `profile`, `selected`, and `call`, and no other member. Every `call`, at either level, MUST satisfy `QGDXC-IO-0003`; a source logs in with its own `call` when present, otherwise the top-level `call`, and a source with neither MUST be rejected. `name` MUST be 1–32 bytes of ASCII U+0020–U+007E, not beginning or ending with a space, and unique within the file under ASCII case-insensitive comparison. `node` MUST be a string in the `--node` endpoint form of `QGDXC-TUI-0001`, without a `/<profile>` suffix; endpoints MUST be unique under that requirement's comparison. `profile` MUST be one of the `QGDXC-TUI-0001` profile names, defaulting to `auto`. `selected` MUST be a boolean, defaulting to false. Invalid JSON or a duplicate member name in any object MUST reject the whole file with a configuration error giving the line and column. A missing or unexpected member, a wrong JSON type, or an invalid value MUST reject the whole file with a configuration error naming the JSON path of the first offending value (for example `sources[2].node`). Errors MUST be sanitized under `QGDXC-TUI-0013`. No part of a rejected file is used. | Test |
| `QGDXC-TUI-0008` | After a valid file is read, the interface MUST show a Sources screen listing every saved source in file order with its name, endpoint, and profile, with the sources marked `selected` already chosen. Up and Down MUST move, Space MUST toggle the source under the cursor, Enter MUST start the chosen sources, and `q`, Escape, or Ctrl-C MUST exit without opening any source, restoring the terminal under `QGDXC-TUI-0052`. Enter with no source chosen or more than eight chosen MUST keep the screen open with `Choose 1 to 8 sources` and open nothing. The chosen sources MUST then run exactly as the equivalent `--call` and `--node` arguments in file order would, under `QGDXC-TUI-0001` through `QGDXC-TUI-0004`, except that each label's base MUST be the source's `name` rather than its host. The screen MUST follow `QGDXC-TUI-0013`, `QGDXC-TUI-0054`, and the themes of F.8. | Test |
| `QGDXC-TUI-0009` | The interface MUST NOT write, create, or rename the sources file or its directory. The file reader MUST be covered by a fuzz target that accepts arbitrary bytes and either returns a validated source list or a configuration error, never panicking or exceeding the bounds of `QGDXC-TUI-0006` and `QGDXC-TUI-0007`. | Test |

## F.2 Merged spot list

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0010` | Every `spot` event from every source MUST enter one list in arrival order at the application, newest at the top. Spots MUST NOT be merged, deduplicated, or reordered by reported spot time; the same DX station spotted by two sources appears twice, each with its own source label. | Test |
| `QGDXC-TUI-0011` | Each spot row MUST allocate separate, nonoverlapping columns for a kind marker, reported UTC time `HHMM`, frequency, DX call, spotter, band under F.3, display mode under F.4, and comment. Wide rows under `QGDXC-TUI-0070` MUST also allocate SNR, WPM, and the source's short label (the portion before its first period), in a column sized to the longest short label between 3 and 8 characters; the complete source label MUST be available in the sources panel or help screen. A human spot MUST carry the interface-owned `▎` marker; a skimmer marker MUST be blank. Fields MUST be sanitized under `QGDXC-TUI-0013` before measurement and clipped to their allocated columns; a truncated non-comment field MUST end with `~`. Every non-comment column MUST have positive width. The comment receives the remaining width, which MAY be zero. Frequency MUST use decimal kHz with exactly one fractional digit, rounded to the nearest 100 Hz with ties upward using integer arithmetic: let `n = (frequency_hz + 50) / 100`, using integer division, and display `n / 10`, `.`, and `n % 10`. Derived band and mode MUST use the original event values, not clipped or rounded display text. A missing optional field MUST show as blank, never as a guessed value; mode `unknown` MUST display as a blank Mode column while still filtering as `unknown`. | Test |
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
| `QGDXC-TUI-0040` | The interface MUST provide four filters, all off at start, combined with AND: **band** (any subset of the bands of F.3, including `other`); **mode** (any subset of `CW`, `SSB`, `AM`, `FM`, `FT8`, `FT4`, `RTTY`, `JTTY`, `other`, `unknown`, where `other` is every recognized F.4 mode not listed); **callsign** (F.5.1); **kind** (`all`, `human`, or `skimmer`, from `spot_kind`). An inactive filter MUST accept every spot. An active band or mode filter MUST accept exactly the selected values; an empty selected set MUST accept no spots. | Test |
| `QGDXC-TUI-0041` | **Callsign** MUST accept up to 16 comma-separated patterns, each 1–20 bytes of ASCII letters, digits, `/`, `-`, `*` (any run, including empty), and `?` (one character). Matching MUST be ASCII case-insensitive against the whole DX call. Matching MUST run in time linear in pattern length times call length; regular expressions MUST NOT be accepted. A callsign filter MUST accept a spot when at least one pattern matches its whole DX call. An empty entry MUST disable the callsign filter. Nonempty entries MUST contain 1–16 nonempty patterns; empty components, excess patterns, and invalid characters or lengths MUST reject the entire entry with a message and leave the previous filter in force. The callsign editor MUST retain at most 335 input bytes; input exceeding that bound MUST reject the edit without applying a truncated prefix, leaving the previous filter in force. The interface MUST NOT enable bracketed paste, so pasted text arrives as individual key events and no input decoder buffers an unbounded paste. | Test |
| `QGDXC-TUI-0042` | Each source's spots MUST be hideable by toggling that source, independent of the four filters. A hidden source keeps its session and keeps counting spots. | Test |
| `QGDXC-TUI-0043` | Whenever the normal interface is visible, one bounded line MUST identify each active filter. Band, mode, and callsign filters MAY be summarized by selected-value or pattern counts; their complete values MUST be available in the corresponding editor or dialog. The line MUST be clipped to the terminal width. The clear key defined by `QGDXC-TUI-0051` MUST disable all four filters and unhide every source. | Test |

### F.5.1 Why wildcards and not regular expressions

A callsign watch list needs prefix and suffix matching (`Q0*`, `*/P`, `Q9?X`).
Wildcards cover that with matching that cannot run away on a hostile call token;
a user-typed regular expression can.

## F.6 Status and keys

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-TUI-0050` | Each source MUST have status details with: label; connection state and `reason` from its latest `connection_state` event; server type; spots received; and seconds since that source's last spot. Both layouts MUST make the full details available on the help screen; the dashboard MUST show source summaries and exceptional status details under `QGDXC-TUI-0070`. A source whose state is `disconnected` or `blocked` with retries exhausted MUST be shown so on its line and MUST NOT be removed. Detail text MAY be clipped to the available width under `QGDXC-TUI-0013`. Before the first connection event, state and reason MUST be blank and server type MUST be `unknown`. Spot counts MUST start at zero and include spots hidden by filters or source visibility. Before the first spot, the age field MUST be omitted; afterward it MUST be the whole seconds elapsed on the interface's monotonic clock since that source's most recent spot entered the merged list, regardless of its reported UTC time. | Test |
| `QGDXC-TUI-0051` | Ctrl-C MUST quit from every input mode. The following shortcuts apply only outside an editor or dialog: `q` quit; `b` band filter; `m` mode filter; `/` callsign filter; `k` cycle kind `all` → `human` → `skimmer`; `1`–`8` toggle that source; `c` clear filters; `p` pause and resume list scrolling; Up, Down, PageUp, PageDown, Home scroll, where scrolling away from the newest matching spot MUST pause as `p` does and Home MUST resume and show the newest spot; `t` cycle themes under `QGDXC-TUI-0071`; `?` help. In the callsign editor, printable characters MUST be treated as entry data rather than global shortcuts; Backspace deletes the preceding character, Enter validates and commits the whole entry, and Escape cancels without changing the filter. Band and mode dialogs MUST use Up and Down to move, Space to toggle a selection, Enter to commit, and Escape to cancel. While paused, new spots MUST still enter retention and counts under `QGDXC-TUI-0012`, without shifting the viewport's top spot solely because newer spots arrive; manual scrolling MUST remain available. If the top spot is evicted, the viewport MUST clamp to the oldest matching retained spot. A filter or visibility change MUST reset the viewport to the newest matching retained spot. Pausing MUST NOT retain a separate spot snapshot. The status line MUST show `PAUSED` and the count of all spots received since the most recent transition into pause, starting at zero. Resuming MUST return to the newest matching retained spot. | Test |
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
| `QGDXC-TUI-0070` | At widths of at least 100 columns, the interface MUST show an inverse ` QG DX Cluster ` logo, live-source count (`1 source live`, otherwise `<n> sources live`), retained-spot count, spots per minute, a 12-cell minute sparkline, and a right-aligned UTC date and time. Below it, two side-by-side rounded boxes MUST show sources and band activity. Each source row MUST show a key superscript `¹`–`⁸`, state dot (`●` ready, `◐` connecting or retrying, `○` hidden or inactive), label, server profile, state, received count, and an eight-cell minute sparkline. Additional detail rows MUST appear only for sources that are not ready, including connecting, retrying, failed, replay-finished, or error states. Each MUST use its key superscript followed by one plain-English status line; ready sources MUST have no detail row. Replay completion MUST read `replay finished`. Full status details remain available under `QGDXC-TUI-0050`. A known retry deadline MUST show a countdown in seconds rounded upward, never negative. The bands box MUST show three rows of vertical block bars, a label row, and a count row, for 160, 80, 60, 40, 30, 20, 17, 15, 12, 10, and 6 meters in that order. Below the panels, a rounded spots box MUST show `newest first`, active filters including hidden-source count, and the columns of `QGDXC-TUI-0011`. While paused it MUST show an inverse `PAUSED` chip and the new-arrival count, with filter information still visible. Its bottom border MUST show total received and `<h> human · <s> skimmer`, where `<h>` and `<s>` are the retained counts under `QGDXC-TUI-0012`, in both layouts. Counts MUST include spots hidden by filters or source visibility. The footer MUST show key hints with emphasized keys and the theme name at the right. At widths below 100 columns, the panels MUST fold into one bounded sources line, preserving each full short label (the portion before its first period) when its allocated share permits, and spot rows MUST omit SNR, WPM, and source columns. The minimum dimensions of `QGDXC-TUI-0054` take precedence. | Test |
| `QGDXC-TUI-0071` | `--theme` MUST accept exactly `color`, `daylight`, `phosphor`, `amber`, or `mono`, rejecting invalid or repeated values before opening sources. With no explicit theme, a set, nonempty `NO_COLOR` MUST select `mono`; otherwise the initial theme MUST be `color`. An explicit theme MUST override `NO_COLOR`. Outside editors and dialogs, `t` MUST cycle in the order listed, wrapping to `color`. Color roles MUST use the F.8.1 palette, subject to `QGDXC-TUI-0072`. Human DX calls MUST be bold and bright; skimmer calls and comments MUST use quieter roles. Band and mode fields MUST use their palette roles; unlisted bands or modes MUST use the text role. SNR MUST use the good role at 20 dB or higher, text at 8–19 dB, and dim below 8 dB. | Test |
| `QGDXC-TUI-0072` | The renderer MUST leave the terminal background unchanged except for the logo, chips, and new-spot highlights. `mono` MUST use terminal-default foreground and background without RGB, indexed, or ANSI palette colors; bold, underline, and inverse attributes MAY preserve emphasis. If `COLORTERM`, compared ASCII case-insensitively, is `truecolor` or `24bit`, color themes MUST emit RGB colors. Otherwise a `TERM` containing `256color` MUST select the nearest xterm-256 palette entry by squared RGB distance, choosing the lowest index on ties. Other terminals MUST use the nearest standard ANSI 16-color entry by the same rule, with black dim, border, spotter, time, and heading roles promoted to bright black for readability. On 16-color terminals, new-spot emphasis MUST use bold instead of a background color. | Test |
| `QGDXC-TUI-0073` | Activity statistics MUST count arrivals across all sources independently of filters, visibility, pause, and retained-spot eviction. A bounded history of at most 900 one-second counter buckets MUST supply rolling 60-second rates and 15-minute band counts; minute sparklines MUST include the current partial minute, with missing minutes zero-filled and their maximum scaled to the highest block. Zero-valued sparkline cells MUST display `▁` in the dim role, preserving the full extent defined by `QGDXC-TUI-0070`. Band bars MUST scale against the largest displayed count. New spots MUST carry their monotonic arrival time. New human spots MUST receive a highlight for approximately three seconds, fading in three one-second steps from the new-highlight role toward the palette background before returning to the terminal background. Skimmer spots, and spots counted as skimmer under `QGDXC-TUI-0012`, MUST NOT be highlighted, so a busy skimmer feed does not keep the list flashing. The 16-color exception of `QGDXC-TUI-0072` applies. The wide SNR column MUST use only integer `snr_db`; WPM MUST use only integer `speed` with `speed_unit` equal to `wpm`. The interface MUST NOT parse additional metrics from comments or guess missing values. As a display-only derivation, the Comment column MUST omit a leading mode/SNR/WPM prefix only when it exactly matches the typed fields fully shown in that layout, preserving the remaining comment text. If an exact derivation is unavailable, it MUST show the original comment. This derivation MUST NOT change retained events or recordings. | Test |

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
