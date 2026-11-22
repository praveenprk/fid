# AGENTS.md — fid

Instructions for every contributor, human or automated, whatever the model
or tool. Tool-specific files (CLAUDE.md and the like) only point here and
carry no rules (eng-principles ENG-STD-005 §6). Rules that bind agents also
bind humans.

fid follows eng-principles, mapped onto fid's own spec system
(docs/DECISIONS.md DECISION-001).

## Roles

- PK (GitHub: praveenprk) is the governor. PK merges `main` into `stable`,
  tags releases, merges security-related changes, accepts decisions,
  confirms violation severities, and answers the questions in
  docs/QUESTIONS.md.
- Contributors propose decisions (Status PROPOSED), file violations with
  evidence, and open pull requests into `main`. An automated contributor
  may merge its own pull request into `main` once all required checks pass
  and no question is OPEN, unless the change is security-related.

## Branches (DECISION-008, eng-principles ENG-DEC-012)

| Branch | Holds | Who merges into it | How |
|--------|-------|--------------------|-----|
| `type/ANCHOR-short-description` | One piece of work | Its author | Commits |
| `main` (default) | Integrated work | A contributor, checks green, no OPEN question; PK for security-related changes | Squash or rebase |
| `stable` | What is released | PK | Merge commit, from `main` |

- Every pull request targets `main`, from a work branch. Nothing is
  committed to `main` or `stable` directly.
- A judgment you need from PK goes in docs/QUESTIONS.md as a `Q-NNN`
  entry, added by the pull request that needs it: a real-world scenario,
  the options and what each changes, your recommendation. PK answers on
  that pull request; record the answer in the entry. The pull request
  cannot merge into `main` while any entry is OPEN. An answer that
  decides something becomes a DECISION entry.
- Security-related changes (eng-principles ENG-STD-005 §2.5: vulnerability
  reports or fixes, SECURITY.md, secrets or workflow permissions, branch
  rules or required checks, integrity-checking code) carry the label
  `security`. Only PK merges them.

## Releases (eng-principles ENG-STD-005 §5.1)

1. In a pull request into `main`: set the version in Cargo.toml,
   bindings/python/Cargo.toml and bindings/python/pyproject.toml, and move
   the Unreleased section of CHANGELOG.md under the new version.
2. PK promotes `main` → `stable` (merge commit).
3. PK tags the merge commit on `stable` with a signed, annotated tag:
   `git tag -s vX.Y.Z -F <message file>`, signed with PK's SSH signing
   key (git `gpg.format ssh`; the key is registered on GitHub as a
   signing key, Q-002). The message names the release,
   then lists the DECISION and V-ENG anchors it closes.
4. The `release` workflow publishes to crates.io and PyPI. Then create the
   GitHub release page from the CHANGELOG section.

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
| Rust formatted and clippy-clean; Python formatted and linted (ruff.toml) | `lint` job in the `ci` workflow: rustfmt, clippy, ruff, all pinned |
| Behaviour changes cite the draft | `spec-ref` workflow (spec/spec_ref.py) |
| Every requirement tested or a listed gap; vectors match | `rust` workflow (spec/traceability.py, conformance/generate.py) |
| `main` changes only through a pull request with the checks green; linear history; no force-push or deletion | GitHub ruleset `main-branch-integrity` |
| `stable` changes only through a pull request from `main` with the checks green, by merge commit | GitHub ruleset `stable-integrity` |
| No pull request into `main` while a question is OPEN | `commit-check / questions`, required on `main` (eng-principles ENG-DEC-010, ENG-DEC-012) |
| Only PK merges into `stable` and merges security-related changes | The agent's local guard refuses those merges; review by PK |
| Everything else | Review by PK |
