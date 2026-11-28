"""Offline tests for the CelesTrak fetch and republish job."""

import csv
import io
import json
import os
import tempfile
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch
from urllib.parse import parse_qs, urlparse

import fetch_tle

FIELDS = [
    "OBJECT_NAME",
    "OBJECT_ID",
    "EPOCH",
    "MEAN_MOTION",
    "ECCENTRICITY",
    "INCLINATION",
    "RA_OF_ASC_NODE",
    "ARG_OF_PERICENTER",
    "MEAN_ANOMALY",
    "EPHEMERIS_TYPE",
    "CLASSIFICATION_TYPE",
    "NORAD_CAT_ID",
    "ELEMENT_SET_NO",
    "REV_AT_EPOCH",
    "BSTAR",
    "MEAN_MOTION_DOT",
    "MEAN_MOTION_DDOT",
]
CSV_SAMPLE = b"""OBJECT_NAME,OBJECT_ID,EPOCH,MEAN_MOTION,ECCENTRICITY,INCLINATION,RA_OF_ASC_NODE,ARG_OF_PERICENTER,MEAN_ANOMALY,EPHEMERIS_TYPE,CLASSIFICATION_TYPE,NORAD_CAT_ID,ELEMENT_SET_NO,REV_AT_EPOCH,BSTAR,MEAN_MOTION_DOT,MEAN_MOTION_DDOT
LANDSAT 8,2013-008A,2026-11-27T14:18:31.464864,14.57119298,.00011804,98.2133,39.6296,93.4255,266.7078,0,U,39084,999,72169,.45313036E-4,.159E-5,0
"""
BASE_URL = "https://fid.example"


def csv_response(norad, mean_motion=15.5, response_norad=None):
    row = {
        "OBJECT_NAME": f"OBJECT {norad}",
        "OBJECT_ID": "2026-001A",
        "EPOCH": "2026-11-27T14:18:31.464864",
        "MEAN_MOTION": str(mean_motion),
        "ECCENTRICITY": ".00011804",
        "INCLINATION": "51.64",
        "RA_OF_ASC_NODE": "39.6296",
        "ARG_OF_PERICENTER": "93.4255",
        "MEAN_ANOMALY": "266.7078",
        "EPHEMERIS_TYPE": "0",
        "CLASSIFICATION_TYPE": "U",
        "NORAD_CAT_ID": str(norad if response_norad is None else response_norad),
        "ELEMENT_SET_NO": "999",
        "REV_AT_EPOCH": "72169",
        "BSTAR": ".45313036E-4",
        "MEAN_MOTION_DOT": ".159E-5",
        "MEAN_MOTION_DDOT": "0",
    }
    text = io.StringIO()
    writer = csv.DictWriter(text, fieldnames=FIELDS, lineterminator="\n")
    writer.writeheader()
    writer.writerow(row)
    return 200, text.getvalue().encode(), {}


def requested_norad(url):
    return int(parse_qs(urlparse(url).query)["CATNR"][0])


class FetchTleTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.out = os.path.join(temporary.name, "data", "tle.json")

    def run_job(self, arguments, side_effect):
        with (
            patch.object(fetch_tle, "fetch", side_effect=side_effect) as fetch_mock,
            patch.object(fetch_tle.time, "sleep") as sleep_mock,
            redirect_stdout(io.StringIO()),
        ):
            code = fetch_tle.main([self.out, BASE_URL, *arguments])
        return code, fetch_mock, sleep_mock

    def read_output(self):
        with open(self.out, encoding="utf-8") as output:
            return json.load(output)

    def test_all_objects_are_in_list_order_with_iss_first(self):
        code, fetch_mock, sleep_mock = self.run_job(
            ["--fetch"], lambda url: csv_response(requested_norad(url))
        )

        self.assertEqual(code, 0)
        value = self.read_output()
        self.assertEqual(
            [obj["norad"] for obj in value["objects"]],
            [norad for norad, _ in fetch_tle.OBJECTS],
        )
        self.assertEqual(value["objects"][0]["name"], "ISS")
        self.assertEqual(value["missing"], [])
        self.assertTrue(value["fetched_at"].endswith("Z"))
        self.assertEqual(fetch_mock.call_count, len(fetch_tle.OBJECTS))
        self.assertEqual(sleep_mock.call_count, len(fetch_tle.OBJECTS) - 1)

    def test_no_gp_data_marks_one_object_missing_and_continues(self):
        decayed = fetch_tle.OBJECTS[4][0]

        def fake_fetch(url):
            norad = requested_norad(url)
            if norad == decayed:
                return 200, b"No GP data found", {}
            return csv_response(norad)

        code, fetch_mock, _ = self.run_job(["--fetch"], fake_fetch)

        self.assertEqual(code, 0)
        value = self.read_output()
        self.assertEqual(value["missing"], [decayed])
        self.assertEqual(len(value["objects"]), len(fetch_tle.OBJECTS) - 1)
        self.assertNotIn(decayed, [obj["norad"] for obj in value["objects"]])
        self.assertEqual(fetch_mock.call_count, len(fetch_tle.OBJECTS))

    def test_wrong_norad_catalog_id_is_missing(self):
        wrong = fetch_tle.OBJECTS[1][0]

        def fake_fetch(url):
            norad = requested_norad(url)
            return csv_response(
                norad, response_norad=999999 if norad == wrong else None
            )

        self.run_job(["--fetch"], fake_fetch)

        value = self.read_output()
        self.assertEqual(value["missing"], [wrong])
        self.assertNotIn(wrong, [obj["norad"] for obj in value["objects"]])

    def test_orbit_regimes(self):
        cases = [
            (15.5, "low LEO"),  # ISS-class orbit
            (1.00273, "GEO"),
            (2.0057, "MEO"),
            (14.2, "LEO"),
        ]
        for mean_motion, expected in cases:
            with self.subTest(mean_motion=mean_motion):
                self.assertEqual(fetch_tle.regime(mean_motion), expected)

    def test_all_fail_and_valid_fallback_is_written_unchanged(self):
        fallback = (
            b'{"fetched_at":"2026-11-27T12:00:00Z",'
            b'"objects":[{"norad":25544}],"missing":[]}\n'
        )

        def fake_fetch(url):
            if "celestrak.org" in url:
                return 503, b"unavailable", {}
            return 200, fallback, {}

        code, _, _ = self.run_job(["--fetch"], fake_fetch)

        self.assertEqual(code, 0)
        with open(self.out, "rb") as output:
            self.assertEqual(output.read(), fallback)
        self.assertEqual(self.read_output()["fetched_at"], "2026-11-27T12:00:00Z")

    def test_all_fail_and_failed_fallback_writes_nothing_and_exits_zero(self):
        code, _, _ = self.run_job(["--fetch"], lambda url: (503, b"unavailable", {}))

        self.assertEqual(code, 0)
        self.assertFalse(os.path.exists(self.out))

    def test_403_on_third_object_stops_after_exactly_three_fetches(self):
        def fake_fetch(url):
            norad = requested_norad(url)
            if norad == fetch_tle.OBJECTS[2][0]:
                return 403, b"too soon", {}
            return csv_response(norad)

        code, fetch_mock, sleep_mock = self.run_job(["--fetch"], fake_fetch)

        self.assertEqual(code, 0)
        value = self.read_output()
        self.assertEqual(
            [obj["norad"] for obj in value["objects"]],
            [norad for norad, _ in fetch_tle.OBJECTS[:2]],
        )
        self.assertEqual(
            value["missing"], [norad for norad, _ in fetch_tle.OBJECTS[2:]]
        )
        self.assertEqual(fetch_mock.call_count, 3)
        self.assertEqual(sleep_mock.call_count, 2)

    def test_csv_sample_has_json_compatible_types(self):
        record = fetch_tle.valid_record(fetch_tle.parse_csv(CSV_SAMPLE), 39084)

        self.assertIsNotNone(record)
        self.assertEqual(record["NORAD_CAT_ID"], 39084)
        self.assertIsInstance(record["NORAD_CAT_ID"], int)
        self.assertEqual(record["ECCENTRICITY"], 0.00011804)
        self.assertIsInstance(record["ECCENTRICITY"], float)
        self.assertEqual(record["BSTAR"], 4.5313036e-05)
        self.assertEqual(record["EPOCH"], "2026-11-27T14:18:31.464864")
        self.assertIsInstance(record["EPOCH"], str)

    def test_without_fetch_only_requests_the_published_file(self):
        fallback = b'{"fetched_at":"2026-11-27T12:00:00Z","objects":[]}'
        code, fetch_mock, sleep_mock = self.run_job([], lambda url: (200, fallback, {}))

        self.assertEqual(code, 0)
        self.assertEqual(fetch_mock.call_count, 1)
        self.assertEqual(fetch_mock.call_args.args[0], BASE_URL + "/data/tle.json")
        self.assertNotIn("celestrak.org", fetch_mock.call_args.args[0])
        sleep_mock.assert_not_called()
        with open(self.out, "rb") as output:
            self.assertEqual(output.read(), fallback)


if __name__ == "__main__":
    unittest.main()
