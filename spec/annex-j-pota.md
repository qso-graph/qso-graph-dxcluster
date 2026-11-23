<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex J: Parks On the Air spots (`qso-graph-pota`)

`qso-graph-pota` reads the activator spots that Parks On the Air publishes at
`https://api.pota.app/spot/activator` (format as retrieved 2026-11-23) into typed values. Like the
other libraries it performs no I/O (`QGDXC-IO-0001` applies): the caller fetches the bytes, for
`qg-dxc` through `qso-graph-http` (Annex I). It depends on no other QSO Graph domain library; turning a
POTA spot into a cluster `spot` event is the application's job (F.14).

POTA's list is a snapshot: the latest spot of each current activation, with a count of its spots. It
is a crowd feed, so one bad entry must not blank the rest. The test sample is a published POTA
response with every callsign replaced by an invented one, 6-character locators removed, coordinates
rounded to 0.1°, and two callsign-shaped strings neutralized, as every capture here is sanitized.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-POTA-0001` | `qso-graph-pota` MUST be `#![forbid(unsafe_code)]`, MUST perform no I/O, and MUST depend on no crate but `serde_json` and `serde_core` at the exact versions the command-line crate pins, without derive macros. | Test, Inspection |
| `QGDXC-POTA-0002` | A product MUST be refused as a whole only when it exceeds 1 MiB, is not UTF-8, does not parse as JSON, nests deeper than 16 levels, repeats a member name in one object, is not an array, or has more than 1,000 entries. | Test |
| `QGDXC-POTA-0003` | Each entry MUST yield a spot when it has: `spotId` a whole number from 1 to 2⁵³; `activator` and `spotter` of 1–32 bytes of ASCII letters, digits, `/`, `-` and `#`; `frequency` a decimal string in kHz (digits, at most one `.`, at most three decimals) converting to 1 Hz–300 GHz; `reference` matching `^[A-Z0-9]{1,4}-[0-9]{4,5}$`; and `spotTime` as `YYYY-MM-DDTHH:MM:SS` with optional fractional seconds of at most 6 digits and optional `Z`, a calendar-valid UTC time from 2000 to 2099. Optional members MUST be kept when valid and absent otherwise: `mode` (0–16 printable ASCII), `name` (the park's name, 0–200 bytes of text), `comments` (0–512 bytes), `source` (0–64 bytes), `locationDesc` (0–64 bytes), `grid4` (a four-character Maidenhead locator) and `count` (0 to 100,000). An entry lacking a required member, or with one invalid, MUST be skipped and counted, not refuse the product. Members not named here MUST be ignored; in particular `grid6`, `latitude` and `longitude` MUST NOT be kept, so a location finer than a four-character locator never enters a program through this crate. | Test |
| `QGDXC-POTA-0004` | A spot MUST be a skimmer spot when its `source` equals `RBN` ignoring ASCII case, and a human spot otherwise. | Test |
| `QGDXC-POTA-0005` | The parser MUST be covered by a fuzz target and by tests on the sample of this annex, plus a planted entry for each skip rule of `QGDXC-POTA-0003` and each refusal of `QGDXC-POTA-0002`. | Test, Inspection |
