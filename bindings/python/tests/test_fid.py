"""Tests for the fid Python package. Run: python -m unittest discover -s tests"""

import math
import os
import tempfile
import unittest
import warnings
from datetime import datetime, timedelta, timezone

import fid

EXAMPLES = os.path.join(os.path.dirname(__file__), "..", "..", "..", "examples")
T0 = datetime(2026, 1, 1, tzinfo=timezone.utc)


class Scoring(unittest.TestCase):
    def test_parse_half_life(self):
        self.assertEqual(fid.parse_half_life("6h"), 21600.0)
        self.assertTrue(math.isinf(fid.parse_half_life("infinite")))
        with self.assertRaises(ValueError):
            fid.parse_half_life("7days")

    def test_source_fidelity(self):
        self.assertEqual(fid.source_fidelity("instrument"), 0.97)
        self.assertIsNone(fid.source_fidelity("unknown"))
        with self.assertRaises(ValueError):
            fid.source_fidelity("human")

    def test_fidelity_matches_formula(self):
        # 0.70 + (0.90 − 0.70) × 0.5
        self.assertAlmostEqual(fid.fidelity(0.9, "1d", "1d", baseline=0.7), 0.8)
        self.assertAlmostEqual(fid.fidelity(0.9, timedelta(days=1), 86400, noise=[0.1]), 0.9 * 0.9 * 0.5)
        self.assertAlmostEqual(fid.fidelity(0.9, "100y", None), 0.9)

    def test_time_to_floor(self):
        self.assertEqual(fid.time_to_floor(0.8, "1d", 0.4), timedelta(days=1))
        self.assertIsNone(fid.time_to_floor(0.97, "2.9h", 0.70, baseline=0.73))
        self.assertEqual(fid.time_to_floor(0.5, "1d", 0.7), timedelta(0))


class Envelopes(unittest.TestCase):
    def test_stamp_check_expire(self):
        a = fid.stamp({"ceo": "Ada"}, source="registry", source_class="database", half_life="1d", now=T0)
        self.assertEqual(a["content"], {"ceo": "Ada"})
        self.assertEqual(a["fid"]["source_class"], "database")
        self.assertEqual(a["fid"]["fid_at_stamp"], 0.9)

        fresh = fid.check(a, floor=0.5, now=T0)
        self.assertIs(fresh.status, fid.Status.SIGNAL)
        self.assertEqual(fresh.status, 0)
        later = fid.check(a, floor=0.5, now=T0 + timedelta(days=1))
        self.assertIs(later.status, fid.Status.NOISE)
        self.assertAlmostEqual(later.fidelity, 0.45)
        self.assertEqual(later.age, timedelta(days=1))

        self.assertEqual(fid.expires_at(a, 0.45), T0 + timedelta(days=1))

    def test_pipeline_restamp(self):
        a = fid.stamp({"v": 1}, source="sensor-12", source_class="instrument", half_life="infinite", now=T0)
        b = fid.stamp(a, source="normalize.py", noise=0.05, now=T0)
        c = fid.stamp(b, source="aggregate.py", noise=0.08, now=T0)
        self.assertEqual([t["step"] for t in c["fid"]["transforms"]], ["normalize.py", "aggregate.py"])
        self.assertAlmostEqual(fid.check(c, now=T0).fidelity, 0.97 * 0.95 * 0.92)
        self.assertIn("aggregate.py", fid.chain(c, now=T0))

    def test_text_content(self):
        a = fid.stamp("plain text", source="x", source_class="hearsay", now=T0)
        self.assertEqual(a["content"], "plain text")

    def test_unknown_source(self):
        a = fid.stamp({}, source="mystery", now=T0)
        self.assertIs(fid.check(a, now=T0).status, fid.Status.UNKNOWN)
        with self.assertRaises(fid.IndeterminateError):
            fid.expires_at(a, 0.5)

    def test_never_expires(self):
        a = fid.stamp({}, source="metar", source_class="instrument", half_life="2.9h", baseline=0.73, now=T0)
        self.assertIsNone(fid.expires_at(a, 0.7))

    def test_tampered_content_is_unknown(self):
        a = fid.stamp({"v": 1}, source="s", source_class="database", now=T0)
        a["content"]["v"] = 2
        self.assertFalse(fid.check(a, now=T0).integrity_ok)
        self.assertIs(fid.check(a, now=T0).status, fid.Status.UNKNOWN)

    def test_restamp_source_class_warns(self):
        a = fid.stamp({}, source="s", source_class="database", now=T0)
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            fid.stamp(a, source="step", source_class="hearsay", now=T0)
        self.assertTrue(any("ignored" in str(w.message) for w in caught))

    def test_errors(self):
        with self.assertRaises(ValueError):
            fid.stamp({}, source="x", noise=1.5)
        with self.assertRaises(ValueError):
            fid.check({"not": "stamped"})
        with self.assertRaises(ValueError):
            fid.stamp({}, source="x", now=datetime(2026, 1, 1))  # naive


class Registries(unittest.TestCase):
    def test_half_life_states(self):
        path = os.path.join(EXAMPLES, "half-lives.yaml")
        h, b = fid.half_life(path, "vessel_position", "underway")
        self.assertTrue(h.endswith("m"))
        with self.assertRaises(ValueError):
            fid.half_life(path, "vessel_position")

    def test_noise_conditions(self):
        path = os.path.join(EXAMPLES, "noise-model.yaml")
        self.assertGreater(fid.noise(path, "flight_leg", "previous_leg_late"), fid.noise(path, "flight_leg"))


class Schedules(unittest.TestCase):
    def test_recheck_at(self):
        now = datetime(2026, 1, 1, tzinfo=timezone.utc)
        s = fid.stamp({"latest": "2.32.3"}, source="pypi", source_class="instrument", source_fidelity=1.0,
                      half_life="80d", baseline=0.1, fast={"share": 0.3, "half_life": "3d"}, now=now)
        self.assertEqual(s["fid"]["fast"], {"share": 0.3, "half_life": "3d"})
        cheap, dear = fid.recheck_at(s, price=1e-4), fid.recheck_at(s, price=1e-2)
        self.assertLess(cheap, dear)
        self.assertIsNone(fid.recheck_at(s, price=1e6))
        # the floor caps it
        self.assertEqual(fid.recheck_at(s, price=1e-2, floor=0.99), fid.expires_at(s, 0.99))
        self.assertEqual(fid.recheck_at(s, price=1e-2, floor=0.5), dear)
        at = fid.recheck_at(s, staleness=0.05)
        every = at - now
        self.assertAlmostEqual(fid.mean_staleness(1.0, "80d", every, baseline=0.1, fast={"share": 0.3, "half_life": "3d"}), 0.05, delta=1e-6)
        with self.assertRaises(ValueError):
            fid.recheck_at(s)
        with self.assertRaises(fid.IndeterminateError):
            fid.recheck_at(fid.stamp({}, source="x", half_life="1d"), price=1e-3)

    def test_registry_fast_share(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "r.yaml")
            with open(path, "w") as f:
                f.write("pkg: { half_life: 80d, fast: { share: 0.3, half_life: 3d } }\n")
            self.assertEqual(fid.registry(path, "pkg")["fast"], {"share": 0.3, "half_life": "3d"})


class Calibration(unittest.TestCase):
    def ledger(self):
        # Half wrong after 10 days, exactly
        for day, n_ok in [(0, 100), (5, 71), (10, 50), (20, 25), (40, 6)]:
            for i in range(100):
                yield (f"claim-{day}-{i}", f"{day}d", i < n_ok)

    def test_calibrate(self):
        cal = fid.calibrate(self.ledger(), "policy_claim")
        self.assertEqual(len(cal.fits), 1)
        self.assertAlmostEqual(cal.fits[0]["half_life_seconds"] / 86400, 10, delta=1)
        self.assertIn("policy_claim:", cal.yaml)
        self.assertIn("held-out Brier", cal.report)

    def test_measured_source_fidelity_reaches_the_label(self):
        cal = fid.calibrate(self.ledger(), "policy_claim")
        self.assertEqual(cal.fits[0]["fresh_checks"], 100)
        self.assertEqual(cal.fits[0]["source_fidelity"], 1.0)
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "half-lives.yaml")
            with open(path, "w") as f:
                f.write(cal.yaml)
            entry = fid.registry(path, "policy_claim")
        self.assertEqual(entry["source_fidelity"], 1.0)
        stamped = fid.stamp({"clause": 1}, source="crm", source_class="database", **entry)
        self.assertEqual(stamped["fid"]["source_fidelity"], 1.0)
        self.assertEqual(fid.check(stamped, 0.95).source, 1.0)

    def test_calibrate_ranges(self):
        cal = fid.calibrate(self.ledger(), "policy_claim")
        lo, hi = cal.fits[0]["half_life_range_seconds"]
        self.assertLessEqual(lo, cal.fits[0]["half_life_seconds"])
        self.assertGreaterEqual(hi, cal.fits[0]["half_life_seconds"])
        self.assertIn("# 95% range", cal.yaml)
        with self.assertRaises(ValueError):
            fid.calibrate(self.ledger(), "policy_claim", max_spread=1.0)

    def test_drift(self):
        cal = fid.calibrate(self.ledger(), "policy_claim")
        faster = [(f"n-{d}-{i}", f"{d}d", i < k) for d, k in [(0, 100), (5, 50), (10, 25), (20, 6), (40, 0)] for i in range(100)]
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "half-lives.yaml")
            with open(path, "w") as f:
                f.write(cal.yaml)
            self.assertEqual(fid.drift(self.ledger(), "policy_claim", path).verdict, "CURRENT")
            result = fid.drift(faster, "policy_claim", path)
        self.assertEqual(result.verdict, "DRIFTED")
        self.assertLess(result.states[0]["p_value"], 0.05)
        self.assertIn("recalibrate", result.report)

    def test_calibrate_states_and_mappings(self):
        rows = [{"entity": e, "age": a, "correct": c, "state": "draft"} for e, a, c in self.ledger()]
        cal = fid.calibrate(rows, "policy_claim")
        self.assertEqual(cal.fits[0]["state"], "draft")
        self.assertIn("states:", cal.yaml)

    def test_ages(self):
        with self.assertRaises(ValueError):
            fid.calibrate([("a", -1, True)], "t")
        with self.assertRaises(ValueError):
            fid.calibrate([("a", "7days", True)], "t")

    def test_calibrate_too_little_data(self):
        with self.assertRaises(ValueError):
            fid.calibrate([("a", 1, True), ("b", 2, False)], "t")


if __name__ == "__main__":
    unittest.main()
