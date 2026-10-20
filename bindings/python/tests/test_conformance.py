"""The fid Python package against conformance/vectors.json (see conformance/README.md)."""

import json
import math
import os
import unittest
from datetime import datetime, timedelta

import fid
from fid import _fid

VECTORS = os.path.join(os.path.dirname(__file__), "..", "..", "..", "conformance", "vectors.json")

with open(VECTORS, encoding="utf-8") as f:
    V = json.load(f)
TOL = V["tolerance"]


def when(text):
    return datetime.fromisoformat(text.replace("Z", "+00:00"))


class Conformance(unittest.TestCase):
    def test_durations(self):
        for c in V["parse_duration"]:
            with self.subTest(c["input"]):
                if "error" in c:
                    with self.assertRaises(ValueError):
                        _fid.parse_duration(c["input"])
                else:
                    self.assertAlmostEqual(_fid.parse_duration(c["input"]), c["seconds"], delta=1e-6)

    def test_half_lives(self):
        for c in V["parse_half_life"]:
            with self.subTest(c["input"]):
                if "error" in c:
                    with self.assertRaises(ValueError):
                        fid.parse_half_life(c["input"])
                elif c["seconds"] is None:
                    self.assertTrue(math.isinf(fid.parse_half_life(c["input"])))
                else:
                    self.assertAlmostEqual(fid.parse_half_life(c["input"]), c["seconds"], delta=1e-6)

    def test_source_classes(self):
        for c in V["source_fidelity"]:
            with self.subTest(c["class"]):
                if "error" in c:
                    with self.assertRaises(ValueError):
                        fid.source_fidelity(c["class"])
                else:
                    self.assertEqual(fid.source_fidelity(c["class"]), c["fidelity"])

    def test_fidelity(self):
        for c in V["fidelity"]:
            with self.subTest(c["name"]):
                # Through _fid: the public fidelity() takes an age of zero or more, and one vector is negative
                got = _fid.fidelity(c["source"], c["age_seconds"], c["half_life_seconds"], c["noise"], c["baseline"])
                self.assertAlmostEqual(got, c["fidelity"], delta=TOL)

    def test_time_to_floor(self):
        for c in V["time_to_floor"]:
            with self.subTest(c["name"]):
                got = fid.time_to_floor(c["fidelity"], c["half_life_seconds"], c["floor"], baseline=c["baseline"])
                if c["seconds"] is None:
                    self.assertIsNone(got)
                else:
                    # timedelta resolution is one microsecond
                    self.assertAlmostEqual(got.total_seconds(), c["seconds"], delta=1e-5)

    def test_echo(self):
        for c in V["echo"]:
            with self.subTest(c["name"]):
                got = _fid.fidelity(c["source"], c["age_seconds"], c["half_life_seconds"], c["noise"], c["baseline"], json.dumps(c["echo"]))
                self.assertAlmostEqual(got, c["fidelity"], delta=TOL)
        for c in V["echo_time_to_floor"]:
            with self.subTest(c["name"]):
                got = fid.time_to_floor(c["fidelity"], c["half_life_seconds"], c["floor"], baseline=c["baseline"], echo=c["echo"])
                if c["seconds"] is None:
                    self.assertIsNone(got)
                else:
                    self.assertAlmostEqual(got.total_seconds(), c["seconds"], delta=1e-5)
        for c in V["echo_errors"]:
            with self.subTest(c["error"]):
                with self.assertRaises(ValueError):
                    fid.fidelity(0.9, 3600, 3600, echo=c["echo"])

    def test_fast(self):
        for c in V["fast"]:
            with self.subTest(c["name"]):
                got = _fid.fidelity(c["fidelity_at_measurement"], c["age_seconds"], c["half_life_seconds"], [], c["baseline"],
                                    None if c["echo"] is None else json.dumps(c["echo"]), json.dumps(c["fast"]))
                self.assertAlmostEqual(got, c["fidelity"], delta=TOL)
        for c in V["fast_time_to_floor"]:
            with self.subTest(c["name"]):
                got = fid.time_to_floor(c["fidelity"], c["half_life_seconds"], c["floor"], baseline=c["baseline"], echo=c["echo"], fast=c["fast"])
                if c["seconds"] is None:
                    self.assertIsNone(got)
                else:
                    self.assertAlmostEqual(got.total_seconds(), c["seconds"], delta=1e-5)
        for c in V["fast_errors"]:
            with self.subTest(c["error"]):
                with self.assertRaises(ValueError):
                    fid.fidelity(0.9, 3600, c["half_life_seconds"], fast=c["fast"])

    def test_schedule(self):
        for c in V["schedule"]:
            with self.subTest(c["name"]):
                kw = dict(baseline=c["baseline"], echo=c["echo"], fast=c["fast"])
                got = fid.recheck_interval(c["fidelity"], c["half_life_seconds"], price=c["price"], **kw)
                if c["seconds"] is None:
                    self.assertIsNone(got)
                else:
                    cost = lambda t: fid.mean_staleness(c["fidelity"], c["half_life_seconds"], t, **kw) + c["price"] * 86400 / t
                    self.assertAlmostEqual(cost(c["seconds"]), c["cost"], delta=1e-7)
                    self.assertLessEqual(cost(got.total_seconds()), c["cost"] + 1e-7)
        for c in V["staleness_target"]:
            with self.subTest(c["name"]):
                got = fid.recheck_interval(c["fidelity"], c["half_life_seconds"], staleness=c["target"],
                                           baseline=c["baseline"], echo=c["echo"], fast=c["fast"])
                if c["seconds"] is None:
                    self.assertIsNone(got)
                else:
                    self.assertAlmostEqual(got.total_seconds() / c["seconds"], 1.0, delta=1e-6)

    def test_envelopes(self):
        for c in V["envelope"]:
            with self.subTest(c["name"]):
                a = fid.check(c["stamped"], c["floor"], now=when(c["now"]))
                self.assertEqual(a.status.name, c["status"])
                self.assertEqual(a.integrity_ok, c["integrity_ok"])
                if c["fidelity"] is None:
                    self.assertIsNone(a.fidelity)
                else:
                    self.assertAlmostEqual(a.fidelity, c["fidelity"], delta=TOL)

                if c["expires_at"] == "unknown":
                    with self.assertRaises(fid.IndeterminateError):
                        fid.expires_at(c["stamped"], c["floor"])
                elif c["expires_at"] == "never":
                    self.assertIsNone(fid.expires_at(c["stamped"], c["floor"]))
                else:
                    self.assertEqual(fid.expires_at(c["stamped"], c["floor"]), when(c["expires_at"]))

    def test_integrity_through_check(self):
        # Content round-trips through Python's json and fid's parser; the hash must survive it
        for c in V["integrity"]:
            with self.subTest(c["canonical"]):
                stamped = {"fid": dict(V["envelope"][0]["stamped"]["fid"], integrity=c["hash"]), "content": c["content"]}
                self.assertTrue(fid.check(stamped, 0.5, now=when(V["envelope"][0]["now"])).integrity_ok)

    def test_ledger_example(self):
        ex = V["ledger_example"]
        path = os.path.join(os.path.dirname(VECTORS), os.path.basename(ex["file"]))
        import csv
        with open(path, newline="") as f:
            rows = [
                (r["entity"], when(r["checked_at"]) - when(r["observed_at"]), r["correct"] == "true")
                for r in csv.DictReader(f)
            ]
        fit = fid.calibrate(rows, type=ex["type"]).fits[0]
        truth = ex["truth"]
        self.assertLessEqual(abs(fit["half_life_seconds"] / (truth["half_life_days"] * 86400) - 1), ex["half_life_within"])
        self.assertLessEqual(abs(fit["baseline"] - truth["baseline"]), ex["baseline_within"])


if __name__ == "__main__":
    unittest.main()
