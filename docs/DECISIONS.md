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
Status  : ACCEPTED (governor, 2026-11-17)

---

## DECISION-006 — The commit check is eng-principles' shared workflow

Date    : 2026-11-16
Refs    : eng-principles ENG-DEC-009 (ACCEPTED), ENG-STD-005 §1.5; eng-principles 74e7e76; DECISION-001
Decision: fid's `commit-check` workflow calls eng-principles'
          .github/workflows/commit-check.yml, pinned to 74e7e76. fid keeps
          only its policy: scripts/hooks/types and scripts/hooks/anchors
          (DECISION, V-ENG, ENG-* exempt, ISSUE for #N).
          scripts/commit_check.py and its tests are removed. The required
          check in the ruleset `main-branch-integrity` is
          `commit-check / check`. This supersedes the enforcement
          paragraph of DECISION-001; the rules checked are the same.
Reason  : One implementation for every project, defined where the
          standard is defined (ENG-DEC-009). fid's own checker was a copy
          of the same rules.
Risk    : fid's CI fetches eng-principles at the pinned commit. Moving the
          pin is a deliberate change with its own PR. If the fetch fails,
          the check fails and the merge is blocked (fail closed).
Status  : ACCEPTED (governor, 2026-11-21)

---

## DECISION-007 — fid runs develop, qa and main

Date    : 2026-11-19 (reasoning expanded 2026-11-21, after PK's review)
Refs    : eng-principles ENG-DEC-010 (ACCEPTED), ENG-STD-005 §2.3, §2.4, §5.1; eng-principles f972b72; DECISION-006; Q-001
Problem : Until now every change waited for PK to merge it into `main`.
          Most of those merges needed no judgment (checks green, nothing
          to decide), yet each one stopped the work. The few that did need
          judgment were asked in chat, where they were easy to lose.
Options : 1 — Trunk and tags, as large open-source projects do (rust-lang/
              rust, tokio, openssh-portable): one integration branch that
              every pull request targets; a release is a tag (Rust also
              keeps `beta` and `stable` branches cut from it). A maintainer
              reviews each pull request. Fits fid's size, but every merge
              still waits for PK, which is the problem.
          2 — git-flow (develop, release/*, main, hotfix/*): built for
              teams shipping several versions at once. More branches than
              fid needs.
          3 — develop, qa, main (chosen): option 1's integration branch
              (`develop`), plus one place where PK judges a batch (`qa`),
              plus `main` holding only what PK has judged and released.
Decision: fid runs three long-lived branches.
          - `develop`: every pull request targets it. An automated
            contributor may merge into it once the required checks pass
            (squash or rebase; linear history).
          - `qa`: PK merges `develop` into it when a batch is ready, after
            answering its questions and judging its decisions.
          - `main`: PK merges `qa` into it. Releases are signed, annotated
            tags on `main` (AGENTS.md).
          Promotions use merge commits. A judgment a contributor needs goes
          in docs/QUESTIONS.md, asked as a real-world scenario, in the
          pull request that raises it. `commit-check / questions`, required
          on `main`, fails a pull request into `main` while any entry is
          OPEN. Rulesets `develop-integrity`, `qa-integrity` and
          `main-branch-integrity` enforce this on GitHub. Which branch is
          GitHub's default is Q-001.
Reason  : PK's time goes to judgment instead of to merges: routine changes
          flow into `develop` without waiting, and judgments arrive
          together, written down, at `qa` (ENG-DEC-010). `main` stays what
          users get from crates.io and PyPI. Promotions use merge commits
          because squash or rebase rewrites the commits, so `develop`,
          `qa` and `main` would drift apart and every promotion would
          conflict. Option 1 is the open-source norm, and `develop` is that
          norm's integration branch; `qa` and `main` are what this project
          adds so that PK judges in batches.
Risk    : An automated contributor merges into `develop` without a human
          review. Bounded by the required checks, by the questions
          register, and by PK reviewing the whole batch at `qa` before
          anything reaches users. `main` history gains merge commits; the
          shared check skips them and still checks every commit they bring.
          Outside contributors must target `develop` (CONTRIBUTING.md;
          Q-001 decides whether GitHub does that by default).
Status  : WITHDRAWN (governor, 2026-11-22): never accepted; replaced by DECISION-008 (eng-principles ENG-DEC-012)

---

## DECISION-008 — fid runs main and stable

Date    : 2026-11-22
Refs    : eng-principles ENG-DEC-012 (ACCEPTED), ENG-STD-005 §2.3–2.5, §5.1; DECISION-007 (withdrawn); Q-001
Decision: fid runs two long-lived branches, as the Rust project does.
          `main` is the default and integration branch: every pull request
          targets it from a work branch, and an automated contributor may
          merge into it once all required checks pass and no question in
          docs/QUESTIONS.md is OPEN. Security-related changes carry the
          label `security` and only PK merges them. `stable` holds what is
          released: PK promotes `main` → `stable` with a merge commit and
          tags releases on it. `stable` starts at v0.2.0 (132746d).
          `develop`, `qa` and their rulesets are removed once their work
          is on `main`. Rulesets: `main-branch-integrity` (squash or
          rebase, linear history, questions check required) and
          `stable-integrity` (merge commit only).
Reason  : eng-principles ENG-DEC-012 (governor, 2026-11-22). Outside
          contributors find the layout they know from Rust: work lands on
          `main`, releases come from `stable`. Judgment is given on the
          pull request that needs it, so routine work does not wait.
Risk    : Changes reach `main` without a human reading each one. Bounded by
          the required checks, the questions check, PK merging
          security-related changes, and PK's review of every promotion to
          `stable` before a release.
Status  : ACCEPTED (governor, 2026-11-22, by ENG-DEC-012)

---

## DECISION-009 — fid keeps a dated ledger of runs in real use

Date    : 2026-11-25
Refs    : #30; eng-principles ENG-STD-009 §2.3, §3.1, CONFORMANCE.md (fid at L3, 2026-11-23); calibration/results/; docs/LEDGER.md (a different ledger: the re-check CSV `fid calibrate` reads)
Problem : eng-principles rates fid L3: it passes its tests on real inputs
          (the calibration studies, committed under calibration/results/).
          L4 asks that fid "runs in a real operating environment, and a
          dated, committed record of runs, inputs, results and issues
          shows it" (ENG-STD-009 §3.1). The studies are dated by their
          commits, but they are experiments on historical data, not fid
          used to make a decision, and nothing records an issue found in
          use. No such record exists.
Options : A — docs/RUNS.md (chosen): an append-only table, one row per
              run in real use, added by pull request. Real use means fid's
              output was used, or meant to be used, for something other
              than testing fid: a stamped feed, a `check` gating an action,
              a registry from `calibrate` put into service, a `drift` check
              on a live registry. The governor's own use counts
              (ENG-STD-009 §3.1). Studies and CI runs do not.
          B — A scheduled workflow runs fid on live public data (e.g.
              `fid drift` on the METAR registry entries each week) and
              records the run. Automatic, but a workflow cannot commit to
              `main` without a pull request (ruleset), and a run nobody
              acts on is closer to a study than to use.
          C — Count the calibration results as the record. Cheapest, but
              ENG-STD-009 §2.3 forbids advancing a level on evidence that
              does not meet it, and they record no issues found in use.
Decision: fid keeps docs/RUNS.md. Each row: date (UTC); fid version or
          commit; where it ran and for whom; input (what, how much, a
          link or path when public); command; result (exit codes and the
          figure that mattered); issues (none, or the issue number opened
          for each). A row is never edited, only followed by a correcting
          row. The file is public: a private run names its target in
          general terms, with no private commit IDs, paths or data, and
          Input reads "private" (phasectl #20).
          fid's maturity is raised in eng-principles CONFORMANCE.md
          only after the ledger shows sustained use; how many rows over
          how long is the governor's ruling, and fid does not claim L4
          before it.
Reason  : It is the record ENG-STD-009 §3.1 names, kept the way fid keeps
          its other registers (append only, by pull request). It separates
          use from study, which C does not, and needs no new machinery,
          which B does. B can be added later as one more source of rows.
Risk    : A ledger filled by hand stays empty if nobody writes to it, and
          fid stays L3; that is the honest outcome. Rows describe use the
          reviewer did not see; each names its input so it can be checked
          where the input is public.
Status  : PROPOSED
