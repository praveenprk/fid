"""Offline tests for entity-bootstrap calibration ranges and their runner."""

import contextlib
import copy
import io
import json
import os
import random
import tempfile
import unittest
from unittest import mock

import fidcal
import robustness

LAGS = [0, fidcal.HOUR, 2 * fidcal.HOUR, 4 * fidcal.HOUR, 8 * fidcal.HOUR]


def observations(entity_count=40, identical=False):
    rows = []
    for i in range(entity_count):
        entity = f"entity-{i:02d}"
        strength = (i * 17) % 40
        for j, lag in enumerate(LAGS):
            correct = j < 3 if identical else j == 0 or strength >= j * 8
            rows.append((entity, lag, correct))
    return rows


def small_result():
    return {
        "experiment": "Synthetic",
        "fact": "fact",
        "truth": "still true",
        "entities": 40,
        "observations": 80,
        "curve": [
            {"lag_s": 0, "lag": "0s", "still_correct": 1.0, "n": 40},
            {
                "lag_s": 7200,
                "lag": "2.0h",
                "still_correct": 0.5,
                "n": 40,
            },
        ],
        "empirical_half_life_s": 7200.0,
        "empirical_half_life": "2.0h",
        "fitted": {"s0": 1.0, "half_life_s": 7200.0, "half_life": "2.0h"},
        "shape_error": 0.02,
        "fitted_with_baseline": {
            "s0": 1.0,
            "baseline": 0.2,
            "half_life_s": 7200.0,
            "half_life": "2.0h",
        },
        "shape_error_with_baseline": 0.01,
        "guess": {"label": "test", "half_life_s": 3600.0, "half_life": "1.0h"},
        "brier_test": {
            "fitted_half_life": 0.2,
            "fitted_half_life_with_baseline": 0.21,
            "constant_base_rate": 0.25,
            "guessed_half_life": 0.3,
        },
        "reliability_test": [],
        "reliability_test_with_baseline": [],
    }


LEGACY_SUMMARY = """### Synthetic: fact

- Still correct means: still true
- 40 entities, 80 observations
- Empirical half-life: **2.0h**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **2.0h**, shape error 0.020
- Fitted with baseline (decay to b): b = 0.20, half-life = **2.0h**, shape error 0.010
- Hand-picked guess (test): 1.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life | 0.2000 |
| fitted half life with baseline | 0.2100 |
| constant base rate | 0.2500 |
| guessed half life | 0.3000 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 40 |
| 2.0h | 0.500 | 40 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
"""


class TallyAndAnalysis(unittest.TestCase):
    def test_by_entity_sums_to_counts(self):
        tally = fidcal.Tally().extend(observations())
        combined = {}
        for entity, counts in tally.by_entity.items():
            for (lag, correct), n in counts.items():
                key = (fidcal.split_train(entity), lag, correct)
                combined[key] = combined.get(key, 0) + n
        self.assertEqual(combined, tally.counts)

    def test_analyze_is_unchanged_without_opt_in(self):
        rows = observations()
        with mock.patch.dict(os.environ, {}, clear=True):
            first = fidcal.analyze(
                "Synthetic", "fact", fidcal.Tally().extend(rows), LAGS
            )
            second = fidcal.analyze(
                "Synthetic", "fact", fidcal.Tally().extend(rows), LAGS
            )
        self.assertNotIn("intervals", first)
        self.assertEqual(first, second)

    def test_analyze_opt_in_contains_the_point_estimate(self):
        with mock.patch.dict(
            os.environ,
            {"FID_CAL_RESAMPLES": "4", "FID_CAL_SEED": "0"},
            clear=True,
        ):
            result = fidcal.analyze(
                "Synthetic", "fact", fidcal.Tally().extend(observations()), LAGS
            )
        intervals = result["intervals"]
        self.assertEqual(intervals["resamples"], 4)
        self.assertEqual(intervals["seed"], 0)
        lo, hi = intervals["ranges"]["half_life_s"]
        self.assertLessEqual(lo, result["fitted"]["half_life_s"])
        self.assertLessEqual(result["fitted"]["half_life_s"], hi)


class Bootstrap(unittest.TestCase):
    def test_seed_is_reproducible_and_changes_the_draws(self):
        tally = fidcal.Tally().extend(observations())
        first = fidcal.bootstrap(tally, LAGS, resamples=2, seed=11)
        second = fidcal.bootstrap(tally, LAGS, resamples=2, seed=11)
        different = fidcal.bootstrap(tally, LAGS, resamples=2, seed=12)
        self.assertEqual(first, second)
        self.assertNotEqual(first, different)
        self.assertNotIn("brier_guessed", first["ranges"])
        self.assertNotIn("diff_guessed_minus_constant", first["ranges"])

    def test_every_resample_draws_both_half_sizes(self):
        tally = fidcal.Tally().extend(observations())
        train_size = sum(fidcal.split_train(entity) for entity in tally.entities)
        test_size = len(tally.entities) - train_size
        choice_sizes = []
        real_random = random.Random

        class RecordingRandom:
            def __init__(self, seed):
                self.rng = real_random(seed)

            def choice(self, entities):
                choice_sizes.append(len(entities))
                return self.rng.choice(entities)

        with mock.patch.object(fidcal.random, "Random", RecordingRandom):
            result = fidcal.bootstrap(tally, LAGS, resamples=2, seed=3)
        expected = [train_size] * train_size + [test_size] * test_size
        self.assertEqual(choice_sizes, expected * 2)
        self.assertEqual(result["train_entities"], train_size)
        self.assertEqual(result["test_entities"], test_size)

    def test_identical_entities_have_zero_width_brier_ranges(self):
        tally = fidcal.Tally().extend(observations(identical=True))
        result = fidcal.bootstrap(tally, LAGS, guess=fidcal.HOUR, resamples=2, seed=5)
        for name, (lo, hi) in result["ranges"].items():
            if name.startswith("brier_"):
                self.assertEqual(lo, hi, name)


class Output(unittest.TestCase):
    def test_results_directory_redirect(self):
        slug = "_test_fidcal_robustness_redirect"
        default_dir = os.path.join(os.path.dirname(fidcal.__file__), "results")
        default_files = [
            os.path.join(default_dir, f"{slug}.{extension}")
            for extension in ("json", "md")
        ]
        self.assertFalse(any(os.path.exists(path) for path in default_files))
        with tempfile.TemporaryDirectory() as directory:
            with mock.patch.dict(
                os.environ, {"FID_CAL_RESULTS_DIR": directory}, clear=True
            ):
                with contextlib.redirect_stdout(io.StringIO()):
                    fidcal.write_results(
                        [small_result()], slug, extra_markdown="\nEXTRA"
                    )
            for extension in ("json", "md"):
                path = os.path.join(directory, f"{slug}.{extension}")
                self.assertTrue(os.path.isfile(path))
            with open(os.path.join(directory, f"{slug}.json")) as f:
                self.assertEqual(json.load(f), [small_result()])
            with open(os.path.join(directory, f"{slug}.md")) as f:
                self.assertTrue(f.read().endswith("\nEXTRA"))
            self.assertFalse(any(os.path.exists(path) for path in default_files))

    def test_summary_is_legacy_without_intervals_and_annotated_with_them(self):
        result = small_result()
        self.assertEqual(fidcal.summary_markdown(result), LEGACY_SUMMARY)
        with_intervals = copy.deepcopy(result)
        with_intervals["intervals"] = {
            "resamples": 200,
            "seed": 0,
            "train_entities": 23,
            "test_entities": 17,
            "ranges": {
                "empirical_half_life_s": [3600.0, 10800.0],
                "half_life_s": [3600.0, 10800.0],
                "half_life_with_baseline_s": [3600.0, 10800.0],
                "baseline": [0.1, 0.3],
                "brier_fitted": [0.18, 0.22],
                "brier_fitted_with_baseline": [0.19, 0.23],
                "brier_constant": [0.23, 0.27],
                "brier_guessed": [0.28, 0.32],
                "diff_baseline_minus_constant": [-0.05, 0.01],
                "diff_guessed_minus_constant": [0.02, 0.08],
            },
        }
        markdown = fidcal.summary_markdown(with_intervals)
        self.assertIn("**2.0h** (95%: 1.0h–3.0h)", markdown)
        self.assertIn("b = 0.20 (95%: 0.10–0.30)", markdown)
        self.assertIn(
            "| Predictor (held-out half) | Brier score (lower is better) | 95% range |",
            markdown,
        )
        self.assertIn("| fitted half life | 0.2000 | 0.1800–0.2200 |", markdown)
        self.assertIn(
            "fitted_with_baseline − constant_base_rate (95% range): -0.0500–0.0100",
            markdown,
        )
        self.assertIn(
            "guessed_half_life − constant_base_rate (95% range): 0.0200–0.0800",
            markdown,
        )
        self.assertIn(
            "Few held-out entities (n = 17): the ranges understate the uncertainty.",
            markdown,
        )


class Comparison(unittest.TestCase):
    def test_only_intervals_may_differ(self):
        committed = [small_result()]
        generated = copy.deepcopy(committed)
        generated[0]["intervals"] = {"ranges": {"half_life_s": [1.0, 2.0]}}
        self.assertEqual(
            robustness.compare_results(committed, generated),
            {"matches_committed": True},
        )

    def test_small_score_change_is_reported_with_its_path(self):
        committed = [small_result()]
        generated = copy.deepcopy(committed)
        generated[0]["brier_test"]["constant_base_rate"] += 1e-6
        comparison = robustness.compare_results(committed, generated)
        self.assertFalse(comparison["matches_committed"])
        self.assertIn(
            "[0].brier_test.constant_base_rate", comparison["differing_paths"]
        )


if __name__ == "__main__":
    unittest.main()
