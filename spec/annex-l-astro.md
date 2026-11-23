<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Annex L: sun, moon and grey line (`qso-graph-astro`)

`qso-graph-astro` says where the sun and the moon are for a place and a moment, when they rise and
set, and where the grey line is, all calculated on the computer with nothing fetched. It is a port of
QSO Graph Desktop's Astro extension (`extensions/astro/src/sun.ts`, `moon.ts` and `greyline.ts`), so
the two give the same answers to the minute, and it takes its places from `qso-graph-grid`
(Annex K). Like the other libraries it performs no I/O (`QGDXC-IO-0001` applies) and reads no clock:
every moment is given to it.

**Methods.** The sun follows NOAA's General Solar Position method as NOAA's Solar Calculator
spreadsheet writes it, itself from Jean Meeus, *Astronomical Algorithms*. The moon follows Meeus,
*Astronomical Algorithms*, 2nd edition (Willmann-Bell, 1998): chapter 47 for its position, from the
ELP-2000/82 series as Meeus truncates it; chapter 12 for sidereal time; chapter 48 for the
illuminated fraction; and chapter 15 for the altitude at which it rises and sets. The independent
reference for the tests is PyEphem 4.2, an ephemeris sharing no code with either program.

**Accuracy.** Rise, set and twilight times are given to the minute and that is the accuracy claimed:
refraction and the local horizon move the event an operator sees by more than the arithmetic does.
Positions are geometric: no refraction is added, which near the horizon lifts what an operator sees
by about half a degree.

**The radio depths are an assumption, written down as one.** −10.1° and −17.2° are the depressions
at which a layer 100 km up (the D and E layers) and one 300 km up (the F2 layer) enter the earth's
shadow, from cos θ = R / (R + h) with R = 6371 km. No standards body defines a grey-line width, and
this annex defines none: it reports times at those depths and says which depth an answer used. The
library gives no rating, score or forecast.

## L.1 Inputs

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-AST-0001` | `qso-graph-astro` MUST be `#![forbid(unsafe_code)]`, MUST perform no I/O, MUST read no clock, and MUST depend on no crate but `qso-graph-grid`. | Test, Inspection |
| `QGDXC-AST-0002` | A moment MUST be UTC as whole milliseconds since 1970-01-01T00:00:00Z, from 1950-01-01T00:00:00Z inclusive to 2100-01-01T00:00:00Z exclusive; a place MUST be a `qso-graph-grid` position. A span MUST be a start moment and a whole number of minutes from 1 to 14,400 (ten days), lying entirely inside that range. Anything else MUST be refused, never clamped. A result computed from accepted inputs, such as a station's day start or a grey-line window's end, MAY fall outside that range and MUST be returned as computed. No input MUST cause a panic. | Test |

## L.2 The sun

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-AST-0010` | For a moment, the library MUST give the sun's declination, equation of time and apparent longitude by NOAA's method as Desktop's `sunAt` computes them; for a moment and a place, the sun's geometric elevation from the hour angle as Desktop's `sunAltitude` computes it, and its azimuth in degrees east of true north, from 0 inclusive to 360 exclusive. | Test |
| `QGDXC-AST-0011` | The library MUST name the altitudes Desktop's Astro page uses: sunrise and sunset at **−0.833°** (34′ of refraction plus the sun's 16′ semidiameter); civil, nautical and astronomical twilight at −6°, −12° and −18°; and the radio depths at **−10.1°** and **−17.2°**, with the derivation of this annex. | Test, Inspection |
| `QGDXC-AST-0012` | The **crossings** of an altitude over a span MUST be found exactly as Desktop's `crossings` finds them, so the two agree to the minute: the body's height above the altitude is sampled at the start and at each whole minute after it up to the span's end; where two neighbouring samples differ in sign (a sample of exactly 0 counts as above), the moment is placed between them by linear interpolation and rounded to the nearest minute, halves upward; the first upward crossing is the rising and the first downward the setting. With neither, the body MUST be reported above or below all span, judged by its height at the start plus half the span (half a minute past a whole minute when the span is odd), above only when that height is greater than 0. A day with no event is an answer, not an error. | Test |
| `QGDXC-AST-0013` | The library MUST give the sun's highest moment over a span as Desktop's `highestOver` does (the first sampled minute of greatest elevation, from the start to the minute before the end), and a station's day start as Desktop's `dayStart` does: the moment of midnight in the place's mean solar time (UTC plus longitude ÷ 15 hours) that begins the date that time has at the given moment. | Test |
| `QGDXC-AST-0014` | The **subsolar point**, where the sun stands overhead, MUST be the latitude of the sun's declination and the longitude at which the sun's hour angle is zero, from −180° exclusive to 180° inclusive. | Test |

## L.3 The moon

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-AST-0020` | For a moment, the library MUST give the moon's geocentric ecliptic longitude, latitude and distance from Meeus's chapter 47 with every term of his Table 47.A and the first 30 terms of Table 47.B, as Desktop's `moonAt` has them, the additive terms A1–A3, the eccentricity factor E, and the nutation in longitude and obliquity Desktop's `moonAt` uses; and from them its right ascension and declination. | Test |
| `QGDXC-AST-0021` | For a moment and a place, the library MUST give the moon's topocentric azimuth (as `QGDXC-AST-0010`) and elevation: the geocentric elevation from the hour angle with Greenwich mean sidereal time from Meeus 12.4, less the parallax in elevation, asin((6378.14 km / distance) × cos elevation). Elevation matters to EME, where a degree is the difference between a path and none. | Test |
| `QGDXC-AST-0022` | Moonrise and moonset over a span MUST be the crossings of `QGDXC-AST-0012` of the moon's geocentric elevation above h0 = 0.7275 π − 0°34′, where π is its horizontal parallax asin(6378.14 km / distance), Meeus's allowance for parallax, semidiameter and refraction together. | Test |
| `QGDXC-AST-0023` | For a moment, the library MUST give the moon's illuminated fraction from the phase angle (Meeus 48.2 and 48.3, the sun at 149,597,870.7 km), its elongation east of the sun from 0 inclusive to 360 exclusive, and the phase's name: `New moon`, `First quarter`, `Full moon` and `Last quarter` within 6° either side of 0°, 90°, 180° and 270°, and between them `Waxing crescent`, `Waxing gibbous`, `Waning gibbous` and `Waning crescent`. | Test |
| `QGDXC-AST-0024` | The **sublunar point** MUST be the moon's declination and the longitude at which its geocentric hour angle is zero, from −180° exclusive to 180° inclusive. | Test |

## L.4 The grey line

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-AST-0030` | The **terminator** at an altitude strictly between −90° and 90° and a moment MUST be the circle of places at which the sun's elevation equals that altitude: the points at an angular distance of 90° minus the altitude from the subsolar point, at bearings from it every *step* degrees, where *step* is from 0.5 to 10 and divides 360 exactly, closed by repeating its first point. It MUST be returned as lines of (longitude, latitude) points split where it crosses the 180th meridian, so that no segment spans a map, as `QGDXC-TUI-0084` splits a path. Every point's sun elevation under `QGDXC-AST-0010` MUST be within 0.01° of the altitude. | Test |
| `QGDXC-AST-0031` | The **grey-line windows** between two places MUST be found as Desktop's `greyLine` finds them: from a moment truncated to its minute, at each whole minute up to 1,440 minutes later, for each radio depth of `QGDXC-AST-0011`, the minutes at which the sun's elevation at both places is below −0.833° and at or above the depth; consecutive such minutes form one window, given by its first and last minute, and a window still open at the last minute ends there. Each answer MUST name its depth. | Test |

## L.5 Tests

| ID | Requirement | Verify |
|---|---|---|
| `QGDXC-AST-0040` | The library MUST give Desktop's answers, recorded in `tests/fixtures/desktop-agreement.json` from Desktop commit `67ca202` over eight places from 78° N to 78° S and twelve dates of 2026: every crossing of every altitude of `QGDXC-AST-0011`, every all-day answer, solar noon, moonrise, moonset and grey-line window equal to the millisecond; sun and moon angles within 10⁻⁶°, the moon's distance within 10⁻³ km, the illuminated fraction within 10⁻⁹, and phase names equal. | Test |
| `QGDXC-AST-0041` | Against PyEphem 4.2: sunrise and sunset within one minute, with the same days having none, over six places from the equator to Longyearbyen and seven dates in `tests/fixtures/sun-reference.json`; moonrise and moonset within one minute, with the same days having none, and declination within 0.05°, distance within 60 km and illuminated fraction within 0.005, in `tests/fixtures/moon-reference.json`; and the sun's azimuth and elevation within 0.02° and the moon's within 0.05°, azimuth differences multiplied by the cosine of the elevation, in `tests/fixtures/position-reference.json`. The scripts that made these files MUST be kept beside them with the command that reruns them; the tests MUST NOT run them. | Test |
| `QGDXC-AST-0042` | Every refusal of `QGDXC-AST-0002` MUST have a test, and a sweep of moments, places, spans, altitudes and steps across and just outside the accepted ranges MUST run without a panic and give only finite numbers. | Test |
