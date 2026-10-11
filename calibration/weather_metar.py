"""How fast does an airport weather observation go stale?

Ground truth: routine hourly METAR reports from 10 major US airports for
all of 2024, from the Iowa Environmental Mesonet ASOS archive. A METAR
is the observation pilots and dispatchers act on. Each report is a
stored fact; later reports from the same airport are the truth it is
checked against.

Three facts:
  1. "the temperature is T"         — still correct if within ±2 °F
  2. "the flight category is C"     — VFR / MVFR / IFR / LIFR unchanged
  3. "the airport is IFR or worse"  — the same, only for adverse reports,
                                      the case where staleness matters

The nearest hand-picked guess in examples/half-lives.yaml is
`weather_forecast: 6h` (sensor_reading is "domain_specific").

Usage: python3 weather_metar.py
       FID_WEATHER_TRAILING_DAYS=365 python3 weather_metar.py   (the last 365 days)
"""

import csv
import os
import urllib.parse
from datetime import date, datetime, timedelta

import fidcal

STATIONS = ["JFK", "ORD", "ATL", "DFW", "DEN", "LAX", "SFO", "SEA", "MIA", "BOS"]
YEAR = 2024


def window():
    """(start, end) dates, end exclusive: calendar YEAR, or the trailing N days."""
    trailing = os.environ.get("FID_WEATHER_TRAILING_DAYS")
    if trailing:
        end = date.today()
        return end - timedelta(days=int(trailing)), end
    return date(YEAR, 1, 1), date(YEAR + 1, 1, 1)

LAG_HOURS = [0, 1, 2, 3, 4, 6, 8, 10, 12, 15, 18, 21, 24, 30, 36, 48, 60, 72]
LAGS = [h * 3600 for h in LAG_HOURS]
TEMP_TOLERANCE_F = 2.0


def fetch(cache_dir, station):
    start, end = window()
    params = {
        "station": station,
        "data": "tmpf,vsby,skyc1,skyl1,skyc2,skyl2,skyc3,skyl3,skyc4,skyl4",
        "year1": start.year, "month1": start.month, "day1": start.day,
        "year2": end.year, "month2": end.month, "day2": end.day,
        "tz": "Etc/UTC", "format": "onlycomma", "latlon": "no",
        "missing": "M", "trace": "T", "direct": "no", "report_type": "3",
    }
    url = "https://mesonet.agron.iastate.edu/cgi-bin/request/asos.py?" + urllib.parse.urlencode(params)
    path = os.path.join(cache_dir, "metar", f"{station}_{start}_{end}.csv")
    return fidcal.download(url, path)


def flight_category(row):
    """FAA flight category from ceiling and visibility. None if missing."""
    try:
        vis = float(row["vsby"])
    except ValueError:
        return None
    ceiling = None
    for i in range(1, 5):
        cover, level = row[f"skyc{i}"], row[f"skyl{i}"]
        if cover in ("BKN", "OVC", "VV"):
            try:
                ceiling = float(level)
            except ValueError:
                return None
            break
    ceil = ceiling if ceiling is not None else float("inf")
    if ceil < 500 or vis < 1:
        return "LIFR"
    if ceil < 1000 or vis < 3:
        return "IFR"
    if ceil <= 3000 or vis <= 5:
        return "MVFR"
    return "VFR"


def load(path):
    """Hourly series: rounded hour → (temp_f or None, category or None)."""
    series = {}
    with open(path, newline="") as f:
        for row in csv.DictReader(f):
            try:
                at = datetime.strptime(row["valid"], "%Y-%m-%d %H:%M")
            except (KeyError, ValueError):
                continue
            hour = (at + timedelta(minutes=30)).replace(minute=0)
            try:
                temp = float(row["tmpf"])
            except ValueError:
                temp = None
            series[hour] = (temp, flight_category(row))
    return series


def observations(all_series):
    temp, cat, adverse = [], [], []
    for station, series in all_series.items():
        for hour, (t0, c0) in series.items():
            for lag_h, lag in zip(LAG_HOURS, LAGS):
                later = series.get(hour + timedelta(hours=lag_h))
                if later is None:
                    continue
                t1, c1 = later
                if t0 is not None and t1 is not None:
                    temp.append((station, lag, abs(t1 - t0) <= TEMP_TOLERANCE_F))
                if c0 is not None and c1 is not None:
                    cat.append((station, lag, c1 == c0))
                    if c0 in ("IFR", "LIFR"):
                        adverse.append((station, lag, c1 in ("IFR", "LIFR")))
    return temp, cat, adverse


def main():
    cache = fidcal.default_cache_dir()
    all_series = {}
    for station in STATIONS:
        all_series[station] = load(fetch(cache, station))
        fidcal.log(f"{station}: {len(all_series[station]):,} hourly reports")

    temp, cat, adverse = observations(all_series)
    guess = dict(guess=6 * fidcal.HOUR, guess_label="weather_forecast: 6h")
    results = [
        fidcal.analyze("METAR temperature", "the temperature at airport A is T",
                       temp, LAGS, truth=f"within ±{TEMP_TOLERANCE_F:.0f} °F", **guess),
        fidcal.analyze("METAR flight category", "airport A is VFR / MVFR / IFR / LIFR",
                       cat, LAGS, truth="same flight category", **guess),
        fidcal.analyze("METAR adverse conditions", "airport A is IFR or worse",
                       adverse, LAGS, truth="still IFR or LIFR", **guess),
    ]
    fidcal.write_results(results, "weather_metar")


if __name__ == "__main__":
    main()
