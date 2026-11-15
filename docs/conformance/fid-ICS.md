# fid — Implementation Conformance Statement (ICS)

This file is the only place that records fid's conformance status. Normative
documents state none (eng-principles ENG-STD-006). Vocabulary:
`conformant` / `partial` / `not yet` / `n/a`. Nothing is `conformant` without evidence.

Not to be confused with `conformance/` at the repository root. That
directory holds the scoring spec and the test vectors that implementations
are checked against (DECISION-004). This file records fid's own status
against its SPECs and the eng-principles standards.

Date of last update: 2026-11-15
Baseline: main bff71b4. `cargo test`: 179 passed, 0 failed, 2 ignored (2026-11-15).

## fid SPECs

No SPECs exist yet (V-ENG-002-01). Per-invariant rows start here once
SPEC-001 is closed.

## eng-principles standards

| Standard | Status | Evidence | Open violations |
|----------|--------|----------|-----------------|
| ENG-STD-001 Language & IDs | not assessed | — | — |
| ENG-STD-002 Lifecycle gates | not yet | No RFC or SPEC. conformance/README.md specifies the formula but is not a gated SPEC | V-ENG-002-01 |
| ENG-STD-003 Design | not assessed | — | — |
| ENG-STD-004 Construction | not yet | §6.1: no formatter or linter pinned or run in CI. Other clauses not assessed | V-ENG-004-01 |
| ENG-STD-005 Change control | partial | scripts/hooks/ (commit-msg, pre-push), with scripts/hooks/test passing. AGENTS.md. GitHub ruleset `main-branch-integrity`. Commits before adoption are unanchored, and 23 carry co-author lines | V-ENG-005-01, V-ENG-005-02 |
| ENG-STD-006 Registers | partial | docs/DECISIONS.md, docs/VIOLATIONS.md, this file. DECISION-001 is PROPOSED, pending the governor | — |
| ENG-STD-007 Quality model | not assessed | — | — |
