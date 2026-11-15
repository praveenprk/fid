"""Tests for the tools that enforce the draft: trace.py and spec_ref.py. No network."""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import spec_ref  # noqa: E402
import traceability  # noqa: E402


def _read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


KNOWN = {"check", "cli", "durations"}


class SpecRef(unittest.TestCase):
    def test_docs_only_needs_nothing(self):
        self.assertIsNone(spec_ref.check("", [], ["docs/ROADMAP.md", "spec/draft-kumar-fid-00.md"], KNOWN))

    def test_behaviour_change_needs_a_spec_line(self):
        reason = spec_ref.check("Fixes things.", [], ["src/check.rs"], KNOWN)
        self.assertIn("no 'Spec:", reason)

    def test_anchor_or_issue_passes(self):
        self.assertIsNone(spec_ref.check("Spec: check, cli", [], ["src/check.rs"], KNOWN))
        self.assertIsNone(spec_ref.check("text\nspec: #13\n", [], ["bindings/python/src/lib.rs"], KNOWN))

    def test_unknown_anchor_fails(self):
        self.assertIn("not anchors", spec_ref.check("Spec: chekc", [], ["src/check.rs"], KNOWN))

    def test_untouched_template_fails(self):
        body = _read(os.path.join(spec_ref.ROOT, ".github", "pull_request_template.md"))
        self.assertIn("no 'Spec:", spec_ref.check(body, [], ["src/check.rs"], KNOWN))
        filled = body.replace("Spec: <!--", "Spec: cli <!--", 1)
        self.assertIsNone(spec_ref.check(filled, [], ["src/check.rs"], KNOWN))

    def test_none_needs_the_label(self):
        self.assertIn("add the label", spec_ref.check("Spec: none", [], ["src/check.rs"], KNOWN))
        self.assertIsNone(spec_ref.check("Spec: none", ["no-behaviour-change"], ["src/check.rs"], KNOWN))

    def test_label_passes(self):
        self.assertIsNone(spec_ref.check("", ["no-behaviour-change"], ["src/check.rs"], KNOWN))

    def test_generator_is_behaviour(self):
        self.assertIsNotNone(spec_ref.check("", [], ["conformance/generate.py"], KNOWN))

    def test_anchors_from_the_draft(self):
        text = _read(spec_ref.DRAFT)
        self.assertTrue({"check", "cli", "durations", "acceptance"} <= spec_ref.anchors(text))


class Trace(unittest.TestCase):
    def test_sections_find_requirements_and_anchors(self):
        text = "front\n--- middle\n# A {#a}\nIt MUST hold.\n## B\nNothing here.\n## C {#c}\nIt MAY.\n--- back\n# D {#d}\nSHOULD.\n"
        self.assertEqual(traceability.sections(text), [("a", "A", True), (None, "B", False), ("c", "C", False), ("d", "D", True)])

    def test_test_tag_pattern(self):
        m = traceability.TAG.match("    // spec: half-life-registry, cli (deviation #11)")
        self.assertEqual((m.group(1), m.group(2)), ("half-life-registry, cli", "11"))
        self.assertEqual(traceability.TAG.match("# spec: check").group(1), "check")


if __name__ == "__main__":
    unittest.main()
