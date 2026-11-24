# Contributing to fid

Thank you for looking. This page tells you how a change gets into fid,
start to finish. The full rules for every contributor, human or automated,
are in [AGENTS.md](AGENTS.md). This page is the short version.

## Before you start

- **Is it already known?** Check the [issues](https://github.com/praveenprk/fid/issues),
  [docs/DECISIONS.md](docs/DECISIONS.md) and [spec/gaps.json](spec/gaps.json).
- **A security problem?** Do not open an issue. See [SECURITY.md](SECURITY.md).
- **A change in behaviour?** fid is specified by an Internet-Draft,
  [spec/draft-kumar-fid-00.md](spec/draft-kumar-fid-00.md). The draft changes
  first, then the tests, then the code ([spec/README.md](spec/README.md)).
  Open an issue to discuss a behaviour change before writing code.

## Build and test

You need Rust (stable; the minimum supported version is the `rust-version`
in Cargo.toml) and Python 3.9 or later.

```bash
cargo test                                   # the crate and the CLI
python3 conformance/generate.py              # vectors; git diff must be empty
python3 spec/traceability.py                 # every requirement tested or a gap

cd bindings/python                           # the Python package
python -m venv .venv && . .venv/bin/activate
pip install "maturin>=1.5,<2" && maturin develop --release
python -m unittest discover -s tests -v
```

Before you push, run the same formatters and linters as CI:

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
pip install ruff==0.17.5 && ruff check . && ruff format --check .
```

CI runs on Linux, and Linux is the reference. Floating-point rounding can
make `conformance/vectors.json` differ in its last digits on other
platforms.

## Your pull request

- **Target branch:** `main`, from a branch of your own. Releases are cut
  from `stable`, which only the maintainer updates (DECISION-008).
- **Who merges:** once every required check passes, the pull request has
  had a review, and no question in docs/QUESTIONS.md is OPEN, any
  authorised contributor may merge it into `main`. Security-related
  changes (label `security`) are merged only by the maintainer, who alone
  promotes `main` to `stable` and tags releases (AGENTS.md, Roles).
- **Branch name:** `type/ANCHOR-short-description`, e.g. `fix/31-ttl-rounding`.
- **Title and every commit:** `type(scope): description — ANCHOR`.
  - Types: feat, fix, test, refactor, docs, chore, design.
  - An anchor is the issue or PR number (`#31`), or a register entry
    (`DECISION-007`, `V-ENG-004-01`). Open an issue first if there is none.
  - Example: `fix(ttl): round half-lives the way the draft says — #31`.
- **Description:** start with a `Spec:` line naming the draft sections or
  issues the change implements. If behaviour does not change, write
  `Spec: none` and ask for the `no-behaviour-change` label.
- **No co-author or tool-attribution lines** (`Co-authored-by:`,
  "Generated with ...") in commits or the description.
- **Pushed history is final.** Add commits. Do not force-push.

The `commit-check` check reports any problem with the title, the
description or a commit, with the reason. The other checks are the test
suites, the formatters and linters, and the spec traceability.

## Versioning

fid follows [Semantic Versioning](https://semver.org/). Before 1.0.0, a
breaking change raises the minor version (DECISION-005). User-visible
changes go in the Unreleased section of [CHANGELOG.md](CHANGELOG.md).

## Conduct

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## Licence

fid is licensed under [Apache-2.0](LICENSE). A contribution you submit is
licensed under the same terms (Apache-2.0, section 5).
