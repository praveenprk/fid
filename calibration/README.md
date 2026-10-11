# Calibration: testing fid's model against reality

`fid` scores a fact as `source × 0.5^(age / half-life) × ∏(1 − noise)`.
Every number in that formula — the half-lives in
`examples/half-lives.yaml`, the source scores, the per-step noise — was
hand-picked. These experiments check them against public data where the
truth is known: the stored fact at one moment, and what reality said
later.

Each experiment fits the model on half the entities (companies,
airports, aircraft, ships) and scores it on the other half with the
Brier score (mean squared error of a probability; lower is better),
against two baselines: the hand-picked half-life, and a constant that
ignores age entirely.

## Results

| Fact | Hand-picked guess | Measured half-life | Data |
|---|---|---|---|
| The CEO of company X is Y | `employee_role: 1y` | **5.8 years** | Wikidata, 2,239 CEO facts × 10 years |
| Airport temperature (±2 °F) | `weather_forecast: 6h` | **2.9 hours**, then cycles daily | METAR, 10 US airports, all of 2024 |
| Airport flight category (VFR/IFR…) | `weather_forecast: 6h` | **2.9 hours**, decaying to 73%, not 0 | same |
| Airport is IFR or worse | `weather_forecast: 6h` | **5.1 hours** | same, 5,906 adverse reports |
| Ship position (±1 km), underway | `shipping_status: 4h` | **3.2 minutes** | NOAA AIS, 1,657 cargo ships and tankers, 15 Mar 2023 |
| Ship position (±1 km), moored or at anchor | `shipping_status: 4h` | **1.6 days** | same |
| Flight schedule is true (source fidelity) | — | **72.9%** on time; 59–80% by airline | BTS, 547,271 US flights, Jan 2024 |
| Noise per flight leg (transform noise) | — | **0.034** per leg on average | same |

Full tables, curves and held-out reliability: [`results/`](results/).

## What the data says about the model

**1. The hand-picked half-lives are wrong — by 6× to 75×.** CEO facts
last 5.8 years, not 1. A moving ship's position lasts 3 minutes, a
moored one's 1.6 days; the guess of 4 hours fits neither. On held-out
data the 1-year CEO guess scores *worse than ignoring age altogether*
(Brier 0.375 vs 0.251). Half-lives have to be measured, not declared.

**2. Facts decay toward a baseline, not toward zero.** A stale "the
airport is VFR" is still right 73% of the time, because VFR is the
usual weather. fid's formula sends every stale fact to 0. The model

    fidelity = b + (s0 − b) × 0.5^(age / half-life)

where `b` is the chance a stale fact is right anyway, fits better or
the same on every dataset: flight-category shape error drops from 0.184
to 0.031, all-ship 1 km from 0.242 to 0.065. Where `b` really is 0 —
a former CEO is never the CEO by chance — the fit finds `b = 0` and the
two models agree.

**3. Half-life depends on the thing's state, not only its type.** Same
fact type, same ships, same day: 3 minutes underway, 1.6 days moored.
For a moving object the half-life is roughly physics — distance
tolerance ÷ speed (1 km at 12 knots ≈ 2.7 minutes). One half-life per
information type, as `half-lives.yaml` assumes, is not enough.

**4. Transform noise is not independent.** fid multiplies per-step
noise, which assumes steps are independent. After an on-time leg, the
next leg is on time 84.6% of the time; after a late leg, 32.8%. The
average per-leg noise (0.034) fits the fleet-wide curve well, but for
any one aircraft the previous leg's outcome matters far more than the
average. Noise should be conditioned on what is known upstream.

**5. Some facts come back.** Temperature is periodic: a 24-hour-old
reading is right 43% of the time, a 12-hour-old one 19%. No monotonic
decay model — exponential or with a baseline — can express that.

**Source fidelity varies within a class.** A flight schedule is true
73% of the time overall, but 59% for one airline and 80% for another,
and 81% for 6 am departures versus 67% for 6 pm. One number per source
class is too coarse.

## Caveats

- Wikidata covers notable companies with maintained CEO histories (283
  companies had a CEO at a snapshot date); its missing end dates make
  CEO facts look longer-lived than they are.
- Ships: one day. Flights: one winter month. Weather: ten US airports.
- "Still correct" thresholds (±2 °F, 1 km, 15 minutes) are choices; the
  half-life scales with them.
- The held-out split is by entity, not by time.

## Running

Python 3 standard library only. Each script downloads its data once
into `calibration/.cache/` (override with `FID_CAL_CACHE`) and writes
`results/<name>.json` and `results/<name>.md`.

```bash
cd calibration
python3 wikidata_ceo.py    # ~2 MB, Wikidata SPARQL
python3 weather_metar.py   # ~10 MB, Iowa Environmental Mesonet
python3 flights_bts.py     # ~28 MB, US Bureau of Transportation Statistics
python3 ships_ais.py       # ~320 MB, NOAA MarineCadastre AIS
```
