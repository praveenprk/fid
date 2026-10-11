# fid

**Information fidelity as a computing primitive.**

The degree to which a stored representation faithfully corresponds to
the reality it represents — measured as a continuous, time-varying
quantity that decays through temporal aging, lossy transformation, and
the evolution of external reality.

---

## The Gap

Every engineering discipline measures the correspondence between a
representation and the thing it represents. Audio engineering calls it
fidelity. Signal processing calls it signal integrity. Quantum
information theory calls it state fidelity. Metrology calls it
calibration validity.

Computing does not measure it. Computing does not name it. Computing
does not have a primitive for it.

A byte on disk from 2019 and a byte from today are structurally
identical. The system treats them identically. There is no degradation.
No decay. No loss of correspondence to reality over time. The bits
haven't changed, so the system treats the information as unchanged.

But the bits corresponding to reality is not the same as the bits being
unchanged. The bits are fine. The reality moved.

Bit rot is when the bits change. Media failure. Computing knows about
this.

What computing has never named:

> The bits stay the same. The reality they represent has moved. The
> information has decayed — not physically, but semantically. The
> correspondence is gone. And nothing in the system knows.

---

## Why Now

This gap has persisted for 78 years since Shannon (1948) because it
didn't matter. When computing was processing payroll, storing inventory
counts, rendering web pages — the assumption of eternal validity was
close enough to true. Data was produced by humans typing into forms.
Refresh rate was human-scale. Consequence of staleness was low.

Three things changed:

**AI outputs at machine speed.** Millions of inferences per hour. Each
one a "measurement" of reality by a statistical model. Each one with
implicit uncertainty bounds that are never recorded. Each one decaying in
relevance at a rate nobody tracks.

**AI-to-AI chains.** Model A's output becomes Model B's input becomes
Model C's input. Each step is a channel crossing in Shannon's model.
Signal degrades at every step. Nobody measures the cumulative
degradation. Nobody even has a unit for it.

**Consequential decisions on AI outputs.** A drug interaction warning. A
credit decision. A diagnostic suggestion. The consequence of the
eternal-validity assumption is no longer "slightly wrong web page." It's
"wrong medical decision based on information that decayed below the
noise floor three transformations ago."

The gap that didn't matter for 50 years now matters critically. And the
primitive to address it doesn't exist — not because it's hard to build,
but because the concept was never imported from information science into
computing.

---

## Theoretical Foundations

### Shannon (1948) — Channel Noise

Signal passes through a noisy channel. Mutual information between source
and receiver degrades at every step. It never spontaneously increases.
Reconstruction requires new measurement.

Every data transformation — a join, a filter, an aggregation, an
inference, a summarization — is a channel crossing. Noise enters. Signal
degrades. Shannon measured this for communication channels. Nobody
measures it for data pipelines.

### Thermodynamics — The Second Law

Entropy increases. Order degrades. A hot cup of coffee cools. It never
uncools. The disorder is irreversible without energy input.

Information in a data store obeys the same principle. The correspondence
between stored bits and external reality degrades over time. It never
spontaneously improves. Re-verification against reality — new
measurement — is the only mechanism that restores fidelity. That
re-verification costs energy, time, and human attention. There is no
free restoration.

### Landauer (1961) — Information Has Thermodynamic Cost

Erasing a bit of information requires a minimum energy dissipation of
kT ln 2. Information is physical. It is not abstract. It has
thermodynamic properties.

If information is physical, then information decay is physical. The
degradation of correspondence between stored data and external reality
is not a metaphor. It is a measurable physical process — or, more
precisely, the absence of the physical process (re-measurement) required
to maintain correspondence.

### Metrology — Calibration Windows

A physicist never writes 23.4°C. They write 23.4 ± 0.2°C, calibrated
2026-08-01, instrument S/N 4471. Remove any of those and the number is
scientifically meaningless.

Every instrument drifts. Every calibration has a window. A measurement
taken today is not the same measurement taken six months from now even
if the reading hasn't changed — because the calibration has drifted from
reality.

A software engineer writes `temperature = 23.4` and stores it forever
with no uncertainty bound, no instrument reference, no calibration date,
and no concept that this value's correspondence to reality is decaying
from the moment it's stored.

---

## What fid Measures

Three axes. Each independent. Each contributes to the total fidelity
score.

### Axis 1 — Temporal Decay

Every information type has an intrinsic half-life: the time after which
the probability of correspondence to reality drops to 50%.

| Information Type     | Approximate Half-Life |
|----------------------|-----------------------|
| Stock price          | seconds               |
| Weather forecast     | hours                 |
| News headline        | days                  |
| Product inventory    | hours to days         |
| Mailing address      | years                 |
| Legal name           | decades               |
| Mathematical constant| infinite              |
| Speed of light       | infinite              |
| LLM inference        | domain-dependent      |

This is not TTL. TTL is a policy: "delete after 90 days." Half-life is
a physical property of the information type: "this type of data loses
half its correspondence to reality every N units of time."

TTL is a garbage collection policy. Half-life is a law of the domain.

The temporal fidelity at time t after measurement:

```
f_temporal(t) = (1/2)^(t / t_half)
```

Where t_half is the domain-specific half-life of the information type.

### Axis 2 — Transformation Noise

Every time data passes through a processing step, noise enters. Signal
degrades.

A raw sensor reading has zero transformation noise. A value that has
been joined, filtered, aggregated, inferred from, summarized, and
re-inferred has accumulated noise at every step.

Three lossy transforms can reduce fidelity below the noise floor even
when temporal decay is zero.

The transformation fidelity after N steps:

```
f_transform = ∏(1 - noise_i) for i = 1..N
```

Where noise_i is the estimated noise contribution of step i. This is
the data pipeline equivalent of a signal integrity budget in electrical
engineering.

### Axis 3 — Source Reliability

A verified sensor reading starts with high fidelity. A hallucinated LLM
inference starts with low fidelity. A hearsay report starts with unknown
fidelity.

The source determines the initial fidelity. Everything after that can
only decrease it.

| Source Class               | Typical Initial Fidelity |
|----------------------------|--------------------------|
| Calibrated instrument      | 0.95 – 0.99             |
| Human direct observation   | 0.80 – 0.95             |
| Human attestation (signed) | 0.70 – 0.90             |
| Verified database record   | 0.85 – 0.95             |
| LLM inference (grounded)   | 0.40 – 0.70             |
| LLM inference (ungrounded) | 0.10 – 0.40             |
| Hearsay / unverified       | 0.05 – 0.30             |
| Unknown source             | indeterminate            |

### Total Fidelity

```
fidelity = f_source × f_temporal(t) × f_transform
```

It is always ≤ the source reliability. It always decreases with time. It
always decreases with transformations. It never spontaneously increases.

New measurement (re-verification against reality) is the only mechanism
that restores it.

**That is the second law of thermodynamics applied to information in
computing systems.**

---

## The Noise Floor

Every measurement system has a noise floor — the level below which
signal is indistinguishable from noise. Below the noise floor, you are
not extracting information. You are extracting randomness.

A data store has a noise floor. Every data store. Nobody has ever
defined what that means, let alone measured it.

`fid` defines it:

> The noise floor of a data store is the fidelity threshold below which
> a record's correspondence to reality is no better than chance. Records
> below the noise floor are not data. They are noise wearing the
> structural shape of data.

The noise floor is domain-specific and configurable. A medical system
might set it at 0.70. A recommendation engine might accept 0.30. The
floor is a policy decision. The measurement is not.

---

## fid as a Unix Primitive

### Core Operations

```bash
# Measure the fidelity of a data artifact
fid report.json
```

```
source:      0.90  (verified sensor, calibrated 2026-07-01)
temporal:    0.72  (age: 22d, half-life: 30d)
transforms:  0.85  (2 transforms, avg noise: 0.08)
fidelity:    0.55  (source × temporal × transforms)
floor:       0.40  (below this = noise)
status:      SIGNAL
```

```bash
# Scan a data store
fid scan ./records/
```

```
records:     14,891
above floor: 11,203  (75.2%)
below floor:  3,688  (24.8%)  ← noise, not data
mean fid:    0.61
median fid:  0.67
oldest:      2024-01-15 (fid: 0.12 — NOISE)
freshest:    2026-08-22 (fid: 0.94 — SIGNAL)
```

```bash
# Pipeline fidelity tracking
cat raw.json | transform | fid stamp | classify | fid stamp > output.json

# Inspect the fidelity chain
fid chain output.json
```

```
raw.json         → fid: 0.92 (source)
transform        → fid: 0.81 (−0.11 transform noise)
classify         → fid: 0.74 (−0.07 transform noise)
temporal (18d)   → fid: 0.55 (−0.13 temporal decay)
TOTAL:             0.55
```

### Exit Codes

```
0 — SIGNAL    fidelity above threshold
1 — NOISE     fidelity below floor
2 — UNKNOWN   source fidelity indeterminate, measurement impossible
```

Three exit codes. The entire epistemology of computing reduced to what
Unix already understands: an integer.

### Composability

```bash
# Gate a decision on fidelity
fid check patient-record.json --floor 0.70 && make_decision || escalate_to_human

# Filter a data set to signal only
cat records.jsonl | fid filter --floor 0.50 > signal.jsonl

# Monitor a data store continuously
fid watch ./database/ --interval 1h --alert-below 0.60

# Compare two snapshots
fid diff ./database-jan/ ./database-aug/
```

stdin/stdout. Pipes. Exit codes. Zero daemon for basic operation.

---

## The Fidelity Envelope

Every `fid`-stamped artifact carries a lightweight metadata envelope.
Like EXIF for epistemic state.

```
┌──────────────────────────────────────────────┐
│ FID ENVELOPE                                 │
│  source_class:  calibrated_instrument        │
│  source_id:     sensor-array-12              │
│  measured_at:   2026-08-01T14:30:00Z         │
│  half_life:     30d                          │
│  transforms:    []                           │
│  fid_at_stamp:  0.95                         │
│  integrity:     sha256:ab3f...               │
├──────────────────────────────────────────────┤
│ CONTENT                                      │
│  { "temperature": 23.4, "unit": "celsius" }  │
└──────────────────────────────────────────────┘
```

After two transforms:

```
┌──────────────────────────────────────────────┐
│ FID ENVELOPE                                 │
│  source_class:  calibrated_instrument        │
│  source_id:     sensor-array-12              │
│  measured_at:   2026-08-01T14:30:00Z         │
│  half_life:     30d                          │
│  transforms:                                 │
│    - step: normalize.py                      │
│      noise: 0.05                             │
│      at: 2026-08-01T15:00:00Z                │
│    - step: aggregate.py                      │
│      noise: 0.08                             │
│      at: 2026-08-02T09:00:00Z                │
│  fid_at_stamp:  0.83                         │
│  integrity:     sha256:d91c...               │
├──────────────────────────────────────────────┤
│ CONTENT                                      │
│  { "avg_temp": 23.7, "region": "north" }     │
└──────────────────────────────────────────────┘
```

The envelope is append-only. Transforms are logged, never removed. The
fidelity at any point in the chain is recomputable from the envelope
alone.

---

## Components

### 1. fid binary

Standalone. C or Rust. Zero runtime dependencies. Reads fidelity
envelopes, computes current fidelity, stamps new envelopes, scans
directories, filters by threshold. Text in, measurement out.

### 2. Half-life registry

A declarative file mapping information types to their decay rates.

```yaml
# /etc/fid/half-lives.yaml
stock_price:      30s
weather_forecast:  6h
news_headline:     3d
product_inventory: 12h
mailing_address:   3y
legal_name:       20y
llm_inference:     7d   # conservative default
sensor_reading:   domain_specific
mathematical:     infinite
```

This is domain knowledge, not computation. Organizations declare their
own half-lives based on their domain reality. The registry is the
equivalent of a calibration certificate — it records the assumption
under which fidelity is computed.

### 3. Noise model

A per-transform noise estimate. Declared by the transform author or
measured empirically.

```yaml
# .fid-noise.yaml (per project)
normalize:   0.02   # deterministic transform, minimal noise
aggregate:   0.08   # lossy by definition
llm_infer:   0.30   # high noise, model-dependent
join:        0.05   # schema-level, low noise if keys match
filter:      0.01   # lossless if predicate is correct
summarize:   0.15   # lossy, language-dependent
```

Does not need to be perfect. Needs to exist. Engineers use nominal
impedance values for signal integrity budgets, not exact measurements.
Approximate noise models are infinitely more useful than no noise model.

### 4. Floor threshold

```yaml
# .fid-floor.yaml (per domain)
medical:      0.70
financial:    0.60
operational:  0.40
informational: 0.20
```

Below the floor, the record is noise. Above the floor, it is signal.
The floor is a policy decision. The measurement is not.

---

## What fid Is Not

- Not a database. It measures data stores. It is not one.
- Not a governance system. It measures fidelity. It does not enforce
  policy on what to do about low fidelity. That is a governance
  concern.
- Not a cache invalidation tool. Cache invalidation is about stale
  copies of data. Fidelity is about the correspondence between ANY
  data (cached or canonical) and external reality.
- Not a monitoring tool. Monitoring tells you whether your system is
  running. fid tells you whether your data is true.
- Not AI-specific. fid applies to any stored information. AI made
  the gap urgent. The gap is universal.

---

## The Timeline That Should Embarrass Us

```
1948  Shannon defines information entropy and channel capacity
1949  Shannon proves noisy channel coding theorem
1961  Landauer proves information erasure costs kT ln 2
1965  Moore's Law — transistor density doubles every two years
1970  Codd defines the relational model
1971  Unix epoch — time becomes a first-class computing primitive
1973  Ethernet — networking becomes a first-class computing primitive
1991  PGP — identity verification becomes a computing primitive
2003  Bennett proves copying has thermodynamic cost
2005  Git — content-addressable storage becomes a computing primitive
2023  LLMs produce millions of inferences per day with no fidelity
      tracking, entering consequential decision paths with no
      measurement of correspondence to reality

2026  Still no computing system measures the fidelity of stored
      information relative to the reality it represents.

      78 years of information science.
      Zero imports into computing infrastructure.
```

---

## License

Apache 2.0.

fid is a measurement. Measurements are public goods. No organization
should have to pay to know whether their data is true.

---

*The danger is not that information decays.*
*The danger is that computing has no way to notice.*
