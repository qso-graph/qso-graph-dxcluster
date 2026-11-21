<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Notices

QG DX Cluster is licensed under GPL-3.0-or-later; see [LICENSE](LICENSE). It includes
the following third-party data under its own license.

## Country file

`crates/qso-graph-dxcluster-cli/src/country_file.rs` holds the country file published
at [country-files.com](https://www.country-files.com/) in the `cty.dat` format, version
given by its `VERSION` constant. It is modified: `scripts/strip-cty.sh` removes the
exact-callsign entries and keeps every entity header and prefix. `qg-dxc tui` uses it to
place callsign prefixes on the map (`QGDXC-TUI-0083`).

Amateur Radio Country Files, <https://www.country-files.com/>. The notice below is the
one WSJT-X distributes with its copy of the same file (`cty.dat_copyright.txt`):

Copyright © 1994-

Permission is hereby granted, free of charge, to any person obtaining a copy of this
software and associated documentation files (the "Software"), to deal in the Software
without restriction, including without limitation the rights to use, copy, modify,
merge, publish, distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice shall be included in all copies
or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF
CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR
THE USE OR OTHER DEALINGS IN THE SOFTWARE.

To update it, download the current `cty.dat` from country-files.com and run
`scripts/strip-cty.sh <cty.dat>`.
