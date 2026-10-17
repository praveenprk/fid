# Milestone 1 — fid stamp + fid check + fid chain

The first working surface. Three subcommands, end to end.

## Goal

A developer can stamp fidelity envelopes onto pipeline outputs,
check fidelity against a threshold with exit codes, and inspect
the fidelity chain of a stamped artifact. All via stdin/stdout.

## Deliverables

### 1. `fid stamp` — write a fidelity envelope

Read content from stdin. Wrap it in a fidelity envelope with:
- source_id (--source)
- source_class (--source-class, default unknown)
- noise for this step (--noise, default 0.0)
- half_life (--half-life, optional)
- derived_from (--derived-from, optional path to upstream .fid artifact)
- measured_at (now, or inherited from upstream)
- integrity hash (sha256 of content)
- fid_at_stamp (computed from all axes at stamp time)

Output: JSON to stdout. The envelope wraps the content.

If stdin is already fid-stamped (has an envelope), the new stamp
appends to the transform chain rather than creating a new root.
This is how pipeline composition works.

### 2. `fid check` — exit code carries epistemic state

Read a fid-stamped file. Compute current fidelity (accounting for
time elapsed since stamp). Compare against --floor threshold.

Exit 0 if fidelity >= floor (SIGNAL).
Exit 1 if fidelity < floor (NOISE).
Exit 2 if source is Unknown (indeterminate).

Human-readable report to stderr. Exit code to the pipeline.

```bash
fid check record.json --floor 0.70 && proceed || escalate
```

### 3. `fid chain` — inspect provenance chain

Read a fid-stamped file. Walk the transform chain. Print each
step with its fidelity contribution.

```
sensor-12        → fid: 0.95 (source: instrument)
normalize.py     → fid: 0.90 (−0.05 transform noise)
aggregate.py     → fid: 0.83 (−0.07 transform noise)
temporal (18d)   → fid: 0.62 (−0.21 temporal decay, half-life: 30d)
TOTAL:             0.62  SIGNAL (floor: 0.50)
```

## Not In Milestone 1

- `fid scan` (directory scanning)
- `fid filter` (jsonl filtering)
- `fid watch` (continuous monitoring)
- `fid measure` (bare `fid <path>` shorthand)
- Config file loading (half-lives.yaml, noise-model.yaml, floor.yaml)
- Binary envelope format (JSON only for M1)
- .fid file extension convention

## Build Order

1. compute.rs — the math (done — has tests)
2. envelope.rs — data structures (done — has serialization tests)
3. stamp command — stdin → envelope → stdout (done — stamp.rs, tests/stamp.rs)
4. check command — read envelope → compute → exit code (done — check.rs, tests/check.rs)
5. chain command — read envelope → walk transforms → report (done — chain.rs)

One at a time.

---

# Milestone 2 — calibrated model

Driven by calibration/ (public ground-truth data). Done:

- `baseline` — facts decay toward the chance they are right anyway,
  not toward zero
- state-dependent half-lives — `--half-lives FILE --type T --state S`
- conditional transform noise — `--noise-model FILE --transform T --condition C`,
  recorded on the transform step
- envelope `fid_version` 0.2; 0.1 envelopes still read
- examples/*.yaml carry the measured values

Open:

- periodic facts (temperature) — no decay model expresses "comes back"
- source fidelity per context (airline, time of day), not only per class

---

# Milestone 3 — calibrate your own data

Goal: turn the calibration study into something anyone can run on their
own records, and make fid usable from other systems. First integration
target: ContextOS (see its `fid-contextos.md`).

## 1. `fid calibrate` — done

Fit half-life and baseline from a log of re-checked facts and write the
half-life registry YAML that `fid stamp --half-lives` reads.

Input: CSV with a header.

| Column | Required | Meaning |
|---|---|---|
| `entity` | yes | what the fact is about; the held-out split is by entity |
| `age` | one of | seconds, or a duration (`30d`, `6h`) |
| `observed_at`, `checked_at` | one of | RFC 3339 times; age = checked − observed |
| `correct` | yes | still true when checked: `true/false`, `1/0`, `yes/no` |
| `state` | no | fit one half-life per state |

    fid calibrate ledger.csv --type company_ceo > half-lives.yaml

- Model: `S(age) = b + (s0 − b) × 0.5^(age / h)`. For a fixed `h` this is
  linear in `b` and `s0`, so each candidate `h` is solved in closed form;
  `h` is searched on a log grid.
- Ages with few distinct values are used as they are; continuous ages
  are grouped into equal-count bins.
- Fit on half the entities, score on the other half (Brier) against
  decay-to-zero and a constant. Keep the baseline only when it scores
  better on the held-out half.
- Report (stderr): the curve, the fit, the held-out scores. Registry
  YAML (stdout). Too little data is an error, never a guessed value.
- Checked against calibration/: on the Wikidata CEO observations it gives
  5.84y (study: 5.8y) and identical held-out Brier scores.

## 2. Library API and Python binding — done

Done: `compute::time_to_floor`, `FidEnvelope::expires_at`, and `fid ttl`
(stdout: expires_at or "never"; exit code as `check`). Python package in
bindings/python (PyO3 + maturin): stamp, check, expires_at, chain,
fidelity, time_to_floor, registry lookups, calibrate — all calling the
Rust library. Checked: the same CEO data gives the same fit from the CLI
and from Python. CI runs both test suites.

Released: fidmeter 0.1.0 on crates.io and PyPI (2026-10-18); 0.1.1 the same
day with the float round-trip fix. Wheels for
Linux, macOS and Windows on x86_64 and arm64.

Publishing: the package is `fidmeter` on both registries (`fid` is
taken); the command, library and import stay `fid`. Pushing a tag vX.Y.Z
runs .github/workflows/release.yml: crate to crates.io, wheels for
Linux/macOS/Windows plus sdist to PyPI. Checked locally: `cargo publish
--dry-run`, and the sdist installs into a clean venv and passes the tests.

- A stable Rust API for scoring an envelope and computing TTL to a floor:
  `ttl = h × log2((source − b) / (floor − b))`.
- `fid ttl` command for the same, for shells and other languages.
- Python package via PyO3 / maturin, so Python systems call one
  implementation instead of porting the formula.

## 3. ContextOS pilot

Designed in the ContextOS project. fid's side: the binding, the
calibrate input format for ledger exports, and conformance test vectors.

- Binding — done (`pip install fidmeter`).
- Ledger export format — done: docs/LEDGER.md (what each column must
  mean, outcome → `correct`, sampling bias, how much data), and
  conformance/ledger-example.csv, a synthetic export with known decay
  (fid recovers 29.96d / 0.30 from a true 30d / 0.30).
- Conformance vectors — done: conformance/ (spec, vectors.json, and
  generate.py, an independent stdlib implementation that writes them).
  Rust and Python suites run every vector; CI fails if the vectors drift
  from generate.py.
  - Found and fixed: serde_json's default float parsing is not correctly
    rounded, so an unchanged artifact holding e.g. 1.2345678901234567e+20
    failed `fid check` with an integrity mismatch. Now parsed with
    `float_roundtrip`. Existing stamps stay valid.
  - The integrity hash depends on JSON number formatting; the spec pins
    fid's canonical form (Python's json differs on exponents).

## 4. Write-up — done

docs/FINDINGS.md: the five findings, method, limitations and how each
finding changed fid. Every number traces to calibration/results/.

## Later

- `scan`, `filter`, `watch`
- periodic facts
- source fidelity per context
