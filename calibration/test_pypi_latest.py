"""Tests for pypi_latest.py: version rules and the latest-version timeline. No network.

    python3 -m unittest test_pypi_latest      (from calibration/)
"""

import unittest
from datetime import datetime, timedelta, timezone

import fidcal
import pypi_latest as pl


def files(day, yanked=False):
    at = datetime(2020, 1, 1, tzinfo=timezone.utc) + timedelta(days=day)
    return [{"upload_time_iso_8601": at.isoformat().replace("+00:00", "Z"), "yanked": yanked}]


class Versions(unittest.TestCase):
    def test_final_releases_only(self):
        for v in ("1.2", "2.0.1", "1!2.0", "3.1.post1", "v4.0"):
            self.assertIsNotNone(pl.version_key(v), v)
        for v in ("1.0a1", "2.0b3", "2.0rc1", "1.0.dev4", "2021-01", "latest"):
            self.assertIsNone(pl.version_key(v), v)

    def test_ordering(self):
        k = pl.version_key
        self.assertLess(k("1.9.4"), k("1.10"))
        self.assertEqual(k("1.0"), k("1.0.0"))
        self.assertLess(k("2.0"), k("2.0.post1"))
        self.assertLess(k("99.0"), k("1!0.1"))


class Timeline(unittest.TestCase):
    def setUp(self):
        data = {"releases": {
            "1.0": files(0),
            "2.0rc1": files(5),       # pre-release: ignored
            "2.0": files(10),
            "1.9.9": files(20),       # backport after 2.0: does not change latest
            "2.1": files(30, yanked=True),  # yanked: ignored
            "2.2": files(40),
            "3.0": [],                # no files: ignored
        }}
        self.changes = pl.timeline(pl.releases(data))

    def test_changes(self):
        self.assertEqual([v for _, v in self.changes], ["1.0", "2.0", "2.2"])

    def test_latest_at(self):
        day = lambda d: datetime(2020, 1, 1, tzinfo=timezone.utc) + timedelta(days=d)
        self.assertIsNone(pl.latest_at(self.changes, day(-1)))
        self.assertEqual(pl.latest_at(self.changes, day(9))[0], "1.0")
        self.assertEqual(pl.latest_at(self.changes, day(25))[0], "2.0")
        self.assertEqual(pl.latest_at(self.changes, day(35))[0], "2.0")
        self.assertEqual(pl.latest_at(self.changes, day(41))[0], "2.2")

    def test_states(self):
        self.assertEqual(pl.state(fidcal.DAY), "just_released")
        self.assertEqual(pl.state(100 * fidcal.DAY), "quiet")
        self.assertEqual(pl.state(2 * fidcal.YEAR), "dormant")

    def test_observations(self):
        # one release in 2014, never replaced: every snapshot stays correct and the package is dormant
        old = datetime(2014, 6, 1, tzinfo=timezone.utc)
        obs = pl.observations({"stable": [(old, "1.0")]})
        self.assertTrue(all(c for _, _, c in obs["all"]))
        self.assertEqual(len(obs["all"]), len(pl.SNAPSHOTS) * len(pl.LAGS))
        self.assertEqual(len(obs["dormant"]), len(obs["all"]) - 6 * len(pl.LAGS))  # quiet Jan–Jun 2015
        # a lag-0 observation is always correct
        new = pl.observations({"busy": [(old + timedelta(days=30 * i), f"1.{i}") for i in range(130)]})
        self.assertTrue(all(c for _, lag, c in new["all"] if lag == 0))
        self.assertFalse(all(c for _, lag, c in new["all"] if lag >= 30 * fidcal.DAY))


if __name__ == "__main__":
    unittest.main()
