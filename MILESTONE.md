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
