# AGENTS.md — fid

Instructions for every contributor, human or automated, whatever the model
or tool. Tool-specific files (CLAUDE.md and the like) only point here and
carry no rules (eng-principles ENG-STD-005 §6). Rules that bind agents also
bind humans.

fid follows eng-principles, mapped onto fid's own spec system
(docs/DECISIONS.md DECISION-001).

## Roles

- PK (GitHub: praveenprk) is the governor and the only one who merges into
  `main`. PK accepts decisions and confirms violation severities.
- Contributors propose decisions (Status PROPOSED), file violations with
  evidence, and open pull requests. They do not merge into `main` or decide.

## Before work

- Read spec/README.md (order of work), docs/DECISIONS.md and
  docs/VIOLATIONS.md. The behaviour, decision or defect may already be
  recorded. Open spec gaps are GitHub issues.
- Plans go in plans/, with a row in plans/INDEX.md.

## Order of work

1. The draft (spec/draft-kumar-fid-00.md) says what fid does.
2. Then conformance vectors (conformance/generate.py) or tests tagged
   `// spec: <anchor>`, which fail first.
3. Then code. No code merges for behaviour the draft does not specify.

## Changes

- Every change traces to an anchor that resolves: DECISION-NNN or
  V-ENG-NNN-NN in this repository, ENG-* in eng-principles, or #N (an
  issue or PR of this repository). No anchor, no work.
- Branch: `type/ANCHOR-short-description` (e.g. `refactor/14-park-entity-baselines`).
- Commit and PR title: `type(scope): description — ANCHOR[, ANCHOR…]`.
  Types: feat fix test refactor docs chore design.
- A PR that changes behaviour has a `Spec: <draft anchors or #issues>` line.
- No co-author, AI attribution, or "Generated with" lines in commits or PR
  descriptions. A PR description written by an automated contributor ends
  with `Session-Id: <session id>`.
- Never force-push or rewrite pushed history.
- A violation fix updates docs/VIOLATIONS.md in the same change.
- Do not mark anything `conformant` in docs/conformance/fid-ICS.md without a
  cited test or commit.

## Enforcement

| Rule | Mechanism |
|------|-----------|
| Title and commit format, anchors resolve, no co-author or attribution lines | `commit-check` workflow: eng-principles' shared check, with fid's scripts/hooks/types and anchors (DECISION-006) |
| Behaviour changes cite the draft | `spec-ref` workflow (spec/spec_ref.py) |
| Every requirement tested or a listed gap; vectors match | `rust` workflow (spec/traceability.py, conformance/generate.py) |
| `main` changes only through a pull request with the checks green; no force-push or deletion | GitHub ruleset `main-branch-integrity` |
| Everything else | Review by PK |
