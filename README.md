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

# Filter to signal only
cat records.jsonl | fid filter --floor 0.50 > signal.jsonl
```

## Exit Codes

```
0 — SIGNAL    fidelity above threshold
1 — NOISE     fidelity below floor
2 — UNKNOWN   source fidelity indeterminate
3 — ERROR     bad arguments, unreadable or malformed input
```

## Three Axes of Fidelity

**Temporal decay.** Every information type has an intrinsic half-life.
A stock price decays in seconds. A mailing address decays over years.
A mathematical constant never decays. The half-life is a property of
the information type, not a policy choice.

**Transformation noise.** Every processing step — join, filter,
aggregate, infer, summarize — introduces noise. Signal degrades at
every step. Three lossy transforms can push fidelity below the noise
floor even with zero temporal decay.

**Source reliability.** A calibrated sensor starts at high fidelity.
An ungrounded LLM inference starts low. The source sets the ceiling.
Everything after can only decrease it.

```
fidelity = source × temporal_decay(t) × transform_noise
```

It never spontaneously increases. New measurement is the only
restoration mechanism.

## Configuration

```yaml
# half-lives.yaml — domain-specific decay rates
stock_price:      30s
weather_forecast:  6h
mailing_address:   3y
llm_inference:     7d

# noise-model.yaml — per-transform noise estimates
normalize:   0.02
aggregate:   0.08
llm_infer:   0.30
summarize:   0.15

# floor.yaml — noise floor thresholds
medical:      0.70
financial:    0.60
operational:  0.40
```

## Install

```bash
cargo install fid
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

## Full Design Document

See [docs/DISCOVERY.md](docs/DISCOVERY.md) for the complete
theoretical foundation and design rationale.

---

*The danger is not that information decays.*
*The danger is that computing has no way to notice.*
