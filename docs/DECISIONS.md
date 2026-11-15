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
Refs    : eng-principles ENG-DEC-001, ENG-DEC-008 (PROPOSED, eng-principles PR #2), ENG-STD-001..007 (at eng-principles c2a3c05); spec/README.md; scripts/commit_check.py
Decision: fid adopts eng-principles by mapping the standards onto the
          system fid already runs. Nothing is kept in two places.
          | eng-principles            | fid                                         |
          |---------------------------|---------------------------------------------|
          | RFC + SPEC gates (STD-002)| spec/draft-kumar-fid-00.md: intent in its Introduction and Scope, behaviour in its sections, decisions in its Decision Record |
          | TEST gate                 | conformance/vectors.json (generate.py) and tests tagged `// spec: <anchor>` |
          | Spec violations           | spec/gaps.json and GitHub issues; deviations in the draft's Deviations appendix |
          | Spec conformance          | spec/TRACEABILITY.md (generated; CI fails if stale) |
          | ENG-standard violations   | docs/VIOLATIONS.md (V-ENG-NNN-NN) |
          | ENG-standard conformance  | docs/conformance/fid-ICS.md |
          | Decisions                 | this file; draft-level decisions stay in the draft |
          Order of work: spec/README.md (draft, then vectors or tagged
          tests, then code).
          Anchors: DECISION-NNN and V-ENG-NNN-NN resolve in this
          repository; ENG-* in eng-principles; #N is an issue or PR of
          this repository (spec gaps are issues).
          Enforcement is on GitHub, not on the contributor's machine:
          the `commit-check` workflow (scripts/commit_check.py) checks the
          PR title and every commit a PR brings to main, and `spec-ref`
          checks that behaviour changes cite the draft. Both are required
          by the ruleset `main-branch-integrity`. No local hooks are
          vendored: a local hook never runs on a merge made on GitHub,
          and it can be skipped.
          Agent instructions live in AGENTS.md; CLAUDE.md only points to
          it. Plans are kept in plans/, indexed in plans/INDEX.md.
Reason  : One discipline across all projects (eng-principles ENG-DEC-001)
          without a second copy of facts fid already records (eng-principles
          ENG-STD-006 §2.5, one fact, one register). Checks that run on the
          host bind every pusher and every merge method; a local hook does
          not (phasectl V-ENG-005-02: an unanchored commit reached main
          through a GitHub merge).
Risk    : Commits made before adoption stay unanchored, and history is not
          rewritten. ENG-* anchors are not checked against eng-principles,
          so a mistyped one is accepted. The checks bind only once the
          ruleset requires `commit-check`.
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

---

## DECISION-005 — The next release is 0.2.0; the software version and the envelope version are separate numbers

Date    : 2026-11-16
Refs    : SemVer 2.0.0 §4, §8; #7 (merged as c3768e1); spec/draft-kumar-fid-00.md §versioning; tags v0.1.0, v0.1.1
Decision: The next release of fidmeter (crate and PyPI package) is
          0.2.0. The envelope format version, 1.0 since #7, is a separate
          number: it names the artifact format the draft specifies, not
          the software. 1.0.0 of the software waits until the draft is
          stable.
Reason  : #7 breaks behaviour: artifacts are written as envelope 1.0
          with JCS hashes, artifacts stamped without a half-life check
          UNKNOWN, and calibrated values shift under maximum likelihood.
          Under SemVer, a breaking change before 1.0.0 raises the minor
          number (§4). 1.0.0 declares a stable public interface (§8),
          and the draft is -00.
Risk    : Users who pin `fidmeter = "0.1"` do not receive 0.2.0 until they
          change the pin. This is intended: the change is breaking.
Status  : PROPOSED
