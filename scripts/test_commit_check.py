import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import commit_check as cc  # noqa: E402

KNOWN = {"decisions": {"DECISION-001"}, "violations": {"V-ENG-005-01"}, "issues": {"14", "18"}}


def run(title, body="", commits=()):
    return cc.check(title, body, list(commits), KNOWN)


class Subject(unittest.TestCase):
    def test_passes_with_each_anchor_kind(self):
        for anchor in ("DECISION-001", "V-ENG-005-01", "ENG-STD-005", "#14"):
            self.assertEqual(run(f"docs(governance): record the adoption — {anchor}"), [], anchor)

    def test_several_anchors_and_ascii_dash(self):
        self.assertEqual(run("fix(calibrate): park per-entity baselines - #14, DECISION-001"), [])

    def test_anchors_follow_the_last_separator(self):
        self.assertEqual(run("chore(governance): adopt — registers, hooks — DECISION-001, #18"), [])

    def test_refuses_free_form(self):
        self.assertIn("is not 'type(scope)", run("Calibration by maximum likelihood (#12)")[0])

    def test_refuses_unknown_type_and_missing_scope(self):
        self.assertTrue(run("feature(cli): something long enough — #14"))
        self.assertTrue(run("fix: something long enough here — #14"))

    def test_refuses_unresolved_anchors(self):
        for anchor, why in (("DECISION-999", "DECISIONS.md"), ("V-ENG-009-09", "VIOLATIONS.md"),
                            ("#99", "not an issue"), ("RFC-001", "is not an anchor")):
            reasons = run(f"docs(governance): record something — {anchor}")
            self.assertEqual(len(reasons), 1, anchor)
            self.assertIn(why, reasons[0])


class Attribution(unittest.TestCase):
    def test_refuses_co_author_in_commit(self):
        msg = "fix(cli): exit 3 on unknown flags — #14\n\nCo-authored-by: Someone <a@b.c>"
        self.assertIn("attribution", run("fix(cli): exit 3 on unknown flags — #14", commits=[msg])[0])

    def test_refuses_generated_with_in_description(self):
        body = "What\n\n---\n🤖 Generated with [Claude Code](https://claude.com/claude-code)"
        self.assertIn("PR description", run("docs(plans): index the plans — DECISION-001", body)[0])

    def test_session_id_trailer_is_fine(self):
        body = "What\n\n---\nSession-Id: session_01CtGNAPRWouzUQNSzbu7BvJ"
        self.assertEqual(run("docs(plans): index the plans — DECISION-001", body), [])


class Commits(unittest.TestCase):
    def test_checks_every_commit_and_skips_empty_records(self):
        good = "docs(plans): index the plans — DECISION-001\n\nbody"
        bad = "update stuff"
        reasons = run("docs(plans): index the plans — DECISION-001", commits=["", good, "\n", bad])
        self.assertEqual(len(reasons), 1)
        self.assertIn("update stuff", reasons[0])


class Registers(unittest.TestCase):
    def test_reads_heading_ids(self):
        ids = cc.headings(os.path.join(cc.ROOT, "docs", "DECISIONS.md"), "##")
        self.assertIn("DECISION-001", ids)
        self.assertTrue(all(i.startswith("DECISION-") for i in ids), ids)


if __name__ == "__main__":
    unittest.main()
