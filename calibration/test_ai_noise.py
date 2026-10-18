"""Tests for ai_noise.py with a fake Messages client: no API calls.

    python3 -m unittest test_ai_noise      (from calibration/)
"""

import json
import re
import tempfile
import unittest
from types import SimpleNamespace

import ai_noise as an


class FakeClient:
    """Answers like a model with known flaws, so the noise is known exactly.

    Extraction reads the true record of the company named in the prompt
    and gets `wrong_field` wrong in every document. The summary keeps only
    the first half of the documents' text (the CEO is lost when it comes
    later); the rewrite is lossless. Documents whose company is listed in
    `refuse` are refused.
    """

    def __init__(self, docs, wrong_field="fiscal_year_end", refuse=()):
        self.by_company = {d["record"]["company"]: d for d in docs}
        self.wrong_field, self.refuse = wrong_field, set(refuse)
        self.messages = self
        self.requests = 0

    def create(self, **request):
        self.requests += 1
        prompt = request["messages"][0]["content"]
        text = re.search(r"<document>\n(.*)\n</document>", prompt, re.S).group(1)
        stop, out = "end_turn", ""
        if "output_config" in request:
            company = re.search(r"facts about (.+?)\.\n", prompt).group(1)
            d = self.by_company[company]
            if company in self.refuse:
                stop = "refusal"
            else:
                fields = {}
                for f in an.FIELDS:
                    truth = d["record"][f]
                    stated = f != "ceo" or d["record"]["ceo"] in text
                    fields[f] = "" if not stated else ("wrong" if f == self.wrong_field else truth)
                out = json.dumps(fields)
        elif prompt.startswith("Summarize"):
            out = text[: len(text) // 2]
        else:
            out = text
        return SimpleNamespace(stop_reason=stop, content=[SimpleNamespace(type="text", text=out)],
                               usage=SimpleNamespace(input_tokens=1, output_tokens=1))


def run(docs, client, cache):
    model = an.Model("fake", None, cache, client=client)
    return model, {d["id"]: an.run_doc(model, d) for d in docs}


class Corpus(unittest.TestCase):
    def test_every_field_is_stated(self):
        for d in an.corpus(80):
            r, text = d["record"], d["text"]
            for f in ("ceo", "founded", "hq_city", "fiscal_year_end"):
                self.assertIn(r[f].lower(), text.lower(), (d["id"], f))
            self.assertIn(f"{int(r['employees']):,}", text)
            self.assertIn(r["company"], text)

    def test_traps_follow_conditions(self):
        for d in an.corpus(80):
            c = d["conditions"]
            self.assertEqual(d["former_ceo"] in d["text"], c["superseded"], d["id"])
            self.assertEqual("billion" in d["text"], c["unit"], d["id"])
            self.assertEqual(d["competitor"]["company"] in d["text"], c["competitor"], d["id"])

    def test_corpus_is_reproducible(self):
        self.assertEqual([d["text"] for d in an.corpus(20)], [d["text"] for d in an.corpus(20)])


class Scoring(unittest.TestCase):
    def test_numbers(self):
        self.assertTrue(an.correct("employees", "4210", "4,210"))
        self.assertTrue(an.correct("revenue_musd", "1200", "1.2 billion"))
        self.assertTrue(an.correct("revenue_musd", "4650.8", "$4,650.8 million"))
        self.assertFalse(an.correct("employees", "4210", "3980"))
        self.assertFalse(an.correct("founded", "1989", ""))

    def test_words(self):
        self.assertTrue(an.correct("ceo", "Rosa Iversen", "rosa iversen."))
        self.assertFalse(an.correct("ceo", "Rosa Iversen", "Tomasz Delacroix"))
        self.assertTrue(an.correct("fiscal_year_end", "October", "Oct"))
        self.assertFalse(an.correct("fiscal_year_end", "October", "March"))


class Pipeline(unittest.TestCase):
    def setUp(self):
        self.cache = tempfile.mkdtemp()
        self.docs = an.corpus(40)

    def test_known_noise_is_measured(self):
        client = FakeClient(self.docs)
        _, results = run(self.docs, client, self.cache)
        rows = an.outcomes(self.docs, results)
        n = an.noise(rows)
        # one field of six always wrong
        self.assertAlmostEqual(n["extract"], 1 / 6)
        # the rewrite loses nothing
        self.assertAlmostEqual(n["rewrite"], 0.0)
        # the summary loses exactly the CEOs stated in the second half of the text
        lost = sum(d["record"]["ceo"] not in d["text"][: len(d["text"]) // 2] for d in self.docs)
        self.assertAlmostEqual(n["accuracy"]["summarize"], (5 * len(self.docs) - lost) / (6 * len(self.docs)))
        self.assertAlmostEqual(n["summarize"], 1 - n["accuracy"]["summarize"] / n["accuracy"]["extract"])

    def test_cache_makes_reruns_free(self):
        client = FakeClient(self.docs)
        run(self.docs, client, self.cache)
        first = client.requests
        model, _ = run(self.docs, client, self.cache)
        # identical requests (the lossless rewrite's extraction is the plain one) are made once
        self.assertLessEqual(first, 5 * len(self.docs))
        self.assertEqual(client.requests, first)
        self.assertEqual(model.calls, 0)

    def test_refused_documents_are_left_out(self):
        refused = {self.docs[0]["record"]["company"], self.docs[1]["record"]["company"]}
        model, results = run(self.docs, FakeClient(self.docs, refuse=refused), self.cache)
        rows = an.outcomes(self.docs, results)
        self.assertEqual(len({r["doc"] for r in rows}), len(self.docs) - 2)
        self.assertGreater(model.refusals, 0)

    def test_breakdown_ranges_and_yaml(self):
        _, results = run(self.docs, FakeClient(self.docs, wrong_field="ceo"), self.cache)
        rows = an.outcomes(self.docs, results)
        b = an.breakdown(rows)
        self.assertEqual(b["field"]["ceo"]["extract"], 1.0)
        self.assertEqual(b["field"]["founded"]["extract"], 0.0)
        lo, hi = b["all"]["range"]["extract"]
        self.assertLessEqual(lo, b["all"]["extract"])
        self.assertGreaterEqual(hi, b["all"]["extract"])
        h = an.held_out(rows)
        self.assertTrue(h["rewrite"]["test_inside"])
        yaml = an.noise_yaml(b)
        self.assertIn("llm_summarize:\n  noise: ", yaml)
        self.assertIn("superseded:", yaml)
        self.assertIn("| summarize |", an.markdown({"model": "fake", "effort": None, "documents": 40, "refused": 0,
                                                     "breakdown": b, "held_out": h, "noise_model": yaml}))


if __name__ == "__main__":
    unittest.main()
