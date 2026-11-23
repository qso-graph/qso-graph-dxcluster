// SPDX-License-Identifier: GPL-3.0-or-later
// Records desktop-agreement.json from QSO Graph Desktop commit 67ca202. From a Desktop checkout,
// copy this file into extensions/astro/, then:
//   npx esbuild extensions/astro/record-desktop.ts --bundle --platform=node --format=esm --outfile=/tmp/record.mjs && node /tmp/record.mjs > desktop-agreement.json
import { ASTRONOMICAL, CIVIL, D_LAYER, F2_LAYER, HORIZON, NAUTICAL, dayStart, highestOver, julian, sunAltitude, sunAt, sunCrossingsOver } from './src/sun.ts';
import { moonAt, moonCrossingsOver, moonPhase, sublunar } from './src/moon.ts';
import { greyLine } from './src/greyline.ts';
const places: [string, number, number][] = [
  ['Denver', 39.7392, -104.9903], ['Greenwich', 51.4769, 0.0], ['Sydney', -33.8688, 151.2093],
  ['Quito', -0.1807, -78.4678], ['Tromso', 69.6492, 18.9553], ['Longyearbyen', 78.2232, 15.6267],
  ['McMurdo', -77.8463, 166.6682], ['Honolulu', 21.3069, -157.8583],
];
const dates = ['2026-01-01','2026-02-14','2026-03-20','2026-04-30','2026-05-21','2026-06-21','2026-07-15','2026-08-26','2026-09-23','2026-10-31','2026-11-23','2026-12-21'];
const ALT: [string, number][] = [['horizon',HORIZON],['civil',CIVIL],['nautical',NAUTICAL],['astronomical',ASTRONOMICAL],['d_layer',D_LAYER],['f2_layer',F2_LAYER]];
const c = (x: any) => ({ rising: x.rising?.getTime() ?? null, setting: x.setting?.getTime() ?? null, all_day: x.allDay ?? null });
const out: any = { days: [], grey_line: [], positions: [] };
for (const [name, lat, lon] of places) for (const date of dates) {
  const start = dayStart(new Date(date + 'T12:00:00Z'), lon);
  const sun: any = {}; for (const [k, a] of ALT) sun[k] = c(sunCrossingsOver(start, 1440, lat, lon, a));
  out.days.push({ place: name, latitude: lat, longitude: lon, date, start, sun, solar_noon: highestOver(start, 1440, lat, lon).getTime(), moon: c(moonCrossingsOver(start, 1440, lat, lon)) });
}
for (const date of dates) for (let h = 0; h < 24; h += 6) {
  const ms = Date.parse(date + 'T00:00:00Z') + h * 3600000; const jd = julian(ms);
  const s = sunAt(jd); const m = moonAt(jd); const p = moonPhase(jd); const sl = sublunar(jd);
  const alts = places.map(([, lat, lon]) => sunAltitude(jd, lat, lon));
  out.positions.push({ ms, sun_declination: s.declination, equation_of_time: s.equationOfTime, sun_apparent_longitude: s.apparentLongitude,
    sun_altitudes: alts, moon_longitude: m.longitude, moon_latitude: m.latitude, moon_distance_km: m.distance, moon_right_ascension: m.rightAscension,
    moon_declination: m.declination, moon_fraction: p.fraction, moon_elongation: p.elongation, moon_phase: p.name, sublunar_latitude: sl.latitude, sublunar_longitude: sl.longitude });
}
const pairs: [number, number][] = [[0,1],[0,2],[1,4],[2,3],[5,6],[7,0]];
for (const [a, b] of pairs) for (const date of ['2026-03-20','2026-06-21','2026-11-23']) {
  const from = Date.parse(date + 'T00:00:00Z') + 37 * 60000 + 13000;
  const g = greyLine(new Date(from), { latitude: places[a][1], longitude: places[a][2] }, { latitude: places[b][1], longitude: places[b][2] });
  out.grey_line.push({ from, here: places[a][0], there: places[b][0], depths: g.map(d => ({ depth: d.depth, windows: d.windows.map(w => [w.start.getTime(), w.end.getTime()]) })) });
}
console.log(JSON.stringify({ places: places.map(([n, la, lo]) => ({ name: n, latitude: la, longitude: lo })), ...out }));
