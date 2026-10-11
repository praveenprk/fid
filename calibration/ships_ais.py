"""How fast does a cargo ship's reported position go stale?

Ground truth: one day (15 March 2023) of AIS broadcasts from every cargo
ship and tanker in US waters, from NOAA / BOEM MarineCadastre. AIS is
what ports, coast guards and other ships use to know where a vessel is.

Fact: "vessel V is at position P". Still correct if a later AIS report
puts V within 1 km of P (berth / collision scale) — or 10 km (port /
approach scale). Observations start every 30 minutes between 00:00 and
12:00 UTC so every lag up to 12 hours is inside the day.

Split by what the ship was doing when observed: underway (≥ 1 knot) or
stationary (moored or at anchor). fid gives a fact type one half-life;
this tests whether one is enough.

Tests examples/half-lives.yaml's `shipping_status: 4h`, the nearest guess.

Usage: python3 ships_ais.py   (downloads ~320 MB once)
"""

import csv
import io
import math
import os
import zipfile
from collections import defaultdict

import fidcal

DAY = "2023_03_15"
URL = f"https://coast.noaa.gov/htdata/CMSP/AISDataHandler/2023/AIS_{DAY}.zip"
LAG_MINUTES = [0, 5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 360, 480, 600, 720]
LAGS = [m * 60 for m in LAG_MINUTES]
OBS_START_MINUTES = range(0, 12 * 60, 30)
MATCH_WINDOW = 5  # minutes either side of the target time
UNDERWAY_KNOTS = 1.0


def is_cargo_or_tanker(vessel_type):
    try:
        return 70 <= int(vessel_type) <= 89
    except ValueError:
        return False


def load(path):
    """Per vessel: minute of day → (lat, lon, sog). First report per minute."""
    tracks = defaultdict(dict)
    with zipfile.ZipFile(path) as z:
        name = next(n for n in z.namelist() if n.endswith(".csv"))
        with z.open(name) as raw:
            reader = csv.reader(io.TextIOWrapper(raw, encoding="utf-8", newline=""))
            header = next(reader)
            i = {col: header.index(col) for col in ("MMSI", "BaseDateTime", "LAT", "LON", "SOG", "VesselType")}
            for row in reader:
                if not is_cargo_or_tanker(row[i["VesselType"]]):
                    continue
                ts = row[i["BaseDateTime"]]  # 2023-03-15T00:00:01
                minute = int(ts[11:13]) * 60 + int(ts[14:16])
                track = tracks[row[i["MMSI"]]]
                if minute in track:
                    continue
                try:
                    track[minute] = (float(row[i["LAT"]]), float(row[i["LON"]]), float(row[i["SOG"]]))
                except ValueError:
                    continue
    return tracks


def haversine_km(a, b):
    lat1, lon1, lat2, lon2 = map(math.radians, (a[0], a[1], b[0], b[1]))
    h = math.sin((lat2 - lat1) / 2) ** 2 + math.cos(lat1) * math.cos(lat2) * math.sin((lon2 - lon1) / 2) ** 2
    return 2 * 6371.0 * math.asin(math.sqrt(h))


def report_near(track, minute):
    """The report closest to `minute`, within MATCH_WINDOW, or None."""
    for d in range(MATCH_WINDOW + 1):
        for m in (minute - d, minute + d):
            if m in track:
                return track[m]
    return None


def observations(tracks, threshold_km):
    """(mmsi, lag, correct, underway) for every observation and lag."""
    for mmsi, track in tracks.items():
        for start in OBS_START_MINUTES:
            first = report_near(track, start)
            if first is None:
                continue
            underway = first[2] >= UNDERWAY_KNOTS
            for lag_min, lag in zip(LAG_MINUTES, LAGS):
                later = report_near(track, start + lag_min)
                if later is None:
                    continue
                yield (mmsi, lag, haversine_km(first, later) <= threshold_km, underway)


def main():
    path = fidcal.download(URL, os.path.join(fidcal.default_cache_dir(), "ais", f"AIS_{DAY}.zip"))
    tracks = load(path)
    fidcal.log(f"{len(tracks):,} cargo ships and tankers")

    guess = dict(guess=4 * fidcal.HOUR, guess_label="shipping_status: 4h")
    results = []
    for threshold in (1.0, 10.0):
        obs = list(observations(tracks, threshold))
        truth = f"next report within {threshold:.0f} km"
        groups = [
            ("all vessels", obs),
            ("underway", [o for o in obs if o[3]]),
            ("moored or at anchor", [o for o in obs if not o[3]]),
        ]
        for label, group in groups:
            results.append(fidcal.analyze(
                f"AIS {threshold:.0f} km, {label}",
                f"vessel V is at position P (±{threshold:.0f} km)",
                ((m, l, c) for m, l, c, _ in group), LAGS, truth=truth, **guess,
            ))
    fidcal.write_results(results, "ships_ais")


if __name__ == "__main__":
    main()
