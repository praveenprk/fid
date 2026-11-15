# fid — Implementation Conformance Statement (ICS)

This file records fid's conformance against the eng-principles standards.
Normative documents state none (eng-principles ENG-STD-006). Vocabulary:
`conformant` / `partial` / `not yet` / `n/a` / `not assessed`. Nothing is
`conformant` without evidence.

fid's conformance to its own specification is not recorded here. It lives
in spec/TRACEABILITY.md, which is generated from the draft and the tests,
and CI fails if it is stale (DECISION-001: one fact, one register). Not to
be confused with `conformance/` at the repository root, which holds the
test vectors (DECISION-004).

Date of last update: 2026-11-16
Baseline: main 99ff3c6 (#18); code unchanged since c3768e1 (#7). CI on c3768e1: success. `cargo test`: 230
passed, 0 failed, 2 ignored (unit 181, integration 8 + 16 + 14 + 11),
2026-11-16. spec/traceability.py: 31 sections traced.

## fid's specification

spec/draft-kumar-fid-00.md is fid's RFC and SPEC (DECISION-001). Status per
requirement: spec/TRACEABILITY.md. Known gaps: spec/gaps.json and issues
#14, #16.

## eng-principles standards

| Standard | Status | Evidence | Open violations |
|----------|--------|----------|-----------------|
| ENG-STD-001 Language & IDs | not assessed | — | — |
| ENG-STD-002 Lifecycle gates | conformant | The draft is the RFC and SPEC gate. Every normative section is tested or a listed gap: spec/TRACEABILITY.md, CI step "Every requirement in the draft is tested or a listed gap". `spec-ref` requires behaviour changes to cite the draft. Unspecified code was taken out before merge (#19). Releases v0.1.0 and v0.1.1 predate the gate (V-ENG-002-01, resolved forward only) | — |
| ENG-STD-003 Design | not assessed | — | — |
| ENG-STD-004 Construction | not yet | §6.1: no formatter or linter is pinned or run in CI. Other clauses not assessed | V-ENG-004-01 |
| ENG-STD-005 Change control | conformant | Ruleset `main-branch-integrity`: PR required, linear history, no force-push or deletion, required checks including `commit-check / check`, eng-principles' shared check on GitHub (ENG-STD-005 §1.5, DECISION-006). AGENTS.md. Commits before adoption stay unanchored and 23 carry co-author lines (V-ENG-005-01, -02, resolved forward only) | — |
| ENG-STD-006 Registers | partial | docs/DECISIONS.md, docs/VIOLATIONS.md, this file, mapped onto the spec registers (DECISION-001). DECISION-001 is PROPOSED, pending the governor | — |
| ENG-STD-007 Quality model | not assessed | — | — |
