# SPDX-License-Identifier: GPL-3.0-or-later
# Makes moon-reference.json:
#   uv run --with ephem==4.2 python -I moon-reference.py > moon-reference.json
# Moonrise, moonset, declination, distance and illuminated fraction from PyEphem 4.2, an
# independent ephemeris sharing no code with qso-graph-astro. Rise and set with refraction off
# and the horizon at -0:34 on the upper limb, topocentric: Meeus's h0. Each day is the place's
# own, from midnight at its mean solar time.
import datetime as dt, json, math
import ephem
places = [('Denver', 39.7392, -104.9903), ('Greenwich', 51.4769, 0.0), ('Sydney', -33.8688, 151.2093),
  ('Quito', -0.1807, -78.4678), ('Tromso', 69.6492, 18.9553)]
dates = ['2026-01-15', '2026-03-20', '2026-06-21', '2026-09-23', '2026-10-22', '2026-12-21']
out = []
for name, lat, lon in places:
  for day in dates:
    o = ephem.Observer(); o.lat = str(lat); o.lon = str(lon); o.elevation = 0
    o.pressure = 0; o.horizon = '-0:34'
    d = dt.datetime.strptime(day, '%Y-%m-%d')
    start = d - dt.timedelta(hours=lon / 15)
    m = ephem.Moon()
    def within(fn):
      try:
        t = ephem.Date(fn(m, start=ephem.Date(start))).datetime()
        return t.strftime('%Y-%m-%dT%H:%M:%SZ') if t < start + dt.timedelta(days=1) else None
      except (ephem.AlwaysUpError, ephem.NeverUpError):
        return None
    noon = ephem.Date(d + dt.timedelta(hours=12))
    g = ephem.Moon(noon)
    out.append({'place': name, 'latitude': lat, 'longitude': lon, 'day': day,
      'rise': within(o.next_rising), 'set': within(o.next_setting),
      'noon': noon.datetime().strftime('%Y-%m-%dT%H:%M:%SZ'),
      'declination': round(float(g.dec) * 180 / math.pi, 4),
      'distance_km': round(g.earth_distance * 149597870.7, 1),
      'illuminated': round(g.moon_phase, 5)})
print(json.dumps(out, indent=1))
