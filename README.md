# fid

Information fidelity measurement for computing systems.

`fid` measures the degree to which stored data still corresponds to
the reality it represents. Fidelity is a continuous, time-varying
quantity that decays through temporal aging, lossy transformation,
and the evolution of external reality.

Every other engineering discipline measures this. Computing never has.

## The Problem

A byte on disk from 2019 and a byte from today are structurally
identical. The system treats them identically. But the reality the
2019 byte represents may have moved. The bits haven't changed. The
correspondence has decayed.

Audio engineering calls this fidelity. Signal processing calls it
signal integrity. Metrology calls it calibration validity. Computing
has no name for it, no measurement for it, and no primitive for it.

`fid` is that primitive.

## Usage

```bash
# Measure fidelity of a data artifact
fid report.json

# Scan a data store
fid scan ./records/

# Check fidelity against a threshold (exit codes carry epistemic state)
fid check patient-record.json --floor 0.70

# Stamp fidelity envelope onto pipeline output
cat raw.json | transform | fid stamp --source transform.py --noise 0.08 | classify | fid stamp --source classify.py --noise 0.12 > output.json

# Inspect the fidelity chain
fid chain output.json

# Look up measured half-lives and noise instead of passing numbers
cat ais.json | fid stamp --source ais-366999 --source-class instrument \
    --half-lives half-lives.yaml --type vessel_position --state underway
cat leg.json | fid stamp --source leg-3 \
    --noise-model noise-model.yaml --transform flight_leg --condition previous_leg_late

# When does this fact fall below the floor? (a TTL derived from measured decay)
fid ttl record.json --floor 0.70      # prints expires_at, or "never"

# Measure your own half-lives from facts you re-checked, instead of guessing
fid calibrate ledger.csv --type policy_claim > half-lives.yaml

# Filter to signal only
cat records.jsonl | fid filter --floor 0.50 > signal.jsonl
```

## Exit Codes

```
0 — SIGNAL    fidelity at or above threshold
1 — NOISE     fidelity below floor
2 — UNKNOWN   source fidelity indeterminate
3 — ERROR     bad arguments, unreadable or malformed input
```

## Three Axes of Fidelity

**Temporal decay.** Every information type has a half-life. A stock
price decays in seconds. A mailing address decays over years. A
mathematical constant never decays. The half-life can also depend on
the state of the thing observed: a ship's position goes stale in
minutes while it is underway, and in days while it is moored.

**Transformation noise.** Every processing step — join, filter,
aggregate, infer, summarize — introduces noise. Signal degrades at
every step. Three lossy transforms can push fidelity below the noise
floor even with zero temporal decay.

**Source reliability.** A calibrated sensor starts at high fidelity.
An ungrounded LLM inference starts low. The source sets the ceiling.
Everything after can only decrease it.

```
fidelity = baseline + (source × transform_noise − baseline) × temporal_decay(t)
```

The baseline is the chance a fully stale fact is right anyway — a
week-old "the airport is VFR" is still right 73% of the time, because
VFR is the usual weather. With no baseline this is
`source × temporal_decay(t) × transform_noise`.

It never spontaneously increases. New measurement is the only
restoration mechanism.

## Configuration

```yaml
# half-lives.yaml — decay per information type, optionally per state
stock_price:      30s
mailing_address:   3y
company_ceo:      5.8y               # measured
airport_flight_category:             # measured; decays toward a baseline
  half_life: 2.9h
  baseline:  0.73
vessel_position:                     # measured; depends on state
  states:
    underway: { half_life: 1.9m, baseline: 0.02 }
    moored:   1.4d

# noise-model.yaml — per-transform noise, optionally per upstream condition
normalize:   0.02
summarize:   0.15
flight_leg:                          # measured
  noise: 0.045
  conditions:
    previous_leg_late: 0.55

# floor.yaml — noise floor thresholds
medical:      0.70
financial:    0.60
operational:  0.40
```

## Install

The package is `fidmeter` (the name `fid` is taken on crates.io and
PyPI); the command, the Rust library and the Python import are all `fid`.

```bash
cargo install fidmeter        # the fid command
pip install fidmeter          # import fid
```

Until the first release is published, build from source:

```bash
cargo install --path .                               # the fid command
cd bindings/python && pip install maturin && maturin develop --release
```

## What fid Is Not

- Not a database. It measures data stores. It is not one.
- Not a governance system. It measures fidelity, not policy.
- Not a cache invalidation tool. Cache is about stale copies.
  Fidelity is about correspondence to reality.
- Not a monitoring tool. Monitoring checks if systems run.
  fid checks if data is true.
- Not AI-specific. AI made the gap urgent. The gap is universal.

## Theoretical Foundations

Shannon (1948) — channel noise degrades mutual information.
Landauer (1961) — information erasure has thermodynamic cost.
Bennett (2003) — copying has thermodynamic cost.
Metrology — every measurement has calibration validity windows.
Thermodynamics — entropy increases; correspondence decays.

78 years of information science. Zero imports into computing
infrastructure. Until now.

## License

Apache 2.0

## Python

```python
import fid
artifact = fid.stamp(record, source="crm", source_class="database", half_life="3y")
fid.check(artifact, floor=0.7).status      # Status.SIGNAL / NOISE / UNKNOWN
fid.expires_at(artifact, floor=0.7)        # when it falls below the floor
```

The Python package calls the same Rust library, so the scores are
identical. See [bindings/python](bindings/python/README.md).

## Calibrate your own data

`fid calibrate` fits a half-life and baseline from facts that were
checked again later, and writes the registry `fid stamp --half-lives`
reads. Input is CSV: `entity`, `correct`, and either `age` (seconds or a
duration like `30d`) or `observed_at` and `checked_at` (RFC 3339);
an optional `state` column fits one half-life per state.

```bash
$ fid calibrate ledger.csv --type company_ceo > half-lives.yaml
company_ceo — 91799 observations, 283 entities
  half-life 5.84y, baseline 0.00, correct when fresh 1.000
  held-out Brier (lower is better): with baseline 0.2008 · decay to 0 0.2008 · constant 0.2507
```

It fits on half the entities and scores on the other half, keeps the
baseline only when it helps there, and refuses to guess from too little
data. On the Wikidata CEO data it reproduces the published study.

## Calibration study

The half-lives above are hand-picked. [`calibration/`](calibration/)
tests the model against public data with known ground truth — Wikidata
CEOs, airport weather, 7 million US flights, and cargo-ship AIS tracks. The short
version: measured half-lives differ from the guesses by 6× to 80×,
facts decay toward a baseline rather than to zero, and transform noise
is not independent. Write-up: [docs/FINDINGS.md](docs/FINDINGS.md);
data and code: [calibration/](calibration/README.md); interactive results:
https://praveenprk.github.io/fid/.

## Full Design Document

See [docs/DISCOVERY.md](docs/DISCOVERY.md) for the complete
theoretical foundation and design rationale.

---

*The danger is not that information decays.*
*The danger is that computing has no way to notice.*
