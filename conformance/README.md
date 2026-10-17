# Conformance

What any implementation of fid's scoring must compute, and test vectors
to prove it. Use this if you port the formula — into Cypher, SQL, plain
Python, JavaScript — instead of calling fid.

- `vectors.json` — inputs and expected outputs, by section
- `generate.py` — writes `vectors.json` from this spec; a second,
  independent implementation (stdlib Python, does not call fid)
- `ledger-example.csv` — a re-verification export in the format
  `fid calibrate` reads, with known decay

fid's Rust tests (`tests/conformance.rs`) and Python tests
(`bindings/python/tests/test_conformance.py`) run every vector, so fid
and `generate.py` have to agree. CI regenerates the file and fails if it
changes. Change the spec here, then `generate.py`, then fid.

Floating-point results must match within `tolerance` (1e-9); durations
within 1e-6 s.

## Durations

`parse_duration(text) → seconds`. Trim, lowercase, then one suffix:

| Suffix | Seconds |
|---|---|
| `s` | 1 |
| `m` | 60 (minutes, not months) |
| `h` | 3600 |
| `d` | 86400 |
| `mo` | 30.44 × 86400 |
| `y` | 365.25 × 86400 |
| none | 1 |

The number is a decimal float (`1.5d`, `0.25y`). Negative, non-finite or
unparseable is an error: `7days`, `30ms`, `-1d`, `inf`, `1e400d`. Zero is
allowed.

`parse_half_life(text)`: `infinite` or `inf` (any case) means no decay;
otherwise `parse_duration`, and zero is also an error. Never treat a bad
half-life as infinite — that hides decay.

## Source classes

| Class | Fidelity |
|---|---|
| `instrument` | 0.97 |
| `human_observation` | 0.87 |
| `human_attestation` | 0.80 |
| `database` | 0.90 |
| `llm_grounded` | 0.55 |
| `llm_ungrounded` | 0.25 |
| `hearsay` | 0.17 |
| `unknown` | none — fidelity is indeterminate |

Names are trimmed and lowercased; `-` is accepted for `_`. Anything else
is an error.

## Fidelity

```
f        = source × ∏ (1 − clamp(noise_i, 0, 1))
temporal = 0.5 ^ (max(age, 0) / half_life)        (1 when no half-life)
b        = clamp(baseline, 0, max(f, 0))
fidelity = b + (f − b) × temporal
```

- Decay goes toward the baseline `b`, not 0. `b = 0` is the original
  model.
- A baseline above `f` is clamped to `f`: decay never raises fidelity.
- Negative age (clock skew) counts as zero.
- Unknown source: no fidelity at all, not 0.

## Time to floor

When fidelity, starting at `f` at measurement, falls below `floor`:

```
f < floor                    → 0
no half-life, or floor ≤ b   → never
otherwise                    → half_life × log2((f − b) / (floor − b))
```

`b` is clamped as above. Exactly at the floor counts as above it.

## Envelopes

An envelope is `{"fid": {...}, "content": ...}`. Fields used in scoring:
`source_class`, `measured_at` (RFC 3339), `half_life` (a duration string
or null), `baseline` (absent in `fid_version` 0.1 envelopes: use 0),
`transforms[].noise`, `integrity`.

**check(stamped, floor, now)**

- age = `now − measured_at`, in whole milliseconds, as seconds
- fidelity as above, with `f = source × ∏(1 − noise)`
- status: `UNKNOWN` if the integrity hash does not match the content,
  or the source is unknown; else `SIGNAL` if fidelity ≥ floor; else
  `NOISE`. (CLI exit codes 0 / 1 / 2.)

**expires_at(stamped, floor)** — `measured_at` + time to floor, rounded
half up to the millisecond, written `YYYY-MM-DDTHH:MM:SS.mmmZ`; or
`never`; or `unknown` for an unknown source. It does not depend on `now`
or on integrity.

## Integrity

`"sha256:" + hex(sha256(canonical JSON of content))`, where canonical is:

- object keys sorted by code point; no whitespace
- strings UTF-8, not `\u`-escaped except control characters (`\u0001`);
  `"` and `\` escaped
- integers as integers; floats with the shortest digits that round-trip
- float exponent form only below 1e-5 or from 1e16 up, written without
  zero padding: `1e-7`, `-2.5e-7`, `1e+16`, `1.5e+300`
- floats from 1e-5 to below 1e-4 written as decimals: `0.00001`,
  `0.000015`
- integral floats keep `.0`: `1.0`, `1000000000000000.0`

Python's `json.dumps(sort_keys=True, separators=(",", ":"),
ensure_ascii=False)` matches except for floats: it pads exponents
(`1e-07`) and switches to exponent form at 1e-5. `generate.py` has a
`canonical()` that matches fid; the `integrity` vectors include the
canonical text so you can see a mismatch, not only a different hash.

Parse floats with correct rounding. A parser that is off by one unit in
the last place (serde_json's default was) changes the hash of content
that has not changed.

## Ledger CSV

What `fid calibrate` reads. See [docs/LEDGER.md](../docs/LEDGER.md) for
how to export one from a store. The `ledger_csv` vectors give CSV text
and the observations it must parse to, or an error.

- Header row, column names case-insensitive, other columns ignored.
- `entity` (required, non-empty): what the fact is about. The held-out
  split is by entity.
- `correct` (required): `true/false`, `t/f`, `1/0`, `yes/no`, `y/n`.
- Age: either `age` (seconds, or a duration such as `30d`), or both
  `observed_at` and `checked_at` (RFC 3339 with a time and offset;
  `checked_at` not before `observed_at`). Not both forms.
- `state` (optional): one half-life per state. Every row has one, or
  none does; an empty cell counts as none.

`ledger_example` names `ledger-example.csv`, generated with half-life
30 d, baseline 0.30 and fresh accuracy 0.95. A conforming calibrator
recovers the half-life within 10 % and the baseline within 0.05; fid
gives 29.96 d and 0.30.
