# How fast facts go stale: five findings from public ground truth

*fid calibration study. Data and code: [`calibration/`](../calibration/).
Interactive results: https://praveenprk.github.io/fid/*

## Summary

Systems that store facts treat their age in one of two ways: not at
all, or with a declared expiry (a TTL, a cache lifetime, a "review
every N days" policy). Both assume someone knows how fast each kind of
fact goes stale. We measured it.

Using four public datasets where the truth is known later — Wikidata
CEO tenures, a year of airport weather reports, all 7.08 million US
domestic flights in 2024, and 2.9 million position checks on 6,592
cargo ships and tankers — we fitted an exponential decay model per
fact type and scored it on held-out entities. Five findings:

1. **Declared lifetimes were wrong by 6× to 80×**, and one scored worse
   than ignoring age entirely.
2. **Facts decay toward a baseline, not toward zero.** Adding a
   baseline improves the fit wherever stale facts are right by chance,
   and the fit recovers a baseline of zero where they are not.
3. **The half-life depends on the state of the thing observed**, by a
   factor of about 680 for the same ships on the same days.
4. **Successive processing steps are not independent.** After a late
   flight leg the next is on time 33.8% of the time; after an on-time
   leg, 85.9%.
5. **Some facts come back.** A 24-hour-old temperature reading is right
   more often than a 12-hour-old one.

## Model and method

A stored fact has a probability of still matching reality, its
*fidelity*. fid's model, per fact type:

    S(age) = b + (s0 − b) × 0.5^(age / h)

`s0` is the fraction correct when fresh, `h` the half-life, and `b` the
baseline: the fraction correct once the fact is fully stale. `b = 0` is
plain exponential decay to zero, the form most decay schemes assume.

For each experiment we record observations `(entity, age, still
correct?)`: a fact taken at one moment and checked against reality
`age` later. Entities are split 50/50 by a hash of their id; models are
fitted on one half and scored on the other with the Brier score (mean
squared error of the predicted probability; lower is better). Every
model is compared against two baselines: the hand-picked half-life from
`examples/half-lives.yaml`, used as fid would use it, and a constant
that ignores age.

| Experiment | Fact | Still correct means | Observations |
|---|---|---|---|
| Wikidata CEOs | the CEO of company X is Y | Y has not left or been succeeded | 91,799 (283 companies, snapshots 2004–2016, ages to 10 years) |
| Airport weather (METAR, 10 US airports, 2024) | temperature is T | within ±2 °F | 1,575,505 |
| | flight category is C | VFR / MVFR / IFR / LIFR unchanged | 1,575,714 |
| | airport is IFR or worse | still IFR or LIFR | 105,909 |
| Ships (NOAA AIS, 6 days of 2023) | vessel is at position P | next report within 1 km | 2,945,357 (6,146 vessels) |
| Flights (BTS, all of 2024) | the k-th leg of an aircraft's day arrives on schedule | within 15 min, not cancelled or diverted | 7,046,561 legs |

## Finding 1: declared lifetimes are wrong

| Fact | Declared | Measured (time until half are wrong) | Off by |
|---|---|---|---|
| Company CEO | 1 year (`employee_role`) | 5.8 years | 5.8× too short |
| Ship position, underway | 4 hours (`shipping_status`) | 3.0 minutes | 80× too long |
| Ship position, moored | 4 hours | 1.4 days (fitted) | 8× too short |
| Airport IFR or worse | 6 hours (`weather_forecast`) | 5.1 hours | close |

On held-out data the declared CEO lifetime scored a Brier of 0.375,
against 0.251 for a constant that ignores age and 0.201 for the fitted
model. A wrong lifetime is worse than none. The one declared value that
landed close, 6 hours for adverse weather, did so by luck: it was
written for forecasts, not observations.

## Finding 2: decay toward a baseline

A week-old report that an airport is VFR is still right 73% of the
time, because clear weather is the normal state. Decay to zero cannot
represent that.

| Fact | Largest gap from the curve, decay to 0 | With baseline | Fitted baseline |
|---|---|---|---|
| Airport flight category | 0.184 | 0.031 | 0.73 |
| Airport IFR or worse | 0.178 | 0.078 | 0.20 |
| Ship position, all vessels | 0.314 | 0.067 | 0.62 |
| Company CEO | 0.035 | 0.035 | 0.00 |

The baseline also scores better on held-out data wherever it is
non-zero (flight category: 0.155 vs 0.166; all vessels: 0.217 vs
0.268). Where nothing is right by chance, as with a former CEO, the fit
finds `b = 0` and both models agree. A baseline costs nothing when it
is not needed.

A consequence: when the baseline sits above a decision threshold, the
fact never falls below it. "Expire after N days" is then the wrong
rule. The fact stays usable, at a known reduced confidence.

## Finding 3: the half-life depends on state

Same fact type, same ships, same days:

| State when observed | Half wrong after | Still within 1 km after 12 h |
|---|---|---|
| Underway (≥ 1 knot) | 3.0 minutes | 1.5% |
| Moored or at anchor | 1.4 days (fitted) | 79% |

The split held on each of six days across 2023. For a moving object
the half-life is close to physics, tolerance divided by speed: 1 km at
12 knots is about 2.7 minutes. A single half-life per fact type, the
usual design of expiry policies, is a blend of states that describes
neither.

## Finding 4: steps are not independent

Treating each leg of an aircraft's day as a processing step on the
day's schedule, the fleet-wide on-time rate falls about 4.5% per leg,
and a model that multiplies a fixed per-step loss fits that average
well. It describes no individual aircraft:

| Previous leg | Next leg on time | Leg pairs |
|---|---|---|
| On time | 85.9% | 4,285,796 |
| Late | 33.8% | 1,032,385 |
| Either | 75.8% | 5,318,181 |

Multiplying per-step loss assumes independence. Where the upstream
outcome is known, the loss should be conditioned on it: after a late
leg the step loses about 55% of fidelity, not 4.5%.

The same data shows that source reliability is not one number per
class. A published schedule is right 77.9% of the time overall, 69.5%
to 84.2% by airline, 68.1% (July) to 85.8% (October) by month, and
89.6% at 5 am against 68.1% at 8 pm.

## Finding 5: some facts come back

A temperature reading is right (±2 °F) 43% of the time at 24 hours old
and 19% at 12 hours, because the daily cycle returns it. No monotonic
decay model expresses this; the best one leaves a gap of 0.18. Periodic
facts need a periodic component, which fid does not yet have.

## What changed in fid

Each finding became a feature:

| Finding | In fid |
|---|---|
| 1 | `fid calibrate` fits half-life and baseline from your own re-check log and writes the registry |
| 2 | envelopes carry a `baseline`; `fid ttl` returns "never" when the baseline is above the floor |
| 3 | registries hold per-state half-lives: `--type vessel_position --state underway` |
| 4 | noise models hold per-condition noise: `--transform flight_leg --condition previous_leg_late` |
| 5 | open |

## Limitations

- **Thresholds are choices.** "Still correct" means ±2 °F, 1 km or 15
  minutes; half-lives scale with them.
- **Wikidata** covers notable companies whose CEO history is
  maintained, and missing end dates make tenures look longer.
- **Moored ships** were observed for 12 hours, so their 1.4-day
  half-life is an extrapolation.
- **The split is by entity, not by time.** Half-lives drift: adverse
  weather measured over the trailing twelve months gives 3.9 hours,
  against 5.1 hours for calendar 2024. Calibration has to be repeated,
  which is why the results site reruns it weekly.
- **None of these are organizational facts.** Policies, prices and
  procedures need their own calibration, from re-verification records
  (see `fid calibrate`).

## Reproducing

```bash
cd calibration
python3 wikidata_ceo.py && python3 weather_metar.py
python3 flights_bts.py && python3 ships_ais.py     # ~2.3 GB of downloads
```

Python standard library only. Results land in `calibration/results/`,
and `fid calibrate` reproduces the CEO fit from the same observations
(5.84 years, identical held-out Brier).
