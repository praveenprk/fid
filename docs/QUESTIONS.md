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
Raised by: chore/ENG-DEC-010-develop-qa-main
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
Raised by: chore/ENG-DEC-010-develop-qa-main
Status   : OPEN
