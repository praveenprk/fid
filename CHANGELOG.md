# Changelog

All notable changes to fidmeter (the `fid` command, the Rust crate and the
Python package) are recorded here. The format follows
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, a breaking change raises the minor version (DECISION-005).

## [Unreleased]

## [0.2.1] - 2026-11-27

A patch release. Behaviour is unchanged; fid computes, stamps and checks
exactly as 0.2.0 did.

### Changed
- The minimum supported Rust version is 1.85, not 1.80. The locked
  dependencies of 0.2.0 already needed 1.85; CI now builds and tests on it.
- The Python package builds against PyO3 0.30 (was 0.29). The command line
  uses clap 4.6.9 (was 4.6.7).
- Every public item of the Rust crate is documented.

### Security
- A release is published to crates.io and PyPI only from an annotated tag
  that is signed, verified by GitHub, and on `stable`.

## [0.2.0] - 2026-11-17

Breaking. If you pin `fidmeter = "0.1"`, you stay on 0.1.x until you change
the pin.

### Changed
- Artifacts are written as envelope format 1.0, with JCS (RFC 8785) hashes.
- An artifact stamped without a half-life checks UNKNOWN instead of being
  scored.
- Calibration fits by maximum likelihood, so calibrated half-lives differ
  from 0.1.x values.

### Added
- fid is specified by an Internet-Draft, spec/draft-kumar-fid-00.md, with
  conformance vectors. Every normative section is tested or listed as a gap.

## [0.1.1] - 2026-10-18

### Fixed
- A float did not survive a round trip through JSON, so the integrity check
  of a stamped artifact could fail. Floats now round-trip exactly.

### Added
- Conformance vectors and a guide to exporting the ledger.

## [0.1.0] - 2026-10-17

### Added
- Commands `fid stamp`, `check`, `chain`, `ttl` and `calibrate`.
- The decay model: a baseline, half-lives that depend on state, and
  conditional noise.
- The Python package (`import fid`).
- Calibration against public ground-truth data (docs/FINDINGS.md).

[Unreleased]: https://github.com/praveenprk/fid/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/praveenprk/fid/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/praveenprk/fid/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/praveenprk/fid/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/praveenprk/fid/releases/tag/v0.1.0
