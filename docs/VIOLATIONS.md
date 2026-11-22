# VIOLATIONS.md
# fid — Violations Register
# Append only. No violation is ever removed. A violation may be marked
# RESOLVED with date and commit reference.
#
# Severity (eng-principles ENG-STD-006 §2.3):
#   CRITICAL — a MUST is broken now · HIGH — a MUST is degraded
#   MEDIUM — a SHOULD is broken · LOW — debt
# Severities confirmed by the governor on 2026-11-16: HIGH, as for the same
# gaps in phasectl (proposed CRITICAL by the literal §2.3 wording).
#
# IDs: V-ENG-<standard>-<NN> = violation of an eng-principles standard.
#      V-<SPEC>-<NN>         = violation of a fid SPEC (none exist yet).
#
# Evidence is read at main bff71b4 (2026-11-15), before adoption.

## Summary (2026-11-15)

| ID | Standard | Severity | Status |
|----|----------|---------------------|--------|
| V-ENG-002-01 | ENG-STD-002 §1.1 | HIGH | RESOLVED (forward only) |
| V-ENG-004-01 | ENG-STD-004 §6.1 | HIGH | RESOLVED |
| V-ENG-005-01 | ENG-STD-005 §1.3, §2.2 | HIGH | RESOLVED (forward only) |
| V-ENG-005-02 | ENG-STD-005 §1.4 | HIGH | RESOLVED (forward only) |
| V-ENG-006-01 | ENG-STD-006 §1 | HIGH | RESOLVED |
| V-ENG-008-01 | ENG-STD-008 §2 (MUST checks) | HIGH (per item, ENG-DEC-013) | OPEN |

---

### V-ENG-002-01
SPEC says : ENG-STD-002 §1.1 — "All work passes through five gates, in order. A gate MUST NOT open until the previous one is closed." §4.1 — work that bypassed a gate gets a CRITICAL violation.
Code does : fid ships stamp, check, chain, ttl, calibrate and drift (src/main.rs, `enum Commands`), released as fidmeter 0.1.0 and 0.1.1 (tags v0.1.0, v0.1.1). No RFC or SPEC exists at bff71b4: `git ls-tree -r --name-only bff71b4 | grep -iE 'rfc|spec'` prints nothing. conformance/README.md specifies the scoring formula, with test vectors, but has no RFC, no INV ids, and no RFC 2119 keywords: `grep -cE '\b(MUST|SHOULD|MAY)\b' conformance/README.md` → 0. MILESTONE.md and docs/DISCOVERY.md describe intent but are not gated specs.
Severity  : HIGH (governor, 2026-11-16; proposed CRITICAL). A MUST is broken now, and §4.1 names CRITICAL for this case.
Impact    : The tests (tests/*.rs, bindings/python/tests) have no SPEC invariants to trace to. A change in behaviour cannot be checked against a contract.
Fix       : Backfill (ENG-STD-002 §4.1). Done in #7 (main c3768e1): spec/draft-kumar-fid-00.md describes what was built and is fid's RFC and SPEC (DECISION-001). Every section with MUST, SHOULD, SHALL or REQUIRED is tested or listed as a gap with an issue: spec/traceability.py, 31 sections, spec/TRACEABILITY.md. CI fails if any is untraced, and `spec-ref` requires behaviour changes to cite the draft. Code the draft does not specify was taken out before merge (#19, per-entity baselines, #14).
When      : Before the next feature.
Status    : RESOLVED — 2026-11-15, c3768e1 (#7). Forward only: releases v0.1.0 and v0.1.1 shipped before the gate.
Anchor    : DECISION-001

### V-ENG-004-01
SPEC says : ENG-STD-004 §6.1 — "Each project MUST pin one formatter and one linter per language and run them in CI."
Code does : .github/workflows/ci.yml runs `cargo test`, a vector-drift check, and the Python unittest suite. No formatter or linter is pinned or run, for Rust or for Python. `grep -nE 'fmt|clippy|ruff|black|flake8'` over .github/workflows/*.yml, Cargo.toml and bindings/python/pyproject.toml prints nothing, and there is no rustfmt.toml.
Severity  : HIGH (governor, 2026-11-16; proposed CRITICAL). The MUST is not met at all.
Impact    : Style and lint regressions are caught only if someone runs the tools by hand.
Fix       : The `lint` job in .github/workflows/ci.yml runs rustfmt and clippy (`-D warnings`) from the pinned toolchain 1.99.0, for both crates, and ruff 0.17.5 (check and format) with the rules in ruff.toml. The ruleset `main-branch-integrity` requires `lint`.
When      : 2026-11-17
Status    : RESOLVED
Anchor    : DECISION-001

### V-ENG-005-01
SPEC says : ENG-STD-005 §1.3 — "Every commit MUST carry at least one anchor that resolves to a governed entry". §2.2 — "The default branch is protected. No direct pushes, no force-pushes."
Code does : None of the 43 non-merge commits up to bff71b4 carries an anchor (`git log --no-merges --format=%s bff71b4`), e.g. fe44784 "fid: stamp and check commands, fidelity envelope". 20 non-merge commits went straight to main, the last being 2d30e81 on 2026-10-18 (`git log --first-parent --no-merges bff71b4`). No hooks were installed. GitHub ruleset `main-branch-integrity` (deletion, non_fast_forward, pull_request) has been active since 2026-10-20 (`gh api repos/praveenprk/fid/rulesets/25491223`).
Severity  : HIGH (governor, 2026-11-16; proposed CRITICAL). §1.3 is broken for every commit before adoption.
Impact    : No machine can trace the reasons for changes made before adoption.
Fix       : The `commit-check` workflow (scripts/commit_check.py) checks the PR title and every commit a PR brings to main: format and resolving anchors (DECISION-001). The ruleset requires it once this change is on main. Host protection is already in place. History is not rewritten (append-only). The 33 commits of #7, rebased onto main on 2026-11-15 before this check existed, are free-form like the rest of the history before adoption.
When      : This change.
Status    : RESOLVED (forward only) — 2026-11-16. The ruleset `main-branch-integrity` requires the host check: `commit-check` from fid #18 (99ff3c6), then eng-principles' shared check `commit-check / check` (ENG-DEC-009, DECISION-006). Commits before it remain unanchored.
Anchor    : DECISION-001

### V-ENG-005-02
SPEC says : ENG-STD-005 §1.4 — "Commits MUST NOT carry AI co-author or attribution lines."
Code does : 23 commit messages up to bff71b4 carry a Co-authored-by line (`git log --format=%B bff71b4 | grep -ciE 'co-authored-by'` → 23), e.g. 4ce9c3a, 34f87d0, f369c37 (`Co-authored-by: Claude …`).
Severity  : HIGH (governor, 2026-11-16; proposed CRITICAL). A MUST NOT is broken in 23 commits.
Impact    : The record shows an automated co-author where accountability must rest with the human who signs (ENG-P-11).
Fix       : The `commit-check` workflow refuses a co-author or tool attribution line in any commit a PR brings to main and in the PR description, which a squash merge turns into the commit body (this change). PR descriptions written by an automated contributor end with `Session-Id: <id>` instead (AGENTS.md). History is not rewritten.
When      : This change.
Status    : RESOLVED (forward only) — 2026-11-16. The required host check refuses co-author and tool attribution lines in commits and PR descriptions (fid #18, then ENG-DEC-009, DECISION-006). The 23 commits remain as they are.
Anchor    : DECISION-001

### V-ENG-006-01
SPEC says : ENG-STD-006 §1 — "Every project MUST keep three registers": DECISIONS, VIOLATIONS, and an ICS.
Code does : None existed at bff71b4 (`git ls-tree -r --name-only bff71b4 | grep -iE 'decision|violation|ics'` prints nothing).
Severity  : HIGH (governor, 2026-11-16; proposed CRITICAL). The MUST was not met at all.
Impact    : Decisions and known defects could not be found in one place.
Fix       : Create docs/DECISIONS.md (with DECISION-002..004 backfilled), this register, and docs/conformance/fid-ICS.md.
When      : This change.
Status    : RESOLVED — 2026-11-15, adoption change on branch chore/DECISION-001-adopt-eng-principles (takes effect on merge)
Anchor    : DECISION-001

### V-ENG-008-01
SPEC says : ENG-STD-008 §2 (eng-principles ENG-DEC-011) — MUST checks `contributing`, `security`, `changelog`, `signed`, `msrv`, `py_min`. §3.3 — "A failing MUST is a gap. A project registers its gaps as violations".
Code does : The first audit (eng-principles audit/audit.py, 2026-11-19, main 132746d) fails these six:
            no CONTRIBUTING.md; no SECURITY.md, so no private way to report a vulnerability; no CHANGELOG.md;
            tag v0.2.0 annotated but not signed (`gh api repos/praveenprk/fid/git/tags/<sha>` → verification.verified false);
            rust-version 1.80 in Cargo.toml, but no workflow builds with 1.80;
            requires-python >=3.9 (and abi3-py39 wheels on PyPI), but CI runs only 3.12.
Severity  : Per item, by eng-principles ENG-DEC-013 (governor, 2026-11-22; replaces the MEDIUM ruling of 2026-11-21):
            contributing HIGH (OpenSSF Best Practices MUST); security HIGH (security floor); changelog HIGH (Best Practices MUST, release_notes);
            signed HIGH (OpenSSF Scorecard Signed-Releases); msrv MEDIUM and py_min MEDIUM (ours, no behaviour change). The entry is HIGH, its highest item.
Impact    : An outside contributor finds no contribution or disclosure route. Users cannot verify a release came from the maintainer. The minimum Rust and Python versions are claimed without evidence.
Fix       : contributing, security, changelog: this change (with private vulnerability reporting turned on, 2026-11-19).
            msrv, py_min: CI jobs `msrv` and `python-min` build and test on the declared minimums. 1.80 does not build
            with the locked dependencies (clap 4.6 and indexmap 2.14 need 1.85), so rust-version becomes 1.85 (Q-004).
            signed: the next release tag is signed (AGENTS.md, Releases; Q-002). v0.2.0 is not re-tagged.
When      : contributing, security, changelog: this change. msrv, py_min: 2026-11-19. signed: next release.
Status    : OPEN
Anchor    : ENG-DEC-011
