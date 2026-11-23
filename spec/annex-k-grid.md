<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex K: Maidenhead locators (`qso-graph-grid`)

`qso-graph-grid` is QSO Graph's one implementation of the Maidenhead locator: checking one, writing
it the way operators write it, the area it covers, and the locator holding a position. It is the
value type the sun and moon library (Annex L) and `qg-dxc` (F.15) take a station's place from, and
it gives the same answers as QSO Graph Desktop's locator functions, so a station is in the same
place in both. Like the other libraries it performs no I/O (`QGDXC-IO-0001` applies).

A locator is pairs of characters, each pair a finer cell inside the last: the **field** (two letters
`A`–`R`, 20° of longitude by 10° of latitude), the **square** (two digits, 2° by 1°), the
**subsquare** (two letters `A`–`X`, 5′ by 2.5′) and the **extended square** (two digits, 30″ by
15″). Longitude comes first in each pair, counted east from 180° W; latitude second, counted north
from 90° S.

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-GRID-0001` | `qso-graph-grid` MUST be `#![forbid(unsafe_code)]`, MUST perform no I/O and MUST have no dependencies. | Test, Inspection |
| `QGDXC-GRID-0002` | A locator MUST be accepted only when it is 2, 4, 6 or 8 ASCII characters whose pairs are, in order, a field (`A`–`R`), a square (`0`–`9`), a subsquare (`A`–`X`) and an extended square (`0`–`9`), letters in either case. Anything else MUST be refused, not corrected: no trimming, no truncation and no padding, so a locator with one character too many is not a locator with it dropped. | Test |
| `QGDXC-GRID-0003` | A locator MUST be written with the field letters in capitals and the subsquare letters in small letters, as operators write them (`QN31pr`, `QN31pr55`), whatever case it was given in. Its length, 2, 4, 6 or 8, MUST be available as its precision. | Test |
| `QGDXC-GRID-0004` | A locator's **bounds** MUST be its south, west, north and east edges in degrees, from the cell sizes of this annex, and its **centre** the midpoint of those edges. | Test |
| `QGDXC-GRID-0005` | A **position** MUST be a finite latitude from −90° to 90° and a finite longitude from −180° to 180°, both inclusive; anything else MUST be refused. The locator of a chosen precision **holding** a position MUST be the cell whose bounds contain it, with the north pole and the 180° meridian belonging to the last row and column at every level. For every locator, the locator of its own precision holding its centre MUST be itself. | Test |
| `QGDXC-GRID-0006` | The crate MUST give QSO Graph Desktop's answers: the bounds of every locator, and the locator at each precision holding every position, in `tests/fixtures/desktop-agreement.json`, recorded from Desktop's `gridBounds` and `gridOf` (`packages/storage/station-contract.ts`, Desktop commit `67ca202`), with locators equal and edges within 10⁻⁹°. The parser MUST be covered by a fuzz target that accepts arbitrary bytes and either refuses them or returns a locator whose written form parses to the same locator. | Test |
