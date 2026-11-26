# Proposal: a draft section for per-entity curves (#14)

Date: 2026-11-27. Status: proposed, for the governor. This is a proposal
for a draft section, not draft text. Nothing in
spec/draft-kumar-fid-00.md or the code changes until the governor
accepts it. Decision 8 still holds: specified first, then vectors and
tests, then code.

## Why

The draft's calibrator writes one curve per (type, state). The benchmark
dev set (calibration/results/decisions_dev.md, #14) shows that entities
within one state differ:
- held-out airports ATL, DEN and DFW were right 44–54% of the time at
  2 h, against about 66% for the training airports;
- grid regions AZPS and FPL go stale fast, PACE and MISO slowly.

With one curve per state, the schedule's predicted staleness missed by
0.028 (airport temperature) and 0.035 (grid demand), against the 0.02
the benchmark asks.

The parked implementation (`claude/entity-speed-wip`, 9859534; one test
failing, not merged) fits each entity's baseline and speed by joint
likelihood and shrinks both toward the state's. This proposal specifies
that model so it can be judged before any code returns.

## 1. Model

For a state whose curve is (s0, b, h, and optionally k, m, h_f, echo
{P, r, w, h_r}), entity e has two parameters:

- **b_e**, its baseline, a unit number;
- **r_e**, its speed, a positive number (1 = the state's pace).

The entity's curve is the state's curve with:
- baseline b_e;
- half-lives divided by its speed: h/r_e and, when present, h_f/r_e;
- everything else unchanged: s0, k, m, the echo's period P, strength r
  and width w. Whether the echo's fade h_r also scales is Q3.

Fidelity, time to floor and staleness then follow the existing sections
unchanged, because the entity's curve is an ordinary curve.

## 2. Estimation (requirements on a calibrator)

1. **Raw estimates.** (b_e, log r_e) are fitted by maximum likelihood on
   the entity's own checks, with the state's other parameters held at
   their fitted values. This is calibrator requirement 3, applied per
   entity.
2. **Shrinkage.** Each raw estimate is shrunk toward the state's (b, 0),
   jointly, by empirical Bayes. The between-entity covariance (2×2) is
   estimated from the raw estimates less their sampling covariance. An
   entity's weight grows with its information. With no between-entity
   spread in the data (covariance not positive definite), every entity
   gets the state's curve exactly.
3. **Held-out evidence.** This is requirement 2, applied in a new way.
   An entity's own parameters cannot be tested on other entities, so
   per-entity curves are kept for a state only if, splitting each
   entity's checks by time (earlier to fit, later to score), the shrunk
   per-entity curves give a lower held-out Brier score than the state's
   curve. The split rule is Q4.
4. **Determinism.** Requirement 1 holds. Entities are processed in
   bytewise UTF-8 order.
5. **Uncertainty.** Each written b_e and r_e carries a 95% range
   (requirement 5).
6. **Scope.** A calibrator MUST NOT write per-entity curves unless
   asked for that type explicitly (Q2). The draft SHOULD say why: for
   facts that change and stay changed (a position, a version, a label),
   an entity with no change in its history gets b_e near 1 and is then
   never re-checked. The benchmark showed exactly that for packages.

## 3. Output and use

- **File.** Per-entity curves go in a separate CSV, not in the registry
  YAML. Columns: `type`, `state`, `entity`, `baseline`, `speed`,
  `baseline_low`, `baseline_high`, `speed_low`, `speed_high`,
  `raw_baseline`, `raw_speed`, `checks`. The registry keeps types and
  states only, so existing registry readers and the "states are whole"
  rule are unchanged.
- **Privacy.** Privacy Considerations already says per-entity output
  SHOULD be treated as personal data and entity names pseudonymized. The
  section would repeat that for this file.
- **Lookup.** (type, state, entity) gives the state's entry, resolved as
  in §1 with the entity's row. An entity with no row is Q5.
- **Stamp.** `fid stamp` gains `--entity NAME` with `--entity-curves
  FILE` (CLI to be specified with the section). It writes the resolved
  curve into the envelope as ordinary `half_life`, `baseline` and, if
  present, `fast`.

## 4. Effect on the envelope

None. The envelope already carries a resolved curve: scorers never need
to know an entity's curve came from shrinkage, and they score it as any
other. No new member, no version bump, no change to vectors.json
`envelope`. Recording the entity or speed in the envelope would add
identifying data to every artifact; if provenance is wanted, `ext` can
carry it without affecting fidelity (Q6).

## 5. Acceptance tests (bounds to fix before any run)

These would extend {{acceptance}}. They use synthetic ledgers from a known
truth, as the existing tests do, with seeds free and bounds that must
hold for any seeds. The proposed bounds are Q1.

| Test | Truth and ledgers | Proposed bound |
|---|---|---|
| No spread, no change | 40 entities sharing (s0 0.95, b 0.2, h 10 d); 200 checks each, ages on the "days" grid; 20 ledgers | per-entity curves kept in ≤ 2 of 20; where kept, every r_e in [0.8, 1.25] and every \|b_e − b\| ≤ 0.05 |
| Real spread, closer to truth | 40 entities, r_e log-uniform on [0.5, 2], b_e uniform on [0.05, 0.35], same s0 and h; 200 checks each; 20 ledgers | kept in ≥ 18 of 20; in each kept ledger, mean absolute fidelity error over the days ages ≤ 0.5 × the state curve's error |
| Little history, the state's curve | as "real spread", plus 5 entities with 5 checks each | those 5 entities' curves within 0.02 of the state's fidelity at every days age |
| Change-and-stay is not modelled | a fact that flips once and stays (b 0); 40 entities | the calibrator does not write per-entity curves unless asked (scope rule); if asked, no entity's b_e above 0.5 |

The benchmark target (prediction error ≤ 0.02 for airport temperature
and grid demand) is not proposed as an acceptance test. Decision 9 keeps
the benchmark as evidence about the model, not part of it. It would be
the evidence the merge PR must show, as it was for richer curves.

## 6. Vectors

- New group `entity_lookup`: (state entry, entity row) → resolved curve
  (h/r_e, h_f/r_e, b_e, unchanged echo P, r, w, and h_r per Q3), plus
  fail-closed cases: a negative or zero speed, a baseline outside
  [0, 1], a non-finite value, a row for an unknown state, and an entity
  with no row per Q5.
- New group `entity_csv`: CSV text → rows, with malformed-file errors,
  as `ledger_csv` does.
- `envelope` and `registry` are unchanged (§4).
- Generated by `conformance/generate.py`, independent of the Rust code,
  as DECISION-004 requires.

## Questions for the governor (each with a recommendation)

These are choices within the proposal, not `Q-NNN` entries; accepting
the proposal accepts the recommendations unless you say otherwise.

- **Q1. Acceptance bounds?**
  - A: as in §5.
  - B: tighter.
  - C: set them after a dry run.
  - **Recommend A.** The richer-curve bounds were fixed after results
    were known, and the draft records that as a weakness. Fixing these
    first avoids repeating it, and A leaves room for seed noise.
- **Q2. How is the scope enforced?**
  - A: explicit opt-in per type: a calibrator flag, plus a registry key
    `per_entity: true` on the type.
  - B: automatic when the state keeps a baseline.
  - C: documentation only.
  - **Recommend A.** B is not enough: underway vessels keep a baseline
    (0.03), and positions are exactly the change-and-stay case. A
    registry key is also checkable by lookup.
- **Q3. Does an echo's fade h_r scale with speed?**
  - A: no; only h and h_f scale.
  - B: yes.
  - **Recommend A.** The period, width and fade describe the cycle (the
    day), which is shared by place; speed describes how fast the
    entity's own value wanders. A is also what #14 states.
- **Q4. Held-out split for per-entity parameters?**
  - A: by time within each entity (first 70% of its checks by measured
    time to fit, the rest to score).
  - B: by checks at random within entity.
  - **Recommend A.** It matches use: a curve fitted on the past
    predicting the future. It is deterministic without a seed.
- **Q5. Entity with no row in the file?**
  - A: the state's curve, with a warning naming the entity.
  - B: an error.
  - **Recommend A.** An entity with no history is the shrinkage limit
    (weight 0), which is the state's curve. The warning catches typos,
    and stamping stays possible for new entities. Fail-closed still
    applies to malformed rows (§6).
- **Q6. Record the entity in the envelope?**
  - A: no.
  - B: an optional `ext.entity`.
  - C: a new member (version 1.1).
  - **Recommend A.** Scoring does not need it, and it would put an
    identifier in every artifact. A producer that wants provenance can
    use `ext` already.

## Order of work, if accepted

1. Draft section (text from §1–§5), with Decision 8 updated and the
   "not part of this document" note in Deviations removed. Docs PR.
2. Vectors (`entity_lookup`, `entity_csv`) from generate.py, failing
   first.
3. Acceptance tests as Rust tests, failing first.
4. Implementation, rebuilt from the parked branch to pass 2 and 3.
5. Benchmark rerun as evidence (temperature and grid within 0.02),
   reported in the merge PR.
