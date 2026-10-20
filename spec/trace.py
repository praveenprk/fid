#!/usr/bin/env python3
"""Check that every requirement in the draft is tested, and write TRACEABILITY.md.

A section of the draft "has requirements" when its own text (up to the
next heading) uses MUST, SHOULD, SHALL or REQUIRED. Each such section
needs an anchor, and needs a test or a vector that names the anchor, or
an entry in gaps.json (an open issue, or an exemption with a reason).

Coverage comes from:
- conformance/vectors.json: every vector's "sections";
- tests: a comment `// spec: a, b` (Rust) or `# spec: a, b` (Python)
  just above a test function; `(deviation #N)` after the anchors marks a
  test that pins fid's current behaviour where the draft says otherwise.

Fails (exit 1) when a section with requirements has neither coverage
nor a gap; when a tag or gap names an anchor the draft does not have;
when a section with requirements has no anchor; or when a vector has
no sections. Standard library only.

Usage: python3 spec/trace.py           # check, and rewrite TRACEABILITY.md
"""

import collections
import glob
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DRAFT = os.path.join(ROOT, "spec", "draft-kumar-fid-00.md")
VECTORS = os.path.join(ROOT, "conformance", "vectors.json")
GAPS = os.path.join(ROOT, "spec", "gaps.json")
OUT = os.path.join(ROOT, "spec", "TRACEABILITY.md")
TEST_GLOBS = ["tests/*.rs", "src/*.rs", "bindings/python/tests/*.py", "calibration/test_*.py"]

KEYWORDS = re.compile(r"\b(MUST|SHOULD|SHALL|REQUIRED)\b")
HEADING = re.compile(r"^(#+)\s+(.*?)\s*(?:\{#([a-z0-9-]+)\})?\s*$")
TAG = re.compile(r"^\s*(?://|#)\s*spec:\s*([a-z0-9, -]+?)\s*(?:\(deviation #(\d+)\))?\s*$")
TEST_FN = re.compile(r"^\s*(?:pub\s+)?(?:async\s+)?(?:fn|def)\s+([A-Za-z0-9_]+)")


def sections(text):
    """[(anchor or None, title, has requirements)], body sections only (middle and back)."""
    out, current, body = [], None, []
    in_body = False
    for line in text.split("\n"):
        if line.strip() == "--- middle":
            in_body = True
            continue
        if not in_body:
            continue
        m = HEADING.match(line)
        if m:
            if current:
                out.append((*current, bool(KEYWORDS.search("\n".join(body)))))
            current, body = (m.group(3), m.group(2)), []
        else:
            body.append(line)
    if current:
        out.append((*current, bool(KEYWORDS.search("\n".join(body)))))
    return out


def test_tags():
    """{anchor: [(file, test, deviation issue or None)]}"""
    found = collections.defaultdict(list)
    for pattern in TEST_GLOBS:
        for path in sorted(glob.glob(os.path.join(ROOT, pattern))):
            lines = open(path, encoding="utf-8").read().split("\n")
            for i, line in enumerate(lines):
                m = TAG.match(line)
                if not m:
                    continue
                name = next((TEST_FN.match(l).group(1) for l in lines[i + 1:i + 6] if TEST_FN.match(l)), None)
                if name is None:
                    sys.exit(f"{os.path.relpath(path, ROOT)}:{i + 1}: spec tag not followed by a test function")
                for anchor in (a.strip() for a in m.group(1).split(",")):
                    found[anchor].append((os.path.relpath(path, ROOT), name, m.group(2)))
    return found


def vector_tags(vectors):
    """{anchor: Counter(group)}, and the groups with an untagged vector."""
    found, untagged = collections.defaultdict(collections.Counter), []
    for group, value in vectors.items():
        items = value if isinstance(value, list) else [value] if isinstance(value, dict) else []
        for v in items:
            if not v.get("sections"):
                untagged.append(group)
            for anchor in v.get("sections", []):
                found[anchor][group + (f" (deviation #{v['deviation_issue']})" if "deviation_issue" in v else "")] += 1
    return found, untagged


def main():
    secs = sections(open(DRAFT, encoding="utf-8").read())
    anchors = {a for a, _, _ in secs if a}
    vectors, untagged = vector_tags(json.load(open(VECTORS, encoding="utf-8")))
    tests = test_tags()
    gaps = json.load(open(GAPS, encoding="utf-8"))
    problems = []

    if untagged:
        problems.append(f"vectors without sections in: {sorted(set(untagged))}")
    for title in (t for a, t, req in secs if req and not a):
        problems.append(f"section '{title}' has requirements but no anchor")
    for anchor in sorted((set(vectors) | set(tests) | set(gaps)) - anchors):
        problems.append(f"'{anchor}' is tagged or listed in gaps.json but is not an anchor in the draft")
    for anchor, entry in gaps.items():
        for g in entry:
            if not (isinstance(g.get("issue"), int) or g.get("exempt")) or not g.get("what"):
                problems.append(f"gaps.json '{anchor}': each entry needs 'what' and an 'issue' number or an 'exempt' reason")

    rows = []
    for anchor, title, req in secs:
        if not anchor:
            continue
        covered = anchor in vectors or anchor in tests
        gap = gaps.get(anchor, [])
        if req and not covered and not gap:
            problems.append(f"section '{anchor}' ({title}) has requirements but no test, vector or gap")
        if not (req or covered or gap):
            continue
        v = ", ".join(f"{g} ({n})" for g, n in sorted(vectors.get(anchor, {}).items()))
        t = ", ".join(sorted({f"`{n}`" + (f" (deviation #{d})" if d else "") for _, n, d in tests.get(anchor, [])}))
        g = "; ".join((f"#{x['issue']}: " if "issue" in x else "exempt: ") + x["what"] for x in gap)
        status = "gap" if not covered else "partial" if gap else "covered"
        rows.append(f"| {anchor} | {title} | {v or '—'} | {t or '—'} | {status}{': ' + g if g else ''} |")

    with open(OUT, "w", encoding="utf-8") as f:
        f.write("# Traceability: draft-kumar-fid-00 → tests\n\n"
                "Generated by `spec/trace.py` from the draft, `conformance/vectors.json`,\n"
                "the `spec:` tags on tests, and `spec/gaps.json`. Do not edit by hand; CI\n"
                "fails if this file is out of date or a section with requirements is\n"
                "neither tested nor listed as a gap.\n\n"
                "| Section | Title | Vectors | Tests | Status |\n|---|---|---|---|---|\n")
        f.write("\n".join(rows) + "\n")

    if problems:
        print("spec/trace.py: " + "\n  ".join(["requirements not traced:"] + problems), file=sys.stderr)
        sys.exit(1)
    print(f"spec/trace.py: {len(rows)} sections traced")


if __name__ == "__main__":
    main()
