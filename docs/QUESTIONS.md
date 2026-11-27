# QUESTIONS.md
# fid — Questions for the governor
# A judgment a contributor needs from PK, added by the pull request into
# `main` that needs it (DECISION-008, eng-principles ENG-DEC-012). PK answers
# on that pull request. A pull request into `main` cannot merge while any
# entry is OPEN (`commit-check / questions`).
# Append only. An answered entry stays, with its answer.
# A question a contributor raises is answered by its Recommend option under
# the governor's standing rule (DECISION-010); a pull request that records
# such an answer is merged by the governor, as a security-related change is.
# Format: eng-principles templates/question.md

---

### Q-001
Scenario : A Rust developer finds fid on GitHub, fixes a typo and opens a
           pull request. GitHub targets the default branch unless they
           change it. Today that is `main`, but work enters at `develop`.
Options  : A — Keep `main` as the default branch. CONTRIBUTING.md says to
               target `develop`. A pull request that arrives on `main` gets
               retargeted to `develop` (one click, by anyone with write
               access). Visitors see the released code on the front page.
           B — Make `develop` the default branch. Pull requests land in the
               right place by default. Visitors see unreleased work on the
               front page, and the questions check, which keys on the
               default branch, would have to name `main` explicitly.
Recommend: A, because the front page then matches what crates.io and PyPI
           ship, and the shared questions check works unchanged.
Raised by: #23 (31b4c93)
Status   : ANSWERED (governor, 2026-11-22): neither; `main` stays the default branch as the integration branch, and releases move to `stable`, as Rust does → DECISION-008

### Q-002
Scenario : You are about to tag the next release on `main`. The tag must be
           signed (eng-principles ENG-STD-005 §5.1), so GitHub shows it as
           Verified and users can check that it came from you. Your machine
           has no signing key configured for git.
Options  : A — Sign with an SSH key: `git config --global gpg.format ssh`,
               `git config --global user.signingkey ~/.ssh/<key>.pub`, and
               add the same key on GitHub as a Signing key. No new tool.
           B — Sign with a GPG key: install GnuPG, create a key, upload the
               public key to GitHub, and keep the key's expiry and backup.
Recommend: A, because it reuses a key you already manage and GitHub verifies
           it the same way.
Raised by: #23 (31b4c93)
Status   : ANSWERED (governor, 2026-11-21): A, sign with an SSH key → AGENTS.md Releases

### Q-003
Scenario : A contributor is insulted in a review thread and wants to tell
           the maintainer privately, without posting in public.
           CODE_OF_CONDUCT.md has to name a private way to reach you.
Options  : A — A dedicated address (for example a new mailbox or alias used
               only for fid conduct reports). Your personal address stays
               out of the repository. You set it up once.
           B — Your existing personal email. Nothing to set up. It becomes
               public in the repository and in its history.
           C — Your GitHub profile's contact only. Nothing new, but GitHub
               has no private messages, so the reporter may have no
               private route.
Recommend: A, because the address is published permanently and only
           conduct reports would reach it.
Raised by: #24 (5dbcf7b)
Status   : ANSWERED (governor, 2026-11-21): justpraveenhere@protonmail.com, for now, until fid has its own domain → CODE_OF_CONDUCT.md

### Q-004
Scenario : A company builds fid on a fixed Rust toolchain, 1.82, because
           Cargo.toml says fid needs 1.80 or later. The build fails: the
           dependencies fid 0.2.0 ships with (clap 4.6, indexmap 2.14)
           need Rust 1.85. fid has claimed 1.80 without testing it.
Options  : A — Correct the claim: rust-version becomes 1.85, the version
               fid actually builds and passes its tests on, and a CI job
               keeps it true. Shipped in the next release, listed under
               Changed in CHANGELOG.md. Users on 1.80 to 1.84 were already
               broken; nothing new breaks.
           B — Keep 1.80: hold clap, indexmap and others at older versions
               in Cargo.toml so 1.80 builds again. The claim becomes true,
               but fid stays on old dependencies and their fixes.
Recommend: A, because B only makes the claim true by holding fid back,
           and A changes nothing for anyone whose build works today.
Raised by: ci/V-ENG-008-01-msrv-python-supply-chain (A is applied there)
Status   : ANSWERED (governor, 2026-11-21): A, rust-version 1.85 → no change (applied in #25)

### Q-005
Scenario : A data team on Python 3.9 runs `pip install fidmeter`. 3.9
           reached end of life in October 2025: no more security fixes,
           and GitHub's newest runners (Ubuntu 26.04) no longer offer it.
           fid still promises 3.9 (requires-python >=3.9, abi3-py39
           wheels), and CI can test it only on the older Ubuntu 24.04
           runner, which GitHub will also retire.
Options  : A — Keep 3.9 for now. CI tests it on Ubuntu 24.04. Revisit when
               that runner is retired or at 0.3.0. Nobody loses support.
           B — Drop 3.9 in the next release: requires-python >=3.10 and
               abi3-py310 wheels. 3.9 users keep 0.2.x. Before 1.0 this is
               allowed in a minor release (DECISION-005).
Recommend: A, because supporting 3.9 costs one CI job today and dropping
           it gains nothing until the runner goes away.
Raised by: ci/V-ENG-008-01-msrv-python-supply-chain
Status   : ANSWERED (governor, 2026-11-21): A, keep Python 3.9 for now → no change

### Q-006
Scenario : Dependabot proposes sha2 0.10.9 → 0.11.1 (#42). fid builds and
           passes its tests with it on current Rust, but sha2 0.11.1 and
           0.11.2 need Rust 1.88, and fid promises Rust 1.85 (Cargo.toml
           rust-version; CI job `msrv`, Q-004). A company that builds fid on a
           fixed 1.85 toolchain would find the next release no longer
           builds. sha2 0.10.9 has no known advisory (`cargo audit` passes;
           the one RustSec entry for sha2, RUSTSEC-2021-0100, is for 0.9.7).
           The fingerprints are unchanged: the conformance vectors pass with
           either version.
Options  : A — Hold sha2 at 0.10 and keep Rust 1.85. dependabot.yml ignores
               sha2 0.11 and later until fid raises its minimum Rust for
               some other reason, or a fix fid needs exists only in 0.11.
               #42 is closed. Nothing changes for any user.
           B — Raise the minimum Rust to 1.88 and take sha2 0.11.2. Users on
               Rust 1.85 to 1.87 can no longer build the next release; it
               goes under Changed in CHANGELOG.md, and the `msrv` job moves
               to 1.88. Rust 1.88 is from June 2025.
           C — Pin sha2 to exactly 0.11.0, the last 0.11 release that still
               builds on 1.85. fid moves to 0.11 but can take no 0.11 patch
               fix without B.
Recommend: A, because 0.11 brings fid nothing it needs today, holding it
           breaks nobody, and it is undone by deleting one ignore rule. B is
           the right move when another reason to raise the minimum Rust
           comes along.
Raised by: chore/Q-006-hold-sha2-msrv (A is applied there), after #42 failed `msrv`
Status   : ANSWERED (governor, standing rule 2026-11-25): A, hold sha2 at 0.10, rust-version stays 1.85 → .github/dependabot.yml ignores sha2 >= 0.11

### Q-007
Scenario : docs/FINDINGS.md and calibration/README.md say the ship study
           covered "6,592 cargo ships and tankers" and the PyPI study "977
           most downloaded packages". The committed results say 6,146
           vessels (calibration/results/ships_ais.md) and 915 packages
           (calibration/results/pypi_latest.md). No committed file contains
           6,592 or 977. They may come from a run whose results were never
           committed (for example a download count before filtering), or
           they may be errors. The paper (#88, #89) cites only committed
           results.
Options  : A — Use the committed numbers (6,146 and 915) everywhere, unless
               and until the run that produced 6,592 and 977 is committed
               with its results; then that run's numbers replace them.
           B — Keep 6,592 and 977 in the docs, and explain the gap to the
               committed results in a note.
           C — Re-run both studies now and commit whatever they give.
Recommend: A, because a published number has to trace to a committed
           result, and A is undone by committing the other run. B keeps a
           number nobody can check; C costs about 2.4 GB of downloads and
           changes results the paper is written against.
Raised by: docs/findings-corrections (A is applied there), while tracing the paper's numbers (#89)
Status   : ANSWERED (governor, standing rule 2026-11-25): A, use the committed numbers → docs/FINDINGS.md, calibration/README.md
