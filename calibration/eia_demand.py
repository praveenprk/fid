"""Does a human rhythm make facts come back? The echo test on electricity demand.

Weather has a daily cycle and no weekly one. Electricity demand should
have both: people sleep at night and keep a working week. This is the
fair test of the echo model in weather_periodic.py on a fact driven by
people, not physics.

Ground truth: hourly demand by balancing authority for all of 2024, from
the EIA API v2 (electricity/rto/region-data, type D). Each hour's reading
is a stored fact; later readings for the same region are the truth it is
checked against.

Fact: "hourly demand in region R is D", still correct if the demand at
the later hour is within ±5% of D. Checked at every lag from 0 to 14 days.

Models, fitted on a train half and scored by Brier on the held-out half,
exactly as weather_periodic.py: constant · decay to 0 · decay to baseline
(fid today) · echo, P = 24h · echo 24h plus a weekly echo at 168h · echo
with P scanned (train only). Two splits: by region (7 train, 7 test) and
by time (Jan–Jun train, Jul–Dec test).

The rule, the same as for temperature: the echo beats decay-to-baseline
on held-out Brier in both splits, its largest gap from the curve is under
0.10, and its reliability is no worse. The weekly echo counts only if it
beats the daily echo in both splits.

Readings of zero or less are dropped as reporting errors; nothing else is
cleaned. Hours are UTC.

Usage: FID_EIA_API_KEY=... python3 eia_demand.py   (~5 MB, 28 requests)
"""

import json
import os
import pickle
import urllib.parse
from datetime import datetime, timedelta

import fidcal
import weather_periodic as wp

URL = "https://api.eia.gov/v2/electricity/rto/region-data/data/"
PAGE = 5000
# The seven largest grid operators and seven large utilities; the hash
# split puts 7 in each half
REGIONS = ["ERCO", "PJM", "MISO", "CISO", "NYIS", "ISNE", "SWPP",
           "FPL", "TVA", "SOCO", "DUK", "BPAT", "PACE", "AZPS"]
YEAR = 2024
TOLERANCE = 0.05
LAG_HOURS = wp.LAG_HOURS  # 0 to 14 days, every hour
STUDY_LAG_HOURS = [0, 1, 2, 3, 4, 6, 8, 12, 18, 24, 36, 48, 72, 96, 120, 144, 168, 192, 240, 336]
FACT = "hourly demand in region R is D"
TRUTH = f"demand at the later hour within ±{TOLERANCE:.0%} of D"


# ── data ─────────────────────────────────────────────────────────────────

def query(region, offset, key):
    params = [
        ("frequency", "hourly"), ("data[0]", "value"),
        ("facets[respondent][]", region), ("facets[type][]", "D"),
        ("start", f"{YEAR}-01-01T00"), ("end", f"{YEAR}-12-31T23"),
        ("sort[0][column]", "period"), ("sort[0][direction]", "asc"),
        ("offset", offset), ("length", PAGE), ("api_key", key),
    ]
    return URL + "?" + urllib.parse.urlencode(params)


def fetch(cache_dir, region, key):
    """Every page of one region's 2024 demand, cached. Returns the rows."""
    rows, offset = [], 0
    while True:
        path = os.path.join(cache_dir, "eia", f"{region}_{YEAR}_{offset}.json")
        fidcal.download(query(region, offset, key), path, pause=0.5, label=f"EIA {region} offset {offset}")
        with open(path) as f:
            response = json.load(f)["response"]
        rows += response["data"]
        offset += PAGE
        if offset >= int(response["total"]):
            return rows


def load(rows):
    """Hourly series: UTC hour → demand in MWh. Missing and non-positive readings left out."""
    series = {}
    for row in rows:
        if row.get("type") != "D" or row.get("value") in (None, ""):
            continue
        value = float(row["value"])
        if value > 0:
            series[datetime.strptime(row["period"], "%Y-%m-%dT%H")] = value
    return series


def still_correct(d0, d1):
    return abs(d1 - d0) <= TOLERANCE * d0


# ── counting ─────────────────────────────────────────────────────────────

def count(all_series, lag_hours=LAG_HOURS):
    """counts[(region, first_half_of_year, lag_h)] = [correct, n], as weather_periodic.count."""
    counts = {}
    for region, series in all_series.items():
        for hour, d0 in series.items():
            h1 = hour.month <= 6
            for lag in lag_hours:
                d1 = series.get(hour + timedelta(hours=lag))
                if d1 is not None:
                    wp._add(counts, (region, h1, lag), still_correct(d0, d1))
    return counts


def cached_counts(cache, all_series):
    path = os.path.join(cache, "eia", f"counts_{YEAR}_{TOLERANCE}_{wp.MAX_LAG_H}h.pickle")
    if os.path.exists(path):
        with open(path, "rb") as f:
            return pickle.load(f)
    fidcal.log("counting every lag from 0 to 14 days")
    counts = count(all_series)
    with open(path, "wb") as f:
        pickle.dump(counts, f)
    return counts


def tally(counts, lag_hours):
    """The counts at `lag_hours` as a fidcal.Tally (lags in seconds), split by region."""
    t = fidcal.Tally()
    wanted = set(lag_hours)
    for (region, _h1, lag), (k, n) in counts.items():
        if lag not in wanted:
            continue
        train = fidcal.split_train(region)
        t.entities.add(region)
        for correct, m in ((True, k), (False, n - k)):
            if m:
                key = (train, lag * fidcal.HOUR, correct)
                t.counts[key] = t.counts.get(key, 0) + m
    return t


# ── the echo test ────────────────────────────────────────────────────────

def run(counts, scan=True):
    """weather_periodic.run_fact, with the period scan on the region split."""
    result = {"splits": {}}
    for split in wp.SPLITS:
        fidcal.log(f"  {split} split")
        train, test = wp.cells(counts, split, True), wp.cells(counts, split, False)
        models = wp.models_for(wp.curve(train), scan=scan and split == "station")
        period_scan = models.pop("period_scan", None)
        test_cur = wp.curve(test)
        result["splits"][split] = {
            "train_observations": sum(n for _, n in train.values()),
            "test_observations": sum(n for _, n in test.values()),
            "brier_test": {name: wp.brier(pred, test) for name, (pred, _) in models.items()},
            "gap_test": {name: wp.gap(pred, test_cur) for name, (pred, _) in models.items()},
            "reliability_error_test": {name: wp.reliability_error(wp.reliability(pred, test))
                                       for name, (pred, _) in models.items()},
            "params": {name: params for name, (_, params) in models.items()},
        }
        if period_scan:
            result["period_scan"] = period_scan

    all_cur = wp.curve(wp.cells(counts, "station", None))
    all_models = wp.models_for(all_cur)
    result["curve"] = [{"lag_hours": t, "still_correct": s, "n": n,
                        **{name: pred(t) for name, (pred, _) in all_models.items()}}
                       for t, s, n in all_cur]
    result["fitted_all"] = {name: params for name, (_, params) in all_models.items()}
    result["gap_all"] = {name: wp.gap(pred, all_cur) for name, (pred, _) in all_models.items()}
    result["rule"] = rule(result)
    return result


def rule(result):
    s, splits = result["splits"], list(wp.SPLITS)
    best_echo = lambda k: min(s[k]["brier_test"]["echo_24h"], s[k]["brier_test"]["echo_24h_and_168h"])
    echo_wins = all(s[k]["brier_test"]["echo_24h"] < s[k]["brier_test"]["decay_to_baseline"] for k in splits)
    reliable = all(s[k]["reliability_error_test"]["echo_24h"] <= s[k]["reliability_error_test"]["decay_to_baseline"]
                   for k in splits)
    shaped = result["gap_all"]["echo_24h"] < wp.MAX_SHAPE_ERROR
    weekly = all(s[k]["brier_test"]["echo_24h_and_168h"] < s[k]["brier_test"]["echo_24h"] for k in splits)
    weekly_reliable = all(s[k]["reliability_error_test"]["echo_24h_and_168h"]
                          <= s[k]["reliability_error_test"]["decay_to_baseline"] for k in splits)
    weekly_shaped = result["gap_all"]["echo_24h_and_168h"] < wp.MAX_SHAPE_ERROR
    return {
        "echo_beats_baseline_both_splits": echo_wins,
        "echo_reliability_no_worse_both_splits": reliable,
        "echo_gap_under_0_10": shaped,
        "echo_passes": echo_wins and reliable and shaped,
        "weekly_beats_daily_both_splits": weekly,
        "weekly_passes": weekly and weekly_reliable and weekly_shaped
                         and all(best_echo(k) < s[k]["brier_test"]["decay_to_baseline"] for k in splits),
    }


def per_region(counts):
    out = {}
    for region in REGIONS:
        cells = wp.station_cells(counts, region)
        if not cells:
            continue
        at = {t: s for t, s, _ in wp.curve(cells)}
        out[region] = {lag: at.get(lag) for lag in (1, 12, 24, 144, 168)}
    return out


# ── output ───────────────────────────────────────────────────────────────

SHOW_HOURS = [0, 1, 2, 3, 6, 12, 18, 24, 36, 48, 72, 96, 120, 144, 156, 162, 166, 167, 168, 169, 170,
              174, 180, 192, 240, 288, 312, 324, 335, 336]


def markdown(r, regions):
    rule = r["rule"]
    lines = ["", "## The echo test: every lag from 0 to 14 days", "",
             "Fitted on one half, Brier score on the other (lower is better). "
             "Split by region (7 train, 7 test) and by time (Jan–Jun train, Jul–Dec test).", "",
             "| Model | Brier, region split | Brier, time split | Largest gap (all data) |", "|---|---|---|---|"]
    for m in wp.NAMES:
        lines.append(f"| {m.replace('_', ' ')} | {r['splits']['station']['brier_test'][m]:.4f} | "
                     f"{r['splits']['time']['brier_test'][m]:.4f} | {r['gap_all'][m]:.3f} |")
    yes = lambda v: "yes" if v else "no"
    lines += ["", f"- Echo beats decay to baseline in both splits: **{yes(rule['echo_beats_baseline_both_splits'])}**",
              f"- Echo reliability no worse in both splits: **{yes(rule['echo_reliability_no_worse_both_splits'])}**",
              f"- Echo largest gap under 0.10: **{yes(rule['echo_gap_under_0_10'])}**",
              f"- **Rule passed (daily echo): {yes(rule['echo_passes'])}**",
              f"- Weekly echo beats daily echo in both splits: **{yes(rule['weekly_beats_daily_both_splits'])}**",
              f"- **Rule passed (daily plus weekly echo): {yes(rule['weekly_passes'])}**", ""]
    lines += ["| Model | Reliability error, region split | time split |", "|---|---|---|"]
    for m in wp.NAMES:
        lines.append(f"| {m.replace('_', ' ')} | {r['splits']['station']['reliability_error_test'][m]:.4f} | "
                     f"{r['splits']['time']['reliability_error_test'][m]:.4f} |")
    e, wk = r["fitted_all"]["echo_24h"], r["fitted_all"]["echo_24h_and_168h"]
    lines += ["", f"Echo fitted on all data: s0 = {e['s0']:.3f}, b = {e['b']:.3f}, h = {e['h']:.2f}h, "
              f"r = {e['r']:.3f}, h_r = {e['h_r']:.1f}h, w = {e['w']:.2f}h.", "",
              f"Daily plus weekly, fitted on all data: b = {wk['b']:.3f}, h = {wk['h']:.2f}h, r = {wk['r']:.3f}, "
              f"h_r = {wk['h_r']:.1f}h, w = {wk['w']:.2f}h, r7 = {wk['r7']:.3f}, w7 = {wk['w7']:.2f}h.", ""]
    if "period_scan" in r:
        top = ", ".join(f"{x['period_hours']:.0f}h" for x in r["period_scan"]["top"])
        lines += [f"Period scan (train half only), best first: {top}.", ""]
    lines += ["| Lag | Still correct | Decay to baseline | Echo 24h | Echo 24h + 168h | n |", "|---|---|---|---|---|---|"]
    by_lag = {c["lag_hours"]: c for c in r["curve"]}
    for t in SHOW_HOURS:
        c = by_lag.get(t)
        if c:
            lines.append(f"| {t}h | {c['still_correct']:.3f} | {c['decay_to_baseline']:.3f} | {c['echo_24h']:.3f} | "
                         f"{c['echo_24h_and_168h']:.3f} | {c['n']:,} |")
    lines += ["", "## By region (all data)", "",
              "| Region | Correct at 1h | 12h | 24h | 144h | 168h |", "|---|---|---|---|---|---|"]
    for region, at in regions.items():
        lines.append(f"| {region} | " + " | ".join(f"{at[l]:.3f}" if at[l] is not None else "—"
                                                   for l in (1, 12, 24, 144, 168)) + " |")
    return "\n".join(lines) + "\n"


def main():
    key = os.environ.get("FID_EIA_API_KEY")
    if not key:
        raise SystemExit("set FID_EIA_API_KEY (free at https://www.eia.gov/opendata/register.php)")
    wp.self_test()
    cache = fidcal.default_cache_dir()
    all_series = {}
    for region in REGIONS:
        series = load(fetch(cache, region, key))
        fidcal.log(f"  {region}: {len(series):,} hours")
        if series:
            all_series[region] = series
    counts = cached_counts(cache, all_series)

    study = fidcal.analyze("EIA hourly demand", FACT, tally(counts, STUDY_LAG_HOURS),
                           [h * fidcal.HOUR for h in STUDY_LAG_HOURS], truth=TRUTH)
    fidcal.log("echo test")
    echo_test = run(counts)
    regions = per_region(counts)
    study["echo_test"] = echo_test
    study["by_region"] = regions
    fidcal.write_results([study], "eia_demand", markdown(echo_test, regions))


if __name__ == "__main__":
    main()
