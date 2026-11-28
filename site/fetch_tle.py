"""Fetch CelesTrak element sets, or republish the last successful fetch.

Usage: python3 site/fetch_tle.py OUT BASE_URL [--fetch]

Only the two-hourly scheduled site build passes --fetch. Other builds copy the
published data/tle.json unchanged so they do not make a second CelesTrak
request within one update cycle.
"""

import argparse
import csv
import gzip
import io
import json
import math
import os
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone

OBJECTS = [
    (25544, "ISS"),
    (48274, "CSS (Tianhe)"),
    (20580, "Hubble"),
    (33591, "NOAA 19"),
    (40697, "Sentinel-2A"),
    (49260, "Landsat 9"),
    (25994, "Terra"),
    (27424, "Aqua"),
    (43873, "GPS III SV01 (NAVSTAR 77)"),
    (36585, "GPS IIF-1 (NAVSTAR 65)"),
    (37846, "Galileo GSAT0101"),
    (41866, "GOES-16"),
    (51850, "GOES-18"),
    (40732, "Meteosat-11"),
    (41836, "Himawari-9"),
    (53544, "Starlink-4303"),
    (51990, "Starlink-3657"),
    (58130, "Starlink-30783"),
    (56710, "OneWeb-0561"),
    (22566, "SL-16 rocket body"),
]

SOURCE = (
    "CelesTrak GP (celestrak.org); element sets from the US Space Force "
    "(18 SDS) via Space-Track.org"
)
CELESTRAK_URL = "https://celestrak.org/NORAD/elements/gp.php?CATNR={}&FORMAT=csv"
INT_FIELDS = {"NORAD_CAT_ID", "ELEMENT_SET_NO", "REV_AT_EPOCH", "EPHEMERIS_TYPE"}
STRING_FIELDS = {"OBJECT_NAME", "OBJECT_ID", "EPOCH", "CLASSIFICATION_TYPE"}
REQUIRED_FIELDS = {
    "OBJECT_NAME",
    "EPOCH",
    "MEAN_MOTION",
    "ECCENTRICITY",
    "INCLINATION",
    "RA_OF_ASC_NODE",
    "ARG_OF_PERICENTER",
    "MEAN_ANOMALY",
    "BSTAR",
    "MEAN_MOTION_DOT",
    "MEAN_MOTION_DDOT",
    "NORAD_CAT_ID",
    "ELEMENT_SET_NO",
    "REV_AT_EPOCH",
    "EPHEMERIS_TYPE",
    "CLASSIFICATION_TYPE",
    "OBJECT_ID",
}


def fetch(url):
    """Return (HTTP status, response bytes, headers), or raise a network error."""
    request = urllib.request.Request(
        url,
        headers={
            "Accept-Encoding": "gzip",
            "User-Agent": "fid-site (+https://github.com/praveenprk/fid)",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.status, response.read(), response.headers
    except urllib.error.HTTPError as error:
        return error.code, error.read(), error.headers


def response_body(body, headers):
    """Decompress a response body when its Content-Encoding is gzip."""
    encoding = next(
        (
            value
            for key, value in (headers or {}).items()
            if key.lower() == "content-encoding"
        ),
        "",
    )
    return gzip.decompress(body) if encoding.lower() == "gzip" else body


def parse_csv(body):
    """Convert CelesTrak's CSV fields to the types used by its JSON records."""
    rows = []
    reader = csv.DictReader(io.StringIO(body.decode("utf-8-sig")))
    for row in reader:
        record = {}
        for key, value in row.items():
            if key is None:
                raise ValueError("CSV row has more values than headers")
            if key in INT_FIELDS:
                record[key] = int(value)
            elif key in STRING_FIELDS:
                record[key] = value
            else:
                record[key] = float(value)
        rows.append(record)
    return rows


def valid_record(records, norad):
    """Return the one matching, complete record, or None."""
    if len(records) != 1 or not isinstance(records[0], dict):
        return None
    record = records[0]
    if record.get("NORAD_CAT_ID") != norad or not REQUIRED_FIELDS <= record.keys():
        return None
    return record


def altitude_km(mean_motion):
    """Semi-major-axis altitude above the Earth's equatorial radius, in km."""
    radians_per_second = mean_motion * 2 * math.pi / 86400
    semi_major_axis = (398600.4418 / radians_per_second**2) ** (1 / 3)
    return semi_major_axis - 6378.137


def regime(mean_motion):
    """Classify an orbit from its mean motion in revolutions per day."""
    altitude = altitude_km(mean_motion)
    if altitude < 500:
        return "low LEO"
    if altitude < 2000:
        return "LEO"
    if 33786 <= altitude <= 37786:
        return "GEO"
    return "MEO"


def write_json(path, value):
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "w", encoding="utf-8") as output:
        json.dump(value, output, indent=2)
        output.write("\n")


def republish(out, base_url):
    """Copy a valid published tle.json byte-for-byte; return whether written."""
    url = base_url.rstrip("/") + "/data/tle.json"
    try:
        status, raw_body, headers = fetch(url)
        body = response_body(raw_body, headers)
        value = json.loads(body) if status == 200 else None
    except Exception:
        return False
    if (
        not isinstance(value, dict)
        or "fetched_at" not in value
        or "objects" not in value
    ):
        return False
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    with open(out, "wb") as output:
        output.write(body)
    return True


def download():
    """Fetch objects sequentially, stopping at the first request failure."""
    objects = []
    missing = []
    for index, (norad, name) in enumerate(OBJECTS):
        if index:
            time.sleep(1)
        try:
            status, raw_body, headers = fetch(CELESTRAK_URL.format(norad))
            if status != 200:
                missing.extend(number for number, _ in OBJECTS[index:])
                break
            body = response_body(raw_body, headers)
        except Exception:
            missing.extend(number for number, _ in OBJECTS[index:])
            break

        if body.decode("utf-8-sig", errors="replace").strip() == "No GP data found":
            missing.append(norad)
            continue
        try:
            record = valid_record(parse_csv(body), norad)
        except (UnicodeError, ValueError, TypeError, csv.Error):
            record = None
        if record is None:
            missing.append(norad)
            continue
        objects.append(
            {
                "norad": norad,
                "name": name,
                "regime": regime(record["MEAN_MOTION"]),
                "altitude_km": round(altitude_km(record["MEAN_MOTION"]), 1),
                "omm": record,
            }
        )
    return objects, missing


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out")
    parser.add_argument("base_url")
    parser.add_argument("--fetch", action="store_true")
    args = parser.parse_args(argv)

    objects = []
    missing = []
    if args.fetch:
        objects, missing = download()
    if objects:
        value = {
            "fetched_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
            "source": SOURCE,
            "objects": objects,
            "missing": missing,
        }
        write_json(args.out, value)
        missing_text = (
            ", ".join(str(number) for number in missing) if missing else "none"
        )
        print(f"fetched {len(objects)} objects; missing: {missing_text}")
    elif republish(args.out, args.base_url):
        print("fallback used")
    else:
        print("no tle.json written")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
