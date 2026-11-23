// SPDX-License-Identifier: GPL-3.0-or-later
// Records desktop-agreement.json from QSO Graph Desktop commit 67ca202. From a Desktop checkout,
// copy this file into packages/storage/, then:
//   npx esbuild packages/storage/record-desktop.ts --bundle --platform=node --format=esm --outfile=/tmp/record.mjs && node /tmp/record.mjs > desktop-agreement.json
import { gridBounds, gridOf } from './station-contract.ts';
let seed = 20261123;
const rnd = () => { seed = (seed * 1103515245 + 12345) % 2147483648; return seed / 2147483648; };
const positions: [number, number][] = [[90, 180], [-90, -180], [0, 0], [90, 0], [0, 180], [-90, 180], [39.7392, -104.9903], [-33.8688, 151.2093], [89.9999999, 179.9999999]];
for (let i = 0; i < 400; i += 1) positions.push([rnd() * 180 - 90, rnd() * 360 - 180]);
const of = positions.map(([lat, lon]) => ({ latitude: lat, longitude: lon, grids: ([2, 4, 6, 8] as const).map(n => gridOf(lat, lon, n)) }));
const locators = new Set<string>(['AA', 'RR', 'AA00', 'RR99', 'AA00aa', 'RR99xx', 'AA00aa00', 'RR99xx99', 'FN31PR', 'Fn31Pr55', 'jo65HA', 'JJ00']);
for (const o of of) for (const g of o.grids) if (g) locators.add(g);
const bounds = [...locators].map(l => ({ locator: l, bounds: gridBounds(l) }));
console.log(JSON.stringify({ grid_of: of, bounds }));
