"""Tests for dailymed_warnings.py: section extraction, the timeline and observations. No network.

    python3 -m unittest test_dailymed_warnings      (from calibration/)
"""

import io
import unittest
import zipfile
from datetime import date

import dailymed_warnings as dw
import fidcal

SPL = b"""<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="urn:hl7-org:v3">
 <component><structuredBody>
  <component><section><code code="34066-1"/><title>WARNING: BLEEDING</title>
   <text><paragraph>May cause   major bleeding.</paragraph></text></section></component>
  <component><section><code code="34084-4"/><title>ADVERSE REACTIONS</title>
   <text><paragraph>Nausea.</paragraph></text></section></component>
  <component><section><code code="43685-7"/><title>5 WARNINGS AND PRECAUTIONS</title>
   <component><section><code code="42229-5"/><title>5.1 Liver</title>
    <text><paragraph>&#8226; Monitor liver tests.</paragraph></text></section></component>
  </section></component>
 </structuredBody></component>
</document>"""


class Text(unittest.TestCase):
    def test_warnings_sections_only(self):
        text = dw.warnings_text(SPL)
        self.assertIn("may cause major bleeding", text)
        self.assertIn("5 1 liver monitor liver tests", text)
        self.assertNotIn("nausea", text)

    def test_no_warnings(self):
        self.assertIsNone(dw.warnings_text(SPL.replace(b"34066-1", b"1").replace(b"43685-7", b"2")))

    def test_formatting_is_not_a_change(self):
        self.assertEqual(dw.normalize("• Monitor  LIVER tests;"), dw.normalize("Monitor liver tests."))
        self.assertNotEqual(dw.normalize("Monitor liver tests."), dw.normalize("Monitor kidney tests."))

    def test_nested_warnings_counted_once(self):
        nested = SPL.replace(b'code="42229-5"', b'code="34071-1"')
        self.assertEqual(dw.warnings_text(nested).count("monitor liver tests"), 1)

    def test_xml_from_zip(self):
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, "w") as z:
            z.writestr("image-01.jpg", b"\xff\xd8")
            z.writestr("label.xml", SPL)
        self.assertEqual(dw.xml_from_zip(buf.getvalue()), SPL)


class Timeline(unittest.TestCase):
    def setUp(self):
        self.changes = dw.timeline([(date(2009, 1, 1), "a"), (date(2010, 1, 1), "a"),
                                    (date(2012, 6, 1), "b"), (date(2020, 1, 1), "b")])

    def test_only_changes_kept(self):
        self.assertEqual(self.changes, [(date(2009, 1, 1), "a"), (date(2012, 6, 1), "b")])

    def test_text_at(self):
        self.assertIsNone(dw.text_at(self.changes, date(2008, 12, 31)))
        self.assertEqual(dw.text_at(self.changes, date(2012, 5, 31)), ("a", date(2009, 1, 1)))
        self.assertEqual(dw.text_at(self.changes, date(2012, 6, 1))[0], "b")

    def test_states(self):
        self.assertEqual(dw.state(100), "recent")
        self.assertEqual(dw.state(800), "settled")
        self.assertEqual(dw.state(3000), "old")

    def test_observations(self):
        obs = dw.observations({"L": self.changes})
        self.assertTrue(all(c for _, lag, c in obs["all"] if lag == 0))
        snaps = [s for s in dw.SNAPSHOTS if s >= date(2009, 1, 1)]
        self.assertEqual(len(obs["all"]), len(snaps) * len(dw.LAG_DAYS))
        # From January 2009, "a" lasts until June 2012: right at 3 years, wrong at 4
        jan09 = [c for _, lag, c in obs["all"][:len(dw.LAG_DAYS)]]
        self.assertEqual(jan09, [lag < 1461 for lag in dw.LAG_DAYS])
        self.assertEqual(len(obs["recent"]) + len(obs["settled"]) + len(obs["old"]), len(obs["all"]))

    def test_every_lag_is_in_the_past(self):
        self.assertLessEqual(max(dw.SNAPSHOTS).toordinal() + max(dw.LAG_DAYS), dw.END.toordinal())
        self.assertEqual(dw.LAGS[-1], max(dw.LAG_DAYS) * fidcal.DAY)


if __name__ == "__main__":
    unittest.main()
