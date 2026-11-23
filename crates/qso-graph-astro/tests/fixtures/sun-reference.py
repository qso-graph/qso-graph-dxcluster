# SPDX-License-Identifier: GPL-3.0-or-later
# Makes sun-reference.json:
#   uv run --with ephem==4.2 python -I sun-reference.py > sun-reference.json
# Sunrise and sunset from PyEphem 4.2, an independent ephemeris sharing no code with
# qso-graph-astro: refraction off and the upper limb at -0:34, which is the sun's centre at
# -0.833 as NOAA's method has it. Each day is the place's own, from midnight at its mean
# solar time. A day with no sunrise or no sunset gives null.
import datetime as dt, json
import ephem
places = [('Denver', 39.7392, -104.9903), ('Greenwich', 51.4769, 0.0), ('Sydney', -33.8688, 151.2093),
  ('Quito', -0.1807, -78.4678), ('Tromso', 69.6492, 18.9553), ('Longyearbyen', 78.2232, 15.6267)]
dates = ['2026-01-15', '2026-03-01', '2026-03-20', '2026-06-21', '2026-08-26', '2026-09-23', '2026-12-21']
out = []
for name, lat, lon in places:
  for day in dates:
    o = ephem.Observer(); o.lat = str(lat); o.lon = str(lon); o.elevation = 0
    o.pressure = 0; o.horizon = '-0:34'
    start = dt.datetime.strptime(day, '%Y-%m-%d') - dt.timedelta(hours=lon / 15)
    s = ephem.Sun()
    def within(fn):
      try:
        t = ephem.Date(fn(s, start=ephem.Date(start))).datetime()
        return t.strftime('%Y-%m-%dT%H:%M:%SZ') if t < start + dt.timedelta(days=1) else None
      except (ephem.AlwaysUpError, ephem.NeverUpError):
        return None
    out.append({'place': name, 'latitude': lat, 'longitude': lon, 'day': day,
      'rise': within(o.next_rising), 'set': within(o.next_setting)})
print(json.dumps(out, indent=1))
