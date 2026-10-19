"""Tests for eia_demand.py: parsing, the ±5% fact, counting and the echo rule. No network.

    python3 -m unittest test_eia_demand      (from calibration/)
"""

import math
import unittest
from datetime import datetime, timedelta

import eia_demand as ed
import fidcal


def row(period, value, kind="D"):
    return {"period": period, "respondent": "TEST", "type": kind, "value": value, "value-units": "megawatthours"}


class Parsing(unittest.TestCase):
    def test_load(self):
        rows = [row("2024-01-01T00", "100"), row("2024-01-01T01", "0"), row("2024-01-01T02", None),
                row("2024-01-01T03", "-5"), row("2024-01-01T04", "120", kind="NG"), row("2024-01-01T05", "104.5")]
        self.assertEqual(ed.load(rows), {datetime(2024, 1, 1, 0): 100.0, datetime(2024, 1, 1, 5): 104.5})

    def test_query_keeps_key_last_and_out_of_the_log(self):
        url = ed.query("ERCO", 5000, "SECRET")
        self.assertTrue(url.endswith("api_key=SECRET"))
        self.assertIn("offset=5000", url)
        self.assertIn("facets%5Btype%5D%5B%5D=D", url)

    def test_tolerance(self):
        self.assertTrue(ed.still_correct(100.0, 105.0))
        self.assertTrue(ed.still_correct(100.0, 95.0))
        self.assertFalse(ed.still_correct(100.0, 105.1))
        self.assertFalse(ed.still_correct(100.0, 94.9))


def weekly_series(weeks=6, start=datetime(2024, 1, 1)):
    """Demand that repeats every 24h on weekdays, lower at weekends: exact echoes at 24h and 168h."""
    out = {}
    for h in range(weeks * 168):
        at = start + timedelta(hours=h)
        daily = 100 + 30 * math.sin(2 * math.pi * at.hour / 24)
        out[at] = daily * (0.7 if at.weekday() >= 5 else 1.0)
    return out


class Counting(unittest.TestCase):
    def setUp(self):
        self.counts = ed.count({"A": weekly_series()}, lag_hours=[0, 1, 12, 24, 168])

    def at(self, lag):
        k, n = [0, 0]
        for (_r, _h1, l), (kk, nn) in self.counts.items():
            if l == lag:
                k, n = k + kk, n + nn
        return k / n, n

    def test_lag_zero_always_correct(self):
        self.assertEqual(self.at(0)[0], 1.0)

    def test_weekly_echo_is_exact(self):
        self.assertEqual(self.at(168)[0], 1.0)
        self.assertEqual(self.at(168)[1], 5 * 168)  # pairs only where the later hour exists

    def test_daily_echo_breaks_at_weekends(self):
        s24, _ = self.at(24)
        self.assertLess(s24, 1.0)       # Fri→Sat and Sun→Mon change by 30%
        self.assertGreater(s24, 0.6)
        self.assertLess(self.at(12)[0], s24)

    def test_tally(self):
        t = ed.tally(self.counts, [0, 24])
        self.assertEqual(t.entities, {"A"})
        lags = {lag for lag, _, _ in t.items()}
        self.assertEqual(lags, {0.0, 24 * fidcal.HOUR})
        self.assertEqual(t.total(), sum(n for (_, _, l), (_, n) in self.counts.items() if l in (0, 24)))


class Rule(unittest.TestCase):
    def result(self, brier, rel, gap):
        split = {"brier_test": brier, "reliability_error_test": rel}
        return {"splits": {"station": split, "time": split}, "gap_all": gap}

    def test_weekly_needs_to_beat_daily(self):
        gap = {"echo_24h": 0.05, "echo_24h_and_168h": 0.04}
        rel = {"decay_to_baseline": 0.02, "echo_24h": 0.01, "echo_24h_and_168h": 0.01}
        r = ed.rule(self.result({"decay_to_baseline": 0.2, "echo_24h": 0.18, "echo_24h_and_168h": 0.17}, rel, gap))
        self.assertTrue(r["echo_passes"])
        self.assertTrue(r["weekly_passes"])
        r = ed.rule(self.result({"decay_to_baseline": 0.2, "echo_24h": 0.18, "echo_24h_and_168h": 0.19}, rel, gap))
        self.assertTrue(r["echo_passes"])
        self.assertFalse(r["weekly_passes"])

    def test_gap_fails_the_echo(self):
        gap = {"echo_24h": 0.15, "echo_24h_and_168h": 0.15}
        rel = {"decay_to_baseline": 0.02, "echo_24h": 0.01, "echo_24h_and_168h": 0.01}
        r = ed.rule(self.result({"decay_to_baseline": 0.2, "echo_24h": 0.18, "echo_24h_and_168h": 0.17}, rel, gap))
        self.assertFalse(r["echo_passes"])
        self.assertFalse(r["weekly_passes"])


if __name__ == "__main__":
    unittest.main()
