"""Tests for decisions.py: replays against brute force, the curve reading, the knob's prediction. No network.

    python3 -m unittest test_decisions      (from calibration/)
"""

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
                checks, stale, known, _ = dc.simulate(e, ttl)
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
            checks, stale, _, _ = dc.simulate(e, ttl, model=lambda s: (1.0, 10 * DAY, 0.0))
            bc, bs = brute_changes(e, ttl)
            self.assertEqual(checks, bc)
            self.assertAlmostEqual(stale, bs, delta=1)

    def test_series(self):
        # 0..9 then 20..29: a check holding 0 is wrong from step 10 on
        e = dc.Series("g", 60.0, [0] * 10 + [20] * 10, lambda a, b: a == b)
        self.assertEqual(dc.simulate(e, 60.0 * 20), (1, 600.0, 1200.0, 0.0))
        self.assertEqual(dc.simulate(e, 60.0 * 5), (4, 0.0, 1200.0, 0.0))
        self.assertEqual(dc.simulate(e, 60.0 * 8)[:2], (3, 360.0))  # checks at 0, 8 (holds 0, wrong 10–15), 16

    def test_series_unknown_samples(self):
        e = dc.Series("g", 60.0, [1, None, None, 2, 2], lambda a, b: a == b)
        checks, stale, known, _ = dc.simulate(e, math.inf)
        self.assertEqual((checks, stale, known), (1, 120.0, 180.0))
        # a check that lands on an unknown sample waits for the next known one
        self.assertEqual(dc.simulate(e, 60.0)[0], 3)

    def test_fixed_grid_reuses_series_replays(self):
        e = dc.Series("g", 3600.0, list(range(50)), lambda a, b: a == b)
        grid = dc.fixed_grid([e])
        for t in dc.T_GRID:
            self.assertEqual(grid[t][0], dc.simulate(e, t))


class Prediction(unittest.TestCase):
    def test_integral_matches_sum(self):
        model = (0.98, 5 * DAY, 0.2)
        length, n = 12 * DAY, 100000
        numeric = sum(1 - dc.fid_curve(model, (k + 0.5) * length / n) for k in range(n)) * length / n
        self.assertAlmostEqual(dc.predicted_stale_time(model, length), numeric, delta=1e-6 * length)
        self.assertAlmostEqual(dc.predicted_stale_time((0.9, math.inf, 0.0), 10.0), 1.0)

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
            rows.append(dc.simulate(e, lambda s: 7 * DAY, model=lambda s: (1.0, h, 0.0)))
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


class Fid(unittest.TestCase):
    def test_durations(self):
        self.assertEqual(dc.parse_duration("29.96d"), 29.96 * DAY)
        self.assertAlmostEqual(dc.parse_duration("3mo"), 3 * 30.44 * DAY)
        self.assertEqual(dc.parse_duration("1.9m"), 114.0)
        self.assertTrue(math.isinf(dc.parse_duration("infinite")))

    def test_times(self):
        self.assertAlmostEqual(dc.parse_time("2026-10-20T10:00:00.123456789Z") - dc.parse_time("2026-10-20T10:00:00Z"),
                               0.123456, places=5)

    def test_ledger_rows(self):
        e = dc.Series("g", 3600.0, [1, 1, 2, None, 2], lambda a, b: a == b)
        rows = list(dc.ledger_rows(e, [0, 3600.0, 2 * 3600.0, 3 * 3600.0], 3600.0))
        self.assertIn(("g", 0, True, None), rows)
        self.assertIn(("g", 2 * 3600.0, False, None), rows)
        # 3h later: from sample 0 lands on the unknown sample 3; only sample 1 → 4 counts
        self.assertEqual([r for r in rows if r[1] == 3 * 3600.0], [("g", 3 * 3600.0, False, None)])


if __name__ == "__main__":
    unittest.main()
