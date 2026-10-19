"""Tests for openfda_labels.py: the history verdict and the report. No network.

    python3 -m unittest test_openfda_labels      (from calibration/)
"""

import unittest

import openfda_labels as ofl


class Verdict(unittest.TestCase):
    def test_current_version_only(self):
        v = ofl.verdict(1, [1, 1, 1])
        self.assertFalse(v["history_available"])
        self.assertEqual(v["sample_labels_with_past_versions"], 0)

    def test_history_needs_both_checks(self):
        self.assertTrue(ofl.verdict(4, [1, 3, 2])["history_available"])
        self.assertFalse(ofl.verdict(4, [1, 1])["history_available"])  # duplicates, but not for revised labels
        self.assertFalse(ofl.verdict(1, [])["history_available"])

    def test_url_quotes_search(self):
        link = ofl.url(ofl.LABEL, search='set_id:"abc"', limit=100)
        self.assertTrue(link.startswith("https://api.fda.gov/drug/label.json?"))
        self.assertIn("set_id%3A%22abc%22", link)

    def test_markdown(self):
        r = {"labels": 10, "labels_with_warnings": 8, "labels_at_version_1": 3, "labels_revised": 7,
             "drugsfda_applications": 5, "drugsfda_applications_with_labeling_supplements": 2,
             **ofl.verdict(1, [1, 1])}
        md = ofl.markdown(r)
        self.assertIn("Most records for any one set_id: **1**", md)
        self.assertIn("Version history of label text available: no", md)


if __name__ == "__main__":
    unittest.main()
