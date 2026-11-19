# QUESTIONS.md
# fid — Questions for the governor
# A judgment a contributor needs from PK, recorded when the work merges into
# `develop` (DECISION-007, eng-principles ENG-DEC-010). PK answers at `qa`.
# A pull request into `main` cannot merge while any entry is OPEN
# (`commit-check / questions`).
# Append only. An answered entry stays, with its answer.
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
Status   : OPEN

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
Status   : OPEN

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
Status   : OPEN

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
Status   : OPEN

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
Status   : OPEN
