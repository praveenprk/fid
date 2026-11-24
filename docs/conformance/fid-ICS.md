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

Date of last update: 2026-11-17
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
| ENG-STD-004 Construction | partial | §6.1: rustfmt and clippy (toolchain 1.99.0), ruff 0.17.5 (ruff.toml), shellcheck 0.10.0 and shfmt 3.12.0 (spec/build.sh), Prettier 3.10.1 and ESLint 10.15.0 (site/*.html and their inline scripts) run in the required `lint` job of .github/workflows/ci.yml (V-ENG-004-01, V-ENG-004-02, resolved). Other clauses not assessed | — |
| ENG-STD-005 Change control | conformant | Ruleset `main-branch-integrity`: PR required, linear history, no force-push or deletion, required checks including `commit-check / check`, eng-principles' shared check on GitHub (ENG-STD-005 §1.5, DECISION-006). AGENTS.md. Roles for merging and releasing, and the release record of §2.6: AGENTS.md Roles and Releases, docs/templates/release-record.md (V-ENG-005-03, resolved; first used at the next release). Commits before adoption stay unanchored and 23 carry co-author lines (V-ENG-005-01, -02, resolved forward only) | — |
| ENG-STD-006 Registers | partial | docs/DECISIONS.md, docs/VIOLATIONS.md, this file, mapped onto the spec registers (DECISION-001). DECISION-001 is PROPOSED, pending the governor | — |
| ENG-STD-007 Quality model | not assessed | — | — |
