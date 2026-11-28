"""Shared analysis for fid calibration experiments.

Every experiment reduces to the same shape: a set of observations, each
an (entity, lag_seconds, still_correct) triple, counted in a Tally. "Entity" is whatever was
observed (a company, a weather station, a vessel). "still_correct" says
whether the stored fact, taken at lag 0, still matched reality `lag`
seconds later.

From those triples we estimate:

  * the empirical survival curve S(lag) = fraction still correct,
  * the empirical half-life (where S crosses 0.5),
  * the best-fitting fid model  S(lag) = s0 × 0.5^(lag / h),
  * how far the real curve departs from that exponential shape,
  * out-of-sample calibration: fit on half the entities, score the other
    half with the Brier score and a reliability table, against two
    baselines — the hand-picked half-life from examples/half-lives.yaml
    and a constant predictor that ignores age entirely.

Standard library only, so every experiment runs with a bare python3.
"""

import hashlib
import json
import math
import os
import random
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone

USER_AGENT = "fid-calibration/0.1 (https://github.com/praveenprk/fid)"

MINUTE = 60.0
HOUR = 3600.0
DAY = 86400.0
YEAR = 365.25 * DAY


def default_cache_dir():
    """Where downloaded data goes. Override with FID_CAL_CACHE."""
    here = os.path.dirname(os.path.abspath(__file__))
    return os.environ.get("FID_CAL_CACHE", os.path.join(here, ".cache"))


def download(url, path, data=None, attempts=5, pause=3.0, deadline=900, label=None):
    """Download `url` to `path` unless it is already there. Returns `path`.

    `label` is logged instead of the URL, for URLs that carry a key.

    Public archives rate-limit, so: pause before each request and retry
    with exponential backoff on connection errors. A stalled transfer can
    trickle bytes forever without tripping the socket timeout, so each
    attempt also has an overall deadline in seconds.
    """
    if os.path.exists(path) and os.path.getsize(path) > 0:
        return path
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".part"
    log(f"downloading {label or url[:120]}")
    for attempt in range(attempts):
        time.sleep(pause * 2**attempt)
        req = urllib.request.Request(url, data=data, headers={"User-Agent": USER_AGENT})
        give_up = time.monotonic() + deadline
        try:
            with (
                urllib.request.urlopen(req, timeout=120) as resp,
                open(tmp, "wb") as out,
            ):
                while True:
                    chunk = resp.read(1 << 16)
                    if not chunk:
                        break
                    out.write(chunk)
                    if time.monotonic() > give_up:
                        raise TimeoutError(f"no complete download within {deadline}s")
            os.replace(tmp, path)
            return path
        except (urllib.error.URLError, ConnectionError, TimeoutError) as e:
            if attempt == attempts - 1:
                raise
            log(f"  {e}; retrying")


def log(msg):
    print(msg, file=sys.stderr, flush=True)


def fmt_duration(seconds):
    """Human-readable duration in the units half-lives.yaml uses."""
    if seconds is None:
        return "—"
    if math.isinf(seconds):
        return "infinite"
    for unit, size in (("y", YEAR), ("d", DAY), ("h", HOUR), ("m", MINUTE)):
        if seconds >= size:
            value = seconds / size
            return f"{value:.1f}{unit}" if value < 10 else f"{value:.0f}{unit}"
    return f"{seconds:.0f}s"


def split_train(entity):
    """Deterministic 50/50 split by entity, so no entity is in both halves."""
    return hashlib.sha256(str(entity).encode()).digest()[0] % 2 == 0


# ── observations ─────────────────────────────────────────────────────────


class Tally:
    """Counts of observations by (train half?, lag, still correct?).

    Every statistic here depends only on those three things, so counting
    is exact and lets an experiment stream millions of observations
    without holding them.
    """

    def __init__(self):
        self.counts = {}
        self.entities = set()
        self._split = {}
        self.by_entity = {}
        self.time_cutoff = None
        self.by_time = {}
        self.time_straddling = 0

    def add(self, entity, lag, correct, recorded_at=None):
        train = self._split.get(entity)
        if train is None:
            train = self._split[entity] = split_train(entity)
            self.entities.add(entity)
            self.by_entity[entity] = {}
        key = (train, lag, bool(correct))
        self.counts[key] = self.counts.get(key, 0) + 1
        entity_key = (lag, bool(correct))
        entity_counts = self.by_entity[entity]
        entity_counts[entity_key] = entity_counts.get(entity_key, 0) + 1
        if recorded_at is not None and self.time_cutoff is not None:
            if recorded_at + lag < self.time_cutoff:
                side = "early"
            elif recorded_at >= self.time_cutoff:
                side = "late"
            else:
                side = None
                self.time_straddling += 1
            if side is not None:
                time_key = (side, lag, bool(correct))
                self.by_time[time_key] = self.by_time.get(time_key, 0) + 1

    def extend(self, observations):
        for observation in observations:
            self.add(*observation)
        return self

    def total(self):
        return sum(self.counts.values())

    def items(self, half=None):
        """(lag, correct, n), optionally only the train (True) or test (False) half."""
        for (train, lag, correct), n in self.counts.items():
            if half is None or train == half:
                yield lag, correct, n


def as_tally(observations, time_cutoff=None):
    if isinstance(observations, Tally):
        if time_cutoff is not None:
            observations.time_cutoff = time_cutoff
        return observations
    tally = Tally()
    tally.time_cutoff = time_cutoff
    return tally.extend(observations)


# ── survival curve ───────────────────────────────────────────────────────


def survival_curve(tally, lags, half=None):
    """Fraction still correct at each lag: a list of (lag, fraction, n)."""
    counts = {lag: [0, 0] for lag in lags}
    for lag, correct, n in tally.items(half):
        c = counts[lag]
        c[0] += n if correct else 0
        c[1] += n
    return [(lag, c[0] / c[1], c[1]) for lag, c in sorted(counts.items()) if c[1] > 0]


def empirical_half_life(curve):
    """First lag where the curve falls to 0.5, linearly interpolated.

    None if it never gets there within the observed lags.
    """
    for (l0, s0, _), (l1, s1, _) in zip(curve, curve[1:]):
        if s0 > 0.5 >= s1:
            if s0 == s1:
                return l1
            return l0 + (s0 - 0.5) * (l1 - l0) / (s0 - s1)
    return None


def exp_model(lag, s0, h, b=0.0):
    """S(lag) = b + (s0 − b) × 0.5^(lag / h).

    b = 0 is fid's current model: decay toward zero. b > 0 decays toward
    a baseline — the rate at which a stale fact is right anyway.
    """
    if h is None or math.isinf(h):
        return s0
    return b + (s0 - b) * 0.5 ** (lag / h)


def _fit_half_life(curve, s0, b):
    """Best half-life for fixed s0 and b. Returns (h, loss)."""
    positive = [lg for lg, _, _ in curve if lg > 0]

    def loss(h):
        return sum(n * (s - exp_model(lg, s0, h, b)) ** 2 for lg, s, n in curve)

    lo, hi = math.log(min(positive) / 100), math.log(max(positive) * 1000)
    best = None
    for _ in range(4):
        grid = [math.exp(lo + (hi - lo) * i / 200) for i in range(201)]
        best = min(grid, key=loss)
        step = (hi - lo) / 200
        lo, hi = math.log(best) - 2 * step, math.log(best) + 2 * step
    # No decay at all fits better than any finite half-life
    if loss(math.inf) <= loss(best):
        return math.inf, loss(math.inf)
    return best, loss(best)


def fit_exponential(curve, with_baseline=False):
    """Fit the decay model by weighted least squares.

    s0 is the fraction correct at lag 0 (fid's source fidelity); h is the
    half-life; b is the baseline, fixed at 0 unless `with_baseline`.
    Grid search, refined — robust and dependency-free. Returns (s0, h, b).
    """
    s0 = curve[0][1] if curve and curve[0][0] == 0 else 1.0
    if not any(lg > 0 for lg, _, _ in curve):
        return s0, math.inf, 0.0
    if not with_baseline:
        h, _ = _fit_half_life(curve, s0, 0.0)
        return s0, h, 0.0
    best = None
    for i in range(100):
        b = i / 100 * s0
        h, loss = _fit_half_life(curve, s0, b)
        if best is None or loss < best[0]:
            best = (loss, h, b)
    return s0, best[1], best[2]


def shape_error(curve, s0, h, b=0.0):
    """Largest gap between the real curve and the fitted model."""
    return max(abs(s - exp_model(lg, s0, h, b)) for lg, s, _ in curve)


# ── calibration ──────────────────────────────────────────────────────────


def brier(predict, tally, half=False):
    total = n_all = 0
    for lag, correct, n in tally.items(half):
        total += n * (predict(lag) - (1.0 if correct else 0.0)) ** 2
        n_all += n
    return total / n_all if n_all else float("nan")


def bootstrap(tally, lags, guess=None, resamples=200, seed=0):
    """Bootstrap 95% ranges by resampling entities within each half."""
    train = sorted(
        (entity for entity in tally.entities if split_train(entity)), key=str
    )
    test = sorted(
        (entity for entity in tally.entities if not split_train(entity)), key=str
    )
    rng = random.Random(seed)
    names = [
        "empirical_half_life_s",
        "half_life_s",
        "half_life_with_baseline_s",
        "baseline",
        "brier_fitted",
        "brier_fitted_with_baseline",
        "brier_constant",
    ]
    if guess is not None:
        names += ["brier_guessed"]
    names += ["diff_baseline_minus_constant"]
    if guess is not None:
        names += ["diff_guessed_minus_constant"]
    samples = {name: [] for name in names}

    for _ in range(resamples):
        resampled = Tally()
        for half, entities in ((True, train), (False, test)):
            for _ in range(len(entities)):
                entity = rng.choice(entities)
                for (lag, correct), n in tally.by_entity[entity].items():
                    key = (half, lag, correct)
                    resampled.counts[key] = resampled.counts.get(key, 0) + n

        curve = survival_curve(resampled, lags)
        train_curve = survival_curve(resampled, lags, half=True)
        s0, h, _ = fit_exponential(train_curve)
        s0b, hb, b = fit_exponential(train_curve, with_baseline=True)
        _s0_all, h_all, _ = fit_exponential(curve)
        _s0b_all, hb_all, b_all = fit_exponential(curve, with_baseline=True)
        train_n = sum(n for _, _, n in resampled.items(True))
        base_rate = sum(n for _, c, n in resampled.items(True) if c) / train_n

        def fitted(lag):
            return exp_model(lag, s0, h)

        def fitted_baseline(lag):
            return exp_model(lag, s0b, hb, b)

        fitted_score = brier(fitted, resampled)
        baseline_score = brier(fitted_baseline, resampled)
        constant_score = brier(lambda _lag: base_rate, resampled)
        values = {
            "empirical_half_life_s": empirical_half_life(curve),
            "half_life_s": h_all,
            "half_life_with_baseline_s": hb_all,
            "baseline": b_all,
            "brier_fitted": fitted_score,
            "brier_fitted_with_baseline": baseline_score,
            "brier_constant": constant_score,
            "diff_baseline_minus_constant": baseline_score - constant_score,
        }
        if guess is not None:
            guessed_score = brier(lambda lag: exp_model(lag, 1.0, guess), resampled)
            values["brier_guessed"] = guessed_score
            values["diff_guessed_minus_constant"] = guessed_score - constant_score
        for name, value in values.items():
            samples[name].append(value)

    ranges = {}
    for name, values in samples.items():
        ordered = sorted(
            values, key=lambda value: (value is None, value if value is not None else 0)
        )
        lo = ordered[round(0.025 * (resamples - 1))]
        hi = ordered[round(0.975 * (resamples - 1))]
        ranges[name] = [lo, hi]
    return {
        "resamples": resamples,
        "seed": seed,
        "train_entities": len(train),
        "test_entities": len(test),
        "ranges": ranges,
    }


def reliability(predict, tally, half=False, bins=10):
    """Bucket predictions; compare mean prediction with actual rate."""
    buckets = [[0.0, 0, 0] for _ in range(bins)]
    for lag, correct, n in tally.items(half):
        p = predict(lag)
        b = buckets[min(int(p * bins), bins - 1)]
        b[0] += p * n
        b[1] += n if correct else 0
        b[2] += n
    out = []
    for i, (psum, k, n) in enumerate(buckets):
        if n:
            out.append(
                {
                    "bin": f"{i / bins:.1f}–{(i + 1) / bins:.1f}",
                    "predicted": psum / n,
                    "actual": k / n,
                    "n": n,
                }
            )
    return out


# ── the full analysis ────────────────────────────────────────────────────


def _time_part(tally, early):
    """One side of the time split, in the shape survival_curve and brier read."""
    part = Tally()
    side = "early" if early else "late"
    for (observation_side, lag, correct), n in tally.by_time.items():
        if observation_side == side:
            key = (False, lag, correct)
            part.counts[key] = part.counts.get(key, 0) + n
    return part


def _analyze_time_split(tally, lags, guess, time_cutoff, fmt):
    early = _time_part(tally, early=True)
    late = _time_part(tally, early=False)
    early_n = early.total()
    late_n = late.total()
    cutoff = (
        datetime.fromtimestamp(time_cutoff, timezone.utc)
        .isoformat()
        .replace("+00:00", "Z")
    )
    if not early_n:
        return {"cutoff": cutoff, "skipped": "no early observations"}
    if not late_n:
        return {"cutoff": cutoff, "skipped": "no late observations"}
    early_curve = survival_curve(early, lags, half=False)
    s0, h, _ = fit_exponential(early_curve)
    s0b, hb, b = fit_exponential(early_curve, with_baseline=True)
    early_correct = sum(n for _, correct, n in early.items(False) if correct)
    base_rate = early_correct / early_n
    scores = {
        "fitted_half_life": brier(lambda lag: exp_model(lag, s0, h), late),
        "fitted_half_life_with_baseline": brier(
            lambda lag: exp_model(lag, s0b, hb, b), late
        ),
        "constant_base_rate": brier(lambda _lag: base_rate, late),
    }
    if guess is not None:
        scores["guessed_half_life"] = brier(
            lambda lag: exp_model(lag, 1.0, guess), late
        )
    return {
        "cutoff": cutoff,
        "early_observations": early_n,
        "late_observations": late_n,
        "straddling_observations": tally.time_straddling,
        "early_lags_s": sorted({lag for lag, _, _ in early.items(False)}),
        "late_lags_s": sorted({lag for lag, _, _ in late.items(False)}),
        "fitted_early": {
            "s0": s0,
            "s0_with_baseline": s0b,
            "half_life_s": h,
            "half_life": fmt(h),
            "baseline": b,
            "half_life_with_baseline_s": hb,
            "half_life_with_baseline": fmt(hb),
        },
        "brier_late": scores,
    }


def analyze(
    name,
    fact,
    observations,
    lags,
    guess=None,
    guess_label=None,
    truth=None,
    fmt=None,
    time_cutoff=None,
):
    """Run the whole analysis and return a JSON-serializable result.

    `fmt` formats a lag for display (default: as a duration in seconds).

    `guess` is the hand-picked half-life in seconds (math.inf for
    infinite), from examples/half-lives.yaml, to test against the data.
    `truth` describes what "still correct" means for this fact.
    """
    fmt = fmt or fmt_duration
    time_split = os.environ.get("FID_CAL_TIME_SPLIT") == "1" and time_cutoff is not None
    tally = as_tally(observations, time_cutoff if time_split else None)

    curve = survival_curve(tally, lags)
    train_curve = survival_curve(tally, lags, half=True)
    s0, h, _ = fit_exponential(train_curve)
    s0b, hb, b = fit_exponential(train_curve, with_baseline=True)
    s0_all, h_all, _ = fit_exponential(curve)
    s0b_all, hb_all, b_all = fit_exponential(curve, with_baseline=True)
    train_n = sum(n for _, _, n in tally.items(True))
    base_rate = sum(n for _, c, n in tally.items(True) if c) / train_n

    def fitted(lag):
        return exp_model(lag, s0, h)

    def fitted_baseline(lag):
        return exp_model(lag, s0b, hb, b)

    scores = {
        "fitted_half_life": brier(fitted, tally),
        "fitted_half_life_with_baseline": brier(fitted_baseline, tally),
        "constant_base_rate": brier(lambda _lag: base_rate, tally),
    }
    if guess is not None:
        # The guess as fid would use it: source fidelity 1, guessed half-life
        scores["guessed_half_life"] = brier(
            lambda lag: exp_model(lag, 1.0, guess), tally
        )

    result = {
        "experiment": name,
        "fact": fact,
        "truth": truth,
        "entities": len(tally.entities),
        "observations": tally.total(),
        "curve": [
            {"lag_s": lg, "lag": fmt(lg), "still_correct": s, "n": n}
            for lg, s, n in curve
        ],
        "empirical_half_life_s": empirical_half_life(curve),
        "empirical_half_life": fmt(empirical_half_life(curve)),
        "fitted": {"s0": s0_all, "half_life_s": h_all, "half_life": fmt(h_all)},
        "shape_error": shape_error(curve, s0_all, h_all),
        "fitted_with_baseline": {
            "s0": s0b_all,
            "baseline": b_all,
            "half_life_s": hb_all,
            "half_life": fmt(hb_all),
        },
        "shape_error_with_baseline": shape_error(curve, s0b_all, hb_all, b_all),
        "guess": {"label": guess_label, "half_life_s": guess, "half_life": fmt(guess)}
        if guess is not None
        else None,
        "brier_test": scores,
        "reliability_test": reliability(fitted, tally),
        "reliability_test_with_baseline": reliability(fitted_baseline, tally),
    }
    if time_split:
        result["time_split"] = _analyze_time_split(tally, lags, guess, time_cutoff, fmt)
    try:
        resamples = int(os.environ.get("FID_CAL_RESAMPLES", "0"))
    except ValueError:
        resamples = 0
    if resamples > 0:
        result["intervals"] = bootstrap(
            tally,
            lags,
            guess,
            resamples=resamples,
            seed=int(os.environ.get("FID_CAL_SEED", "0")),
        )
    return result


def summary_markdown(result):
    r = result
    intervals = r.get("intervals")
    ranges = intervals["ranges"] if intervals else {}

    def duration_range(name):
        lo, hi = ranges[name]
        return f" (95%: {fmt_duration(lo)}–{fmt_duration(hi)})"

    def baseline_range():
        lo, hi = ranges["baseline"]
        return f" (95%: {lo:.2f}–{hi:.2f})"

    lines = [
        f"### {r['experiment']}: {r['fact']}",
        "",
        f"- Still correct means: {r['truth']}",
        f"- {r['entities']:,} entities, {r['observations']:,} observations",
        f"- Empirical half-life: **{r['empirical_half_life'] or '—'}**"
        + (duration_range("empirical_half_life_s") if intervals else "")
        + (
            ""
            if r["empirical_half_life_s"]
            else " (never fell to 50% in the observed range)"
        ),
        f"- Fitted fid model (decay to 0): s0 = {r['fitted']['s0']:.3f}, half-life = **{r['fitted']['half_life']}**"
        + (duration_range("half_life_s") if intervals else "")
        + ", "
        f"shape error {r['shape_error']:.3f}"
        + (" — poor fit" if r["shape_error"] > 0.10 else ""),
        f"- Fitted with baseline (decay to b): b = {r['fitted_with_baseline']['baseline']:.2f}"
        + (baseline_range() if intervals else "")
        + ", "
        f"half-life = **{r['fitted_with_baseline']['half_life']}**"
        + (duration_range("half_life_with_baseline_s") if intervals else "")
        + ", "
        f"shape error {r['shape_error_with_baseline']:.3f}"
        + (" — poor fit" if r["shape_error_with_baseline"] > 0.10 else ""),
    ]
    if r["guess"]:
        lines.append(
            f"- Hand-picked guess ({r['guess']['label']}): {r['guess']['half_life']}"
        )
    if intervals:
        lines += [
            "",
            "| Predictor (held-out half) | Brier score (lower is better) | 95% range |",
            "|---|---|---|",
        ]
    else:
        lines += [
            "",
            "| Predictor (held-out half) | Brier score (lower is better) |",
            "|---|---|",
        ]
    interval_names = {
        "fitted_half_life": "brier_fitted",
        "fitted_half_life_with_baseline": "brier_fitted_with_baseline",
        "constant_base_rate": "brier_constant",
        "guessed_half_life": "brier_guessed",
    }
    predictor_order = [
        key for key, _ in sorted(r["brier_test"].items(), key=lambda kv: kv[1])
    ]
    for k in predictor_order:
        v = r["brier_test"][k]
        if intervals:
            lo, hi = ranges[interval_names[k]]
            lines.append(f"| {k.replace('_', ' ')} | {v:.4f} | {lo:.4f}–{hi:.4f} |")
        else:
            lines.append(f"| {k.replace('_', ' ')} | {v:.4f} |")
    if intervals:
        lines.append("")
        for key, label in (
            (
                "diff_baseline_minus_constant",
                "fitted_with_baseline − constant_base_rate",
            ),
            ("diff_guessed_minus_constant", "guessed_half_life − constant_base_rate"),
        ):
            if key in ranges:
                lo, hi = ranges[key]
                lines.append(
                    f"- Paired Brier difference, {label} (95% range): {lo:.4f}–{hi:.4f}"
                )
        if intervals["test_entities"] < 20:
            lines.append(
                f"Few held-out entities (n = {intervals['test_entities']}): "
                "the ranges understate the uncertainty."
            )
    if "time_split" in r:
        split = r["time_split"]
        lines += [
            "",
            f"#### Time split (fitted on facts recorded and re-checked before {split['cutoff']}, scored on facts recorded after it)",
            "",
        ]
        if "skipped" in split:
            lines.append(f"- {split['skipped']}")
        else:
            early = split["fitted_early"]
            lag_labels = {c["lag_s"]: c["lag"] for c in r["curve"]}
            early_max_lag = max(split["early_lags_s"])
            late_max_lag = max(split["late_lags_s"])
            lines += [
                f"- {split['early_observations']:,} early observations, {split['late_observations']:,} late observations",
                f"- Half-life fitted on early facts: **{early['half_life']}**; "
                f"with baseline: b = {early['baseline']:.2f}, half-life = **{early['half_life_with_baseline']}**",
                f"- Early side covers lags up to {lag_labels[early_max_lag]}; "
                f"late side up to {lag_labels[late_max_lag]}; "
                f"{split['straddling_observations']:,} observations straddle the cutoff and are not used",
                "",
                "| Predictor (later facts) | Brier score (lower is better) |",
                "|---|---|",
            ]
            for key in predictor_order:
                lines.append(
                    f"| {key.replace('_', ' ')} | {split['brier_late'][key]:.4f} |"
                )
    lines += ["", "| Lag | Still correct | n |", "|---|---|---|"]
    for c in r["curve"]:
        lines.append(f"| {c['lag']} | {c['still_correct']:.3f} | {c['n']:,} |")
    for key, label in (
        ("reliability_test", "decay to 0"),
        ("reliability_test_with_baseline", "decay to baseline"),
    ):
        lines += [
            "",
            f"Reliability on the held-out half, {label}:",
            "",
            "| Predicted bin | Mean predicted | Actual | n |",
            "|---|---|---|---|",
        ]
        for b in r[key]:
            lines.append(
                f"| {b['bin']} | {b['predicted']:.3f} | {b['actual']:.3f} | {b['n']:,} |"
            )
    return "\n".join(lines) + "\n"


def _finite(value):
    """JSON has no Infinity: write infinite/NaN numbers as null.

    The human-readable fields beside them (e.g. "half_life": "infinite")
    keep the meaning.
    """
    if isinstance(value, float) and not math.isfinite(value):
        return None
    if isinstance(value, dict):
        return {k: _finite(v) for k, v in value.items()}
    if isinstance(value, list):
        return [_finite(v) for v in value]
    return value


def write_results(results, slug, extra_markdown=""):
    """Write results/<slug>.json and results/<slug>.md next to this file.

    `extra_markdown` is appended to the summaries, for analyses beyond them.
    """
    here = os.path.dirname(os.path.abspath(__file__))
    out_dir = os.environ.get("FID_CAL_RESULTS_DIR", os.path.join(here, "results"))
    os.makedirs(out_dir, exist_ok=True)
    with open(os.path.join(out_dir, f"{slug}.json"), "w") as f:
        json.dump(_finite(results), f, indent=2)
        f.write("\n")
    md = "\n".join(summary_markdown(r) for r in results) + extra_markdown
    with open(os.path.join(out_dir, f"{slug}.md"), "w") as f:
        f.write(md)
    print(md)
