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
| Solar flux history | `/json/f107_cm_flux.json` | 64 KiB | the entries whose `reporting_schedule` is `Noon` and `frequency` is 2800: `time_tag`, `flux` (0–999), and `ninety_day_mean` (0–999) when not null; at most 400 entries in the product |

## H.2 Requirements

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-SWX-0001` | `qso-graph-spaceweather` MUST be `#![forbid(unsafe_code)]`, MUST perform no network, file, clock, environment or process I/O, and MUST depend on no crate but `serde_json` and `serde_core` at the exact versions the command-line crate pins, without derive macros. It MUST offer one function per product of H.1 taking the product's bytes and returning its kept values or an error. | Test, Inspection |
| `QGDXC-SWX-0002` | Each function MUST refuse, with an error naming the product and the reason and no partial result: more bytes than the product's limit; text that is not UTF-8; JSON that does not parse, nests deeper than 16 levels, or repeats a member name in one object; a required member that is missing or of the wrong JSON type; a value outside its range in H.1; more entries than its bound; and an empty product where a value is required. Members not named in H.1 MUST be ignored. | Test |
| `QGDXC-SWX-0003` | `time_tag`, `DateStamp`/`TimeStamp` and `issue_datetime` MUST be read as UTC: `YYYY-MM-DDTHH:MM:SS` with an optional `Z`, `YYYY-MM-DD` with `HH:MM:SS`, and `YYYY-MM-DD HH:MM:SS` with optional fractional seconds of at most 6 digits, respectively, with calendar-valid dates from 2000 to 2099 and times through `23:59:59`. A time in any other form MUST refuse the product. | Test |
| `QGDXC-SWX-0004` | Planetary K entries and X-ray entries MUST be returned in time order, oldest first, whatever order the product lists them in; two entries with the same time MUST keep the later one in the product. | Test |
| `QGDXC-SWX-0005` | The library MUST give an X-ray flux `f` its class: `X` for `f ≥ 1e-4`, `M` for `f ≥ 1e-5`, `C` for `f ≥ 1e-6`, `B` for `f ≥ 1e-7`, otherwise `A`, followed by `f` divided by the class's lower bound (`A` 1e-8), truncated, not rounded, to one decimal place, so `2.57e-6` is `C2.5` and `1.23e-3` is `X12.3`. | Test |
| `QGDXC-SWX-0006` | An alert's headline MUST be the first line of its `message` that begins, after optional spaces, with `ALERT:`, `WARNING:`, `WATCH:`, `SUMMARY:`, `CANCEL WARNING:`, `CANCEL WATCH:`, `EXTENDED WARNING:` or `CONTINUED ALERT:`, without its line ending; an alert with none MUST have no headline and MUST still be returned. Messages are at most 16 KiB each; a longer one MUST refuse the product. Alerts MUST be returned newest first by `issue_datetime`. The text is returned as received; callers escape it for display. | Test |
| `QGDXC-SWX-0008` | The solar flux history MUST return the kept Noon entries oldest first, one per date, keeping the later entry when a date repeats; other `reporting_schedule` values and other frequencies MUST be skipped, not refused. `QGDXC-SWX-0002`–`0004` and `0007` apply to it as to the other products. | Test |
| `QGDXC-SWX-0007` | Every function, including those of H.3, MUST be covered by a fuzz target, and by vectors made from the products retrieved for this annex plus planted refusals for each rule of `QGDXC-SWX-0002`. | Test, Inspection |

## H.3 Alerts in force

NOAA's alert texts say when each item applies. The library reads that, so a program can show
only what is in force now and only what matters on HF. These requirements add to
`QGDXC-SWX-0006`; the caller supplies the time, as the library reads no clock.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-SWX-0009` | For each alert the library MUST read, from lines of its `message`, times in the form `<label>: YYYY Mon DD HHMM UTC` (English month abbreviations) and serials as decimal whole numbers: `Serial Number`; `Issue Time`; for a warning, `Valid From` and `Valid To`, or `Now Valid Until` with `Extension to Serial Number`; for a cancellation, `Cancel Serial Number`; and for a watch, the days and levels of the line after `Highest Storm Level Predicted by Day:` (`Mon DD:  <level>` up to three times, each level optionally followed by a parenthesized description, the year taken from `Issue Time` and advanced when the month wraps from December to January). Its kind MUST be the headline's label (`ALERT`, `CONTINUED ALERT`, `WARNING`, `EXTENDED WARNING`, `WATCH`, `CANCEL WARNING`, `CANCEL WATCH`, `SUMMARY`). Any of these that is absent or malformed MUST be absent from the result without refusing the product. | Test |
| `QGDXC-SWX-0010` | An alert's family MUST be taken from its headline, ignoring ASCII case: `geomagnetic` when it contains `Geomagnetic`; `radio blackout` when it contains `X-ray` or `Radio Blackout`; `solar radiation` when it contains `Proton` or `Solar Radiation`; otherwise `other`. | Test |
| `QGDXC-SWX-0011` | Given alerts and a UTC time `t`, the library MUST return the items in force at `t`, most severe first (alert, then warning, then watch), then newest first by `Issue Time`. In force means: an `ALERT` or `CONTINUED ALERT` for 3 hours from its `Issue Time`; a `WARNING` from `Valid From` until `Valid To`, and an `EXTENDED WARNING` until `Now Valid Until`, which also ends the warning whose serial it extends; a `WATCH` from `Issue Time` until the end (23:59:59 UTC) of its last forecast day whose level is not `None`, and only the newest watch of a family counts. A `CANCEL WARNING` or `CANCEL WATCH` ends, from its `Issue Time`, the item whose serial it cancels. An extension takes effect from its own `Issue Time`. A warning's end is exclusive and a watch's last second is inclusive. A watch superseded by a newer one of its family stays superseded even after the newer one ends or is cancelled. Items of family `other`, `SUMMARY` items, and items without the times their kind needs MUST NOT be returned. | Test |
