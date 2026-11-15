# AGENTS.md — fid

Instructions for every contributor, human or automated, whatever the model
or tool. Tool-specific files (CLAUDE.md and the like) only point here and
carry no rules (eng-principles ENG-STD-005 §6). Rules that bind agents also
bind humans.

fid follows eng-principles (docs/DECISIONS.md DECISION-001).

## Roles

- PK (GitHub: praveenprk) is the governor and the only one who pushes to
  `main` and merges. PK accepts decisions and confirms violation severities.
- Contributors propose decisions (Status PROPOSED), file violations with
  evidence, and open pull requests. They do not merge or decide.

## Before work

- Read docs/DECISIONS.md and docs/VIOLATIONS.md. The decision or defect may already be recorded.
- Install the hooks once per clone: `scripts/hooks/install`.

## Changes

- Every change traces to an anchor that resolves: DECISION-NNN or
  V-ENG-NNN-NN in this repository, or ENG-* in eng-principles. No anchor, no work.
- Branch: `type/ANCHOR-short-description`.
- Commit: `type(scope): description — ANCHOR[, ANCHOR…]`.
- No co-author, AI attribution, or "Generated with" lines in commits or PR bodies.
- Never bypass the hooks (`--no-verify` is a violation). Never force-push or rewrite pushed history.
- A violation fix updates docs/VIOLATIONS.md in the same change.
- Do not mark anything `conformant` in docs/conformance/fid-ICS.md without a cited test or commit.

## Enforcement

| Rule | Mechanism |
|------|-----------|
| Commit format, anchor resolves, no co-author lines | `scripts/hooks/commit-msg` |
| No push to `main`, no force-push, no remote deletion (local guardrail) | `scripts/hooks/pre-push` |
| `main` changes only through a pull request; no force-push or deletion | GitHub ruleset `main-branch-integrity` |
| Everything else | Review by PK |
