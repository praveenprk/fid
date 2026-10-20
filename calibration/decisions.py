"""Does acting on fid beat a fixed re-check schedule? The decision benchmark.

Every study so far asks whether fid's trust numbers are honest. This one
asks whether they are useful: a store that re-checks a fact when fid says
its trust has fallen below a floor, against the same store re-checking
on a fixed schedule (a TTL), replayed on real histories where the truth
is known at every moment.

A store holds one fact per entity and re-reads the truth at each check.
Between checks it serves the stored value. Two costs:

  re-checks  checks per fact per day
  staleness  the fraction of time the served value is wrong: the chance
             that a decision made at a random moment acts on a wrong fact

Six kinds of fact, from the earlier studies:

  ship_position            AIS, 2 days of 2023, cargo ships and tankers, ±1 km;
                           state at the check: underway or moored
  airport_temperature      METAR, 10 US airports, 2024, ±2 °F
  airport_flight_category  same, VFR / MVFR / IFR / LIFR
  grid_demand              EIA, 14 balancing authorities, 2024, ±5%
  package_latest_version   PyPI, ~1,000 packages, 2015–2024;
                           state: age of the latest release at the check
  drug_label_warnings      DailyMed, 400 prescription labels, 2010 to Oct 2026

fid is run, not re-implemented: for each kind, the training entities'
re-checks are written as a ledger, `fid calibrate` fits the registry
entry (per state where there is one), and `fid stamp` + `fid ttl` give
the re-check interval for each floor. Every comparison is then scored
on the other entities (split by entity: vessel, airport, region,
package, label), which neither fid nor the tuning saw.

Policies:

  one TTL           one re-check interval for every fact
  tuned TTL         one interval per kind of fact, chosen on the training
                    entities to minimize staleness for each re-check budget
                    (the best a careful engineer could pick without fid)
  fid, no state     one floor for every fact; fid's interval per kind
  fid               one floor; fid's interval per kind and state

The rule, set before running:

  1. Per kind with a state, fid beats the best fixed interval for that
     kind if it needs fewer re-checks for the same held-out staleness at
     the 5%, 10% and 20% targets it reaches, with the bootstrap 95% range
     of the ratio below 1 (200 resamples of held-out entities).
  2. Across the six kinds, the same against the tuned TTL.
  3. The floor is an honest knob: on held-out entities the staleness fid
     predicts for each floor is within 0.02 of what happens, averaged over
     floors from 0.5 to 0.95, for every kind.

Usage: cargo build --release && python3 decisions.py
       (data as the earlier studies: ~640 MB of AIS, ~500 MB of PyPI,
        DailyMed and EIA caches; FID_EIA_API_KEY for EIA if not cached)
"""

import argparse
import bisect
import csv
import json
import math
import os
import random
import subprocess
from datetime import date, datetime, timezone

import fidcal

HERE = os.path.dirname(os.path.abspath(__file__))
FID_BIN = os.environ.get("FID_BIN", os.path.join(HERE, "..", "target", "release", "fid"))

T_GRID = [60.0 * 1.3 ** k for k in range(64)]  # 1 minute to ~30 years
FLOORS = [0.30, 0.40, 0.50, 0.60, 0.70, 0.75, 0.80, 0.85, 0.90, 0.93, 0.95, 0.97, 0.98, 0.99]
HONEST_FLOORS = [f for f in FLOORS if 0.5 <= f <= 0.95]
TARGETS = [0.02, 0.05, 0.10, 0.20]
RULE_TARGETS = [0.05, 0.10, 0.20]
LAMBDAS = [10 ** (k / 10) for k in range(-100, 41)]
BOOTSTRAP = 200
MIN_TTL = 60.0  # seconds: no store re-checks faster than once a minute
MAX_HONEST_ERROR = 0.02


def epoch(d):
    if isinstance(d, date) and not isinstance(d, datetime):
        d = datetime(d.year, d.month, d.day, tzinfo=timezone.utc)
    if d.tzinfo is None:
        d = d.replace(tzinfo=timezone.utc)
    return d.timestamp()


# ── what a store holds: two kinds of truth history ──────────────────────

class Series:
    """The truth sampled every `step` seconds; None where unknown.

    `match(stored, now)` says whether a stored value is still right;
    `state(value)` is the thing's state when it is read, or None.
    """

    def __init__(self, group, step, values, match, state=None):
        self.group, self.step, self.values, self.match, self.state = group, step, values, match, state


class Changes:
    """A value that changes at known times: values[i] holds from times[i] until times[i + 1].

    Observed from `start` to `end`. `state(t, since)` is the state at t,
    given the time the current value took effect, or None.
    """

    def __init__(self, group, times, values, start, end, state=None):
        self.group, self.times, self.values = group, times, values
        self.start, self.end, self.state = start, end, state


def fid_curve(model, age):
    s0, h, b = model
    return s0 if math.isinf(h) else b + (s0 - b) * 0.5 ** (age / h)


def predicted_stale_time(model, length):
    """∫₀ᴸ (1 − fidelity) dt: the staleness fid expects over one re-check interval, in seconds."""
    s0, h, b = model
    if math.isinf(h):
        return length * (1 - s0)
    return length * (1 - b) - (s0 - b) * h / math.log(2) * (1 - 0.5 ** (length / h))


# ── replaying a policy ───────────────────────────────────────────────────

def simulate(e, ttl, model=None):
    """(checks, stale seconds, known seconds, predicted stale seconds) for one entity.

    `ttl` is a fixed interval in seconds (math.inf: check once) or a
    function of the state. `model(state)` gives fid's (s0, h, b) to
    predict staleness, or None.
    """
    if isinstance(e, Series):
        return _simulate_series(e, ttl, model)
    if not callable(ttl) and model is None:
        return _fixed_changes(e, ttl)
    return _simulate_changes(e, ttl, model)


def _next_known(values, i):
    while i < len(values) and values[i] is None:
        i += 1
    return i


def _simulate_series(e, ttl, model):
    values, step, n_all = e.values, e.step, len(e.values)
    checks = stale = known = 0
    predicted = 0.0
    i = _next_known(values, 0)
    while i < n_all:
        held = values[i]
        state = e.state(held) if e.state else None
        t = ttl(state) if callable(ttl) else ttl
        j = n_all if math.isinf(t) else min(i + max(1, round(max(t, MIN_TTL) / step)), n_all)
        m = model(state) if model else None
        for k in range(i, j):
            now = values[k]
            if now is None:
                continue
            known += 1
            if not e.match(held, now):
                stale += 1
            if m:
                predicted += 1 - fid_curve(m, (k - i) * step)
        checks += 1
        i = _next_known(values, j)
    return checks, stale * step, known * step, predicted * step


def _value_index(e, t):
    return bisect.bisect_right(e.times, t) - 1


def stale_between(e, held, a, b):
    """Seconds in [a, b) where the value is not `held`."""
    i = bisect.bisect_right(e.times, a)
    current, pos, total = e.values[i - 1], a, 0.0
    while True:
        nxt = e.times[i] if i < len(e.times) and e.times[i] < b else b
        if current != held:
            total += nxt - pos
        if nxt >= b:
            return total
        pos, current, i = nxt, e.values[i], i + 1


def _fixed_changes(e, ttl):
    """A fixed interval over a change history in O(changes): only intervals with a change can be stale."""
    span = e.end - e.start
    t = max(ttl, MIN_TTL)
    checks = 1 if math.isinf(t) else math.ceil(span / t)
    stale, seen = 0.0, set()
    lo = bisect.bisect_right(e.times, e.start)
    hi = bisect.bisect_left(e.times, e.end)
    for c in range(lo, hi):
        k = 0 if math.isinf(t) else int((e.times[c] - e.start) // t)
        if k in seen:
            continue
        seen.add(k)
        a = e.start + (0 if math.isinf(t) else k * t)
        b = e.end if math.isinf(t) else min(a + t, e.end)
        stale += stale_between(e, e.values[_value_index(e, a)], a, b)
    return checks, stale, span, 0.0


def _simulate_changes(e, ttl, model):
    checks, stale, predicted, t = 0, 0.0, 0.0, e.start
    while t < e.end:
        i = _value_index(e, t)
        held = e.values[i]
        state = e.state(t, e.times[i]) if e.state else None
        interval = ttl(state) if callable(ttl) else ttl
        nxt = e.end if math.isinf(interval) else min(t + max(interval, MIN_TTL), e.end)
        stale += stale_between(e, held, t, nxt)
        if model:
            predicted += predicted_stale_time(model(state), nxt - t)
        checks += 1
        t = nxt
    return checks, stale, e.end - e.start, predicted


def fixed_grid(entities):
    """{interval: [simulate(e, interval) for e in entities]} for every interval in T_GRID.

    On a series, intervals that round to the same number of samples give
    the same replay, so each is run once.
    """
    out = {t: [] for t in T_GRID}
    for e in entities:
        done = {}
        for t in T_GRID:
            if isinstance(e, Series):
                key = min(max(1, round(max(t, MIN_TTL) / e.step)), len(e.values))
                if key not in done:
                    done[key] = simulate(e, key * e.step)
                out[t].append(done[key])
            else:
                out[t].append(simulate(e, t))
    return out


# ── ledgers for fid calibrate ────────────────────────────────────────────

def ledger_rows(e, lags, stride):
    """(entity, age seconds, correct, state): re-checks of a fact stored at regular start times."""
    if isinstance(e, Series):
        values, step = e.values, e.step
        for i in range(0, len(values), max(1, round(stride / step))):
            held = values[i]
            if held is None:
                continue
            state = e.state(held) if e.state else None
            for lag in lags:
                k = i + round(lag / step)
                if k < len(values) and values[k] is not None:
                    yield e.group, lag, e.match(held, values[k]), state
    else:
        t = e.start
        while t < e.end:
            i = _value_index(e, t)
            state = e.state(t, e.times[i]) if e.state else None
            for lag in lags:
                if t + lag <= e.end:
                    yield e.group, lag, e.values[_value_index(e, t + lag)] == e.values[i], state
            t += stride


def write_ledger(path, rows, with_state):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["entity", "age", "correct"] + (["state"] if with_state else []))
        n = 0
        for entity, age, correct, state in rows:
            w.writerow([entity, f"{age:.0f}", "true" if correct else "false"] + ([state] if with_state else []))
            n += 1
    return n


# ── running fid ──────────────────────────────────────────────────────────

def run_fid(args, stdin=""):
    if not os.path.exists(FID_BIN):
        raise SystemExit(f"no fid binary at {FID_BIN}: run `cargo build --release` or set FID_BIN")
    return subprocess.run([FID_BIN] + args, input=stdin, capture_output=True, text=True)


def calibrate(ledger, kind):
    """Registry YAML from `fid calibrate`, or None with its reason when it refuses."""
    out = run_fid(["calibrate", ledger, "--type", kind])
    if out.returncode != 0 or not out.stdout.strip():
        return None, (out.stderr.strip().splitlines() or ["no output"])[-1]
    return out.stdout, out.stderr.strip()


def parse_duration(s):
    """fid's duration format (compute.rs parse_duration)."""
    s = s.strip().lower()
    if s in ("infinite", "inf"):
        return math.inf
    for suffix, mult in (("mo", 30.44 * 86400), ("s", 1), ("m", 60), ("h", 3600), ("d", 86400), ("y", 365.25 * 86400)):
        if s.endswith(suffix):
            return float(s[: -len(suffix)]) * mult
    return float(s)


def parse_time(s):
    s = s.strip().replace("Z", "+00:00")
    if "." in s:  # fid writes nanoseconds; Python reads microseconds
        head, rest = s.split(".", 1)
        frac, zone = rest[:rest.index("+")], rest[rest.index("+"):]
        s = f"{head}.{frac[:6]}{zone}"
    return datetime.fromisoformat(s).timestamp()


def fid_policy(registry, kind, states):
    """{state: (s0, h, b)} and {floor: {state: TTL seconds}}, from `fid stamp` and `fid ttl`.

    Also the largest gap between fid's TTL and the closed form
    h × log2((s0 − b) / (floor − b)), as a check that the model used to
    predict staleness is the one fid runs.
    """
    models, ttls, worst = {}, {f: {} for f in FLOORS}, 0.0
    for state in states:
        args = ["stamp", "--source", "benchmark", "--source-class", "instrument", "--half-lives", registry, "--type", kind]
        if state is not None:
            args += ["--state", state]
        stamped = run_fid(args, "{}")
        if stamped.returncode != 0:
            raise RuntimeError(f"fid stamp {kind} {state}: {stamped.stderr.strip()}")
        env = json.loads(stamped.stdout)["fid"]
        s0, b = env["fid_at_stamp"], env.get("baseline") or 0.0
        h = parse_duration(env["half_life"]) if env.get("half_life") else math.inf
        models[state] = (s0, h, b)
        measured = parse_time(env["measured_at"])
        for floor in FLOORS:
            out = run_fid(["ttl", "--floor", str(floor), "-"], stamped.stdout).stdout.strip()
            ttl = math.inf if out == "never" else max(0.0, parse_time(out) - measured)
            ttls[floor][state] = ttl
            if not math.isinf(ttl) and floor < s0 and floor > b:
                worst = max(worst, abs(ttl - h * math.log2((s0 - b) / (floor - b))))
    return models, ttls, worst


# ── the data ─────────────────────────────────────────────────────────────

def haversine_km(a, b):
    import ships_ais
    return ships_ais.haversine_km(a, b)


def ship_entities(cache, per_day, days=("2023_01_15", "2023_07_15")):
    import ships_ais
    out = {}
    for day in days:
        url = ships_ais.URL.format(year=day[:4], day=day)
        tracks = ships_ais.load(fidcal.download(url, os.path.join(cache, "ais", f"AIS_{day}.zip")))
        chosen = sorted(tracks, key=lambda m: fidcal.hashlib.sha256(m.encode()).hexdigest())
        n = 0
        for mmsi in chosen:
            track = tracks[mmsi]
            lo, hi = min(track), max(track)
            if hi - lo < 6 * 60:  # at least six hours of reports
                continue
            values = [ships_ais.report_near(track, m) for m in range(lo, hi + 1)]
            out[f"{mmsi}/{day}"] = Series(
                mmsi, 60.0, values, lambda a, b: haversine_km(a, b) <= 1.0,
                lambda v: "underway" if v[2] >= ships_ais.UNDERWAY_KNOTS else "moored")
            n += 1
            if n >= per_day:
                break
        fidcal.log(f"  AIS {day}: {n} vessels")
        del tracks
    return out


def hourly(series_by_hour, pick):
    hours = sorted(series_by_hour)
    first = hours[0]
    n = int((hours[-1] - first).total_seconds() // 3600) + 1
    values = [None] * n
    for h in hours:
        values[int((h - first).total_seconds() // 3600)] = pick(series_by_hour[h])
    return values


def weather_entities(cache):
    import weather_metar
    temperature, category = {}, {}
    for station in weather_metar.STATIONS:
        series = weather_metar.load(weather_metar.fetch(cache, station))
        temperature[station] = Series(station, 3600.0, hourly(series, lambda v: v[0]),
                                      lambda a, b: abs(a - b) <= weather_metar.TEMP_TOLERANCE_F)
        category[station] = Series(station, 3600.0, hourly(series, lambda v: v[1]), lambda a, b: a == b)
    return temperature, category


def grid_entities(cache):
    import eia_demand
    out = {}
    for region in eia_demand.REGIONS:
        try:
            series = eia_demand.load(eia_demand.fetch(cache, region, os.environ.get("FID_EIA_API_KEY", "")))
        except Exception as e:
            fidcal.log(f"  EIA {region}: {e}")
            continue
        if series:
            out[region] = Series(region, 3600.0, hourly(series, lambda v: v),
                                 lambda a, b: abs(b - a) <= eia_demand.TOLERANCE * a)
    return out


PYPI_WINDOW = (epoch(date(2015, 1, 1)), epoch(date(2025, 1, 1)))


def package_entities(cache, n):
    import pypi_latest
    packages, _ = pypi_latest.fetch(cache, n)
    out = {}
    for name, changes in packages.items():
        times = [epoch(at) for at, _ in changes]
        start = max(PYPI_WINDOW[0], times[0])
        if start < PYPI_WINDOW[1]:
            out[name] = Changes(name, times, [v for _, v in changes], start, PYPI_WINDOW[1],
                                lambda t, since: pypi_latest.state(t - since))
    return out


DRUG_WINDOW = (epoch(date(2010, 1, 1)), epoch(date(2026, 10, 1)))


def label_entities(cache, n):
    import dailymed_warnings
    labels, _, _ = dailymed_warnings.load_labels(cache, n)
    out = {}
    for setid, changes in labels.items():
        times = [epoch(at) for at, _ in changes]
        texts = [t for _, t in changes]
        first = next(i for i, t in enumerate(texts) if t)
        start = max(DRUG_WINDOW[0], times[first])
        if start < DRUG_WINDOW[1]:
            out[setid] = Changes(setid, times, texts, start, DRUG_WINDOW[1])
    return out


def kinds(cache, args):
    """name → (entities, lags in seconds for the ledger, ledger stride in seconds, states or [None])."""
    import dailymed_warnings
    import pypi_latest
    import ships_ais
    H, D = fidcal.HOUR, fidcal.DAY
    hourly_lags = [h * H for h in (0, 1, 2, 3, 4, 6, 8, 12, 18, 24, 36, 48, 72)]
    fidcal.log("loading AIS")
    ships = ship_entities(cache, args.ships_per_day)
    fidcal.log("loading METAR")
    temperature, category = weather_entities(cache)
    fidcal.log("loading EIA")
    grid = grid_entities(cache)
    fidcal.log("loading PyPI")
    packages = package_entities(cache, args.packages)
    fidcal.log("loading DailyMed")
    labels = label_entities(cache, args.labels)
    out = {
        "ship_position": (ships, [m * 60.0 for m in ships_ais.LAG_MINUTES], 30 * 60.0, ["underway", "moored"]),
        "airport_temperature": (temperature, hourly_lags, 7 * H, [None]),
        "airport_flight_category": (category, hourly_lags, 7 * H, [None]),
        "grid_demand": (grid, hourly_lags, 7 * H, [None]),
        "package_latest_version": (packages, [float(l) for l in pypi_latest.LAGS], 30.44 * D,
                                   ["just_released", "quiet", "dormant"]),
        "drug_label_warnings": (labels, [float(l) for l in dailymed_warnings.LAGS], 30.44 * D, [None]),
    }
    return {k: v for k, v in out.items() if v[0]}


# ── scoring ──────────────────────────────────────────────────────────────

def split(entities):
    train = {k: e for k, e in entities.items() if fidcal.split_train(e.group)}
    test = {k: e for k, e in entities.items() if not fidcal.split_train(e.group)}
    return train, test


def summarize(rows, index=None):
    """Re-checks per fact per day, staleness and predicted staleness over entities (optionally resampled)."""
    pick = rows if index is None else [rows[i] for i in index]
    checks = sum(r[0] for r in pick)
    stale = sum(r[1] for r in pick)
    known = sum(r[2] for r in pick)
    predicted = sum(r[3] for r in pick)
    if known == 0:
        return None
    return {"rate": checks / (known / fidcal.DAY), "stale": stale / known, "predicted": predicted / known}


class Summaries:
    """summarize() of one kind's replays, on all held-out entities (b = None) or bootstrap resample b, cached."""

    def __init__(self, resamples):
        self.resamples, self.cache = resamples, {}

    def __call__(self, rows, b):
        key = (id(rows), b)
        if key not in self.cache:
            self.cache[key] = summarize(rows, None if b is None else self.resamples[b])
        return self.cache[key]


def rate_for(points, target):
    """Fewest re-checks per fact-day that reach staleness ≤ target, along a policy's curve.

    `points` are (rate, staleness), one per setting of the policy. The
    cheapest setting at or below the target, interpolated in log(rate)
    with the next cheaper setting above it; None if no setting reaches it.
    """
    above = None
    for rate, stale in sorted(points):
        if stale <= target:
            if above is None:
                return rate
            r0, s0 = above
            return math.exp(math.log(r0) + (s0 - target) / (s0 - stale) * (math.log(rate) - math.log(r0)))
        above = (rate, stale)
    return None


def ratio_with_range(points_a, points_b, target):
    """rate_a / rate_b at a staleness target, with the 2.5–97.5 percentile range over resamples.

    `points_x(b)` gives a policy's curve on all held-out entities (b = None) or on resample b.
    """
    def ratio(b):
        a, c = rate_for(points_a(b), target), rate_for(points_b(b), target)
        return a / c if a and c else None

    point = ratio(None)
    draws = sorted(r for r in (ratio(b) for b in range(BOOTSTRAP)) if r is not None)
    if point is None or len(draws) < 0.9 * BOOTSTRAP:
        return {"ratio": point, "low": None, "high": None, "resamples": len(draws)}
    return {"ratio": point, "low": draws[int(0.025 * len(draws))], "high": draws[int(0.975 * len(draws)) - 1],
            "resamples": len(draws)}


def verdict(r):
    if r["ratio"] is None:
        return "not reached"
    if r["high"] is None:
        return "no range"
    if r["high"] < 1:
        return "fid better"
    if r["low"] > 1:
        return "fid worse"
    return "tie"


# ── the benchmark ────────────────────────────────────────────────────────

def run(all_kinds, work):
    rng = random.Random(20261020)
    out = {"kinds": {}, "ttl_check_max_error_s": 0.0}
    per_kind = {}
    for name, (entities, lags, stride, states) in all_kinds.items():
        fidcal.log(f"{name}: {len(entities)} entities")
        train, test = split(entities)
        info = {"entities": len(entities), "train": len(train), "test": len(test),
                "train_groups": len({e.group for e in train.values()}),
                "test_groups": len({e.group for e in test.values()})}

        # fid, fitted on the training entities only
        variants = {"fid": states != [None], "fid_no_state": False} if states != [None] else {"fid": False}
        fitted = {}
        for variant, with_state in variants.items():
            ledger = os.path.join(work, f"{name}_{variant}.csv")
            info[f"{variant}_ledger_rows"] = write_ledger(
                ledger, (r for e in train.values() for r in ledger_rows(e, lags, stride)), with_state)
            registry, log = calibrate(ledger, name)
            info[f"{variant}_calibrate"] = log
            if registry is None:
                fidcal.log(f"  {variant}: fid calibrate refused: {log}")
                continue
            path = os.path.join(work, f"{name}_{variant}.yaml")
            with open(path, "w") as f:
                f.write(registry)
            info[f"{variant}_registry"] = registry
            models, ttls, worst = fid_policy(path, name, states if with_state else [None])
            out["ttl_check_max_error_s"] = max(out["ttl_check_max_error_s"], worst)
            fitted[variant] = (models, ttls)
            info[f"{variant}_models"] = {str(s): {"s0": m[0], "half_life_s": m[1], "baseline": m[2]}
                                         for s, m in models.items()}
            info[f"{variant}_ttl_s"] = {str(f): {str(s): t for s, t in v.items()} for f, v in ttls.items()}

        # Replays: every fixed interval on both halves (tuning needs the training half), fid on held-out
        test_list = list(test.values())
        fidcal.log(f"  fixed intervals ({len(T_GRID)})")
        fixed_train = fixed_grid(list(train.values()))
        fixed_test = fixed_grid(test_list)
        fid_test = {}
        for variant, (models, ttls) in fitted.items():
            fidcal.log(f"  {variant} floors ({len(FLOORS)})")
            model = (lambda m: (lambda s: m[s] if s in m else m[None]))(models)
            fid_test[variant] = {}
            for floor in FLOORS:
                table = ttls[floor]
                policy = (lambda tb: (lambda s: tb[s] if s in tb else tb[None]))(table)
                fid_test[variant][floor] = [simulate(e, policy, model) for e in test_list]
        summ = Summaries([[rng.randrange(len(test_list)) for _ in test_list] for _ in range(BOOTSTRAP)])
        per_kind[name] = dict(fixed_train=fixed_train, fixed_test=fixed_test, fid_test=fid_test, summ=summ)

        # Per kind: fid against the best fixed interval for this kind
        def curve_points(table, b):
            return [(s["rate"], s["stale"]) for s in (summ(rows, b) for rows in table.values()) if s]

        info["fixed_curve"] = [dict(ttl_s=t, **summ(rows, None)) for t, rows in fixed_test.items() if summ(rows, None)]
        for variant, by_floor in fid_test.items():
            curve = [dict(floor=f, **summ(rows, None)) for f, rows in by_floor.items() if summ(rows, None)]
            info[f"{variant}_curve"] = curve
            info[f"{variant}_vs_fixed"] = {
                str(t): ratio_with_range(lambda b, bf=by_floor: curve_points(bf, b), lambda b: curve_points(fixed_test, b), t)
                for t in TARGETS}
            honest = [abs(c["predicted"] - c["stale"]) for c in curve if c["floor"] in HONEST_FLOORS]
            info[f"{variant}_honesty_error"] = sum(honest) / len(honest) if honest else None
            info[f"{variant}_needed"] = {str(t): rate_for([(c["rate"], c["stale"]) for c in curve], t) for t in TARGETS}
        info["fixed_needed"] = {str(t): rate_for([(c["rate"], c["stale"]) for c in info["fixed_curve"]], t) for t in TARGETS}
        out["kinds"][name] = info

    out["portfolio"] = portfolio(per_kind)
    out["rule"] = rule(out)
    return out


def portfolio(per_kind):
    """All kinds at once, one fact of each: the average re-check rate and staleness across kinds."""
    names = list(per_kind)

    def mean_point(summaries):
        if any(s is None for s in summaries):
            return None
        return (sum(s["rate"] for s in summaries) / len(summaries), sum(s["stale"] for s in summaries) / len(summaries))

    # Tuned TTL: for each price of a re-check, each kind picks its interval on the training half
    train_summary = {n: {t: summarize(rows) for t, rows in per_kind[n]["fixed_train"].items()} for n in names}
    choices = []
    for lam in LAMBDAS:
        pick = {}
        for n in names:
            options = [(s["stale"] + lam * s["rate"], t) for t, s in train_summary[n].items() if s]
            pick[n] = min(options)[1]
        if pick not in choices:
            choices.append(pick)

    def summ(name, rows, b):
        return per_kind[name]["summ"](rows, b)

    def points(policy, b):
        pts = []
        if policy == "one_ttl":
            for t in T_GRID:
                p = mean_point([summ(n, per_kind[n]["fixed_test"][t], b) for n in names])
                if p:
                    pts.append(p)
        elif policy == "tuned_ttl":
            for pick in choices:
                p = mean_point([summ(n, per_kind[n]["fixed_test"][pick[n]], b) for n in names])
                if p:
                    pts.append(p)
        else:
            for floor in FLOORS:
                rows = []
                for n in names:
                    fid = per_kind[n]["fid_test"]
                    variant = policy if policy in fid else "fid"
                    if variant not in fid:
                        return []
                    rows.append(summ(n, fid[variant][floor], b))
                p = mean_point(rows)
                if p:
                    pts.append(p)
        return pts

    policies = ["one_ttl", "tuned_ttl", "fid_no_state", "fid"]
    result = {"kinds": names, "tuned_choices": len(choices), "needed": {}, "vs_tuned": {}, "curves": {}}
    for p in policies:
        pts = points(p, None)
        result["curves"][p] = [{"rate": r, "stale": s} for r, s in pts]
        result["needed"][p] = {str(t): rate_for(pts, t) for t in TARGETS}
    for p in ("one_ttl", "fid_no_state", "fid"):
        result["vs_tuned"][p] = {
            str(t): ratio_with_range(lambda b, p=p: points(p, b), lambda b: points("tuned_ttl", b), t)
            for t in TARGETS}
    return result


def rule(out):
    r = {"per_kind_with_state": {}, "honest_knob": {}}
    for name, info in out["kinds"].items():
        if "fid_no_state_curve" in info:  # kinds with a state
            r["per_kind_with_state"][name] = {t: verdict(info["fid_vs_fixed"][str(t)]) for t in RULE_TARGETS
                                               if info["fid_vs_fixed"][str(t)]["ratio"] is not None}
        if info.get("fid_honesty_error") is not None:
            r["honest_knob"][name] = info["fid_honesty_error"] <= MAX_HONEST_ERROR
    r["portfolio_vs_tuned"] = {t: verdict(out["portfolio"]["vs_tuned"]["fid"][str(t)]) for t in RULE_TARGETS}
    return r


# ── output ───────────────────────────────────────────────────────────────

def fmt_rate(r):
    if r is None:
        return "—"
    if r >= 1:
        return f"{r:,.1f}/day"
    days = 1 / r
    return f"every {fidcal.fmt_duration(days * fidcal.DAY)}"


def fmt_ratio(x):
    if x["ratio"] is None:
        return "—"
    rng = f" ({x['low']:.2f}–{x['high']:.2f})" if x["high"] is not None else ""
    return f"{x['ratio']:.2f}{rng}"


def markdown(out):
    L = ["# Decision benchmark: re-check by fid, or on a fixed schedule?", "",
         "Re-checks needed per fact to keep held-out staleness at or below each target, "
         "and the ratio fid ÷ best fixed interval (95% range over 200 resamples of held-out entities; "
         "below 1 means fid needs fewer re-checks).", ""]
    for name, k in out["kinds"].items():
        L += [f"## {name}", "",
              f"{k['entities']} entities ({k['train_groups']} train / {k['test_groups']} held-out groups).", ""]
        for variant in ("fid", "fid_no_state"):
            if f"{variant}_models" in k:
                ms = ", ".join(
                    f"{s if s != 'None' else 'all'}: half-life {fidcal.fmt_duration(m['half_life_s'])}, "
                    f"baseline {m['baseline']:.2f}, s0 {m['s0']:.3f}" for s, m in k[f"{variant}_models"].items())
                L.append(f"- `fid calibrate` on the training half ({variant.replace('_', ' ')}): {ms}")
            elif f"{variant}_calibrate" in k:
                L.append(f"- `fid calibrate` refused ({variant.replace('_', ' ')}): {k[f'{variant}_calibrate']}")
        L += ["", "| Staleness target | Best fixed interval | fid | fid ÷ fixed |" +
              (" fid, no state | no state ÷ fixed |" if "fid_no_state_curve" in k else ""),
              "|---|---|---|---|" + ("---|---|" if "fid_no_state_curve" in k else "")]
        for t in TARGETS:
            row = f"| {t:.0%} | {fmt_rate(k['fixed_needed'][str(t)])} | "
            row += f"{fmt_rate(k.get('fid_needed', {}).get(str(t)))} | {fmt_ratio(k['fid_vs_fixed'][str(t)]) if 'fid_vs_fixed' in k else '—'} |"
            if "fid_no_state_curve" in k:
                row += f" {fmt_rate(k['fid_no_state_needed'][str(t)])} | {fmt_ratio(k['fid_no_state_vs_fixed'][str(t)])} |"
            L.append(row)
        if "fid_curve" in k:
            L += ["", "The floor as a knob (held-out): staleness fid predicts against what happened.", "",
                  "| Floor | Re-checks | Predicted staleness | Actual |", "|---|---|---|---|"]
            for c in k["fid_curve"]:
                L.append(f"| {c['floor']:.2f} | {fmt_rate(c['rate'])} | {c['predicted']:.3f} | {c['stale']:.3f} |")
            L += ["", f"Mean |predicted − actual| for floors 0.5–0.95: **{k['fid_honesty_error']:.3f}**", ""]
        L.append("")
    p = out["portfolio"]
    L += ["## All kinds together", "",
          "One fact of each kind; re-checks and staleness averaged across kinds. The tuned TTL picks one interval per "
          "kind on the training half for each re-check budget.", "",
          "| Staleness target | One TTL | Tuned TTL per kind | fid, no state | fid |", "|---|---|---|---|---|"]
    for t in TARGETS:
        L.append(f"| {t:.0%} | " + " | ".join(fmt_rate(p["needed"][q][str(t)])
                                              for q in ("one_ttl", "tuned_ttl", "fid_no_state", "fid")) + " |")
    L += ["", "| Staleness target | One TTL ÷ tuned | fid, no state ÷ tuned | fid ÷ tuned |", "|---|---|---|---|"]
    for t in TARGETS:
        L.append(f"| {t:.0%} | " + " | ".join(fmt_ratio(p["vs_tuned"][q][str(t)])
                                              for q in ("one_ttl", "fid_no_state", "fid")) + " |")
    r = out["rule"]
    L += ["", "## The rule", ""]
    for name, v in r["per_kind_with_state"].items():
        L.append(f"- {name}, fid against the best fixed interval: " +
                 ", ".join(f"{t:.0%}: **{x}**" for t, x in v.items()))
    L.append("- All kinds, fid against the tuned TTL: " +
             ", ".join(f"{t:.0%}: **{x}**" for t, x in r["portfolio_vs_tuned"].items()))
    L.append("- Honest knob (error ≤ 0.02): " +
             ", ".join(f"{n}: **{'yes' if ok else 'no'}**" for n, ok in r["honest_knob"].items()))
    L += ["", f"fid's TTL against the closed form: largest gap {out['ttl_check_max_error_s']:.3g} s.", ""]
    return "\n".join(L)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ships-per-day", type=int, default=250)
    ap.add_argument("--packages", type=int, default=1000)
    ap.add_argument("--labels", type=int, default=400)
    args = ap.parse_args(argv)
    cache = fidcal.default_cache_dir()
    work = os.path.join(cache, "decisions")
    out = run(kinds(cache, args), work)
    with open(os.path.join(HERE, "results", "decisions.json"), "w") as f:
        json.dump(fidcal._finite(out), f, indent=1)
        f.write("\n")
    md = markdown(out)
    with open(os.path.join(HERE, "results", "decisions.md"), "w") as f:
        f.write(md)
    print(md)


if __name__ == "__main__":
    main()
