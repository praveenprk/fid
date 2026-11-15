# DECISIONS.md
# fid — Decisions Register
# Append only. A decision may be superseded. It is never removed.
# Format: ID — Title | Date | Refs | Decision | Reason | Risk | Status
#
# DECISION-002..004 were backfilled on 2026-11-15 from decisions that git
# history and the repository's docs already record. Each one gives only the
# reason its cited commit or doc states. Nothing beyond that is reconstructed.

---

## DECISION-001 — Adopt eng-principles

Date    : 2026-11-15
Refs    : eng-principles ENG-DEC-001, ENG-DEC-008 (PROPOSED, eng-principles PR #2), ENG-STD-001..007 (at eng-principles c2a3c05)
Decision: fid adopts eng-principles. The registers live in docs/:
          DECISIONS.md, VIOLATIONS.md, and conformance/fid-ICS.md. The
          eng-principles templates are copied to docs/templates/. The hooks are
          vendored from eng-principles hooks/ into scripts/hooks/, with
          fid's own `anchors` policy: DECISION-NNN and V-ENG-NNN-NN resolve
          in this repository, and ENG-* anchors are exempt because they live
          in eng-principles. Agent instructions live in AGENTS.md, and
          CLAUDE.md only points to it. Gaps found on adoption are recorded as
          V-ENG-NNN-NN (a violation of an ENG standard, numbered by standard).
Reason  : One discipline across all projects (eng-principles ENG-DEC-001).
          eng-principles registers fid as a project (ENG-DEC-008). Adoption
          records fid's actual state instead of leaving it implied.
          The shape follows phasectl DECISION-001.
Risk    : Commits made before adoption stay unanchored, and history is not
          rewritten. The hooks do not check that ENG-* anchors exist, so a
          mistyped ENG-* anchor is accepted.
Status  : PROPOSED

---

## DECISION-002 — The package is published as fidmeter; the command, library and import stay fid

Date    : 2026-10-17 (backfilled 2026-11-15)
Refs    : commit f6759b2; README.md §Install; Cargo.toml `[lib] name = "fid"`
Decision: The crate and the PyPI package are named `fidmeter`. The
          command, the Rust library name, and the Python import stay `fid`.
Reason  : As recorded in f6759b2: "The name fid is taken on both registries."
          The commit also records that "no code changes for users" follow.
Risk    : none recorded
Status  : PROPOSED (backfilled; in effect since the cited commit, pending governor acceptance)

---

## DECISION-003 — The Python package wraps the Rust library; it does not reimplement it

Date    : 2026-10-12 (backfilled 2026-11-15)
Refs    : commit e69e35f; MILESTONE.md Milestone 3 §2; bindings/python
Decision: The Python package (PyO3 / maturin) calls the Rust library for
          stamp, check, expires_at, chain, fidelity, time_to_floor,
          registry lookups, and calibrate.
Reason  : As recorded in e69e35f: "bindings/python wraps the Rust library
          (no reimplementation)". MILESTONE.md says this is "so Python
          systems call one implementation instead of porting the formula".
Risk    : none recorded
Status  : PROPOSED (backfilled; in effect since the cited commit, pending governor acceptance)

---

## DECISION-004 — Conformance vectors come from an independent implementation; CI fails if they drift

Date    : 2026-10-18 (backfilled 2026-11-15)
Refs    : commit beea159; conformance/README.md; .github/workflows/ci.yml step "Conformance vectors match the spec"
Decision: conformance/README.md specifies the scoring. conformance/generate.py
          is an independent implementation in stdlib Python, and it writes
          conformance/vectors.json. The Rust and Python suites run every
          vector. CI regenerates the vectors and fails on any difference.
Reason  : As recorded in beea159 and conformance/README.md: "fid and
          `generate.py` have to agree". The order of change is "Change the
          spec here, then `generate.py`, then fid."
Risk    : none recorded
Status  : PROPOSED (backfilled; in effect since the cited commit, pending governor acceptance)
