# Traceability: draft-kumar-fid-00 → tests

Every requirement in the draft maps to the tests that check it. A row
with no test is a gap; gaps are closed by adding a vector to
`conformance/generate.py` (for readers and scorers) or a test that names
the section (for calibrators and the CLI), before any code change it
would cover.

Sections are the draft's anchors (`{#...}`).

| Section | Requirement | Tests | Status |
|---|---|---|---|
| durations | grammar (short form and ISO 8601), units, zero allowed, non-finite rejected | `parse_duration` (20 vectors) | covered; vectors do not yet reject `1e3d`, `+5d`, `.5d`, `5.d` (issue #10) |
| half-lives | `infinite`/`inf`, zero rejected, never infinite on error | `parse_half_life` (14) | covered |
| timestamps | age in whole ms, negative age counts as 0, output form and rounding | `envelope` "clock skew", `expires_at` fields | partial: no vector for rounding half up at .5 ms |
| source-classes | defaults, name normalization, unknown indeterminate | `source_fidelity` (12), `envelope` "unknown source" | covered |
| at-measurement | product of (1 − clamped noise) | `fidelity` (16), `envelope` "transform chain" | covered |
| temporal | one clock, shape, fast share, echo, cap at 1 | `fidelity`, `echo` (9), `fast` (11) | covered; no vector combines echo with shape or fast share |
| temporal | curve constraints rejected | `echo_errors` (5), `fast_errors` (5), `shape_errors` (4) | covered |
| fidelity | baseline clamp to f | `fidelity` | covered |
| time-to-floor | closed forms, searches within 1e-6 s, never cases, equal counts as above | `time_to_floor` (8), `echo_time_to_floor` (4), `fast_time_to_floor` (8), `envelope` "exactly at the floor" | covered |
| staleness | best interval within 1e-7 of min cost, never cases | `schedule` (8) | covered |
| staleness | interval for a target, error when 1 − f > S | `staleness_target` (5) | covered |
| staleness | cap at the floor's TTL | Rust `tests/check.rs` (schedule CLI) | not a vector |
| envelope | exactly two top-level members | — | gap |
| envelope-fields | types, ranges, required fields; I-JSON content | — | gap: no invalid-envelope vectors (issue #9) |
| envelope-fields | null half-life answers UNKNOWN; `ext` ignored | `envelope` "no half-life never decays" contradicts the draft | gap (issue #9) |
| stamping | derived stamp keeps source and measured_at, appends one step | Rust `src/stamp.rs` unit tests | not a vector |
| integrity | legacy form (0.x), hash | `integrity` (10), `envelope` "content changed since stamp" | covered |
| integrity | JCS (RFC 8785) for 1.0 | — | gap (issue #8) |
| versioning | 0.1 to 0.4 readable | `envelope` "version 0.1 envelope" | partial: unknown versions and members not yet UNKNOWN (issue #9) |
| check | UNKNOWN on bad integrity or unknown source; SIGNAL at ≥ F | `envelope` (20) | covered |
| expiry | `never`, `unknown`, independent of now and integrity | `envelope` `expires_at` | covered |
| recheck | price, target, floor cap, unknown | `schedule`, `staleness_target` | partial: no envelope-level vectors |
| registries | YAML entry forms, unknown keys rejected, inheritance, fail closed | Rust `src/registry.rs` unit tests | gap: no vectors; whole-state rule not implemented (issue #11) |
| ledger | columns, booleans, age forms, all-or-none states, row-numbered errors | `ledger_csv` (13) | covered |
| calibration (1) | output independent of row order | — | gap (issue #12) |
| calibration (2–5) | held-out keeps, refusal, ranges, s0 written only when measured | Rust `src/calibrate.rs`, `src/fast.rs`, `src/echo.rs` tests | not named by section |
| acceptance | coverage ≥ 34/40; spread narrows; refusal at 60 checks | Rust `test_ranges_cover_the_truth`, `test_ranges_narrow_with_more_data`, `test_too_uncertain_is_error` | covered |
| acceptance | richer-curve false keeps and power | `measure_fast_keeps` (ignored, prints only) | gap: bounds fixed in the draft, not yet tests (issue #12) |
| drift | verdicts, refusal for richer curves | Rust `src/drift.rs` tests, `tests/` | not named by section |
| cli | exit codes 0/1/2/3, stdout/stderr split | Rust `tests/check.rs`, `tests/stamp.rs`, `tests/calibrate.rs` | partial: unimplemented commands exit 2, calibrate refusal exits 3 (issue #13) |
| entity curves | not specified (issue #14) | — | out of scope until specified |
