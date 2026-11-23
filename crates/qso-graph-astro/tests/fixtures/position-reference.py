# SPDX-License-Identifier: GPL-3.0-or-later
# Makes position-reference.json:
#   uv run --with ephem==4.2 python -I position-reference.py > position-reference.json
# Sun and moon azimuth and elevation from PyEphem 4.2, an independent ephemeris sharing
# no code with qso-graph-astro: topocentric, refraction off (pressure 0), sea level.
import datetime as dt, json, math
import ephem
places = [
  ('Denver', 39.7392, -104.9903), ('Greenwich', 51.4769, 0.0), ('Sydney', -33.8688, 151.2093),
  ('Quito', -0.1807, -78.4678), ('Tromso', 69.6492, 18.9553), ('McMurdo', -77.8463, 166.6682),
]
times = [dt.datetime(2026, m, d, h, 17) for (m, d) in [(1, 3), (4, 11), (7, 22), (10, 5), (11, 23)] for h in (0, 5, 10, 15, 20)]
out = []
for name, lat, lon in places:
  for t in times:
    o = ephem.Observer(); o.lat = str(lat); o.lon = str(lon); o.elevation = 0; o.pressure = 0
    o.date = ephem.Date(t)
    s = ephem.Sun(o); m = ephem.Moon(o)
    deg = lambda a: round(float(a) * 180 / math.pi, 4)
    out.append({'place': name, 'latitude': lat, 'longitude': lon, 'utc': t.strftime('%Y-%m-%dT%H:%M:%SZ'),
      'sun_azimuth': deg(s.az), 'sun_elevation': deg(s.alt), 'moon_azimuth': deg(m.az), 'moon_elevation': deg(m.alt)})
print(json.dumps(out, indent=1))
