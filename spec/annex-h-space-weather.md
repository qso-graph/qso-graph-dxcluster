<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex H: space weather

The library crate `qso-graph-spaceweather` reads the space-weather products that the
NOAA Space Weather Prediction Center publishes at <https://services.swpc.noaa.gov>
(formats as retrieved 2026-11-22). It turns each product's bytes into typed values for
any QSO Graph program, the terminal interface and QSO Graph Desktop alike. Like the
cluster library it performs no I/O (`QGDXC-IO-0001` applies to it in full): the caller
fetches the bytes and supplies them. NOAA SWPC data are a work of the United States
government; the test samples are its published products, unchanged except that the alerts
sample's product IDs (shaped like callsigns, such as a letter, two digits and a letter) are
replaced by invented `QXnn` IDs, as every capture in this repository is sanitized.

## H.1 Products

| Product | Path under the base | Byte limit | Kept |
|---|---|---|---|
| Solar flux | `/products/summary/10cm-flux.json` | 4 KiB | `flux` (SFI, 0–999), `time_tag` |
| Planetary K | `/products/noaa-planetary-k-index.json` | 64 KiB | each entry's `time_tag`, `Kp` (0–9), `a_running` (0–400); at most 200 entries |
| NOAA scales | `/products/noaa-scales.json` | 16 KiB | entry `"0"`: `DateStamp` and `TimeStamp`, and the `Scale` of `R`, `S` and `G` (`"0"`–`"5"`) |
| X-rays | `/json/goes/primary/xrays-6-hour.json` | 512 KiB | the entries whose `energy` is `0.1-0.8nm`: `time_tag`, `flux` (finite, 1e-10 to 1e-1 W/m²); at most 1,000 |
| Solar wind speed | `/products/summary/solar-wind-speed.json` | 4 KiB | `proton_speed` (0–3,000 km/s), `time_tag` |
| Solar wind field | `/products/summary/solar-wind-mag-field.json` | 4 KiB | `bt` (0–200 nT), `bz_gsm` (−200 to 200 nT), `time_tag` |
| Alerts | `/products/alerts.json` | 256 KiB | each entry's `product_id` (1–16 printable ASCII), `issue_datetime`, and the headline of `message`; at most 100 entries |

## H.2 Requirements

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-SWX-0001` | `qso-graph-spaceweather` MUST be `#![forbid(unsafe_code)]`, MUST perform no network, file, clock, environment or process I/O, and MUST depend on no crate but `serde_json` and `serde_core` at the exact versions the command-line crate pins, without derive macros. It MUST offer one function per product of H.1 taking the product's bytes and returning its kept values or an error. | Test, Inspection |
| `QGDXC-SWX-0002` | Each function MUST refuse, with an error naming the product and the reason and no partial result: more bytes than the product's limit; text that is not UTF-8; JSON that does not parse, nests deeper than 16 levels, or repeats a member name in one object; a required member that is missing or of the wrong JSON type; a value outside its range in H.1; more entries than its bound; and an empty product where a value is required. Members not named in H.1 MUST be ignored. | Test |
| `QGDXC-SWX-0003` | `time_tag`, `DateStamp`/`TimeStamp` and `issue_datetime` MUST be read as UTC: `YYYY-MM-DDTHH:MM:SS` with an optional `Z`, `YYYY-MM-DD` with `HH:MM:SS`, and `YYYY-MM-DD HH:MM:SS` with optional fractional seconds of at most 6 digits, respectively, with calendar-valid dates from 2000 to 2099 and times through `23:59:59`. A time in any other form MUST refuse the product. | Test |
| `QGDXC-SWX-0004` | Planetary K entries and X-ray entries MUST be returned in time order, oldest first, whatever order the product lists them in; two entries with the same time MUST keep the later one in the product. | Test |
| `QGDXC-SWX-0005` | The library MUST give an X-ray flux `f` its class: `X` for `f ≥ 1e-4`, `M` for `f ≥ 1e-5`, `C` for `f ≥ 1e-6`, `B` for `f ≥ 1e-7`, otherwise `A`, followed by `f` divided by the class's lower bound (`A` 1e-8), truncated, not rounded, to one decimal place, so `2.57e-6` is `C2.5` and `1.23e-3` is `X12.3`. | Test |
| `QGDXC-SWX-0006` | An alert's headline MUST be the first line of its `message` that begins, after optional spaces, with `ALERT:`, `WARNING:`, `WATCH:`, `SUMMARY:`, `CANCEL WARNING:`, `CANCEL WATCH:`, `EXTENDED WARNING:` or `CONTINUED ALERT:`, without its line ending; an alert with none MUST have no headline and MUST still be returned. Messages are at most 16 KiB each; a longer one MUST refuse the product. Alerts MUST be returned newest first by `issue_datetime`. The text is returned as received; callers escape it for display. | Test |
| `QGDXC-SWX-0007` | Every function MUST be covered by a fuzz target, and by vectors made from the products retrieved for this annex plus planted refusals for each rule of `QGDXC-SWX-0002`. | Test, Inspection |
