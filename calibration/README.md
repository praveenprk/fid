# Calibration: testing fid's model against reality

`fid` originally scored a fact as `source × 0.5^(age / half-life) × ∏(1 − noise)`.
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
| Airport temperature (±2 °F) | `weather_forecast: 6h` | **2.9 hours**, then cycles daily: an echo every 24h (strength 0.31) beats every monotonic model on held-out data | METAR, 10 US airports, all of 2024, every lag to 14 days |
| Airport flight category (VFR/IFR…) | `weather_forecast: 6h` | **2.9 hours**, decaying to 73%, not 0 | same |
| Airport is IFR or worse | `weather_forecast: 6h` | **5.1 hours** | same, 5,906 adverse reports |
| Ship position (±1 km), underway | `shipping_status: 4h` | **3.0 minutes** | NOAA AIS, 6,592 cargo ships and tankers, 6 days across 2023, 2.9M observations |
| Ship position (±1 km), moored or at anchor | `shipping_status: 4h` | **1.4 days** (fitted; 79% still within 1 km after 12 h) | same |
| Flight schedule is true (source fidelity) | — | **77.9%** on time; 69.5–84.2% by airline, 68.1–85.8% by month | BTS, 7,079,061 US flights, all of 2024 |
| Noise per flight leg (transform noise) | — | **0.045** per leg on average | same |
| The latest version of package X is Y, just released | — | **29 days** (5% still latest after a year) | PyPI, 977 most downloaded packages, monthly snapshots 2015–2024, 1.0M observations |
| The latest version of package X is Y, 30 days to 1 year old | — | **80 days** | same |
| The latest version of package X is Y, no release for a year | — | **230 days**, decaying to 24% | same |
| LLM extraction from a document (transform noise) | `llm_extract: 0.20` | **0.000**: 0 errors in 4,800 field checks | generated company documents, Claude Opus 5.5 and Haiku 5.5, 200 documents per run |
| LLM summary, then extraction | `summarize: 0.15` | **0.015** Opus 5.5 (0.019 at low effort), **0.070** Haiku 5.5 | same, hard corpus |
| LLM plain-language rewrite, then extraction | — | **0.002** Opus 5.5, **0.001** Haiku 5.5 | same, hard corpus |

Full tables, curves and held-out reliability: [`results/`](results/).

## What the data says about the model

**1. The hand-picked half-lives are wrong — by 6× to 80×.** CEO facts
last 5.8 years, not 1. A moving ship's position lasts 3 minutes, a
moored one's 1.4 days; the guess of 4 hours fits neither. On held-out
data the 1-year CEO guess scores *worse than ignoring age altogether*
(Brier 0.375 vs 0.251). Half-lives have to be measured, not declared.

**2. Facts decay toward a baseline, not toward zero.** A stale "the
airport is VFR" is still right 73% of the time, because VFR is the
usual weather. fid's formula sends every stale fact to 0. The model

    fidelity = b + (s0 − b) × 0.5^(age / half-life)

where `b` is the chance a stale fact is right anyway, fits better or
the same on every dataset: flight-category shape error drops from 0.184
to 0.031, all-ship 1 km from 0.314 to 0.067. Where `b` really is 0 —
a former CEO is never the CEO by chance — the fit finds `b = 0` and the
two models agree.

**3. Half-life depends on the thing's state, not only its type.** Same
fact type, same ships, same days: 3 minutes underway, 1.4 days moored,
about 680× apart, and the same across six days spread through the year.
For a moving object the half-life is roughly physics — distance
tolerance ÷ speed (1 km at 12 knots ≈ 2.7 minutes). One half-life per
information type, as `half-lives.yaml` assumes, is not enough. Software
shows the same: "the latest version of X is Y" lasts 29 days when Y
has just come out and 230 days when the package has been quiet for a
year, 8× apart, and the state (the age of the latest release) is known
at the moment the fact is stored.

**4. Transform noise is not independent.** fid multiplies per-step
noise, which assumes steps are independent. After an on-time leg, the
next leg is on time 85.9% of the time; after a late leg, 33.8%, over
5.3 million leg pairs in 2024. The average per-leg noise (0.045) fits the fleet-wide curve well, but for
any one aircraft the previous leg's outcome matters far more than the
average. Noise should be conditioned on what is known upstream.

**5. Some facts come back.** Temperature is periodic: a 24-hour-old
reading is right 43% of the time, a 12-hour-old one 19%. No monotonic
decay model — exponential or with a baseline — can express that.

**6. LLM steps lose facts by leaving them out, not by getting them
wrong.** The hand-picked 0.20 for extraction and 0.15 for summaries
are 10× to 100× too high for short business documents. Extraction
never took the bait: not a former CEO, an older headcount, a revenue
figure corrected later, a forecast, a subsidiary's figures, a former
headquarters or name, or a competitor's numbers. Every one of the 128
errors across the three hard-corpus runs was an empty field, mostly in
summaries, and mostly the minor facts (Haiku 5.5 leaves the fiscal
year end out of 26.5% of summaries). Burying the facts among unrelated
figures is what raises the loss: summary noise 0.028 against 0.003
for Opus 5.5, 0.113 against 0.031 for Haiku 5.5. The model matters
more than the effort setting.

**Source fidelity varies within a class.** A flight schedule is true
78% of the time overall, but 69.5% for one airline and 84.2% for
another, 89.6% for 5 am departures versus 68.1% for 8 pm, and 68.1% in
July versus 85.8% in October. One number per source class is too coarse.

## Caveats

- Wikidata covers notable companies with maintained CEO histories (283
  companies had a CEO at a snapshot date); its missing end dates make
  CEO facts look longer-lived than they are.
- Ships: six days (the 15th of every other month, 2023), each observed for
  12 hours, so long moored half-lives are extrapolated. Flights: 2024.
  Weather: ten US airports.
- "Still correct" thresholds (±2 °F, 1 km, 15 minutes) are choices; the
  half-life scales with them.
- The held-out split is by entity, not by time.
- AI noise: short generated English documents with every fact stated.
  Long reports, tables, scanned text or other languages may lose more.
  A summary that drops the fiscal month is not a bad summary; the
  number is what the summary loses for a later extraction.

## Running

Python 3 standard library only. Each script downloads its data once
into `calibration/.cache/` (override with `FID_CAL_CACHE`) and writes
`results/<name>.json` and `results/<name>.md`.

```bash
cd calibration
python3 wikidata_ceo.py    # ~2 MB, Wikidata SPARQL
python3 weather_metar.py   # ~10 MB, Iowa Environmental Mesonet
python3 weather_periodic.py  # same data; the echo test, every lag to 14 days (slow: tens of minutes)
python3 flights_bts.py     # ~340 MB (12 months), US Bureau of Transportation Statistics
python3 ships_ais.py       # ~1.9 GB (6 days), NOAA MarineCadastre AIS
python3 pypi_latest.py     # ~500 MB (1,000 package histories), PyPI JSON API
```

### AI transform noise

`ai_noise.py` measures how much an LLM step loses: extraction, summary
then extraction, rewrite then extraction, on documents whose true
content is known (generated company records with real-world traps: a
former CEO, an older headcount, revenue in billions, a competitor's
figures). It writes the noise per step, field and trap as a
`noise-model.yaml` block. Unlike the others it calls the Claude API, so
it needs `pip install anthropic` and a key in `FID_ANTHROPIC_API_KEY`
(cloud sessions keep `ANTHROPIC_API_KEY` for their own sign-in, and
network secrets are never sent to `api.anthropic.com`), and it costs
money: 5 requests per document, 1,000 at the default 200 documents.
Responses are cached, so reruns are free.

`--hard` adds traps a strong model can miss: facts buried among
unrelated figures, a revenue figure corrected later in the text, a
forecast, a subsidiary with its own city, chief and headcount, an
earlier headquarters, and an earlier company name. The standard corpus
does not change. Each model, effort and corpus writes its own file
(`results/ai_noise_hard_claude-haiku-5-5.md`, for example); the default
run writes `results/ai_noise.md`.

```bash
python3 ai_noise.py --dry-run          # the corpus and request count, no API calls
python3 ai_noise.py --docs 200         # default model claude-opus-5-5; --model, --effort
python3 ai_noise.py --docs 200 --hard  # the harder corpus
python3 -m unittest test_ai_noise      # the pipeline against a fake client, no API calls
```
