# Security policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.2.x   | Yes       |
| < 0.2   | No        |

Fixes are released as a new patch version of the latest minor release.

## Reporting a vulnerability

Please report privately. Do not open a public issue.

Use GitHub's private vulnerability reporting:
**[Report a vulnerability](https://github.com/praveenprk/fid/security/advisories/new)**
(the repository's Security tab → "Report a vulnerability").

Include:

- the fidmeter version, and whether you used the CLI, the Rust crate or the
  Python package;
- what an attacker can do, and the steps or input that show it;
- any fix you suggest.

You will get an answer in the advisory thread. Once a fix is released, the
advisory is published with credit to you, unless you ask not to be named.

## In scope

- Crafted input that makes fid break the draft: an integrity check that
  passes for content changed without the envelope, UNKNOWN reported as
  SIGNAL, or a wrong exit code.
- Crashes, hangs or unbounded work caused by crafted input files (the draft
  requires the work per input to be bounded).
- Problems in the published packages: the `fidmeter` crate on crates.io
  and the `fidmeter` package on PyPI.

## Not a vulnerability

The envelope is not signed, by design. Anyone who can write an artifact can
write a matching envelope. Authenticity needs a signature over the whole
artifact. See the Security Considerations section of
[spec/draft-kumar-fid-00.md](spec/draft-kumar-fid-00.md).
