# fid for Python

How much a stored fact can still be trusted, from Python. The scoring,
envelopes, registries and calibration all run in fid's Rust library; this
package gives them a Python shape.

```
fidelity = baseline + (source × ∏(1 − noise) − baseline) × 0.5^(age / half_life)
```

See the [fid repository](https://github.com/praveenprk/fid) for the model
and the [results](https://praveenprk.github.io/fid/) it was calibrated on.

## Install

```bash
pip install fidmeter          # then: import fid
```

## Build from source

```bash
pip install maturin
cd bindings/python
maturin develop --release     # into the active virtualenv
python -m unittest discover -s tests
```

## Use

```python
import fid
from datetime import timedelta

# Score and gate
artifact = fid.stamp({"ceo": "Ada"}, source="registry", source_class="database",
                     half_life="5.8y")
fid.check(artifact, floor=0.7).status          # Status.SIGNAL (== 0, the CLI exit code)
fid.expires_at(artifact, floor=0.7)            # datetime when it falls below the floor

# Pipelines: stamping a stamped artifact appends a transform step
step = fid.stamp(artifact, source="summarize.py", noise=0.15)
print(fid.chain(step))

# Measured values from registries
half_life, baseline = fid.half_life("half-lives.yaml", "vessel_position", "underway")
noise = fid.noise("noise-model.yaml", "flight_leg", "previous_leg_late")

# The formula on its own
fid.fidelity(0.97, timedelta(hours=1), "2.9h", baseline=0.73)
fid.time_to_floor(0.97, "7d", 0.70, baseline=0.2)   # timedelta, or None if never

# Calibrate from facts you re-checked: (entity, age, still correct?, [state])
cal = fid.calibrate(rows, "policy_claim")
open("half-lives.yaml", "w").write(cal.yaml)
print(cal.report)
```

| Function | Returns |
|---|---|
| `stamp(content, *, source, source_class, noise, half_life, baseline, type, state, condition, upstream, now)` | stamped dict `{"fid": …, "content": …}` |
| `check(stamped, floor, *, now)` | `Assessment` with `status` (`Status.SIGNAL / NOISE / UNKNOWN`), `fidelity`, `age`, … |
| `expires_at(stamped, floor)` | `datetime`, or `None` if it never falls below; `IndeterminateError` for an unknown source |
| `chain(stamped, floor, *, now)` | the `fid chain` report |
| `fidelity(source, age, half_life, *, noise, baseline)` | float |
| `time_to_floor(fidelity, half_life, floor, *, baseline)` | `timedelta`, or `None` for never |
| `half_life(path, type, state)` / `noise(path, transform, condition)` | registry lookups |
| `calibrate(rows, type)` | `Calibration` with `yaml`, `report`, `fits` |

Durations are strings (`"30d"`, `"6h"`), seconds, or `timedelta`. Times
must be timezone-aware. Errors raise `ValueError`.
