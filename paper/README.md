# paper/: the fid arXiv paper (draft)

Status: **Sections 4–6 drafted** (method, results, re-check scheduling).
Everything else in `main.tex` is a placeholder. The plan for the paper
(scope, outline, gaps, arXiv logistics) is the governor's; this directory
follows its outline.

| File | Section |
|---|---|
| `main.tex` | skeleton: title, author, placeholders for §1–3 and §7–9 |
| `sections/04-method.tex` | §4 Method: observations, datasets, model, fitting, held-out scoring, rules fixed in advance |
| `sections/05-results.tex` | §5 Results: declared lifetimes, baseline, state, conditioned steps, echoes |
| `sections/06-scheduling.tex` | §6 Re-check scheduling: the pre-registered benchmark, and why fid lost |

Author line: Praveen Kumar, independent, with fid's public contact
address (justpraveenhere@protonmail.com, docs/QUESTIONS.md Q-003,
CODE_OF_CONDUCT.md). No personal address goes in this directory.

## Rules for the text

- **Committed data only.** Every number comes from a file committed in
  this repository (`calibration/results/*.md|json`, the calibration
  scripts, `docs/`). No new experiment was run for this draft.
- **Every number is traceable.** Each table and paragraph is followed by a
  `% src:` comment naming the file, and the field or table row, it comes
  from. Where a number is derived (a ratio, a sum), the comment shows the
  arithmetic.
- **No claim beyond the data.** Where the repo's own prose (FINDINGS,
  calibration/README) says more than a committed result supports, the
  draft follows the result and leaves a `% NOTE:` comment.

## Where the draft departs from docs/FINDINGS.md

Found while tracing each number. docs/FINDINGS.md and calibration/README.md
are corrected to the committed results in the same chain: items 3–6
directly, and items 1–2 by docs/QUESTIONS.md Q-007 (use the committed
numbers unless the other run is committed).

1. **Vessels: 6,146, not 6,592.** FINDINGS and calibration/README say
   "6,592 cargo ships and tankers"; `ships_ais.md` says 6,146 entities. No
   committed file contains 6,592.
2. **PyPI packages: 915, not 977.** calibration/README says "977 most
   downloaded packages"; `pypi_latest.md` says 915 entities (and the
   benchmark uses 923).
3. **"The split held on each of six days."** No per-day result is
   committed, so the draft does not claim it.
4. **"About 680×"** divides a fitted half-life (moored, 1.4 d) by an
   empirical one (underway, 3.0 min). The draft gives both 680×
   (fitted ÷ empirical) and 990× (fitted ÷ fitted) and says which is which.
5. **"One scored worse than ignoring age."** The declared lifetime scored
   worse than the constant in 8 of 10 committed comparisons (six of them on
   the same ship data). The draft reports all ten.
6. **"A baseline costs nothing when it is not needed."** It scores worse
   held out in two drug-label states, both by under 0.001. The draft says so.
7. **"Fits better or the same on every dataset"** (calibration/README):
   the baseline's largest gap is larger for ships underway at 10 km (0.278
   against 0.273).

## Not yet possible from committed data

- Bootstrap 95% intervals over entities for §4–5 numbers. Only the
  re-check benchmark (§6) has intervals.
- Time-based splits beyond the echo tests (weather, grid demand).
- The second scheduling policy (`fid schedule`): only its development run
  is committed (`decisions_dev.md`), so §6 reports nothing from it.

## Building

Not built yet: no LaTeX toolchain was available when this was drafted,
so the draft has not been compiled. `pdflatex main.tex` (twice, for
cross-references) should build it. It uses only amsmath, booktabs,
geometry and hyperref.
