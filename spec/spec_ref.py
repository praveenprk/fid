#!/usr/bin/env python3
"""A pull request that changes behaviour must say which part of the draft it implements.

Behaviour lives in BEHAVIOUR paths. If a PR changes any of them, its
description needs a line

    Spec: <anchor>, <anchor>, #<issue>

naming at least one anchor of the draft or an issue, or the PR carries
the label `no-behaviour-change`. Anchors are checked against the draft.

Inputs come from the environment, never from the command line, so a
crafted description cannot reach a shell: PR_BODY, PR_LABELS (JSON list
of names), and CHANGED_FILES (newline-separated paths).
"""

import json
import os
import re
import sys


def _read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DRAFT = os.path.join(ROOT, "spec", "draft-kumar-fid-00.md")
BEHAVIOUR = ("src/", "bindings/python/src/", "bindings/python/python/", "conformance/generate.py")
LABEL = "no-behaviour-change"


def anchors(draft_text):
    return set(re.findall(r"\{#([a-z0-9-]+)\}", draft_text))


def check(body, labels, changed, known):
    """None if the PR passes, else the reason it does not."""
    touched = sorted(f for f in changed if f.startswith(BEHAVIOUR))
    if not touched:
        return None
    if LABEL in labels:
        return None
    text = re.sub(r"<!--.*?-->", "", body or "", flags=re.S)  # the template's hint is not an answer
    lines = [m.group(1) for m in re.finditer(r"(?im)^\s*spec:[ \t]*(.*?)\s*$", text)]
    refs = [r.strip() for line in lines for r in line.split(",") if r.strip()]
    if [r.lower() for r in refs] == ["none"]:
        return f"'Spec: none' but this PR changes behaviour ({', '.join(touched[:5])}); add the label '{LABEL}' if it does not"
    issues = [r for r in refs if re.fullmatch(r"#\d+", r)]
    named = [r for r in refs if not re.fullmatch(r"#\d+", r)]
    unknown = [r for r in named if r not in known]
    if unknown:
        return f"'Spec:' names {unknown}, which are not anchors in the draft"
    if not (issues or named):
        return (f"this PR changes behaviour ({', '.join(touched[:5])}{', ...' if len(touched) > 5 else ''}) "
                f"but its description has no 'Spec: <draft anchor or #issue>' line; "
                f"add one, or the label '{LABEL}' if behaviour does not change")
    return None


def main():
    known = anchors(_read(DRAFT))
    labels = json.loads(os.environ.get("PR_LABELS") or "[]")
    changed = [f for f in os.environ.get("CHANGED_FILES", "").split("\n") if f]
    reason = check(os.environ.get("PR_BODY", ""), labels, changed, known)
    if reason:
        print(f"spec_ref: {reason}", file=sys.stderr)
        sys.exit(1)
    print("spec_ref: ok")


if __name__ == "__main__":
    main()
