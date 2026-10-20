---
title: "Fidelity Envelopes: Recording and Computing How Far Stored Data Still Holds"
abbrev: "fid"
docname: draft-kumar-fid-00
category: exp
ipr: trust200902
submissiontype: independent
area: Applications and Real-Time
keyword:
  - data quality
  - staleness
  - provenance
  - decay
stand_alone: yes
pi: [toc, sortrefs, symrefs]
author:
  - name: Praveen Kumar
    email: incopraveen@gmail.com

normative:
  RFC3339:
  RFC4180:
  RFC5234:
  RFC6234:
  RFC7493:
  RFC8259:
  RFC8785:
  IEEE754:
    title: "IEEE Standard for Floating-Point Arithmetic"
    seriesinfo:
      IEEE: Std 754-2019
    date: 2019
  YAML:
    title: "YAML Ain't Markup Language Version 1.2.2"
    target: https://yaml.org/spec/1.2.2/
    date: 2021

informative:
  RFC6838:
  RFC8126:
  ISO8601:
    title: "Date and time — Representations for information interchange"
    seriesinfo:
      ISO: 8601-1:2019
    date: 2019
  FINDINGS:
    title: "fid calibration findings"
    target: https://github.com/praveenprk/fid/blob/main/docs/FINDINGS.md
  ISSUES:
    title: "fid issue tracker"
    target: https://github.com/praveenprk/fid/issues

--- abstract

Stored data describes a world that keeps changing. A ship's recorded
position, a package's latest version, a drug label's warnings: each was
right when measured and becomes less likely to be right with age. This
document specifies a fidelity envelope, a JSON wrapper that records
where a value came from, when it was measured, what transformed it, and
how its chance of still being right decays; the computations any
implementation must perform on it (fidelity at an age, the time it
falls below a floor, when to re-check it); the formats of the
registries and re-check ledgers that supply measured decay; and the
requirements on calibrating decay from re-checks. It also specifies the
exit-code contract of a command-line tool so that fidelity can gate
decisions in ordinary pipelines.

--- middle

# Introduction

Systems store facts and later act on them as if they were still true.
Whether a fact is still true depends on how fast that kind of fact
changes, how reliable its source was, and what happened to it in
between. Nothing in common data formats records any of that.

This document defines:

1. A numeric model of fidelity: the probability that a stored value is
   still right, as a function of source reliability, transform noise,
   and age ({{model}}).
2. The fidelity envelope, a JSON object carrying the model's inputs
   with the value ({{envelope}}).
3. Operations on envelopes: check against a floor, time to floor,
   re-check time ({{operations}}).
4. Registry and ledger formats, by which decay is measured rather than
   guessed ({{registries}}, {{ledger}}).
5. Requirements on calibration, with acceptance tests fixed in advance
   ({{calibration}}).
6. A command-line contract ({{cli}}).

## Design principle: fail closed

Every rule in this document that could go either way goes the way that
cannot make a stale fact look fresh. Where an implementation cannot
determine fidelity (an unknown source, an unknown decay, a field or
version it does not understand, content that no longer matches its
hash), it answers UNKNOWN, never a number. Where input is malformed, it
reports an error, never a default.

## Scope and non-goals

The envelope records evidence about a value; it does not authenticate
it ({{security}}). Fidelity is a probability under a model fitted to past
re-checks; it is not a guarantee about any single value. Choosing a
floor is a policy decision and outside this document.

## Status of this draft

This draft specifies envelope version 1.0. The reference implementation,
fid, writes version 0.4; where it does not yet do what this document
says, {{deviations}} lists the difference and the issue that tracks it
{{ISSUES}}. Decisions taken in writing this draft, and their reasons,
are in {{decisions}}.

A change to behaviour is made in this document first, then in the test
vectors and the second implementation ({{conformance}}), then in code.
A bound in an acceptance test is never moved to make a failing run
pass; changing it is a change to this document, with its reason.

## Requirements Language

{::boilerplate bcp14-tagged}

## Terminology

Fact:
: A value about an entity that can be right or wrong at a given time.

Entity:
: The thing a fact is about (a vessel, an airport, a package).

Information type:
: A named kind of fact with one decay behaviour, such as
  `vessel_position`.

State:
: A condition of the entity that changes how fast its facts decay,
  such as `underway` or `moored`.

Measurement:
: The moment a fact was observed at its source (`measured_at`).

Age:
: Time since measurement, in seconds.

Source fidelity (s):
: The probability that a fact is right at age zero.

Baseline (b):
: The probability that a fully stale fact is right anyway.

Temporal factor:
: A number in \[0, 1\] that falls from 1 at age zero as the fact goes
  stale; defined in {{temporal}}.

Fidelity:
: The probability that the fact is right at a given age.

Floor:
: A fidelity below which a consumer will not act on the fact.

Re-check:
: Observing the fact again at its source and recording whether the
  stored value was still right.

Ledger:
: A table of re-checks ({{ledger}}).

Registry:
: A file mapping information types (and states) to measured decay
  parameters ({{registries}}).

Writer, reader, scorer, calibrator:
: The conformance roles of {{conformance}}.

# Data Types {#types}

## Numbers {#numbers}

All computation uses IEEE 754 binary64 {{IEEE754}}. A value that is not
finite MUST be rejected wherever this document says "number" unless the
field explicitly allows infinity.

A "unit number" is a number in the closed interval \[0, 1\].

## Durations {#durations}

A duration is a non-negative fixed length of time. It is written in
the short form, or as an ISO 8601 duration {{ISO8601}} (ABNF,
{{RFC5234}}; strings are case-insensitive):

~~~ abnf
duration     = *WSP ( short-form / iso-duration ) *WSP
short-form   = number [ unit ]
number       = 1*DIGIT [ "." 1*DIGIT ]
unit         = "s" / "m" / "h" / "d" / "mo" / "y"
iso-duration = "P" ( date-part [ time-part ] / time-part
                     / number "W" )
date-part    = number "Y" [ number "M" ] [ number "D" ]
             / number "M" [ number "D" ]
             / number "D"
time-part    = "T" ( number "H" [ number "M" ] [ number "S" ]
                   / number "M" [ number "S" ]
                   / number "S" )
~~~

| Short unit | ISO designator | Seconds |
|---|---|---|
| (none), `s` | `S` (time part) | 1 |
| `m` | `M` (time part) | 60 |
| `h` | `H` | 3600 |
| `d` | `D` | 86400 |
| — | `W` | 7 × 86400 |
| `mo` | `M` (date part) | 30.44 × 86400 |
| `y` | `Y` | 365.25 × 86400 |

In the short form `m` is always minutes. A duration is a fixed number
of seconds: ISO 8601's calendar meaning of years, months and days is
not used. The value is the sum of each number times its unit's seconds,
computed in binary64. A duration that does not match the grammar, or
whose value is not finite, MUST be rejected. Zero is valid.

A writer MUST write durations in the short form.

## Half-lives {#half-lives}

~~~ abnf
half-life   = duration / *WSP infinite *WSP
infinite    = "infinite" / "inf"     ; case-insensitive
~~~

`infinite` means no decay. A half-life of zero MUST be rejected. An
implementation MUST NOT treat an invalid or missing half-life as
infinite: that would hide decay.

## Timestamps {#timestamps}

Timestamps are RFC 3339 `date-time` values {{RFC3339}} with a time
offset. Ages are computed in whole milliseconds: `age = (now −
measured_at)` truncated to milliseconds, as seconds. A negative age
(clock skew) counts as zero in every formula.

A timestamp this document produces is in UTC, to the millisecond:
`YYYY-MM-DDTHH:MM:SS.mmmZ`. A time computed as `measured_at + t`
seconds is rounded half up to the millisecond.

# Source Classes {#source-classes}

When no measured source fidelity is available, a source class supplies
a default:

| Class | Default source fidelity |
|---|---|
| `instrument` | 0.97 |
| `human_observation` | 0.87 |
| `human_attestation` | 0.80 |
| `database` | 0.90 |
| `llm_grounded` | 0.55 |
| `llm_ungrounded` | 0.25 |
| `hearsay` | 0.17 |
| `unknown` | none |

Class names are matched after trimming whitespace and lowercasing; `-`
MUST be accepted in place of `_`. Any other name MUST be rejected.

A fact from an `unknown` source has no fidelity: implementations MUST
report it as indeterminate, never as 0 and never as a default.

These defaults are conventions, not measurements; measured source
fidelities in {{FINDINGS}} range widely within a class. A measured
`source_fidelity` ({{envelope-fields}}) replaces the default for every
class except `unknown`.

# The Fidelity Model {#model}

## Fidelity at measurement {#at-measurement}

~~~
f = s × ∏ᵢ (1 − clamp(noiseᵢ, 0, 1))
~~~

where s is the source fidelity and noiseᵢ is each transform's noise
({{envelope-fields}}), in order. If s is indeterminate, f is
indeterminate.

## Temporal factor {#temporal}

A curve has a main half-life h (possibly infinite), and optionally a
shape k, a fast share (m, h_f), and an echo (P, r, w, h_r).

One clock:

~~~
clock(t, H) = 0.5 ^ ((max(t, 0) / H) ^ k)       (k = 1 when absent)
~~~

and clock(t, H) = 1 when H is infinite.

Decay:

~~~
D(t) = (1 − m) × clock(t, h) + m × clock(t, h_f)   (m = 0 when absent)
~~~

Echo, when present:

~~~
E(t) = Σ over integers n ≥ 1 with |t − nP| ≤ 30w of
       0.5^(nP / h_r) × 0.5^(|t − nP| / w)
~~~

where t is max(age, 0), and 0.5^(nP / h_r) is 1 when h_r is infinite.

Temporal factor:

~~~
T(t) = min(1, D(t) + r × E(t))        (r × E(t) = 0 when no echo)
~~~

An implementation MUST reject a curve that violates any of:

- k is a number in \[0.1, 10\].
- m is a unit number; h_f is a finite, positive half-life, shorter than
  h. (Swapping the two clocks gives the same curve; requiring h_f < h
  keeps one encoding per curve.)
- An echo has P and w finite and positive, w < P/2, r ≥ 0, and h_r a
  positive half-life, which may be infinite.
- A shape, a fast share, or an echo requires a finite h.

## Fidelity at an age {#fidelity}

~~~
b' = clamp(b, 0, max(f, 0))
fidelity(t) = b' + (f − b') × T(t)
~~~

A baseline above f is clamped to f, so decay never raises fidelity
above its value at measurement (an echo can raise it back toward f).

## Time to floor {#time-to-floor}

The time to floor F is the smallest t ≥ 0 at which fidelity(t) < F.

- If f < F, it is 0.
- If h is infinite, or F ≤ b', it is never.
- With no shape, no fast share and no echo, it is h × log2((f − b') /
  (F − b')).
- With a shape and no fast share or echo, it is h × (log2((f − b') /
  (F − b')))^(1/k).
- Otherwise it has no closed form. An implementation MUST return a time
  within 1e-6 s of the first crossing. With an echo, it is never if no
  crossing occurs within 10,000 periods.

Fidelity exactly equal to F counts as at or above the floor.

## Staleness and re-check intervals {#staleness}

A fact re-measured every T seconds (each re-check restoring age zero)
is served wrong on average for the fraction

~~~
staleness(T) = 1 − (1/T) × ∫₀ᵀ fidelity(t) dt,   staleness(0) = 1 − f
~~~

Best interval for a price p > 0 (the staleness one more re-check per
day must buy):

~~~
cost(T) = staleness(T) + p × 86400 / T
~~~

The best interval is the T in \[1 s, 10,000 × L\] that minimizes
cost(T), where L is the longest of h, P and h_r that is finite. It is
never when there is no decay, or when never re-checking (staleness
1 − b') costs no more than that minimum. An implementation MUST return a
T whose cost is within 1e-7 of the minimum over the interval. p MUST be
a finite positive number.

Interval for a staleness target S in \[0, 1\]: the largest T such that
staleness(u) ≤ S for every u ≤ T; never if staleness never exceeds S.
It is an error if 1 − f > S.

When a floor F is also given, either interval is capped at the time to
floor.

Integrals SHOULD be computed in closed form: for one clock,
∫₀ᵀ clock(t, H) dt = H × ln2^(−1/k) / k × Γ(1/k) × P(1/k, ln2 × (T/H)^k),
with P the regularized lower incomplete gamma function (for k = 1,
H/ln2 × (1 − 0.5^(T/H))). Numerical results MUST agree with the closed
forms within the tolerances of {{tolerances}}.

## Numerical tolerances {#tolerances}

A conforming implementation's results MUST match the test vectors
({{conformance}}) within 1e-9 absolute for fidelities, factors and
staleness, and within 1e-6 s for durations, except where a section
states a different bound.

# The Fidelity Envelope {#envelope}

## Structure

A stamped artifact is a JSON object {{RFC8259}} with exactly two
members:

~~~
{"fid": { ... envelope ... }, "content": <any JSON value>}
~~~

A reader MUST reject an artifact with any other top-level member. The
artifact MUST be I-JSON {{RFC7493}}; a writer MUST reject content that
is not, such as an integer whose magnitude exceeds 2^53, which
different parsers read as different numbers.

## Fields {#envelope-fields}

| Field | Type | Req. | Since | Meaning |
|---|---|---|---|---|
| `fid_version` | string | yes | 0.1 | Envelope version ({{versioning}}) |
| `source_class` | string | yes | 0.1 | {{source-classes}} |
| `source_id` | string | yes | 0.1 | Who or what produced the value |
| `measured_at` | timestamp | yes | 0.1 | When the value was observed at its source |
| `half_life` | half-life or null | yes | 0.1 | h; null means the decay is not known |
| `baseline` | unit number | no | 0.2 | b; absent means 0 |
| `source_fidelity` | unit number | no | 0.3 | Measured s; replaces the class default |
| `echo` | object | no | 0.3 | `{period, strength, width, fade}`: P, r, w (durations), h_r (half-life) |
| `fast` | object | no | 0.4 | `{share, half_life}`: m, h_f |
| `shape` | number | no | 0.4 | k |
| `info_type` | string | no | 0.2 | Information type |
| `state` | string | no | 0.2 | State at measurement |
| `transforms` | array | yes | 0.1 | Transform steps, oldest first |
| `fid_at_stamp` | number or null | yes | 0.1 | Fidelity when last stamped; informative |
| `derived_from` | string or null | yes | 0.1 | Upstream artifact reference, as given |
| `integrity` | string | yes | 0.1 | {{integrity}} |
| `ext` | object | no | 1.0 | Extensions that do not affect fidelity ({{versioning}}) |

A transform step is `{step, noise, at, condition?}`: `step` a string
naming the transform, `noise` a unit number, `at` a timestamp,
`condition` an optional string naming the upstream condition its noise
was measured under.

A null `half_life` means the decay is not known. Fidelity then cannot
be computed: a scorer MUST answer UNKNOWN ({{check}}). A fact that does
not decay MUST be written with `"infinite"`. A writer that writes a
null half-life MUST warn.

`fid_at_stamp` is a convenience copy. Readers MUST compute fidelity from
the other fields and MUST NOT trust `fid_at_stamp`.

## Stamping and derivation {#stamping}

A root stamp creates a new envelope: `measured_at` is the stamp time,
`transforms` is empty unless the stamp records noise above 0 or a
condition, in which case it holds one step.

A derived stamp (over a stamped artifact) MUST keep `source_class`,
`source_id`, `source_fidelity` and `measured_at` from upstream, MUST
append exactly one transform step, and MAY replace `half_life`,
`baseline`, `echo`, `fast`, `shape`, `info_type` and `state`. Decay
always counts from the original measurement.

Every stamp MUST set `fid_version` to the version it writes, recompute
`integrity` over the content, and recompute `fid_at_stamp`. A writer
conforming to this document writes `1.0`.

## Integrity {#integrity}

For `fid_version` 1.0:

~~~
integrity = "sha256:" + lowercase-hex( SHA-256( JCS(content) ) )
~~~

with SHA-256 as in {{RFC6234}} and JCS the JSON Canonicalization Scheme
{{RFC8785}}. Envelopes of versions 0.1 to 0.4 use the legacy canonical
form of {{legacy-canonical}} instead; a reader MUST verify each envelope
with the form of its version.

A scorer MUST answer UNKNOWN for an artifact whose integrity does not
match its content ({{check}}). Integrity detects accidental change; it
does not authenticate ({{security}}).

## Versioning {#versioning}

`fid_version` is `MAJOR.MINOR`, each a decimal integer. Within a MAJOR
version, a higher MINOR only adds optional fields. This document
defines 1.0 and reads 0.1 to 0.4, which differ only by the fields
marked in {{envelope-fields}}.

A field a scorer does not know may change fidelity; computing without
it would give a wrong number with no sign. So a scorer MUST answer
UNKNOWN for an envelope:

- whose MAJOR version it does not implement, or whose MINOR version is
  higher than the highest it implements for that MAJOR; or
- with a member, in the envelope or in any of its objects, that is not
  defined for its version, other than members of `ext`.

`ext` holds extensions: members that do not change fidelity, such as
operational metadata. Scorers MUST ignore `ext`. A field that changes
fidelity MUST NOT be placed in `ext`; it requires a new version.

A reader that does not score (one that only routes or stores artifacts)
MAY pass unknown members through unchanged.

# Operations {#operations}

## Check {#check}

check(artifact, floor F, time now) returns one of:

- UNKNOWN: the integrity does not match; the source is unknown; the
  half-life is null; or the version or a member is not understood
  ({{versioning}});
- SIGNAL: fidelity(now − measured_at) ≥ F;
- NOISE: otherwise.

F MUST be a unit number.

## Expiry {#expiry}

expires_at(artifact, F) is `measured_at` plus the time to floor
({{time-to-floor}}), or `never`, or `unknown` for any reason other than
integrity that check would return UNKNOWN. It does not depend on the
current time or on integrity. With an echo, fidelity can rise above F
again after expires_at.

## Re-check time {#recheck}

recheck_at(artifact, price p \| staleness S, floor F?) is
`measured_at` plus the best interval for p, or the interval for S
({{staleness}}), capped at the time to F when F is given; or `never`; or
`unknown` as for expires_at.

# Registries {#registries}

## Half-life registry

A half-life registry is a YAML 1.2 {{YAML}} mapping from information
type to an entry. An entry is either a half-life string, or a mapping
with any of `half_life`, `baseline`, `source_fidelity`, `echo`, `fast`,
`shape`, and `states`. `states` maps a state name to an entry of the
same form, except that a state MUST NOT have `states`. Any other key
MUST be rejected.

~~~ yaml
vessel_position:
  states:
    underway: { half_life: 8.67m, baseline: 0.03, shape: 3.626,
                fast: { share: 0.828, half_life: 2.64m } }
    moored: 1.31d
airport_temperature:
  half_life: 2.72h
  baseline: 0.09
  echo: { period: 1d, strength: 0.44, width: 5.17h, fade: 2.73d }
~~~

Lookup of (type, state) uses the state's entry alone: no field of the
type's own entry applies to it. A state's curve was fitted as a whole;
mixing in a field fitted for another state gives a curve no data
supports. Lookup of a type without a state uses the type's own fields.

The result MUST have a half-life and MUST satisfy {{temporal}}. Lookups
fail closed: an unknown type, an unknown state, a state requested for a
type without states, or no state requested for a type whose own entry
has no half-life MUST be errors, never defaults.

## Noise model

A noise model is a YAML mapping from transform name to either a unit
number, or `{noise, conditions}` with `conditions` mapping a condition
name to a unit number. Lookups fail closed in the same way.

# Re-check Ledgers {#ledger}

A ledger is CSV {{RFC4180}} with a header row. Header names are matched
case-insensitively; unknown columns MUST be ignored.

| Column | Required | Content |
|---|---|---|
| `entity` | yes | Non-empty string |
| `correct` | yes | `true`/`false`, `t`/`f`, `1`/`0`, `yes`/`no`, `y`/`n` (case-insensitive) |
| `age` | one form | Duration ({{durations}}) |
| `observed_at`, `checked_at` | other form | Timestamps; `checked_at` not before `observed_at` |
| `state` | no | String; empty means none |

Exactly one age form MUST be present. Either every row has a non-empty
state or none does. A row that violates this section MUST cause the
whole ledger to be rejected, with the row number.

A re-check whose outcome is inconclusive MUST be left out, not written
as `false` (that would read as decay that did not happen). Producers
SHOULD sample re-checks at random rather than by suspicion
({{privacy}}).

# Calibration {#calibration}

## Requirements on any calibrator

A calibrator reads a ledger and writes a registry entry per state.

1. Determinism: the output MUST be a function of the multiset of rows.
   It MUST NOT depend on row order, the time of day, or unseeded
   randomness. Where a procedure orders rows, ties in age MUST be broken
   by entity (bytewise on UTF-8) and then by outcome (`false` first).
2. Held-out evidence: a parameter that changes the curve's form (a
   baseline, a shape, a fast share, an echo) MUST be kept only if it
   improves prediction on entities not used to fit it.
3. Estimation: parameters MUST be estimated by maximum likelihood of the
   re-checks (each a Bernoulli outcome with the curve's probability at
   its age), or of bins of them. Any model selection by likelihood (BIC)
   MUST use these estimates.
4. Refusal over guessing: a calibrator MUST refuse, with a reason, when
   the data cannot determine the half-life to within its spread limit,
   and MUST NOT write a value it refused.
5. Uncertainty: every written half-life and baseline MUST carry a 95%
   range.
6. A calibrator MUST NOT write `source_fidelity` unless checks at ages
   where the fitted decay factor is at least 0.9 measure it.
7. Acceptance: a calibrator MUST pass the tests of {{acceptance}}.

How the likelihood is maximized (grid, simplex, starting points) is not
specified; the acceptance tests judge the outcome.

## Reference procedure

The procedure of the reference implementation. A calibrator MAY use
other estimators and settings if it meets the requirements above.

Split:
: An entity is in the training half when the first byte of SHA-256 of
  its UTF-8 name is even.

Minimum data:
: Per state, at least 50 rows, at least 10 in each half, and at least
  3 distinct ages, some above zero.

Bins:
: Up to 40 distinct ages: one bin per age. Otherwise, rows at age zero
  form their own bin and the rest are cut into 20 bins of equal count
  in the order of requirement 1; a bin's age is the mean of its rows'
  ages.

s0 measured at age zero:
: With at least 20 checks at age zero, k right of n, and
  sqrt(p(1 − p)/n) < 0.005 where p = (k + 1)/(n + 2), s0 is fixed at
  k/n. Otherwise s0 is fitted.

Baseline:
: Fitted with and without a baseline on the training half; the baseline
  is kept only if the held-out Brier score improves by more than 1e-9.

Spread:
: 200 bootstrap resamples of entities. The 95% range is the 2.5th to
  97.5th percentile of the refitted half-lives. A range end beyond 10 ×
  the oldest age counts as no decay. The spread is high ÷ low (1 when
  both ends are beyond, infinite when one is). The default limit is 2.5.
  The reference implementation draws entities with SplitMix64 seeded
  with 0x5eedf1d0, index = next() mod (number of entities).

Richer curves:
: Three forms are tried: a shape; two clocks; both. A form passes when,
  fitted on the training half, its held-out Brier score is below one
  clock's by at least 0.1%; fitted on all data, its largest absolute
  gap from the binned data is at least 20% smaller than one clock's;
  and its BIC on all data is lower than one clock's (decay parameters:
  1 for one clock; 2, 3 and 4 for the forms). Of the passing forms the
  lowest BIC is chosen; if its spread exceeds the limit, the next is
  tried, then one clock.

Echo:
: Tested only when a period is given; kept on the same held-out rule.

## Acceptance tests {#acceptance}

Each test generates synthetic ledgers from a known truth: one entity
per check, each check right with the truth's probability at its age,
independently. "Days" ages are 0, 1, 3, 7, 14, 30, 61, 91, 182, 274,
365, 548 and 730 days; "minutes" ages are every minute from 0 to 30.
The generator's seeds are free; the bounds must hold for any seeds.

| Test | Truth and ledgers | Bound |
|---|---|---|
| Coverage | s0 0.95, b 0, h 10 d; 1500 checks at ages uniform on \[0, 40 d\]; 40 ledgers | true h inside the 95% range in ≥ 34 |
| Narrowing | same truth; 300 and 5000 checks | spread smaller with 5000 |
| Refusal | s0 0.95, b 0, h 6 d; 60 checks, ages uniform on \[0, 20 d\] | refuses at limit 2.5 |
| No false richer curve | 20 ledgers of (s0 1, b 0.1, h 80 d) and 20 of (s0 0.95, b 0, h 20 d); 500 checks per days age | a richer curve kept in ≤ 3 of 40 |
| Shape found | s0 1, b 0.03, h 180 s, k 3; 100 checks per minutes age; 20 ledgers | kept in ≥ 18 of 20 |
| Two clocks found | s0 1, b 0.07, h 60 d, m 0.3, h_f 2 d; 500 checks per days age; 20 ledgers | kept in ≥ 18 of 20 |
| Right curve | the two "found" truths, in every ledger where a richer curve is kept | written curve within 3 standard errors of the truth at every age, sqrt(p(1 − p)/n) with p the truth and n the checks at that age |

The bounds for the last four tests were fixed after the reference
implementation's results on the same truths were known: richer curve
kept on one-clock truths in 1 of 40 ledgers, a shape in 20 of 20, two
clocks in 20 of 20 (fid 0.4). They bind every later change. The "right
curve" test has not yet been run in this form ({{deviations}}).

## Drift

A drift test compares a registry entry with recent re-checks by a
likelihood ratio on the raw checks, with degrees of freedom equal to
the values the entry fixes, divided by the design effect of repeated
checks per entity when above 1. At significance 0.05 the verdict is
DRIFTED; otherwise UNKNOWN when the recent half-life's spread exceeds
the limit, else CURRENT. Drift is defined only for one-clock entries
without a shape or fast share; for others a tester MUST refuse.

# Command-Line Interface {#cli}

A conforming command-line tool named `fid` MUST map outcomes to exit
codes:

| Code | Meaning |
|---|---|
| 0 | SIGNAL / CURRENT |
| 1 | NOISE / DRIFTED |
| 2 | UNKNOWN: fidelity indeterminate, or the evidence cannot tell |
| 3 | ERROR: bad arguments, unreadable or malformed input, or a command it does not implement |

Code 2 MUST mean only an epistemic answer. An operational failure MUST
NOT exit 2. Results go to standard output and reports and reasons to
standard error, so that output can be piped.

| Command | Standard output | Exit |
|---|---|---|
| `fid stamp` | stamped artifact | 0, or 3 |
| `fid check FILE --floor F` | nothing | {{check}} |
| `fid ttl FILE --floor F` | expires_at, `never` or `unknown` | as check at F |
| `fid schedule FILE (--price P \| --staleness S) [--floor F]` | re-check time, `never` or `unknown` | as check at F, or at 0.5 |
| `fid chain FILE [--floor F]` | one line per contribution | 0, or 3 |
| `fid calibrate CSV --type T` | registry YAML; nothing on refusal | 0; 2 on refusal; 3 on error |
| `fid drift CSV --type T --half-lives R` | `current`, `drifted` or `unknown` | 0/1/2, or 3 |

# Conformance {#conformance}

Roles:

Writer:
: Produces envelopes ({{envelope}}).

Reader:
: Parses and validates envelopes, registries and ledgers.

Scorer:
: A reader that computes {{model}} and {{operations}}.

Calibrator:
: Writes registries per {{calibration}}.

The test vectors in `conformance/vectors.json` are normative for
writers, readers and scorers. They are produced by a second
implementation (`conformance/generate.py`, standard-library Python,
independent of fid) from this text. Every MUST in {{types}} to
{{operations}} SHOULD have at least one vector naming its section; the
mapping and its gaps are kept in `spec/TRACEABILITY.md`.

# Security Considerations {#security}

An envelope is not signed. Anyone who can write an artifact can write
any envelope; integrity only detects content changed without the
envelope being updated. Consumers that need authenticity MUST obtain it
by other means (a signature over the whole artifact).

A consumer that gates an action on fidelity is trusting the registry
that supplied the decay. A registry is configuration with direct effect
on decisions and SHOULD be reviewed and version-controlled like code.
A drifted registry ({{calibration}}) gives wrong fidelity without any
error.

An UNKNOWN status MUST NOT be treated as SIGNAL. Treating an
operational error as UNKNOWN, or UNKNOWN as an error, lets an attacker
who can corrupt inputs choose the branch a pipeline takes; hence the
distinct exit codes in {{cli}}. For the same reason a scorer answers
UNKNOWN for what it does not understand ({{versioning}}) instead of
computing around it.

Fidelity rises near each echo return, so a check can return SIGNAL after
expires_at. Consumers that cache expiry MUST NOT assume monotonic
decay when an echo is present.

Inputs are parsed from untrusted files. Implementations MUST bound the
work done per input: the echo sum is bounded by 30w around each return;
searches are bounded by the limits stated in {{model}}.

# Privacy Considerations {#privacy}

Ledgers name entities, which may be people. Calibration needs only a
stable identifier per entity, not the entity's identity; producers
SHOULD pseudonymize entity names before export. Registries can reveal
how often a group's facts change; any per-entity output SHOULD be
treated as personal data where entities are people.

# IANA Considerations {#iana}

This document has no IANA actions yet. If it moves forward, it would
request:

- a media type `application/fid+json` for stamped artifacts {{RFC6838}};
- a registry of source classes ({{source-classes}}) with Specification
  Required policy {{RFC8126}}.

--- back

# Legacy canonical form (versions 0.1 to 0.4) {#legacy-canonical}

Envelopes of versions 0.1 to 0.4 compute integrity over this form, not
JCS:

- object members sorted by key, comparing Unicode code points; no
  insignificant whitespace;
- strings in UTF-8, escaping only `"`, `\` and control characters
  (`\u0001`);
- integers as integers; other numbers in the shortest form that
  round-trips; exponent form only below 1e-5 or from 1e16 up, written
  without zero padding (`1e-7`, `-2.5e-7`, `1e+16`); numbers from 1e-5
  to below 1e-4 as decimals (`0.00001`); integral non-integers keep
  `.0` (`1.0`).

It differs from JCS in number formatting (JCS writes `1.0` as `1` and
switches to exponent form at 1e21 and below 1e-6) and in key order (JCS
compares UTF-16 code units).

# Deviations of fid 0.4 from this document {#deviations}

Where the reference implementation does not yet do what this document
says. Each is tracked by an issue {{ISSUES}} and is fixed in code, with
its test vectors first, before fid writes version 1.0.

| # | Deviation | Issue |
|---|---|---|
| 1 | Integrity uses the legacy form for every version; JCS is not implemented. | [#8](https://github.com/praveenprk/fid/issues/8) |
| 2 | Envelope reading: unknown members and versions are ignored, not answered UNKNOWN; `ext` is not recognised; a null half-life is scored as no decay; non-I-JSON content is accepted. | [#9](https://github.com/praveenprk/fid/issues/9) |
| 3 | Durations: any Rust float literal is accepted as the number (`1e3d`, `+5d`, `.5d`, `5.d`); ISO 8601 durations are not. | [#10](https://github.com/praveenprk/fid/issues/10) |
| 4 | Registry lookup inherits each field a state does not give from its type. | [#11](https://github.com/praveenprk/fid/issues/11) |
| 5 | Calibration: fits are least squares on bins, but richer curves are selected by a likelihood-based BIC; equal-count bins split ties in row order; the "right curve" acceptance test and the false-keep and power bounds are not run as tests (only an ignored measurement). | [#12](https://github.com/praveenprk/fid/issues/12) |
| 6 | CLI: `fid measure`, `scan`, `filter` and `watch` are listed but not implemented and exit 2; `fid calibrate` exits 3 on refusal. | [#13](https://github.com/praveenprk/fid/issues/13) |

Per-entity curves (each entity's own baseline and speed, shrunk toward
its state's) are not part of this document. A first implementation is
parked unmerged until a section specifying them, with acceptance tests,
is added here ([#14](https://github.com/praveenprk/fid/issues/14)).

# Decision Record {#decisions}

Decisions taken for this draft on 20 October 2026, by the author, each
the option that fails closed or uses an existing standard:

1. Integrity: RFC 8785 (JCS) from version 1.0. A private canonical form
   is a cost to every implementer and a source of hash mismatches.
   Earlier versions keep their form so existing artifacts still verify.
2. Durations: ISO 8601 accepted on input; the short form written. One
   grammar, strict: loose number forms are rejected.
3. Versions and unknown members: a scorer answers UNKNOWN. A field it
   does not know may change fidelity; `ext` carries what does not.
4. A null half-life is unknown decay, answered UNKNOWN; no decay is
   written `infinite`.
5. h_f shorter than h: required (already enforced by fid 0.4).
6. Registry states are whole: no field inherited from the type.
7. Richer-curve acceptance bounds as in {{acceptance}}: at most 3 false
   keeps in 40, at least 18 of 20 found, the written curve right within
   3 standard errors.
8. Per-entity curves: specified here before any code is merged.
9. The re-check benchmark (calibration/decisions.py) is evidence about
   the model, not part of it; its rule stays with it.
10. Calibration refusal is an epistemic answer: exit 2.
11. Estimation by maximum likelihood is required; search methods are
    not specified, the acceptance tests judge the result.

# Change Log

-00: first version. Specifies envelope version 1.0; fid 0.4's
differences are in {{deviations}}.
