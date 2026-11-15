#!/usr/bin/env python3
"""What a pull request brings to `main` follows eng-principles ENG-STD-005 §1.

Checked: the PR title and every non-merge commit in the PR. A squash merge
turns the title (and description) into the commit on `main`; a rebase merge
brings the commits themselves, so both are checked.

Format:

    type(scope): description — ANCHOR[, ANCHOR…]

`type` is one of TYPES. The anchors follow the last " — " (or " - "). Each
anchor must resolve:

    DECISION-NNN    a "## DECISION-NNN" heading in docs/DECISIONS.md
    V-ENG-NNN-NN    a "### V-ENG-NNN-NN" heading in docs/VIOLATIONS.md
    ENG-...         an entry in eng-principles (not checked here)
    #N              an issue or pull request of this repository

No message, and not the PR description, may carry a co-author line or a
tool attribution line (ENG-STD-005 §1.4).

Inputs come from the environment, never from the command line, so crafted
text cannot reach a shell: PR_TITLE, PR_BODY, COMMITS_FILE (messages
separated by the ASCII record separator, 0x1e), ISSUES_FILE (one existing
issue or PR number per line). Exit 0 when everything passes, 1 otherwise,
with one reason per line on stderr.
"""

import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TYPES = ("feat", "fix", "test", "refactor", "docs", "chore", "design")
SUBJECT = re.compile(r"^(%s)\([A-Za-z0-9_.-]+\): .{10,} (—|-) .+$" % "|".join(TYPES))
ATTRIBUTION = re.compile(r"(?im)^\s*co-authored-by:|generated (with|by) \[?claude")


def _read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def headings(path, marker):
    """IDs of the headings that start with `marker` + space in the file at `path`."""
    if not os.path.exists(path):
        return set()
    return {m.group(1) for m in re.finditer(r"(?m)^%s ([A-Z][A-Z0-9-]*)\b" % re.escape(marker), _read(path))}


def refs(subject):
    """The anchors after the last ' — ' (else ' - ')."""
    for sep in (" — ", " - "):
        if sep in subject:
            return [r.strip() for r in subject.rsplit(sep, 1)[1].split(",") if r.strip()]
    return []


def unresolved(anchor, known):
    """None if `anchor` resolves, else why not."""
    if re.fullmatch(r"#\d+", anchor):
        return None if anchor[1:] in known["issues"] else f"{anchor} is not an issue or PR of this repository"
    if re.fullmatch(r"DECISION-\d{3}", anchor):
        return None if anchor in known["decisions"] else f"{anchor} is not in docs/DECISIONS.md"
    if re.fullmatch(r"V-ENG-\d{3}-\d{2}", anchor):
        return None if anchor in known["violations"] else f"{anchor} is not in docs/VIOLATIONS.md"
    if re.fullmatch(r"ENG-[A-Z]+-\d+", anchor):
        return None
    return f"'{anchor}' is not an anchor (DECISION-NNN, V-ENG-NNN-NN, ENG-..., #N)"


def check_subject(what, subject, known):
    """Reasons `subject` fails, each prefixed with `what`."""
    if not SUBJECT.match(subject):
        return [f"{what}: '{subject}' is not 'type(scope): description — ANCHOR' (types: {' '.join(TYPES)})"]
    anchors = refs(subject)
    if not anchors:
        return [f"{what}: no anchor after ' — '"]
    return [f"{what}: {r}" for r in (unresolved(a, known) for a in anchors) if r]


def check(title, body, commits, known):
    """All reasons the PR fails; empty when it passes."""
    reasons = check_subject("PR title", title.strip(), known)
    if ATTRIBUTION.search(body or ""):
        reasons.append("PR description: co-author or tool attribution line (ENG-STD-005 §1.4)")
    for message in commits:
        message = message.strip("\n")
        if not message:
            continue
        subject = message.splitlines()[0].strip()
        reasons += check_subject(f"commit '{subject[:60]}'", subject, known)
        if ATTRIBUTION.search(message):
            reasons.append(f"commit '{subject[:60]}': co-author or tool attribution line (ENG-STD-005 §1.4)")
    return reasons


def main():
    known = {
        "decisions": headings(os.path.join(ROOT, "docs", "DECISIONS.md"), "##"),
        "violations": headings(os.path.join(ROOT, "docs", "VIOLATIONS.md"), "###"),
        "issues": set(_read(os.environ["ISSUES_FILE"]).split()) if os.environ.get("ISSUES_FILE") else set(),
    }
    commits = _read(os.environ["COMMITS_FILE"]).split("\x1e") if os.environ.get("COMMITS_FILE") else []
    reasons = check(os.environ.get("PR_TITLE", ""), os.environ.get("PR_BODY", ""), commits, known)
    for r in reasons:
        print(f"commit_check: {r}", file=sys.stderr)
    if reasons:
        sys.exit(1)
    print(f"commit_check: ok (title and {sum(1 for c in commits if c.strip())} commits)")


if __name__ == "__main__":
    main()
