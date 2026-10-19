# Exporting a re-verification ledger for `fid calibrate`

A store that re-checks facts already holds the data fid needs to measure
decay: each re-check says whether a fact of some type, at some age, was
still true. This page is how to turn those records into the CSV
`fid calibrate` reads. The exact format, with test vectors, is in
[conformance/README.md](../conformance/README.md#ledger-csv);
[conformance/ledger-example.csv](../conformance/ledger-example.csv) is
a complete example.

## One row per re-check

```csv
claim_id,entity,observed_at,checked_at,correct,outcome
c1,company-00001,2026-01-01T01:00:00Z,2026-01-31T01:00:00Z,true,confirmed
c2,company-00002,2026-01-01T02:00:00Z,2026-03-02T02:00:00Z,false,superseded
```

| Column | Write |
|---|---|
| `entity` | A stable id for what was checked — the claim, or the thing it is about (company, vessel, customer) — the same on every re-check of it. Not a per-row id: fid splits train and test by entity, so re-checks of one thing must land on the same side. |
| `observed_at` | When the value being checked was last measured — the fact's `measured_at`. Not when it was ingested, and not the previous re-check unless that re-check re-measured it. |
| `checked_at` | When it was checked against the truth. |
| `correct` | Was the stored value still true then? See below. |
| `state` | Optional: the thing's state at `observed_at` (`underway`, `moored`), to get one half-life per state. All rows or none. |

Other columns (`claim_id`, `outcome`, notes) are ignored; keep them for
audit. Write one file per fact type, and pass the type with `--type`:

```bash
fid calibrate ceo-ledger.csv --type company_ceo > half-lives.yaml
```

## What counts as correct

The question is only: *was the stored value right at `checked_at`?*

| Re-check outcome | `correct` |
|---|---|
| Confirmed unchanged | `true` |
| Superseded: the world changed, a new value replaced it | `false` |
| Rejected: the value was wrong (possibly from the start) | `false` |
| Edited, and the edit changes the meaning | `false` |
| Edited cosmetically (format, spelling) | `true` |
| Inconclusive, source unreachable, not checked | leave the row out |

Rejected-at-age-zero rows are not a problem: they lower "correct when
fresh", which is the source's fidelity, and fid fits that separately
from decay. Leaving out inconclusive checks matters — writing them as
`false` reads as decay that did not happen.

## Sampling bias

A ledger of checks someone *chose* to run over-represents suspicious
facts, so it overstates decay. Re-check a random sample of each fact
type on a schedule, and either export only those rows or export them as
a separate file and compare the two fits. The random sample is the
honest one.

## How much data

Per state: at least 50 rows, at least 10 in each held-out half, and at
least 3 distinct ages with some above zero; otherwise `fid calibrate`
says so instead of guessing. In practice the fit is useful when ages
span past one half-life — if almost every check happens within a week,
a 6-year half-life cannot be measured, only bounded.

Enough rows is not enough data. `fid calibrate` resamples entities 200
times and refits; the middle 95% of the refitted half-lives is its
range. If the high end is more than 2.5× the low end (`--max-spread`),
it refuses: the data cannot tell, say, 3 days from 9. On synthetic
ledgers with a known half-life the range contains the true value in 38
of 40 trials. Ranges are written as comments in the registry.

Check ages also have to reach the decay. Bins hold equal numbers of
checks, so if a fact decays in hours but most checks are weeks old, the
youngest bin averages days and the fit cannot see the decay. The range
shows it (a 3-hour half-life checked uniformly over 40 days: range 6.6h
to 17h, refused), and the source fidelity is then not written.

## Re-check the calibration

Run `fid drift` on each new batch of random re-checks against the
registry in force. Exit 1 means the facts no longer decay the way the
registry says: recalibrate from recent checks. Exit 2 means the batch
is too small to tell either way.

## Review the fit before committing it

`fid calibrate` prints the curve, the fitted model beside it, and
held-out Brier scores against decay-to-zero and a constant. Commit the
registry it writes only if the model beats the constant on the held-out
half and the curve's largest gap is small. Treat the registry as a
reviewed artifact: a half-life change changes every TTL derived from it.
