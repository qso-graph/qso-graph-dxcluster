<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Changelog

## [0.5.0]

### Features

- lib: carry the NCDXF/IARU beacon schedule in the library
- tui: NCDXF/IARU beacon view with B
- tui: show the latest WWV/WCY solar figures

### Bug Fixes

- tui: beacon view header says what the view uses

### Documentation

- readme: install with brew, apt or dnf, and drop the old status line

## [0.4.0]

### Features

- tui: hide self-spots with s, and drop a repeated mode word
- tui: put All at the top of the band and mode dialogs
- tui: show a spot relayed by several nodes once

### Bug Fixes

- tui: say how to start when there is no sources file

## [0.3.0]

### Features

- tui: world map of spot paths, toggled with M

### Documentation

- spec: spot map, country file and header version

## [0.2.0]

### Features

- tui: recognize ADIF 3.1.8 modes and filter JTTY
- tui: choose nodes from your own sources file at startup

### Bug Fixes

- tui: highlight only new human spots, not skimmer spots

### Documentation

- spec: choose saved sources when the TUI starts

## [0.1.2]

- `qg-dxc --version` reports the version built; 0.1.1 still said 0.1.0. Release
  builds now fail if the two differ.

## [0.1.1]

- Name the Linux packages `qso-graph-dxcluster`, like the other QSO Graph
  packages; they still install the `qg-dxc` command.

## [0.1.0]

- Add a runtime Limits editor (`l`) for both TUI spot caps, with immediate
  per-kind eviction when lowered, including while paused (`QGDXC-TUI-0012`).

- Add independent TUI spot retention with `--keep-human` and `--keep-skimmer`
  (100 each by default, 10–10,000 each), per-kind eviction, and retained counts
  in the bottom border (`QGDXC-TUI-0012`, `QGDXC-TUI-0070`).

- Scrolling down the TUI spot list holds the view while spots keep arriving;
  Home returns to the newest spots (`QGDXC-TUI-0051`).

- Add a terminal spot list that merges several cluster nodes and Reverse Beacon
  Network feeds, with display filters and offline replay.
- Add signed platform archives, Linux packages, and a manual release workflow
  that publishes the library and CLI before tagging the release.

- Request readiness tokens and retry jitter per attempt, using fresh OS randomness
  in the CLI and constant engine storage (QGDXC-DSP-0004, QGDXC-SES-0010).

- Add the dependency-free receive-only library, bounded framing, passive Telnet
  stripping, all four server profiles, and automatic profile detection.
- Add caller-driven session timing, paced startup, retry budgets, and privacy
  transformations with explicit evidence status.
- Add `qg-dxc connect` and offline `replay`, Annex A JSON output, terminal escaping,
  optional exact recording, and Ctrl-C shutdown.
- Add independently specified sanitized vectors, requirement coverage checks,
  source policy, offline verification, and a three-platform CI matrix.
- Clarify AR-Cluster banner extension context, terminal notices during EOF
  finalization, synthetic privacy-test addresses, vector license metadata, and local configuration-rejection vectors.
