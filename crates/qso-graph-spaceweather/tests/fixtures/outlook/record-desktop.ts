// SPDX-License-Identifier: GPL-3.0-or-later
// Records desktop-agreement.json from QSO Graph Desktop commit 67ca202. From a Desktop checkout,
// copy this file into extensions/conditions/, then:
//   npx esbuild extensions/conditions/record-desktop.ts --bundle --platform=node --format=esm --outfile=/tmp/record.mjs && node /tmp/record.mjs > desktop-agreement.json
import { bandOutlook } from './src/conditions.ts';
const sfis = [0, 50, 79, 79.9, 80, 89, 90, 99, 100, 119, 120, 129, 130, 139, 140, 149, 149.99, 150, 159, 160, 200, 300, 999];
const kps = [0, 1, 2, 2.99, 3, 3.33, 3.67, 3.99, 4, 4.33, 4.9, 4.99, 5, 5.67, 7, 9];
const cases = [];
for (const sfi of sfis) for (const kp of kps) cases.push({ sfi, kp, bands: bandOutlook(Math.trunc(sfi), kp) });
console.log(JSON.stringify({ cases }));
