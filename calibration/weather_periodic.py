"""Do periodic facts need a periodic model? The echo test.

Finding 5: a 24-hour-old temperature reading is right more often than a
12-hour-old one, because the daily cycle brings it back. fid's model
decays monotonically, so it cannot say that. This experiment tests a
model that can, against the same METAR data (10 US airports, all of
2024), with every lag from 0 to 14 days:

  echo:  S(t) = b + (s0 − b) × [ 0.5^(t/h) + Σₙ r × 0.5^(nP/h_r) × 0.5^(|t − nP|/w) ]

The fresh fact's agreement comes back once per period P, as an echo of
strength r and width w that fades with half-life h_r over periods.

Models, each fitted on a train half and scored by Brier on the held-out half:

  constant · decay to 0 · decay to baseline (fid today) · echo, P = 24h ·
  echo 24h plus a weekly echo at 168h · echo with P scanned (train only)

Two splits: by station (as in the study) and by time (fit on January to
June, score July to December).

The rule for building it into fid, set before running: the echo beats
decay-to-baseline on held-out Brier in both splits, its largest gap
from the curve is under 0.10, and its reliability is no worse. The
weekly echo counts only if it beats the daily echo in both splits.

Usage: python3 weather_periodic.py      (data: as weather_metar.py, ~10 MB)
"""

import math
import random
from datetime import timedelta

import fidcal
import weather_metar

MAX_LAG_H = 14 * 24
LAG_HOURS = list(range(MAX_LAG_H + 1))
DAY_H, WEEK_H = 24.0, 168.0
FACTS = ["temperature", "flight_category", "adverse"]
FACT_LABELS = {
    "temperature": ("METAR temperature", "the temperature at airport A is T", "within ±2 °F"),
    "flight_category": ("METAR flight category", "airport A is VFR / MVFR / IFR / LIFR", "same flight category"),
    "adverse": ("METAR adverse conditions", "airport A is IFR or worse", "still IFR or LIFR"),
}
# Gap and reliability thresholds from the rule above
MAX_SHAPE_ERROR = 0.10


# ── counting ─────────────────────────────────────────────────────────────

def count(all_series):
    """counts[fact][(station, first_half_of_year, lag_h)] = [correct, n]."""
    counts = {f: {} for f in FACTS}
    for station, series in all_series.items():
        for hour, (t0, c0) in series.items():
            h1 = hour.month <= 6
            for lag in LAG_HOURS:
                later = series.get(hour + timedelta(hours=lag))
                if later is None:
                    continue
                t1, c1 = later
                if t0 is not None and t1 is not None:
                    _add(counts["temperature"], (station, h1, lag), abs(t1 - t0) <= weather_metar.TEMP_TOLERANCE_F)
                if c0 is not None and c1 is not None:
                    _add(counts["flight_category"], (station, h1, lag), c1 == c0)
                    if c0 in ("IFR", "LIFR"):
                        _add(counts["adverse"], (station, h1, lag), c1 in ("IFR", "LIFR"))
    return counts


def cached_counts(cache, all_series):
    """count(), kept in the cache dir: counting ~30M pairs per fact takes minutes."""
    import pickle, os
    path = os.path.join(cache, "metar", f"periodic_counts_{MAX_LAG_H}h.pickle")
    if os.path.exists(path):
        with open(path, "rb") as f:
            return pickle.load(f)
    fidcal.log("counting every lag from 0 to 14 days")
    counts = count(all_series)
    with open(path, "wb") as f:
        pickle.dump(counts, f)
    return counts


def _add(cells, key, correct):
    c = cells.get(key)
    if c is None:
        c = cells[key] = [0, 0]
    c[0] += 1 if correct else 0
    c[1] += 1


SPLITS = {
    "station": lambda station, h1: fidcal.split_train(station),
    "time": lambda station, h1: h1,
}


def cells(counts, split, train):
    """{lag_h: [correct, n]} for one half of a split (train=None: all)."""
    out = {}
    side = SPLITS[split]
    for (station, h1, lag), (k, n) in counts.items():
        if train is not None and side(station, h1) != train:
            continue
        c = out.setdefault(lag, [0, 0])
        c[0] += k
        c[1] += n
    return out


def station_cells(counts, station):
    out = {}
    for (s, _h1, lag), (k, n) in counts.items():
        if s == station:
            c = out.setdefault(lag, [0, 0])
            c[0] += k
            c[1] += n
    return out


def curve(cells_by_lag):
    """[(lag_h, fraction, n)], sorted by lag."""
    return [(lag, k / n, n) for lag, (k, n) in sorted(cells_by_lag.items()) if n]


# ── models (t in hours) ──────────────────────────────────────────────────

def decay(t, s0, b, h):
    return b + (s0 - b) * 0.5 ** (t / h) if h > 0 and not math.isinf(h) else s0


def echo(t, s0, b, h, r, h_r, w, period=DAY_H, r7=0.0, w7=1.0):
    """The echo model, with an optional weekly echo (r7, w7) sharing h_r."""
    s = 0.5 ** (t / h)
    if r > 0:
        s += r * _echoes(t, period, h_r, w)
    if r7 > 0:
        s += r7 * _echoes(t, WEEK_H, h_r, w7)
    return min(max(b + (s0 - b) * s, 0.0), 1.0)


def _echoes(t, period, h_r, w):
    """Σₙ≥₁ 0.5^(nP/h_r) × 0.5^(|t − nP|/w), over echoes within 30 widths of t.

    A farther echo contributes under 0.5^30 ≈ 1e-9 of its peak. Echoes
    past MAX_LAG_H + P are left out: the model is defined on the lags
    measured here.
    """
    reach = 30 * w
    first = max(1, math.ceil((t - reach) / period))
    last = min(math.floor((t + reach) / period), math.ceil((MAX_LAG_H + period) / period))
    return sum(0.5 ** (n * period / h_r) * 0.5 ** (abs(t - n * period) / w) for n in range(first, last + 1))


# ── fitting ──────────────────────────────────────────────────────────────

def nelder_mead(f, x0, step=0.5, iters=600, tol=1e-12):
    """Minimize f over R^n. Small, dependency-free, good enough for 4–7 parameters."""
    n = len(x0)
    simplex = [list(x0)] + [[x + (step if i == j else 0.0) for j, x in enumerate(x0)] for i in range(n)]
    values = [f(p) for p in simplex]
    for _ in range(iters):
        order = sorted(range(n + 1), key=values.__getitem__)
        simplex = [simplex[i] for i in order]
        values = [values[i] for i in order]
        if abs(values[-1] - values[0]) < tol:
            break
        centroid = [sum(p[j] for p in simplex[:-1]) / n for j in range(n)]
        worst = simplex[-1]

        def toward(scale):
            return [c + scale * (wv - c) for c, wv in zip(centroid, worst)]

        reflected = toward(-1.0)
        fr = f(reflected)
        if fr < values[0]:
            expanded = toward(-2.0)
            fe = f(expanded)
            simplex[-1], values[-1] = (expanded, fe) if fe < fr else (reflected, fr)
        elif fr < values[-2]:
            simplex[-1], values[-1] = reflected, fr
        else:
            contracted = toward(0.5)
            fc = f(contracted)
            if fc < values[-1]:
                simplex[-1], values[-1] = contracted, fc
            else:
                best = simplex[0]
                simplex = [best] + [[bv + 0.5 * (pv - bv) for bv, pv in zip(best, p)] for p in simplex[1:]]
                values = [values[0]] + [f(p) for p in simplex[1:]]
    i = min(range(n + 1), key=values.__getitem__)
    return simplex[i], values[i]


def sigmoid(x):
    return 1 / (1 + math.exp(-x)) if x > -500 else 0.0


def logit(p):
    p = min(max(p, 1e-6), 1 - 1e-6)
    return math.log(p / (1 - p))


def sse(model, cur):
    return sum(n * (s - model(t)) ** 2 for t, s, n in cur)


def fit_decay(cur, with_baseline):
    """fidcal's fit (seconds there, hours here). Returns (s0, b, h)."""
    s0, h, b = fidcal.fit_exponential([(t * 3600.0, s, n) for t, s, n in cur], with_baseline=with_baseline)
    return s0, b, h / 3600.0


def fit_echo(cur, period=DAY_H, weekly=False, starts=None):
    """Fit b, h, r, h_r, w (and r7, w7 if weekly) for a fixed period. s0 = curve at lag 0.

    Echo widths are kept below half their period: a wider "echo" is not
    one, and it lets the fit trade width against strength.
    """
    s0 = cur[0][1] if cur[0][0] == 0 else 1.0

    def unpack(x):
        p = dict(b=s0 * sigmoid(x[0]), h=math.exp(x[1]), r=math.exp(x[2]), h_r=math.exp(x[3]), w=period / 2 * sigmoid(x[4]))
        if weekly:
            p.update(r7=math.exp(x[5]), w7=WEEK_H / 2 * sigmoid(x[6]))
        return p

    def loss(x):
        try:
            p = unpack(x)
        except OverflowError:
            return math.inf
        return sse(lambda t: echo(t, s0, period=period, **p), cur)

    if starts is None:
        starts = [dict(b=0.15, h=2.4, r=0.5, h_r=48.0, w=3.2), dict(b=0.25, h=1.8, r=0.2, h_r=200.0, w=2.0),
                  dict(b=0.05, h=4.0, r=1.0, h_r=24.0, w=5.0)]
    best = None
    for st in starts:
        x0 = [logit(st["b"] / s0), math.log(st["h"]), math.log(max(st["r"], 1e-4)), math.log(st["h_r"]), logit(st["w"] / (period / 2))]
        if weekly:
            x0 += [math.log(max(st.get("r7", 0.05), 1e-4)), logit(st.get("w7", st["w"]) / (WEEK_H / 2))]
        x, v = nelder_mead(loss, x0)
        x, v = nelder_mead(loss, x, step=0.1)  # restart from the optimum: Nelder–Mead stalls otherwise
        if best is None or v < best[1]:
            best = (x, v)
    p = unpack(best[0])
    p["s0"] = s0
    return p


def echo_of(p, period=DAY_H):
    q = dict(p)
    s0 = q.pop("s0")
    return lambda t: echo(t, s0, period=period, **q)


# ── scoring ──────────────────────────────────────────────────────────────

def brier(predict, cells_by_lag):
    total = n_all = 0
    for lag, (k, n) in cells_by_lag.items():
        p = predict(lag)
        total += k * (1 - p) ** 2 + (n - k) * p ** 2
        n_all += n
    return total / n_all


def reliability(predict, cells_by_lag, bins=10):
    buckets = [[0.0, 0, 0] for _ in range(bins)]
    for lag, (k, n) in cells_by_lag.items():
        p = predict(lag)
        b = buckets[min(int(p * bins), bins - 1)]
        b[0] += p * n
        b[1] += k
        b[2] += n
    return [{"bin": f"{i / bins:.1f}–{(i + 1) / bins:.1f}", "predicted": ps / n, "actual": k / n, "n": n}
            for i, (ps, k, n) in enumerate(buckets) if n]


def reliability_error(rows):
    """Observation-weighted mean |predicted − actual| across bins (lower is better)."""
    total = sum(r["n"] for r in rows)
    return sum(r["n"] * abs(r["predicted"] - r["actual"]) for r in rows) / total


def gap(predict, cur):
    return max(abs(s - predict(t)) for t, s, _ in cur)


# ── the experiment ───────────────────────────────────────────────────────

def models_for(train_cur, scan=False):
    """name → (predict, params). Fitted on the train curve."""
    s0, _, h0 = fit_decay(train_cur, False)
    s0b, b, hb = fit_decay(train_cur, True)
    k = sum(s * n for _, s, n in train_cur)
    base_rate = k / sum(n for _, _, n in train_cur)
    daily = fit_echo(train_cur)
    weekly = fit_echo(train_cur, weekly=True, starts=[dict(daily, r7=0.05, w7=daily["w"]), dict(daily, r7=0.3, w7=6.0)])
    out = {
        "constant": (lambda t: base_rate, {"rate": base_rate}),
        "decay_to_0": (lambda t: decay(t, s0, 0.0, h0), {"s0": s0, "h_hours": h0}),
        "decay_to_baseline": (lambda t: decay(t, s0b, b, hb), {"s0": s0b, "b": b, "h_hours": hb}),
        "echo_24h": (echo_of(daily), dict(daily, period_hours=DAY_H)),
        "echo_24h_and_168h": (echo_of(weekly), dict(weekly, period_hours=DAY_H)),
    }
    if scan:
        out["period_scan"] = scan_period(train_cur, daily)
    return out


def scan_period(train_cur, daily):
    """Best period on the train curve alone: does the data pick 24h?"""
    periods = [float(p) for p in range(6, 48)] + [float(p) for p in range(48, MAX_LAG_H + 1, 4)]
    rows = []
    for period in periods:
        fidcal.log(f"  period scan: {period:.0f}h")
        start = dict((k, v) for k, v in daily.items() if k != "s0")
        p = fit_echo(train_cur, period=period, starts=[start])
        rows.append((sse(echo_of(p, period), train_cur), period, p))
    rows.sort(key=lambda r: r[0])
    return {"best_period_hours": rows[0][1],
            "top": [{"period_hours": per, "train_sse": v} for v, per, _ in rows[:5]],
            "all": [{"period_hours": per, "train_sse": v} for v, per, _ in sorted(rows, key=lambda r: r[1])]}


def run_fact(fact, counts):
    result = {"splits": {}}
    for split in SPLITS:
        fidcal.log(f"  {split} split")
        train, test = cells(counts, split, True), cells(counts, split, False)
        models = models_for(curve(train), scan=(fact == "temperature" and split == "station"))
        scan = models.pop("period_scan", None)
        test_cur = curve(test)
        result["splits"][split] = {
            "train_observations": sum(n for _, n in train.values()),
            "test_observations": sum(n for _, n in test.values()),
            "brier_test": {name: brier(pred, test) for name, (pred, _) in models.items()},
            "gap_test": {name: gap(pred, test_cur) for name, (pred, _) in models.items()},
            "reliability_error_test": {name: reliability_error(reliability(pred, test)) for name, (pred, _) in models.items()},
            "params": {name: params for name, (_, params) in models.items()},
        }
        if scan:
            result["period_scan"] = scan

    # Fitted on all data: the curve, the published parameters, the largest gap
    all_cur = curve(cells(counts, "station", None))
    all_models = models_for(all_cur)
    result["curve"] = [
        {"lag_hours": t, "still_correct": s, "n": n,
         **{name: pred(t) for name, (pred, _) in all_models.items()}}
        for t, s, n in all_cur
    ]
    result["fitted_all"] = {name: params for name, (_, params) in all_models.items()}
    result["gap_all"] = {name: gap(pred, all_cur) for name, (pred, _) in all_models.items()}
    result["reliability_all_test_station"] = {
        name: reliability(pred, cells(counts, "station", False))
        for name, (pred, _) in models_for(curve(cells(counts, "station", True))).items()
        if name in ("decay_to_baseline", "echo_24h")
    }

    # The rule
    s = result["splits"]
    echo_wins = all(s[k]["brier_test"]["echo_24h"] < s[k]["brier_test"]["decay_to_baseline"] for k in SPLITS)
    reliable = all(s[k]["reliability_error_test"]["echo_24h"] <= s[k]["reliability_error_test"]["decay_to_baseline"] for k in SPLITS)
    shaped = result["gap_all"]["echo_24h"] < MAX_SHAPE_ERROR
    weekly = all(s[k]["brier_test"]["echo_24h_and_168h"] < s[k]["brier_test"]["echo_24h"] for k in SPLITS)
    result["rule"] = {
        "echo_beats_baseline_both_splits": echo_wins,
        "echo_reliability_no_worse_both_splits": reliable,
        "echo_gap_under_0_10": shaped,
        "echo_passes": echo_wins and reliable and shaped,
        "weekly_beats_daily_both_splits": weekly,
    }
    return result


def per_station(counts):
    out = {}
    for station in weather_metar.STATIONS:
        cur = curve(station_cells(counts, station))
        p = fit_echo(cur)
        out[station] = {**p, "gap": gap(echo_of(p), cur),
                        "still_correct_12h": dict((t, s) for t, s, _ in cur).get(12),
                        "still_correct_24h": dict((t, s) for t, s, _ in cur).get(24)}
    return out


def check_published(counts):
    """Decay-to-baseline at the study's lags and split must give the published numbers."""
    lags = set(weather_metar.LAG_HOURS)
    sub = {k: v for k, v in counts["temperature"].items() if k[2] in lags}
    train, test = cells(sub, "station", True), cells(sub, "station", False)
    s0, b, h = fit_decay(curve(train), True)
    return {"h_hours": h, "b": b, "brier_test": brier(lambda t: decay(t, s0, b, h), test),
            "published": {"h_hours": 1.8, "b": 0.25, "brier_test": 0.1809}}


def self_test():
    """The echo with r = 0 is the baseline model; the fit recovers known parameters."""
    for t in (0.0, 1.0, 7.5, 30.0, 200.0):
        assert abs(echo(t, 0.97, 0.2, 3.0, 0.0, 48.0, 2.0) - decay(t, 0.97, 0.2, 3.0)) < 1e-12
    truth = dict(b=0.14, h=2.4, r=0.5, h_r=48.0, w=3.2)
    rng = random.Random(7)
    cur = [(t, echo(t, 1.0, **truth) + rng.gauss(0, 0.002), 1000) for t in range(0, 120)]
    cur[0] = (0, 1.0, 1000)
    p = fit_echo(cur, starts=[dict(b=0.3, h=1.0, r=0.2, h_r=100.0, w=1.5)])
    for k, v in truth.items():
        assert abs(p[k] / v - 1) < 0.05, (k, p[k], v)
    fidcal.log("self-test ok")


# ── output ───────────────────────────────────────────────────────────────

NAMES = ["constant", "decay_to_0", "decay_to_baseline", "echo_24h", "echo_24h_and_168h"]
SHOW_HOURS = list(range(0, 49, 3)) + [60, 72, 84, 96, 120, 144, 156, 168, 180, 192, 240, 288, 312, 336]


def markdown(results, stations, published):
    lines = ["# Periodic facts: the echo test", "",
             "Fitted on one half, Brier score on the other (lower is better). "
             "Split by station (6 train, 4 test) and by time (Jan–Jun train, Jul–Dec test).", "",
             f"Check against the published study (decay to baseline, study lags): h = {published['h_hours']:.1f}h, "
             f"b = {published['b']:.2f}, held-out Brier {published['brier_test']:.4f} "
             f"(published: 1.8h, 0.25, 0.1809).", ""]
    for fact, r in results.items():
        name, claim, truth = FACT_LABELS[fact]
        lines += [f"## {name}: {claim}", "", f"Still correct means: {truth}.", "",
                  "| Model | Brier, station split | Brier, time split | Largest gap (all data) |", "|---|---|---|---|"]
        for m in NAMES:
            lines.append(f"| {m.replace('_', ' ')} | {r['splits']['station']['brier_test'][m]:.4f} | "
                         f"{r['splits']['time']['brier_test'][m]:.4f} | {r['gap_all'][m]:.3f} |")
        rule = r["rule"]
        lines += ["", f"- Echo beats decay to baseline in both splits: **{'yes' if rule['echo_beats_baseline_both_splits'] else 'no'}**",
                  f"- Echo reliability no worse in both splits: **{'yes' if rule['echo_reliability_no_worse_both_splits'] else 'no'}**",
                  f"- Echo largest gap under 0.10: **{'yes' if rule['echo_gap_under_0_10'] else 'no'}**",
                  f"- **Rule passed: {'yes' if rule['echo_passes'] else 'no'}**",
                  f"- Weekly echo beats daily echo in both splits: **{'yes' if rule['weekly_beats_daily_both_splits'] else 'no'}**", ""]
        e = r["fitted_all"]["echo_24h"]
        lines += [f"Echo fitted on all data: s0 = {e['s0']:.3f}, b = {e['b']:.3f}, h = {e['h']:.2f}h, "
                  f"r = {e['r']:.3f}, h_r = {e['h_r']:.1f}h, w = {e['w']:.2f}h.", ""]
        wk = r["fitted_all"]["echo_24h_and_168h"]
        lines += [f"Weekly term fitted on all data: r7 = {wk['r7']:.4f}, w7 = {wk['w7']:.2f}h.", ""]
        if "period_scan" in r:
            top = ", ".join(f"{x['period_hours']:.0f}h" for x in r["period_scan"]["top"])
            lines += [f"Period scan (train half only), best first: {top}.", ""]
        lines += ["| Lag | Still correct | Decay to baseline | Echo 24h | n |", "|---|---|---|---|---|"]
        by_lag = {c["lag_hours"]: c for c in r["curve"]}
        for t in SHOW_HOURS:
            c = by_lag.get(t)
            if c:
                lines.append(f"| {t}h | {c['still_correct']:.3f} | {c['decay_to_baseline']:.3f} | {c['echo_24h']:.3f} | {c['n']:,} |")
        lines.append("")
    lines += ["## Temperature echo by station (all data)", "",
              "| Station | Correct at 12h | at 24h | b | h | r | h_r | w | Largest gap |", "|---|---|---|---|---|---|---|---|---|"]
    for st, p in stations.items():
        lines.append(f"| {st} | {p['still_correct_12h']:.3f} | {p['still_correct_24h']:.3f} | {p['b']:.3f} | {p['h']:.2f}h | "
                     f"{p['r']:.3f} | {p['h_r']:.0f}h | {p['w']:.2f}h | {p['gap']:.3f} |")
    return "\n".join(lines) + "\n"


def main():
    self_test()
    cache = fidcal.default_cache_dir()
    all_series = {s: weather_metar.load(weather_metar.fetch(cache, s)) for s in weather_metar.STATIONS}
    counts = cached_counts(cache, all_series)
    published = check_published(counts)
    fidcal.log(f"published check: {published}")
    results = {}
    for fact in FACTS:
        fidcal.log(f"fitting {fact}")
        results[fact] = run_fact(fact, counts[fact])
    stations = per_station(counts["temperature"])

    import json, os
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "results")
    with open(os.path.join(out, "weather_periodic.json"), "w") as f:
        json.dump(fidcal._finite({"published_check": published, "facts": results, "temperature_by_station": stations}), f, indent=1)
        f.write("\n")
    md = markdown(results, stations, published)
    with open(os.path.join(out, "weather_periodic.md"), "w") as f:
        f.write(md)
    print(md)


if __name__ == "__main__":
    main()
