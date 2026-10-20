"""Does acting on fid beat a fixed re-check schedule? The decision benchmark.

A store holds one fact per entity and re-reads the truth at each check;
between checks it serves the stored value. Two costs: re-checks per fact
per day, and staleness, the fraction of time the served value is wrong
(the chance that a decision made at a random moment acts on a wrong
fact). Six kinds of fact, replayed on real histories:

  ship_position            AIS cargo ships and tankers, ±1 km; state at the check:
                           underway or stationary
  airport_temperature      METAR, ±2 °F
  airport_flight_category  METAR; state: the stored category (a stale VFR is
                           usually right, a stale IFR is not)
  grid_demand              EIA hourly demand, ±5%
  package_latest_version   PyPI; state: age of the latest release at the check
  drug_label_warnings      DailyMed label versions

Entities are split in two (by vessel, airport, region, package, label).
The scored half keeps the first quarter of each entity's window as its
own history and is scored on the rest; every policy is scored on the
same three quarters. fid is run, not re-implemented: `fid calibrate
--entity-baselines` fits on the training half and the scored half's
history, and the fid Python package (the same Rust core) gives each
fact's TTL and re-check schedule from its stamped envelope.

Policies:
  fixed interval    one re-check interval per kind (the curve over all intervals)
  one TTL           one interval for every fact
  tuned TTL         an interval per kind, chosen on the training half for each budget
  fid floor         re-check when trust falls below one floor (v1's policy)
  fid schedule      `fid schedule --price`: one price per re-check for every kind
                    and state, each fact's interval from its own curve
  fid schedule + floor 0.5   the same, never later than the floor's TTL

Two sets: `dev` (the data of the first benchmark, used to build the
fixes) and `confirm` (data neither the fixes nor this file's settings
have seen: other AIS days, other airports, other balancing authorities,
the next 1,000 PyPI packages, the next 400 labels).

The rule, set before the confirm run (a ratio is fid's re-checks ÷ the
other policy's for the same held-out staleness; its 95% range is over
200 resamples of held-out entities):

  1. Across the six kinds, fid schedule beats the tuned TTL: range below
     1 at 5%, 10% and 20% staleness.
  2. For each kind with a state, fid schedule beats the best fixed
     interval: range below 1 at each of those targets it reaches.
  3. For each kind without a state, fid schedule loses nothing: the
     range's upper end is at most 1.05 at each target it reaches.
  4. The schedule's predicted staleness is within 0.02 of what happens,
     averaged over the prices whose staleness is 1% to 30%, for every kind.

Usage: cargo build --release
       (cd ../bindings/python && maturin develop --release)   # the fid package
       python3 decisions.py --set dev        # or --set confirm
"""

import argparse
import bisect
import csv
import hashlib
import json
import math
import os
import random
import subprocess
import time
from datetime import date, datetime, timedelta, timezone

import fidcal

try:
    import fid
except ImportError:  # the benchmark runs fid; it cannot stand in for it
    fid = None

HERE = os.path.dirname(os.path.abspath(__file__))
FID_BIN = os.environ.get("FID_BIN", os.path.join(HERE, "..", "target", "release", "fid"))

T_GRID = [60.0 * 1.3 ** k for k in range(64)]  # 1 minute to ~30 years
FLOORS = [0.30, 0.40, 0.50, 0.60, 0.70, 0.75, 0.80, 0.85, 0.90, 0.93, 0.95, 0.97, 0.98, 0.99]
PRICES = [10 ** (k / 5) for k in range(-40, 6)]  # 1e-8 to 10 staleness per daily re-check
GUARD_FLOOR = 0.5
HONEST_FLOORS = [f for f in FLOORS if 0.5 <= f <= 0.95]
HONEST_STALENESS = (0.01, 0.30)
TARGETS = [0.02, 0.05, 0.10, 0.20]
RULE_TARGETS = [0.05, 0.10, 0.20]
LAMBDAS = [10 ** (k / 10) for k in range(-100, 41)]
BOOTSTRAP = 200
MIN_TTL = 60.0  # seconds: no store re-checks faster than once a minute
MAX_HONEST_ERROR = 0.02
MAX_NO_LOSS = 1.05
MIN_STATE_SHARE = 0.01
HISTORY = 0.25  # share of each scored entity's window that is its own history
T0 = datetime(2026, 1, 1, tzinfo=timezone.utc)

SETS = {
    "dev": dict(ais_days=("2023_01_15", "2023_07_15"),
                stations=("JFK", "ORD", "ATL", "DFW", "DEN", "LAX", "SFO", "SEA", "MIA", "BOS"),
                regions=("ERCO", "PJM", "MISO", "CISO", "NYIS", "ISNE", "SWPP", "FPL", "TVA", "SOCO", "DUK", "BPAT",
                         "PACE", "AZPS"),
                packages=(0, 1000), labels=(0, 400)),
    "confirm": dict(ais_days=("2023_03_15", "2023_09_15"),
                    stations=("PHX", "LAS", "MSP", "DTW", "IAH", "CLT", "PHL", "SLC", "MCO", "PDX"),
                    regions=("BANC", "LDWP", "NEVP", "PSCO", "WACM", "SC", "SCEG", "TEC", "FMPP", "IPCO", "PGE", "PSEI",
                             "SRP", "TEPC"),
                    packages=(1000, 1000), labels=(400, 400)),
}


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
    given the time the current value took effect, or None. `boundaries`
    (seconds after a change) are the only other times the state can
    change; None if unknown, which makes replays check every interval.
    """

    def __init__(self, group, times, values, start, end, state=None, boundaries=None):
        self.group, self.times, self.values = group, times, values
        self.start, self.end, self.state, self.boundaries = start, end, state, boundaries


def fid_curve(model, age):
    """fid's fidelity at `age` for model (s0, h, b, share, h_fast): two clocks toward a baseline."""
    s0, h, b, share, hf = model
    if math.isinf(h):
        return s0
    decay = (1 - share) * 0.5 ** (age / h) + (share * 0.5 ** (age / hf) if share else 0.0)
    return b + (s0 - b) * decay


def _decay_integral(length, h):
    if math.isinf(h):
        return length
    return -h / math.log(2) * math.expm1(-length * math.log(2) / h)


def predicted_stale_time(model, length):
    """∫₀ᴸ (1 − fidelity) dt: the staleness fid expects over one re-check interval, in seconds."""
    s0, h, b, share, hf = model
    if math.isinf(h):
        return length * (1 - s0)
    decay = (1 - share) * _decay_integral(length, h) + (share * _decay_integral(length, hf) if share else 0.0)
    return length * (1 - b) - (s0 - b) * decay


# ── replaying a policy ───────────────────────────────────────────────────

def simulate(e, ttl, model=None):
    """(checks, stale seconds, known seconds, predicted stale seconds, by state) for one entity.

    `by state` is {state: [stale seconds, known seconds]}, by the thing's
    true state at each moment (a ship moving now, a package released last
    week), so a policy that neglects one state shows it.

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


def _add(by_state, state, stale, known):
    cell = by_state.get(state)
    if cell is None:
        cell = by_state[state] = [0.0, 0.0]
    cell[0] += stale
    cell[1] += known


def _simulate_series(e, ttl, model):
    values, step, n_all = e.values, e.step, len(e.values)
    checks = stale = known = 0
    predicted = 0.0
    by_state = {}
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
            wrong = not e.match(held, now)
            if wrong:
                stale += 1
            _add(by_state, e.state(now) if e.state else None, step if wrong else 0.0, step)
            if m:
                predicted += 1 - fid_curve(m, (k - i) * step)
        checks += 1
        i = _next_known(values, j)
    return checks, stale * step, known * step, predicted * step, by_state


def _value_index(e, t):
    return bisect.bisect_right(e.times, t) - 1


def segments(e, a, b):
    """(from, to, value, state) pieces of [a, b), split at changes and state boundaries."""
    i = _value_index(e, a)
    pos = a
    while pos < b:
        since = e.times[i]
        following = e.times[i + 1] if i + 1 < len(e.times) else math.inf
        end = min([following, b] + [since + d for d in e.boundaries or () if since + d > pos])
        yield pos, end, e.values[i], e.state(pos, since) if e.state else None
        if end >= following:
            i += 1
        pos = end


def stale_by_state(e, held, a, b, by_state):
    """Seconds in [a, b) where the value is not `held`, added to by_state[state][0]; returns the total."""
    total = 0.0
    for u0, u1, value, state in segments(e, a, b):
        if value != held and u1 > u0:
            total += u1 - u0
            _add(by_state, state, u1 - u0, 0.0)
    return total


def _known_by_state(e, by_state):
    for u0, u1, _, state in segments(e, e.start, e.end):
        _add(by_state, state, 0.0, u1 - u0)
    return by_state


def _fixed_changes(e, ttl):
    """A fixed interval over a change history in O(changes): only intervals with a change can be stale."""
    span = e.end - e.start
    t = max(ttl, MIN_TTL)
    checks = 1 if math.isinf(t) else math.ceil(span / t)
    stale, seen, by_state = 0.0, set(), {}
    lo = bisect.bisect_right(e.times, e.start)
    hi = bisect.bisect_left(e.times, e.end)
    for c in range(lo, hi):
        k = 0 if math.isinf(t) else int((e.times[c] - e.start) // t)
        if k in seen:
            continue
        seen.add(k)
        a = e.start + (0 if math.isinf(t) else k * t)
        b = e.end if math.isinf(t) else min(a + t, e.end)
        stale += stale_by_state(e, e.values[_value_index(e, a)], a, b, by_state)
    return checks, stale, span, 0.0, _known_by_state(e, by_state)


def _simulate_changes(e, ttl, model):
    checks, stale, predicted, t, by_state = 0, 0.0, 0.0, e.start, {}
    while t < e.end:
        i = _value_index(e, t)
        held = e.values[i]
        state = e.state(t, e.times[i]) if e.state else None
        interval = ttl(state) if callable(ttl) else ttl
        if not math.isinf(interval):
            interval = max(interval, MIN_TTL)
        if not math.isinf(interval) and (e.state is None or e.boundaries is not None):
            # Checks before the next change or state boundary all find the same value and state:
            # count them at once and replay only the last, whose interval may hold the change
            horizon = min([e.end] + [e.times[i] + b for b in e.boundaries or () if e.times[i] + b > t] +
                          ([e.times[i + 1]] if i + 1 < len(e.times) else []))
            same = math.ceil((horizon - t) / interval) - 1
            if same > 0:
                checks += same
                if model:
                    predicted += same * predicted_stale_time(model(state), interval)
                t += same * interval
        nxt = e.end if math.isinf(interval) else min(t + interval, e.end)
        stale += stale_by_state(e, held, t, nxt, by_state)
        if model:
            predicted += predicted_stale_time(model(state), nxt - t)
        checks += 1
        t = nxt
    return checks, stale, e.end - e.start, predicted, _known_by_state(e, by_state)


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


# ── an entity's own history ──────────────────────────────────────────────

def history_split(e, share=HISTORY):
    """(history, scored): the first `share` of the entity's window, and the rest."""
    if isinstance(e, Series):
        k = int(len(e.values) * share)
        return (Series(e.group, e.step, e.values[:k], e.match, e.state),
                Series(e.group, e.step, e.values[k:], e.match, e.state))
    cut = e.start + share * (e.end - e.start)
    return (Changes(e.group, e.times, e.values, e.start, cut, e.state, e.boundaries),
            Changes(e.group, e.times, e.values, cut, e.end, e.state, e.boundaries))


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


def ledger_id(group):
    # fid calibrate splits entities with the same hash as this benchmark, so every training
    # entity would land in one of its halves; a fixed prefix gives it a split of its own
    return f"ledger/{group}"


def write_ledger(path, rows, with_state):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["entity", "age", "correct"] + (["state"] if with_state else []))
        n = 0
        for entity, age, correct, state in rows:
            w.writerow([ledger_id(entity), f"{age:.0f}", "true" if correct else "false"] + ([state] if with_state else []))
            n += 1
    return n


# ── running fid ──────────────────────────────────────────────────────────

def run_fid(args):
    if not os.path.exists(FID_BIN):
        raise SystemExit(f"no fid binary at {FID_BIN}: run `cargo build --release` or set FID_BIN")
    return subprocess.run([FID_BIN] + args, capture_output=True, text=True)


def calibrate(ledger, kind, baselines_path):
    """Registry YAML and per-entity baselines from `fid calibrate`, or (None, None, reason) when it refuses."""
    out = run_fid(["calibrate", ledger, "--type", kind, "--entity-baselines", baselines_path])
    if out.returncode != 0 or not out.stdout.strip():
        return None, None, (out.stderr.strip().splitlines() or ["no output"])[-1]
    baselines = {}
    with open(baselines_path, newline="") as f:
        for row in csv.DictReader(f):
            baselines[(row["entity"], row["state"] or None)] = float(row["baseline"])
    return out.stdout, baselines, out.stderr.strip()


class FidPolicy:
    """fid's decisions for one kind, from its calibrated registry: per (state, baseline), a stamped
    envelope, and from it the TTL at each floor and the re-check interval at each price."""

    def __init__(self, registry, kind, baselines):
        self.registry, self.kind, self.baselines = registry, kind, baselines
        self.specs, self.cache = {}, {}

    def spec(self, state):
        if state not in self.specs:
            self.specs[state] = fid.registry(self.registry, self.kind, state)
        return self.specs[state]

    def baseline(self, group, state):
        """The entity's own baseline when calibrate wrote one, else the state's; to 0.001."""
        b = self.baselines.get((ledger_id(group), state), self.spec(state)["baseline"] or 0.0)
        return round(b, 3)

    def decisions(self, state, b):
        """(model, {floor: ttl}, {price: interval}, {price: guarded interval}) in seconds; inf for never."""
        key = (state, b)
        if key in self.cache:
            return self.cache[key]
        spec = self.spec(state)
        stamped = fid.stamp({}, source="benchmark", source_class="instrument", half_life=spec["half_life"], baseline=b,
                            source_fidelity=spec["source_fidelity"], fast=spec["fast"], now=T0)
        env = stamped["fid"]
        h = fid.parse_half_life(env["half_life"])
        fast = env.get("fast")
        share, hf = (fast["share"], fid.parse_half_life(fast["half_life"])) if fast else (0.0, 0.0)
        model = (env["fid_at_stamp"], h, b, share, hf)

        def seconds(at):
            return math.inf if at is None else (at - T0).total_seconds()

        ttls = {fl: seconds(fid.expires_at(stamped, fl)) for fl in FLOORS}
        every = {p: seconds(fid.recheck_at(stamped, price=p)) for p in PRICES}
        guarded = {p: seconds(fid.recheck_at(stamped, price=p, floor=GUARD_FLOOR)) for p in PRICES}
        self.cache[key] = (model, ttls, every, guarded)
        return self.cache[key]


# ── the data ─────────────────────────────────────────────────────────────

def ship_state(v):
    # At anchor and moored were tried as separate states on the dev set: fid calibrate refused
    # at anchor (half-life 1.8 to 5.9 days, beyond 24-hour tracks), so they stay one state
    return "underway" if v[2] >= 1.0 else "stationary"


def ship_entities(cache, days, per_day):
    import ships_ais
    out = {}
    for day in days:
        url = ships_ais.URL.format(year=day[:4], day=day)
        tracks = ships_ais.load(fidcal.download(url, os.path.join(cache, "ais", f"AIS_{day}.zip")), status=True)
        chosen = sorted(tracks, key=lambda m: hashlib.sha256(m.encode()).hexdigest())
        n = 0
        for mmsi in chosen:
            track = tracks[mmsi]
            lo, hi = min(track), max(track)
            if hi - lo < 6 * 60:  # at least six hours of reports
                continue
            values = [ships_ais.report_near(track, m) for m in range(lo, hi + 1)]
            out[f"{mmsi}/{day}"] = Series(mmsi, 60.0, values, lambda a, b: ships_ais.haversine_km(a, b) <= 1.0, ship_state)
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


def weather_entities(cache, stations):
    import weather_metar
    temperature, category = {}, {}
    for station in stations:
        series = weather_metar.load(weather_metar.fetch(cache, station))
        if not series:
            continue
        temperature[station] = Series(station, 3600.0, hourly(series, lambda v: v[0]),
                                      lambda a, b: abs(a - b) <= weather_metar.TEMP_TOLERANCE_F)
        category[station] = Series(station, 3600.0, hourly(series, lambda v: v[1]), lambda a, b: a == b, lambda v: v)
    return temperature, category


def grid_entities(cache, regions):
    import eia_demand
    out, failed = {}, []
    for region in regions:
        try:
            series = eia_demand.load(eia_demand.fetch(cache, region, os.environ.get("FID_EIA_API_KEY", "")))
        except Exception as e:  # a balancing authority the API does not serve
            fidcal.log(f"  EIA {region}: {e}")
            failed.append(region)
            continue
        if series:
            out[region] = Series(region, 3600.0, hourly(series, lambda v: v),
                                 lambda a, b: abs(b - a) <= eia_demand.TOLERANCE * a)
    if not out:
        raise RuntimeError(f"no EIA region could be read ({', '.join(failed)})")
    return out


PYPI_WINDOW = (epoch(date(2015, 1, 1)), epoch(date(2025, 1, 1)))


def package_entities(cache, start, n):
    import pypi_latest
    packages, _ = pypi_latest.fetch(cache, n, start)
    out = {}
    for name, changes in packages.items():
        times = [epoch(at) for at, _ in changes]
        first = max(PYPI_WINDOW[0], times[0])
        if first < PYPI_WINDOW[1]:
            out[name] = Changes(name, times, [v for _, v in changes], first, PYPI_WINDOW[1],
                                lambda t, since: pypi_latest.state(t - since),
                                (pypi_latest.JUST_RELEASED, pypi_latest.DORMANT))
    return out


DRUG_WINDOW = (epoch(date(2010, 1, 1)), epoch(date(2026, 10, 1)))


def label_entities(cache, skip, n):
    import dailymed_warnings
    labels, _, _ = dailymed_warnings.load_labels(cache, n, skip)
    out = {}
    for setid, changes in labels.items():
        times = [epoch(at) for at, _ in changes]
        texts = [t for _, t in changes]
        first = next(i for i, t in enumerate(texts) if t)
        start = max(DRUG_WINDOW[0], times[first])
        if start < DRUG_WINDOW[1]:
            out[setid] = Changes(setid, times, texts, start, DRUG_WINDOW[1])
    return out


def kinds(cache, which, ships_per_day):
    """name → (entities, ledger lags in seconds, ledger stride in seconds, has states),
    and {source: error} for the sources that could not be reached."""
    import dailymed_warnings
    import pypi_latest
    import ships_ais
    cfg = SETS[which]
    H, D = fidcal.HOUR, fidcal.DAY
    hourly_lags = [h * H for h in (0, 1, 2, 3, 4, 6, 8, 12, 18, 24, 36, 48, 72)]
    unavailable = {}

    def load(source, loader):
        """A source that cannot be reached is left out and reported, not replaced."""
        fidcal.log(f"loading {source}")
        try:
            return loader()
        except Exception as e:
            fidcal.log(f"  {source} unavailable: {e}")
            unavailable[source] = str(e)
            return None

    ships = load("AIS (coast.noaa.gov)", lambda: ship_entities(cache, cfg["ais_days"], ships_per_day))
    weather = load("METAR (mesonet.agron.iastate.edu)", lambda: weather_entities(cache, cfg["stations"])) or ({}, {})
    grid = load("EIA (api.eia.gov)", lambda: grid_entities(cache, cfg["regions"]))
    packages = load("PyPI (pypi.org)", lambda: package_entities(cache, *cfg["packages"]))
    labels = load("DailyMed (dailymed.nlm.nih.gov)", lambda: label_entities(cache, *cfg["labels"]))
    out = {
        "ship_position": (ships, [m * 60.0 for m in ships_ais.LAG_MINUTES], 30 * 60.0, True),
        "airport_temperature": (weather[0], hourly_lags, 7 * H, False),
        "airport_flight_category": (weather[1], hourly_lags, 7 * H, True),
        "grid_demand": (grid, hourly_lags, 7 * H, False),
        "package_latest_version": (packages, [float(l) for l in pypi_latest.LAGS], 30.44 * D, True),
        "drug_label_warnings": (labels, [float(l) for l in dailymed_warnings.LAGS], 30.44 * D, False),
    }
    return {k: v for k, v in out.items() if v[0]}, unavailable


# ── scoring ──────────────────────────────────────────────────────────────

def split(entities):
    train = {k: e for k, e in entities.items() if fidcal.split_train(e.group)}
    test = {k: e for k, e in entities.items() if not fidcal.split_train(e.group)}
    return train, test


def summarize(rows, index=None):
    """Re-checks per fact per day, staleness, predicted staleness and the worst state's staleness."""
    pick = rows if index is None else [rows[i] for i in index]
    checks = sum(r[0] for r in pick)
    stale = sum(r[1] for r in pick)
    known = sum(r[2] for r in pick)
    predicted = sum(r[3] for r in pick)
    if known == 0:
        return None
    states = {}
    for r in pick:
        for state, (s_, k_) in r[4].items():
            _add(states, state, s_, k_)
    worst = max(s_ / k_ for s_, k_ in states.values() if k_ >= MIN_STATE_SHARE * known)
    return {"rate": checks / (known / fidcal.DAY), "stale": stale / known, "predicted": predicted / known,
            "worst": worst, "by_state": {str(st): s_ / k_ for st, (s_, k_) in states.items() if k_ > 0}}


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


def verdict(r, no_loss=False):
    if r["ratio"] is None:
        return "not reached"
    if r["high"] is None:
        return "no range"
    if no_loss:
        return "no loss" if r["high"] <= MAX_NO_LOSS else "loss"
    if r["high"] < 1:
        return "fid better"
    if r["low"] > 1:
        return "fid worse"
    return "tie"


# ── the benchmark ────────────────────────────────────────────────────────

FID_POLICIES = ("fid_floor", "fid_schedule", "fid_schedule_guarded")


def run(all_kinds, work):
    rng = random.Random(20261020)
    out = {"kinds": {}, "ttl_check_max_error_s": 0.0}
    per_kind = {}
    for name, (entities, lags, stride, has_states) in all_kinds.items():
        began = time.monotonic()
        fidcal.log(f"{name}: {len(entities)} entities")
        train, test = split(entities)
        halves = {k: history_split(e) for k, e in test.items()}
        scored = [s for _, s in halves.values()]
        info = {"entities": len(entities), "train_groups": len({e.group for e in train.values()}),
                "test_groups": len({e.group for e in test.values()}), "has_states": has_states}

        # fid, fitted on the training half and the scored half's own history
        ledger = os.path.join(work, f"{name}.csv")
        sources = list(train.values()) + [h for h, _ in halves.values()]
        info["ledger_rows"] = write_ledger(ledger, (r for e in sources for r in ledger_rows(e, lags, stride)), has_states)
        registry, baselines, log = calibrate(ledger, name, os.path.join(work, f"{name}_baselines.csv"))
        info["calibrate"] = log
        policy = None
        if registry is None:
            fidcal.log(f"  fid calibrate refused: {log}")
        else:
            path = os.path.join(work, f"{name}.yaml")
            with open(path, "w") as f:
                f.write(registry)
            info["registry"] = registry
            info["entity_baselines"] = len(baselines)
            policy = FidPolicy(path, name, baselines)

        fidcal.log(f"  fixed intervals ({len(T_GRID)})")
        fixed_train = fixed_grid(list(train.values()))
        fixed_test = fixed_grid(scored)
        fid_test = {p: {} for p in FID_POLICIES} if policy else {}
        if policy:
            fidcal.log(f"  fid: {len(FLOORS)} floors, {len(PRICES)} prices")
            for e in scored:
                tables = {}

                def table_for(state, e=e, tables=tables):
                    if state not in tables:
                        tables[state] = policy.decisions(state, policy.baseline(e.group, state))
                    return tables[state]

                for which, settings, pick in (("fid_floor", FLOORS, 1), ("fid_schedule", PRICES, 2),
                                              ("fid_schedule_guarded", PRICES, 3)):
                    for setting in settings:
                        ttl = (lambda s, setting=setting, pick=pick: table_for(s)[pick][setting])
                        fid_test[which].setdefault(setting, []).append(simulate(e, ttl, lambda s: table_for(s)[0]))
            info["fid_states"] = sorted({str(k[0]) for k in policy.cache})
            info["fid_models"] = {f"{k[0]}|{k[1]}": v[0] for k, v in sorted(policy.cache.items(), key=str)[:200]}
        summ = Summaries([[rng.randrange(len(scored)) for _ in scored] for _ in range(BOOTSTRAP)])
        per_kind[name] = dict(fixed_train=fixed_train, fixed_test=fixed_test, fid_test=fid_test, summ=summ)

        def curve_points(table, b, key="stale"):
            return [(s["rate"], s[key]) for s in (summ(rows, b) for rows in table.values()) if s]

        info["fixed_curve"] = [dict(ttl_s=t, **summ(rows, None)) for t, rows in fixed_test.items() if summ(rows, None)]
        info["fixed_needed"] = {str(t): rate_for(curve_points(fixed_test, None), t) for t in TARGETS}
        info["fixed_worst_needed"] = {str(t): rate_for(curve_points(fixed_test, None, "worst"), t) for t in TARGETS}
        for which, table in fid_test.items():
            curve = [dict(setting=s, **summ(rows, None)) for s, rows in table.items() if summ(rows, None)]
            info[f"{which}_curve"] = curve
            info[f"{which}_needed"] = {str(t): rate_for(curve_points(table, None), t) for t in TARGETS}
            info[f"{which}_vs_fixed"] = {
                str(t): ratio_with_range(lambda b, tb=table: curve_points(tb, b), lambda b: curve_points(fixed_test, b), t)
                for t in TARGETS}
            info[f"{which}_worst_vs_fixed"] = {
                str(t): ratio_with_range(lambda b, tb=table: curve_points(tb, b, "worst"),
                                         lambda b: curve_points(fixed_test, b, "worst"), t)
                for t in TARGETS}
            if which == "fid_floor":
                honest = [abs(c["predicted"] - c["stale"]) for c in curve if c["setting"] in HONEST_FLOORS]
            else:
                lo, hi = HONEST_STALENESS
                honest = [abs(c["predicted"] - c["stale"]) for c in curve if lo <= c["stale"] <= hi]
            info[f"{which}_honesty_error"] = (sum(honest) / len(honest), len(honest)) if honest else None
        info["seconds"] = time.monotonic() - began
        fidcal.log(f"  {name} done in {info['seconds']:.0f}s")
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
        pick = {n: min((s["stale"] + lam * s["rate"], t) for t, s in train_summary[n].items() if s)[1] for n in names}
        if pick not in choices:
            choices.append(pick)

    def summ(name, rows, b):
        return per_kind[name]["summ"](rows, b)

    def points(policy, b):
        pts = []
        if policy == "one_ttl":
            settings = [{n: t for n in names} for t in T_GRID]
        elif policy == "tuned_ttl":
            settings = choices
        else:
            if any(policy not in per_kind[n]["fid_test"] for n in names):
                return []
            for setting in per_kind[names[0]]["fid_test"][policy]:
                p = mean_point([summ(n, per_kind[n]["fid_test"][policy][setting], b) for n in names])
                if p:
                    pts.append(p)
            return pts
        for pick in settings:
            p = mean_point([summ(n, per_kind[n]["fixed_test"][pick[n]], b) for n in names])
            if p:
                pts.append(p)
        return pts

    policies = ["one_ttl", "tuned_ttl"] + list(FID_POLICIES)
    result = {"kinds": names, "tuned_choices": len(choices), "needed": {}, "vs_tuned": {}, "curves": {}}
    for p in policies:
        pts = points(p, None)
        result["curves"][p] = [{"rate": r, "stale": s} for r, s in pts]
        result["needed"][p] = {str(t): rate_for(pts, t) for t in TARGETS}
    for p in ["one_ttl"] + list(FID_POLICIES):
        result["vs_tuned"][p] = {
            str(t): ratio_with_range(lambda b, p=p: points(p, b), lambda b: points("tuned_ttl", b), t) for t in TARGETS}
    return result


def rule(out):
    r = {"1_all_kinds_vs_tuned": {}, "2_states_vs_fixed": {}, "3_no_state_no_loss": {}, "4_honest_schedule": {}}
    r["1_all_kinds_vs_tuned"] = {t: verdict(out["portfolio"]["vs_tuned"]["fid_schedule"][str(t)]) for t in RULE_TARGETS}
    for name, k in out["kinds"].items():
        if "fid_schedule_vs_fixed" not in k:
            r["2_states_vs_fixed" if k["has_states"] else "3_no_state_no_loss"][name] = {"refused": k["calibrate"]}
            r["4_honest_schedule"][name] = False
            continue
        cmp = k["fid_schedule_vs_fixed"]
        reached = [t for t in RULE_TARGETS if cmp[str(t)]["ratio"] is not None]
        if k["has_states"]:
            r["2_states_vs_fixed"][name] = {t: verdict(cmp[str(t)]) for t in reached}
        else:
            r["3_no_state_no_loss"][name] = {t: verdict(cmp[str(t)], no_loss=True) for t in reached}
        h = k["fid_schedule_honesty_error"]
        r["4_honest_schedule"][name] = h is not None and h[0] <= MAX_HONEST_ERROR
    passed = (all(v == "fid better" for v in r["1_all_kinds_vs_tuned"].values())
              and all(isinstance(v, dict) and "refused" not in v and v and all(x == "fid better" for x in v.values())
                      for v in r["2_states_vs_fixed"].values())
              and all(isinstance(v, dict) and "refused" not in v and all(x == "no loss" for x in v.values())
                      for v in r["3_no_state_no_loss"].values())
              and all(r["4_honest_schedule"].values()))
    r["passed"] = passed
    return r


# ── output ───────────────────────────────────────────────────────────────

def fmt_rate(r):
    if r is None:
        return "—"
    if r >= 1:
        return f"{r:,.1f}/day"
    return f"every {fidcal.fmt_duration(fidcal.DAY / r)}"


def fmt_ratio(x):
    if x is None or x["ratio"] is None:
        return "—"
    rng = f" ({x['low']:.2f}–{x['high']:.2f})" if x["high"] is not None else ""
    return f"{x['ratio']:.2f}{rng}"


NAMES = {"fid_floor": "fid floor", "fid_schedule": "fid schedule", "fid_schedule_guarded": "schedule + floor 0.5",
         "one_ttl": "one TTL", "tuned_ttl": "tuned TTL"}


def markdown(out, which):
    L = [f"# Decision benchmark v2 ({which} set): fid schedule against fixed intervals", "",
         "Re-checks per fact needed to keep held-out staleness at or below each target; ratios are fid's re-checks "
         "÷ the other policy's (95% range over 200 resamples of held-out entities; below 1: fid needs fewer).", ""]
    for name, k in out["kinds"].items():
        L += [f"## {name}", "", f"{k['entities']} entities ({k['train_groups']} train / {k['test_groups']} scored groups); "
              f"states: {'yes' if k['has_states'] else 'no'}.", ""]
        if "registry" not in k:
            L += [f"`fid calibrate` refused: {k['calibrate']}", ""]
            continue
        L += ["`fid calibrate` (training half and the scored half's history):", "", "```yaml",
              k["registry"].strip(), "```", "", f"Entity baselines written: {k['entity_baselines']}.", "",
              "| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |",
              "|---|---|---|---|---|---|---|---|"]
        for t in TARGETS:
            q = str(t)
            L.append(f"| {t:.0%} | {fmt_rate(k['fixed_needed'][q])} | " + " | ".join(
                f"{fmt_rate(k[f'{w}_needed'][q])} | {fmt_ratio(k[f'{w}_vs_fixed'][q])}" for w in FID_POLICIES[1:] + FID_POLICIES[:1]) + " |")
        L += ["", "Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |", "|---|---|---|"]
        for t in TARGETS:
            L.append(f"| {t:.0%} | {fmt_rate(k['fixed_worst_needed'][str(t)])} | {fmt_ratio(k['fid_schedule_worst_vs_fixed'][str(t)])} |")
        h = k.get("fid_schedule_honesty_error")
        L += ["", f"Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: "
              f"**{h[0]:.3f}** ({h[1]} prices)" if h else "Schedule honesty: no price in range.", ""]
        L += ["| Price | Re-checks | Predicted | Actual |", "|---|---|---|---|"]
        for c in k["fid_schedule_curve"]:
            if HONEST_STALENESS[0] / 2 <= c["stale"] <= 0.5:
                L.append(f"| {c['setting']:.1e} | {fmt_rate(c['rate'])} | {c['predicted']:.3f} | {c['stale']:.3f} |")
        L.append("")
    p = out["portfolio"]
    L += ["## All kinds together", "", "One fact of each kind; re-checks and staleness averaged across kinds.", "",
          "| Staleness | " + " | ".join(NAMES[q] for q in ["one_ttl", "tuned_ttl"] + list(FID_POLICIES)) + " |",
          "|---|" + "---|" * (2 + len(FID_POLICIES))]
    for t in TARGETS:
        L.append(f"| {t:.0%} | " + " | ".join(fmt_rate(p["needed"][q][str(t)]) for q in ["one_ttl", "tuned_ttl"] + list(FID_POLICIES)) + " |")
    L += ["", "| Staleness | " + " | ".join(f"{NAMES[q]} ÷ tuned" for q in ["one_ttl"] + list(FID_POLICIES)) + " |",
          "|---|" + "---|" * (1 + len(FID_POLICIES))]
    for t in TARGETS:
        L.append(f"| {t:.0%} | " + " | ".join(fmt_ratio(p["vs_tuned"][q][str(t)]) for q in ["one_ttl"] + list(FID_POLICIES)) + " |")
    if out.get("unavailable"):
        L += ["", "Not included, because the source could not be reached: " + "; ".join(out["unavailable"]) + "."]
    r = out["rule"]
    L += ["", "## The rule", "",
          "1. All kinds, fid schedule against the tuned TTL: " + ", ".join(f"{t:.0%} **{x}**" for t, x in r["1_all_kinds_vs_tuned"].items())]
    for n, v in r["2_states_vs_fixed"].items():
        L.append(f"2. {n}, against the best fixed interval: " + (f"refused ({v['refused']})" if "refused" in v else
                                                                  ", ".join(f"{t:.0%} **{x}**" for t, x in v.items())))
    for n, v in r["3_no_state_no_loss"].items():
        L.append(f"3. {n}, no loss (upper end ≤ {MAX_NO_LOSS}): " + (f"refused ({v['refused']})" if "refused" in v else
                                                                   ", ".join(f"{t:.0%} **{x}**" for t, x in v.items())))
    L.append("4. Honest schedule (gap ≤ 0.02): " + ", ".join(f"{n} **{'yes' if ok else 'no'}**" for n, ok in r["4_honest_schedule"].items()))
    L += ["", f"**Rule passed: {'yes' if r['passed'] else 'no'}**", ""]
    return "\n".join(L)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--set", choices=sorted(SETS), default="dev")
    ap.add_argument("--ships-per-day", type=int, default=250)
    args = ap.parse_args(argv)
    if fid is None:
        raise SystemExit("the fid Python package is needed: (cd ../bindings/python && maturin develop --release)")
    cache = fidcal.default_cache_dir()
    work = os.path.join(cache, "decisions", args.set)
    loaded, unavailable = kinds(cache, args.set, args.ships_per_day)
    out = run(loaded, work)
    out["set"] = args.set
    out["unavailable"] = unavailable
    slug = f"decisions_{args.set}"
    with open(os.path.join(HERE, "results", f"{slug}.json"), "w") as f:
        json.dump(fidcal._finite(out), f, indent=1)
        f.write("\n")
    md = markdown(out, args.set)
    with open(os.path.join(HERE, "results", f"{slug}.md"), "w") as f:
        f.write(md)
    print(md)


if __name__ == "__main__":
    main()
