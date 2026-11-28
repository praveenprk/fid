"""Offline tests for calibration robustness, time splits and observation times."""

import contextlib
import copy
import io
import json
import os
import random
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from unittest import mock

import dailymed_warnings as dw
import eia_demand as ed
import fidcal
import flights_bts as fb
import pypi_latest as pl
import robustness
import ships_ais as sa
import weather_metar as wm
import wikidata_ceo as wc

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


def decay_observations(half_life, recorded_at, n=1000):
    rows = []
    for lag in LAGS:
        correct_n = round(n * 2 ** (-lag / half_life))
        for i in range(n):
            entity = f"{recorded_at}-{lag}-{i}"
            rows.append((entity, lag, i < correct_n, recorded_at))
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

    def test_time_split_is_unchanged_without_opt_in(self):
        rows = observations()
        timed = [(*row, 0 if i < 100 else 2000) for i, row in enumerate(rows)]
        with mock.patch.dict(os.environ, {}, clear=True):
            plain = fidcal.analyze("Synthetic", "fact", rows, LAGS)
            timed_result = fidcal.analyze(
                "Synthetic", "fact", timed, LAGS, time_cutoff=1000
            )
        self.assertEqual(timed_result, plain)
        self.assertNotIn("time_split", timed_result)

    def test_time_split_recovers_early_half_life_and_scores_drift(self):
        half_life = 2 * fidcal.HOUR
        early = decay_observations(half_life, 0)
        drift = early + decay_observations(2 * half_life, 2000)
        no_drift = early + decay_observations(half_life, 2000)
        with mock.patch.dict(os.environ, {"FID_CAL_TIME_SPLIT": "1"}, clear=True):
            drift_result = fidcal.analyze(
                "Synthetic",
                "fact",
                drift,
                LAGS,
                guess=half_life,
                time_cutoff=1000,
            )
            no_drift_result = fidcal.analyze(
                "Synthetic", "fact", no_drift, LAGS, time_cutoff=1000
            )
        split = drift_result["time_split"]
        self.assertAlmostEqual(
            split["fitted_early"]["half_life_s"], half_life, delta=0.05 * half_life
        )
        self.assertEqual(split["cutoff"], "1970-01-01T00:16:40Z")
        self.assertGreater(
            split["brier_late"]["fitted_half_life"],
            no_drift_result["time_split"]["brier_late"]["fitted_half_life"],
        )
        self.assertIn("guessed_half_life", split["brier_late"])

    def test_time_split_counts_every_timed_observation(self):
        rows = observations()
        timed = [(*row, 0 if i < 100 else 2000) for i, row in enumerate(rows)]
        with mock.patch.dict(os.environ, {"FID_CAL_TIME_SPLIT": "1"}, clear=True):
            result = fidcal.analyze("Synthetic", "fact", timed, LAGS, time_cutoff=1000)
        split = result["time_split"]
        self.assertEqual(split["early_observations"], 100)
        self.assertEqual(split["late_observations"], 100)
        self.assertEqual(
            split["early_observations"] + split["late_observations"], len(timed)
        )

    def test_time_split_ignores_three_tuples(self):
        rows = observations()
        timed = [(*row, 0 if i < 100 else 2000) for i, row in enumerate(rows)]
        timed[0] = timed[0][:3]
        with mock.patch.dict(os.environ, {"FID_CAL_TIME_SPLIT": "1"}, clear=True):
            result = fidcal.analyze("Synthetic", "fact", timed, LAGS, time_cutoff=1000)
        split = result["time_split"]
        self.assertEqual(
            split["early_observations"] + split["late_observations"], len(timed) - 1
        )

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

    def test_summary_adds_the_time_split_only_when_present(self):
        result = small_result()
        self.assertNotIn("Time split (fitted", fidcal.summary_markdown(result))
        result["time_split"] = {
            "cutoff": "2024-07-01T00:00:00Z",
            "early_observations": 40,
            "late_observations": 40,
            "fitted_early": {
                "s0": 1.0,
                "half_life_s": 3600.0,
                "baseline": 0.2,
                "half_life_with_baseline_s": 7200.0,
            },
            "brier_late": {
                "fitted_half_life": 0.4,
                "fitted_half_life_with_baseline": 0.3,
                "constant_base_rate": 0.2,
                "guessed_half_life": 0.1,
            },
        }
        markdown = fidcal.summary_markdown(result)
        self.assertIn(
            "Time split (fitted on facts recorded before 2024-07-01T00:00:00Z, scored on later ones)",
            markdown,
        )
        self.assertIn("Half-life fitted on early facts: **1.0h**", markdown)
        self.assertIn("b = 0.20, half-life = **2.0h**", markdown)
        rows = [
            "| fitted half life | 0.4000 |",
            "| fitted half life with baseline | 0.3000 |",
            "| constant base rate | 0.2000 |",
            "| guessed half life | 0.1000 |",
        ]
        self.assertEqual(
            [markdown.index(row) for row in rows],
            sorted(markdown.index(row) for row in rows),
        )


class ObservationTimes(unittest.TestCase):
    def test_wikidata_uses_the_snapshot_not_the_later_check(self):
        spans = [
            (
                datetime(2000, 1, 1, tzinfo=timezone.utc),
                datetime(2020, 1, 1, tzinfo=timezone.utc),
            )
        ]
        rows = list(wc.observations({"company": spans}))
        expected = datetime(2004, 1, 1, tzinfo=timezone.utc).timestamp()
        self.assertGreater(rows[1][1], 0)
        self.assertEqual(rows[0][3], expected)
        self.assertEqual(rows[1][3], expected)

    def test_metar_uses_the_report_hour_for_all_three_facts(self):
        recorded = datetime(2024, 1, 1, 0)
        later = recorded + timedelta(hours=1)
        temp, cat, adverse = wm.observations(
            {"JFK": {recorded: (50.0, "IFR"), later: (51.0, "LIFR")}}
        )
        expected = recorded.replace(tzinfo=timezone.utc).timestamp()
        for rows in (temp, cat, adverse):
            row = next(row for row in rows if row[1] == fidcal.HOUR)
            self.assertEqual(row[3], expected)
            self.assertNotEqual(row[3], later.replace(tzinfo=timezone.utc).timestamp())

    def test_ais_uses_the_actual_report_not_the_later_check(self):
        track = {1: (0.0, 0.0, 2.0), 6: (0.0, 0.0, 2.0)}
        rows = list(sa.observations({"vessel": track}, 1.0, "2023_01_15"))
        row = next(row for row in rows if row[1] == 5 * fidcal.MINUTE)
        expected = datetime(2023, 1, 15, 0, 1, tzinfo=timezone.utc).timestamp()
        checked = datetime(2023, 1, 15, 0, 6, tzinfo=timezone.utc).timestamp()
        self.assertTrue(row[3])
        self.assertEqual(row[4], expected)
        self.assertNotEqual(row[4], checked)

    def test_flight_legs_use_the_flight_date(self):
        flights = [
            {"tail": "N1", "date": "2024-06-30", "dep": 900, "on_time": False},
            {"tail": "N1", "date": "2024-06-30", "dep": 800, "on_time": True},
        ]
        rows = list(fb.leg_observations(flights))
        expected = datetime(2024, 6, 30, tzinfo=timezone.utc).timestamp()
        self.assertEqual([row[1] for row in rows], [0, 1])
        self.assertEqual([row[3] for row in rows], [expected, expected])
        self.assertIsNone(rows[0][4])
        self.assertTrue(rows[1][4])

    def test_pypi_uses_the_snapshot_not_the_later_check(self):
        old = pl.SNAPSHOTS[0] - timedelta(days=400)
        rows = pl.observations({"package": [(old, "1.0")]}, with_recorded_at=True)[
            "all"
        ]
        expected = pl.SNAPSHOTS[0].timestamp()
        self.assertGreater(rows[1][1], 0)
        self.assertEqual(rows[0][3], expected)
        self.assertEqual(rows[1][3], expected)

    def test_dailymed_uses_the_snapshot_not_the_later_check(self):
        snapshot = dw.SNAPSHOTS[0]
        rows = dw.observations(
            {"label": [(snapshot, "warnings")]}, with_recorded_at=True
        )["all"]
        expected = datetime.combine(
            snapshot, datetime.min.time(), tzinfo=timezone.utc
        ).timestamp()
        self.assertGreater(rows[1][1], 0)
        self.assertEqual(rows[0][3], expected)
        self.assertEqual(rows[1][3], expected)
        cutoff = dw.SNAPSHOTS[len(dw.SNAPSHOTS) // 2]
        self.assertEqual(
            dw.TIME_CUTOFF,
            datetime.combine(
                cutoff, datetime.min.time(), tzinfo=timezone.utc
            ).timestamp(),
        )

    def test_eia_uses_the_demand_hour_not_the_later_check(self):
        recorded = datetime(2024, 1, 1, 0)
        later = recorded + timedelta(hours=1)
        all_series = {"A": {recorded: 100.0, later: 100.0}}
        rows = list(ed.study_observations(all_series))
        row = next(row for row in rows if row[1] == fidcal.HOUR)
        self.assertEqual(row[3], recorded.replace(tzinfo=timezone.utc).timestamp())
        self.assertNotEqual(row[3], later.replace(tzinfo=timezone.utc).timestamp())
        legacy = ed.tally(
            ed.count(all_series, lag_hours=ed.STUDY_LAG_HOURS), ed.STUDY_LAG_HOURS
        )
        self.assertEqual(fidcal.Tally().extend(rows).counts, legacy.counts)


class Comparison(unittest.TestCase):
    def test_only_optional_analysis_blocks_may_differ(self):
        committed = [small_result()]
        generated = copy.deepcopy(committed)
        generated[0]["intervals"] = {"ranges": {"half_life_s": [1.0, 2.0]}}
        generated[0]["time_split"] = {"early_observations": 40}
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

    def test_readme_marks_which_experiments_have_a_time_split(self):
        rows = [
            {"slug": "yes", "matches_committed": True, "time_split": True},
            {"slug": "no", "matches_committed": True, "time_split": False},
        ]
        with tempfile.TemporaryDirectory() as directory:
            os.makedirs(os.path.join(directory, "results", "robustness"))
            with mock.patch.object(robustness, "HERE", directory):
                robustness.write_readme(rows, 2, 0)
            with open(
                os.path.join(directory, "results", "robustness", "README.md")
            ) as f:
                markdown = f.read()
        self.assertIn("| yes | 2 | 0 | yes | yes |", markdown)
        self.assertIn("| no | 2 | 0 | no | yes |", markdown)


if __name__ == "__main__":
    unittest.main()
