# Issue #30: audit gaps and the path to L4

Date: 2026-11-25. Source: eng-principles audit report 2026-11-22T23:29Z,
issue #30. One pull request per gap, none merged by the session that
opened it. engineering-fa reviews each.

| Gap | Severity | What fid does | PR |
|-----|----------|---------------|----|
| `dependabot` | HIGH | .github/dependabot.yml: cargo (two crates), pip, github-actions. V-ENG-008-02 | #31 |
| `signed` | HIGH | Already V-ENG-008-01 (next release is signed). The release workflow now refuses an unsigned tag or one off `stable` | #32 |
| `docs` | MEDIUM | Crate doc, 98 doc comments, `#![warn(missing_docs)]` under clippy `-D warnings`. V-ENG-008-03 | #33 |
| `fmt_lint` | HIGH | shellcheck and shfmt for spec/build.sh; Prettier and ESLint for the site's inline scripts. V-ENG-004-02 | #34 |
| L3 → L4 | — | DECISION-009 (PROPOSED): docs/RUNS.md, a dated ledger of runs in real use | this plan's PR |

Order: any. The PRs touch different files except docs/VIOLATIONS.md's
summary table, where the second to merge needs a one-line conflict fix.

Open after these: the next release must be signed (V-ENG-008-01 stays OPEN
until then); dependabot PR titles fail `commit-check` until they gain an
anchor (an eng-principles question, not fid's); CI `run:` steps are not
linted (actionlint would do it); L4 waits on real-use rows in docs/RUNS.md.
