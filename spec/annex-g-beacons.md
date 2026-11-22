<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex G: NCDXF/IARU beacons

The NCDXF/IARU International Beacon Project runs 18 beacons around the world. Each
transmits for 10 seconds on 14.100, 18.110, 21.150, 24.930 and 28.200 MHz in turn,
and the 18 take turns, so every band carries one beacon at a time and the whole
rotation repeats every three minutes. Listening for them, or reading the Reverse
Beacon Network's reports of them, is a quick check of which paths are open.

The schedule is the Northern California DX Foundation's, published at
<https://www.ncdxf.org/beacon/> (retrieved 2026-11-22); the beacons are built and run
by NCDXF, the IARU and their volunteer hosts. NCDXF states no restriction on its use
and lists the programs that use it.

The library carries the schedule so that every client answers "who is on now" and
"which beacon did that report hear" the same way. Like the rest of the library it
reads no clock and fetches nothing (`QGDXC-IO-0001`): the caller supplies UTC time,
and a beacon that is off the air is still scheduled.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-BCN-0001` | The library MUST carry the 18 beacons in NCDXF's transmit order, each with its call and location, and the five frequencies above with their bands, in one source file (`src/beacons.rs`). On the band with index `b` (0 for 14.100 MHz through 4 for 28.200 MHz), at `t` whole UTC seconds since the Unix epoch supplied by the caller, the beacon transmitting MUST be number `(⌊(t mod 180) / 10⌋ − b) mod 18`, counting the first in the order as 0, and the next MUST be the one after it in the order. The whole seconds left in the current transmission MUST be `10 − (t mod 10)`. | Test |
| `QGDXC-BCN-0002` | An event MUST be identified as a report of beacon `n` on band `b` exactly when it is a `spot` whose `spot_type` is `NCDXF B`, whose `call` equals beacon `n`'s call ignoring ASCII case, and whose integer `frequency_hz` is within 1,000 Hz of band `b`'s frequency. Any other event MUST be identified as no beacon report. | Test |
