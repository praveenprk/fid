"""Tests for decisions.py: replays against brute force, the curve reading, the knob's prediction. No network.

    python3 -m unittest test_decisions      (from calibration/)
"""

import json
import math
import random
import unittest

import decisions as dc
import fidcal

DAY = fidcal.DAY


def brute_changes(e, ttl, step=600.0):
    """Replay a change history on a fine grid: staleness sampled every `step` seconds."""
    checks, stale, t = 0, 0.0, e.start
    while t < e.end:
        i = dc._value_index(e, t)
        held = e.values[i]
        interval = ttl(e.state(t, e.times[i]) if e.state else None) if callable(ttl) else ttl
        nxt = e.end if math.isinf(interval) else min(t + max(interval, dc.MIN_TTL), e.end)
        u = t
        while u < nxt:
            if e.values[dc._value_index(e, u)] != held:
                stale += min(step, nxt - u)
            u += step
        checks += 1
        t = nxt
    return checks, stale


def random_changes(rng, n=12, span=400 * DAY):
    times = sorted(rng.uniform(0, span) for _ in range(n))
    times = [0.0] + [round(t / 600) * 600 for t in times]
    values = [rng.choice("abc") for _ in times]
    return dc.Changes("g", times, values, 0.0, span)


class Replay(unittest.TestCase):
    def test_fixed_shortcut_matches_brute_force(self):
        rng = random.Random(1)
        for _ in range(20):
            e = random_changes(rng)
            for ttl in (3 * 3600.0, 2 * DAY, 45 * DAY, 1000 * DAY, math.inf):
                checks, stale, known, _, _ = dc.simulate(e, ttl)
                bc, bs = brute_changes(e, ttl)
                self.assertEqual(checks, bc)
                self.assertAlmostEqual(stale, bs, delta=1e-6 * known + 1)
                self.assertEqual(known, e.end - e.start)

    def test_state_policy_matches_brute_force(self):
        rng = random.Random(2)
        state = lambda t, since: "fresh" if t - since < 20 * DAY else "old"
        ttl = lambda s: {"fresh": 2 * DAY, "old": 30 * DAY}[s]
        for _ in range(20):
            e = random_changes(rng)
            e.state = state
            bc, bs = brute_changes(e, ttl)
            for boundaries in (None, (20 * DAY,)):  # undeclared: every check replayed; declared: runs skipped
                e.boundaries = boundaries
                checks, stale, _, _, _ = dc.simulate(e, ttl, model=lambda s: (1.0, 10 * DAY, 0.0, 0.0, 0.0))
                self.assertEqual(checks, bc)
                self.assertAlmostEqual(stale, bs, delta=1)

    def test_skipped_checks_keep_the_prediction(self):
        e = dc.Changes("g", [0.0, 100 * DAY], ["a", "b"], 0.0, 200 * DAY)
        model = lambda s: (0.9, 5 * DAY, 0.1, 0.0, 0.0)
        fast = dc.simulate(e, 3600.0, model=model)
        e.state, e.boundaries = (lambda t, since: None), None  # forces every check to be replayed
        slow = dc.simulate(e, 3600.0, model=model)
        self.assertEqual(fast[:3], slow[:3])
        self.assertEqual(fast[4], slow[4])
        self.assertAlmostEqual(fast[3], slow[3], delta=1e-6 * fast[3])

    def test_series(self):
        # 0..9 then 20..29: a check holding 0 is wrong from step 10 on
        e = dc.Series("g", 60.0, [0] * 10 + [20] * 10, lambda a, b: a == b)
        self.assertEqual(dc.simulate(e, 60.0 * 20)[:4], (1, 600.0, 1200.0, 0.0))
        self.assertEqual(dc.simulate(e, 60.0 * 5)[:4], (4, 0.0, 1200.0, 0.0))
        self.assertEqual(dc.simulate(e, 60.0 * 8)[:2], (3, 360.0))  # checks at 0, 8 (holds 0, wrong 10–15), 16

    def test_series_unknown_samples(self):
        e = dc.Series("g", 60.0, [1, None, None, 2, 2], lambda a, b: a == b)
        checks, stale, known, _, _ = dc.simulate(e, math.inf)
        self.assertEqual((checks, stale, known), (1, 120.0, 180.0))
        # a check that lands on an unknown sample waits for the next known one
        self.assertEqual(dc.simulate(e, 60.0)[0], 3)

    def test_fixed_grid_reuses_series_replays(self):
        e = dc.Series("g", 3600.0, list(range(50)), lambda a, b: a == b)
        grid = dc.fixed_grid([e])
        for t in dc.T_GRID:
            self.assertEqual(grid[t][0], dc.simulate(e, t))


class ByState(unittest.TestCase):
    def test_states_add_up(self):
        rng = random.Random(4)
        state = lambda t, since: "fresh" if t - since < 20 * DAY else "old"
        for _ in range(10):
            e = random_changes(rng)
            e.state, e.boundaries = state, (20 * DAY,)
            for policy in (5 * DAY, (lambda s: 2 * DAY if s == "fresh" else 30 * DAY)):
                checks, stale, known, _, by_state = dc.simulate(e, policy, model=lambda s: (1.0, DAY, 0.0, 0.0, 0.0))
                self.assertAlmostEqual(sum(v[0] for v in by_state.values()), stale, delta=1e-6)
                self.assertAlmostEqual(sum(v[1] for v in by_state.values()), known, delta=1e-6)
            # time in each state does not depend on the policy
            a = dc.simulate(e, 5 * DAY)[4]
            b = dc.simulate(e, lambda s: DAY, model=lambda s: (1.0, DAY, 0.0, 0.0, 0.0))[4]
            for st in a:
                self.assertAlmostEqual(a[st][1], b[st][1], delta=1e-6)

    def test_worst_state(self):
        # moving ships are always stale, moored ones never: the average hides it, the worst state does not
        rows = [(1, 10.0, 100.0, 0.0, {"underway": [10.0, 10.0], "moored": [0.0, 90.0]})]
        s = dc.summarize(rows)
        self.assertAlmostEqual(s["stale"], 0.1)
        self.assertAlmostEqual(s["worst"], 1.0)

    def test_series_state_is_the_current_one(self):
        e = dc.Series("g", 60.0, [("m", 0)] * 3 + [("u", 5)] * 3, lambda a, b: a == b, lambda v: v[0])
        by_state = dc.simulate(e, math.inf)[4]
        self.assertEqual(by_state, {"m": [0.0, 180.0], "u": [180.0, 180.0]})


class Prediction(unittest.TestCase):
    def test_integral_matches_sum(self):
        model = (0.98, 5 * DAY, 0.2, 0.3, 0.5 * DAY)
        length, n = 12 * DAY, 100000
        numeric = sum(1 - dc.fid_curve(model, (k + 0.5) * length / n) for k in range(n)) * length / n
        self.assertAlmostEqual(dc.predicted_stale_time(model, length), numeric, delta=1e-6 * length)
        self.assertAlmostEqual(dc.predicted_stale_time((0.9, math.inf, 0.0, 0.0, 0.0), 10.0), 1.0)

    def test_honest_on_its_own_world(self):
        # Values that change as a Poisson process with half-life h: fid's prediction is the expected staleness
        rng = random.Random(3)
        h = 10 * DAY
        rate = math.log(2) / h
        rows = []
        for _ in range(300):
            t, times = 0.0, [0.0]
            while t < 2000 * DAY:
                t += rng.expovariate(rate)
                times.append(t)
            e = dc.Changes("g", times, list(range(len(times))), 0.0, 2000 * DAY)
            rows.append(dc.simulate(e, lambda s: 7 * DAY, model=lambda s: (1.0, h, 0.0, 0.0, 0.0)))
        s = dc.summarize(rows)
        self.assertAlmostEqual(s["predicted"], s["stale"], delta=0.01)


class Curves(unittest.TestCase):
    def test_rate_for(self):
        pts = [(1.0, 0.40), (10.0, 0.10), (100.0, 0.01)]
        self.assertAlmostEqual(dc.rate_for(pts, 0.10), 10.0)
        self.assertAlmostEqual(dc.rate_for(pts, 0.25), math.sqrt(10.0))  # halfway in log rate
        self.assertIsNone(dc.rate_for(pts, 0.001))
        self.assertEqual(dc.rate_for(pts, 0.5), 1.0)

    def test_verdict(self):
        self.assertEqual(dc.verdict({"ratio": 0.5, "low": 0.4, "high": 0.6}), "fid better")
        self.assertEqual(dc.verdict({"ratio": 1.0, "low": 0.9, "high": 1.1}), "tie")
        self.assertEqual(dc.verdict({"ratio": 1.5, "low": 1.2, "high": 1.8}), "fid worse")
        self.assertEqual(dc.verdict({"ratio": None, "low": None, "high": None}), "not reached")


class Ledger(unittest.TestCase):
    def test_ledger_rows(self):
        e = dc.Series("g", 3600.0, [1, 1, 2, None, 2], lambda a, b: a == b)
        rows = list(dc.ledger_rows(e, [0, 3600.0, 2 * 3600.0, 3 * 3600.0], 3600.0))
        self.assertIn(("g", 0, True, None), rows)
        self.assertIn(("g", 2 * 3600.0, False, None), rows)
        # 3h later: from sample 0 lands on the unknown sample 3; only sample 1 → 4 counts
        self.assertEqual([r for r in rows if r[1] == 3 * 3600.0], [("g", 3 * 3600.0, False, None)])

    def test_ledger_gives_calibrate_both_halves(self):
        # Training entities all sit on one side of fidcal's split, which fid calibrate shares
        import csv, os, tempfile
        train = [f"vessel{i}" for i in range(200) if fidcal.split_train(f"vessel{i}")]
        path = os.path.join(tempfile.mkdtemp(), "ledger.csv")
        dc.write_ledger(path, ((e, 0, True, None) for e in train), with_state=False)
        with open(path) as f:
            halves = {fidcal.split_train(row["entity"]) for row in csv.DictReader(f)}
        self.assertEqual(halves, {True, False})

    def test_history_split(self):
        e = dc.Series("g", 60.0, list(range(100)), lambda a, b: a == b)
        h, rest = dc.history_split(e)
        self.assertEqual((len(h.values), len(rest.values)), (25, 75))
        self.assertEqual(rest.values[0], 25)
        c = dc.Changes("g", [0.0, 50.0], ["a", "b"], 0.0, 100.0)
        h, rest = dc.history_split(c)
        self.assertEqual((h.start, h.end, rest.start, rest.end), (0.0, 25.0, 25.0, 100.0))
        # the scored part still knows the value in force at its start
        self.assertEqual(rest.values[dc._value_index(rest, rest.start)], "a")


@unittest.skipIf(dc.fid is None, "the fid Python package is not installed")
class AgainstFid(unittest.TestCase):
    """The benchmark's model maths must be fid's."""

    def test_prediction_matches_fid(self):
        for h, b, share, hf in ((DAY, 0.1, 0.0, 0.0), (80 * DAY, 0.1, 0.3, 3 * DAY)):
            fast = {"share": share, "half_life": f"{hf}s"} if share else None
            for length in (3600.0, 5 * DAY, 200 * DAY):
                ours = dc.predicted_stale_time((0.99, h, b, share, hf), length) / length
                theirs = dc.fid.mean_staleness(0.99, h, length, baseline=b, fast=fast)
                self.assertAlmostEqual(ours, theirs, delta=1e-9)
                self.assertAlmostEqual(dc.fid_curve((0.99, h, b, share, hf), length),
                                       dc.fid.fidelity(0.99, length, h, baseline=b, fast=fast), delta=1e-12)

    def test_echo_prediction_is_fids(self):
        echo = {"period": "24h", "strength": 0.31, "width": "3.75h", "fade": "345h"}
        model = (1.0, 2.4 * 3600, 0.13, 0.0, 0.0, json.dumps(echo, sort_keys=True))
        for age in (0.0, 12 * 3600.0, 24 * 3600.0):
            self.assertEqual(dc.fid_curve(model, age), dc.fid.fidelity(1.0, age, 2.4 * 3600, baseline=0.13, echo=echo))
        length = 30 * 3600.0
        self.assertAlmostEqual(dc.predicted_stale_time(model, length) / length,
                               dc.fid.mean_staleness(1.0, 2.4 * 3600, length, baseline=0.13, echo=echo), delta=1e-12)

    def test_policy_reads_the_registry(self):
        import os, tempfile
        d = tempfile.mkdtemp()
        path = os.path.join(d, "r.yaml")
        with open(path, "w") as f:
            f.write("pkg:\n  states:\n    quiet: { half_life: 80d, baseline: 0.1, source_fidelity: 0.99, fast: { share: 0.3, half_life: 3d } }\n")
        policy = dc.FidPolicy(path, "pkg", {(dc.ledger_id("requests"), "quiet"): 0.25})
        self.assertEqual(policy.baseline("requests", "quiet"), 0.25)
        self.assertEqual(policy.baseline("numpy", "quiet"), 0.1)
        model, ttls, every, guarded = policy.decisions("quiet", 0.1)
        self.assertEqual(model, (0.99, 80 * DAY, 0.1, 0.3, 3 * DAY, None))
        # fid's TTL is where its curve crosses the floor; guarded never later than either
        for floor, t in ttls.items():
            if math.isfinite(t) and 0 < t:
                self.assertAlmostEqual(dc.fid_curve(model, t), floor, delta=1e-6)
        for p in dc.PRICES:
            self.assertLessEqual(guarded[p], min(every[p], ttls[dc.GUARD_FLOOR]) + 1e-3)


if __name__ == "__main__":
    unittest.main()
