# fid specification

`draft-kumar-fid-00.md` is fid written as an Internet-Draft, in kramdown-rfc
markdown, the IETF's source format. It is the normative definition of
fid; the code implements it.

## Order of work

1. Change the draft. A behaviour change, a new field, a new bound in an
   acceptance test: the text comes first, with its reason.
2. Change `conformance/generate.py` (the second implementation, which
   does not call fid) and regenerate `conformance/vectors.json`; or add
   the test that names the draft section.
3. Change fid until the vectors and tests pass.

A test bound is never moved to make a failing run pass. If a bound is
wrong, that is a change to the draft, made and reviewed before the run
that is judged by it.

`TRACEABILITY.md` maps each requirement to its tests and lists the gaps.

## Building

```sh
gem install kramdown-rfc -v 1.7.43
pip install xml2rfc==3.34.1
spec/build.sh        # writes spec/build/draft-kumar-fid-00.txt and .html
```

CI runs the same script and fails on any error, or any kramdown-rfc
warning; the built text and HTML are attached to each CI run.

kramdown-rfc fetches references from bib.ietf.org; set
`KRAMDOWN_OFFLINE=1` to build without network (references are then
placeholders).
