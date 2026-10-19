"""How much does an AI step lose? Transform noise for LLM extraction and summaries.

fid multiplies fidelity by (1 − noise) for every processing step. For
steps done by a language model — pull fields out of a document,
summarize it, rewrite it — the noise has been a guess. This measures it
against documents whose true content is known.

Ground truth: company records (CEO, founding year, headquarters city,
employees, revenue, fiscal year end), generated from a fixed seed and
rendered as documents in four styles: press release, email, fact sheet,
and a market note that also describes a competitor. The records are
fictional so the model cannot answer from memory; every document states
every field. Documents carry the traps real ones do, each recorded as a
condition: a former CEO named beside the current one ("superseded"), a
headcount "up from" an older figure ("old_figure"), revenue in billions
rather than millions ("unit"), and a competitor's figures in the same
text ("competitor").

Transforms, each followed by the same structured extraction:
  extract              document → fields
  summarize → extract  document → 3-sentence briefing → fields
  rewrite → extract    document → plain-language rewrite → fields

Noise, in fid's terms:
  extract   noise = 1 − accuracy(extract)
  summarize noise = 1 − accuracy(summarize → extract) / accuracy(extract)
  (likewise rewrite), so that fid's product (1 − n_s)(1 − n_x) is the
  accuracy of the two steps together. Per field and per condition, with
  95% ranges from resampling documents, fitted on half the documents
  (split by hash of their id) and checked on the other half.

No refusal fallback is configured: a fallback would answer on another
model, and the noise is per model. Refusals are counted and left out.

Needs the Anthropic SDK (pip install anthropic) and ANTHROPIC_API_KEY.
Responses are cached on disk (FID_CAL_CACHE, default calibration/.cache),
so a rerun costs nothing.

Usage:
  python3 ai_noise.py --dry-run            # corpus and request count, no API calls
  python3 ai_noise.py --docs 200           # measure; writes results/ai_noise.{json,md}
  python3 ai_noise.py --model claude-opus-5-5 --effort low
  python3 ai_noise.py --docs 200 --hard    # harder corpus; writes results/ai_noise_hard.{json,md}
"""

import argparse
import hashlib
import json
import os
import random
import re
import sys
from concurrent.futures import ThreadPoolExecutor

import fidcal

DEFAULT_MODEL = "claude-opus-5-5"
SEED = 20261018
BOOTSTRAP = 1000

FIELDS = ["ceo", "founded", "hq_city", "employees", "revenue_musd", "fiscal_year_end"]
MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August",
          "September", "October", "November", "December"]
CITIES = ["Rotterdam", "Columbus", "Osaka", "Porto", "Calgary", "Leipzig", "Tucson", "Adelaide",
          "Valencia", "Raleigh", "Gothenburg", "Pune", "Lyon", "Wichita", "Busan", "Nantes"]
SYLLABLES = ["tal", "ver", "in", "mo", "ra", "ken", "sol", "dar", "vi", "nex", "lu", "or", "an", "cor", "bel", "tri"]
SUFFIXES = ["Systems", "Logistics", "Labs", "Materials", "Analytics", "Foods", "Robotics", "Health"]
FIRST = ["Amara", "Jonas", "Priya", "Mateo", "Ingrid", "Kwame", "Leila", "Tomasz", "Yuki", "Rosa",
         "Henrik", "Adaeze", "Callum", "Ines", "Ravi", "Selin"]
LAST = ["Okafor", "Lindqvist", "Ramanathan", "Ferreira", "Novak", "Haddad", "Brennan", "Takeda",
        "Moreau", "Castellanos", "Adeyemi", "Kowalski", "Iversen", "Mbeki", "Sato", "Delacroix"]
STYLES = ["press_release", "email", "fact_sheet", "market_note"]

SCHEMA = {
    "type": "object",
    "properties": {f: {"type": "string"} for f in FIELDS},
    "required": FIELDS,
    "additionalProperties": False,
}

EXTRACT = """From the document below, give the current facts about {company}.
- ceo: full name of the current chief executive
- founded: year founded
- hq_city: city of the headquarters
- employees: current number of employees, as a plain integer
- revenue_musd: most recent annual revenue in millions of US dollars, as a number
- fiscal_year_end: month the fiscal year ends
Use an empty string for anything the document does not state.

<document>
{text}
</document>"""

SUMMARIZE = """Summarize this document in three sentences for an executive briefing.

<document>
{text}
</document>"""

REWRITE = """Rewrite this document in plain, simple language. Keep all of the information.

<document>
{text}
</document>"""


# ── corpus ───────────────────────────────────────────────────────────────

def _name(rng):
    return "".join(rng.choice(SYLLABLES) for _ in range(rng.choice([2, 3]))).capitalize() + " " + rng.choice(SUFFIXES)


def _person(rng):
    return f"{rng.choice(FIRST)} {rng.choice(LAST)}"


def make_record(rng, doc_id):
    ceo = _person(rng)
    record = {
        "company": _name(rng),
        "ceo": ceo,
        "founded": str(rng.randint(1952, 2019)),
        "hq_city": rng.choice(CITIES),
        "employees": str(rng.randint(60, 90000)),
        "revenue_musd": f"{rng.randint(80, 48000) / 10:.1f}" if rng.random() < 0.5 else str(rng.randint(1200, 30000)),
        "fiscal_year_end": rng.choice(MONTHS),
    }
    former = _person(rng)
    while former == ceo:
        former = _person(rng)
    conditions = {
        "superseded": rng.random() < 0.5,
        "old_figure": rng.random() < 0.5,
        "unit": float(record["revenue_musd"]) >= 1000 and rng.random() < 0.7,
        "competitor": False,
    }
    style = STYLES[doc_id % len(STYLES)]
    if style == "market_note":
        conditions["competitor"] = True
    return {"id": f"doc-{doc_id:04d}", "style": style, "record": record, "former_ceo": former,
            "old_employees": str(max(10, int(int(record["employees"]) * rng.uniform(0.7, 0.95)))),
            "competitor": {"company": _name(rng), "ceo": _person(rng), "employees": str(rng.randint(60, 90000)),
                           "hq_city": rng.choice(CITIES)},
            "conditions": conditions}


def _revenue(doc):
    return _money(float(doc["record"]["revenue_musd"]), doc["conditions"]["unit"])


def render(doc):
    """The document text. Every field is stated; the traps depend on the conditions."""
    r, c = doc["record"], doc["conditions"]
    emp = f"{int(r['employees']):,}"
    ceo_line = (f"{r['ceo']} took over as chief executive from {doc['former_ceo']}, who stepped down last year"
                if c["superseded"] else f"{r['ceo']} is chief executive")
    emp_line = (f"{emp} people, up from {int(doc['old_employees']):,} two years ago" if c["old_figure"] else f"{emp} people")
    comp = doc["competitor"]
    s = doc["style"]
    if s == "press_release":
        return (f"{r['hq_city'].upper()} — {r['company']}, founded in {r['founded']}, today reported annual revenue of "
                f"{_revenue(doc)} for the fiscal year ending in {r['fiscal_year_end']}. {ceo_line}. "
                f"The company now employs {emp_line}, and remains headquartered in {r['hq_city']}.")
    if s == "email":
        return (f"Hi all,\n\nQuick background before Thursday's call with {r['company']}. They were set up in "
                f"{r['founded']} and are based in {r['hq_city']}. {ceo_line}. Headcount is {emp_line}. "
                f"Last fiscal year (it closes in {r['fiscal_year_end']}) they booked {_revenue(doc)} in revenue.\n\nThanks")
    if s == "fact_sheet":
        lines = [f"Company: {r['company']}", f"Founded: {r['founded']}", f"Headquarters: {r['hq_city']}",
                 f"Leadership: {ceo_line}", f"Employees: {emp_line}", f"Revenue: {_revenue(doc)}",
                 f"Fiscal year end: {r['fiscal_year_end']}"]
        return "\n".join(lines)
    return (f"Market note. {comp['company']} ({comp['hq_city']}, {int(comp['employees']):,} staff, led by {comp['ceo']}) "
            f"keeps pressure on {r['company']}. Founded in {r['founded']} and based in {r['hq_city']}, {r['company']} "
            f"has {emp_line}; {ceo_line}. Its revenue was {_revenue(doc)} in the fiscal year to {r['fiscal_year_end']}.")


def _money(musd, billions):
    if billions:
        return f"${musd / 1000:.2f} billion".replace(".00 ", " ")
    return f"${musd:,.1f} million".replace(".0 ", " ")


FILLER = [
    "The company runs {n} distribution centres and buys from about {k:,} suppliers.",
    "{chair} chairs the board, which met {n} times during the year.",
    "Capital spending came to ${m:,} million, mostly on new equipment.",
    "It trains about {k:,} contract workers a year through its academy.",
    "In {y} it opened a research office in {city}.",
    "Its largest customer accounts for {n}% of sales.",
    "The company lists {n} product lines and sells in {n2} countries.",
    "A {y} plant expansion added {k:,} square metres of floor space.",
]


def make_hard(rng, doc):
    """Harder traps on top of a document, from their own random stream so the standard corpus is unchanged."""
    r = doc["record"]
    musd, emp, founded = float(r["revenue_musd"]), int(r["employees"]), int(r["founded"])
    others = [c for c in CITIES if c != r["hq_city"]]
    hard = {
        "buried": rng.random() < 0.5,
        "correction": rng.random() < 0.5,
        "forecast": rng.random() < 0.5,
        "subsidiary": rng.random() < 0.5,
        "relocated": rng.random() < 0.5,
        "renamed": rng.random() < 0.5,
    }
    ceo, chair = r["ceo"], _person(rng)
    while chair in (ceo, doc["former_ceo"]):
        chair = _person(rng)
    pool = rng.sample(FILLER, 6)
    doc["hard"] = {
        "wrong_revenue_musd": musd * rng.choice([rng.uniform(0.7, 0.88), rng.uniform(1.12, 1.35)]),
        "forecast": (int(emp * rng.uniform(1.08, 1.25)), musd * rng.uniform(1.1, 1.3)),
        "subsidiary": {"company": _name(rng), "ceo": _person(rng), "hq_city": rng.choice(others),
                       "employees": max(5, int(emp * rng.uniform(0.15, 0.4)))},
        "old_hq": rng.choice(others),
        "moved": rng.randint(min(founded + 1, 2024), 2024),
        "old_name": _name(rng),
        "renamed": rng.randint(min(founded + 3, 2024), 2024),
        "filler": [s.format(n=rng.randint(3, 40), n2=rng.randint(5, 60), k=rng.randint(200, 90000),
                            m=rng.randint(40, 4000), y=rng.randint(1990, 2024), city=rng.choice(others), chair=chair)
                   for s in pool],
    }
    doc["conditions"].update(hard)


def render_hard(doc):
    """The document with the hard traps that are switched on. Every true field is still stated."""
    text, c, h, r = render(doc), doc["conditions"], doc["hard"], doc["record"]
    billions = c["unit"]
    if c["correction"]:
        right = _revenue(doc)
        text = text.replace(right, _money(h["wrong_revenue_musd"], billions), 1)
    extras = []
    if c["relocated"]:
        extras.append(f"{r['company']} moved its headquarters to {r['hq_city']} from {h['old_hq']} in {h['moved']}.")
    if c["renamed"]:
        extras.append(f"It was founded in {r['founded']} as {h['old_name']} and took its current name in {h['renamed']}.")
    if c["subsidiary"]:
        s = h["subsidiary"]
        extras.append(f"Its subsidiary {s['company']}, based in {s['hq_city']} and led by {s['ceo']}, "
                      f"employs {s['employees']:,} of the group's staff.")
    if c["forecast"]:
        emp, musd = h["forecast"]
        extras.append(f"Management expects headcount to reach {emp:,} and revenue of {_money(musd, billions)} "
                      f"next fiscal year.")
    head, tail = "", ""
    if doc["style"] == "email":
        head, text, tail = "Hi all,\n\n", text[len("Hi all,\n\n"):-len("\n\nThanks")], "\n\nThanks"
    if extras:
        text += ("\nNotes: " if doc["style"] == "fact_sheet" else " ") + " ".join(extras)
    paragraphs = [text]
    if c["buried"]:
        f = h["filler"]
        paragraphs = [" ".join(f[:3]), text, " ".join(f[3:])]
    if c["correction"]:
        paragraphs.append(f"Correction: the revenue figure above is wrong; the correct figure is {_revenue(doc)}.")
    return head + "\n\n".join(paragraphs) + tail


HARD_CONDITIONS = {"buried": FIELDS, "correction": ["revenue_musd"], "forecast": ["employees", "revenue_musd"],
                   "subsidiary": ["ceo", "hq_city", "employees"], "relocated": ["hq_city"], "renamed": ["founded"]}


def corpus(n, hard=False):
    rng = random.Random(SEED)
    docs = [make_record(rng, i) for i in range(n)]
    hard_rng = random.Random(SEED + 1)
    for d in docs:
        if hard:
            make_hard(hard_rng, d)
        d["text"] = render_hard(d) if hard else render(d)
    return docs


# ── scoring ──────────────────────────────────────────────────────────────

def _number(text):
    """A number from model output: '4,210', '1.2 billion', '$350 million'."""
    t = text.lower().replace(",", "").replace("$", "").strip()
    m = re.search(r"-?\d+(\.\d+)?", t)
    if not m:
        return None
    v = float(m.group())
    if "billion" in t:
        v *= 1000
    return v


def _words(text):
    return " ".join(re.sub(r"[^\w\s]", " ", text.lower()).split())


def correct(field, truth, answer):
    """Is the extracted value right? Numbers within 0.5%, words after normalizing case and punctuation."""
    answer = (answer or "").strip()
    if not answer:
        return False
    if field in ("employees", "revenue_musd", "founded"):
        v, t = _number(answer), float(truth)
        return v is not None and abs(v - t) <= 0.005 * abs(t) + (0.5 if field != "revenue_musd" else 0.0)
    if field == "fiscal_year_end":
        a = _words(answer)
        return truth.lower() in a.split() or a[:3] == truth.lower()[:3]
    return _words(answer) == _words(truth)


# ── the model ────────────────────────────────────────────────────────────

class Model:
    """Calls the Messages API with an on-disk cache keyed by the full request."""

    def __init__(self, model, effort, cache_dir, client=None):
        self.model, self.effort = model, effort
        self.cache_dir = os.path.join(cache_dir, "ai_noise")
        os.makedirs(self.cache_dir, exist_ok=True)
        self._client = client
        self.calls = self.cached = self.refusals = 0

    def client(self):
        if self._client is None:
            import anthropic  # only needed when something is not cached
            self._client = anthropic.Anthropic()
        return self._client

    def ask(self, prompt, schema=None):
        """Text (or parsed JSON with a schema), or None on a refusal."""
        request = {"model": self.model, "max_tokens": 16000, "messages": [{"role": "user", "content": prompt}]}
        config = {}
        if self.effort:
            config["effort"] = self.effort
        if schema:
            config["format"] = {"type": "json_schema", "schema": schema}
        if config:
            request["output_config"] = config
        key = hashlib.sha256(json.dumps(request, sort_keys=True).encode()).hexdigest()
        path = os.path.join(self.cache_dir, key + ".json")
        if os.path.exists(path):
            self.cached += 1
            with open(path) as f:
                saved = json.load(f)
        else:
            self.calls += 1
            response = self.client().messages.create(**request)
            text = next((b.text for b in response.content if b.type == "text"), "")
            saved = {"stop_reason": response.stop_reason, "text": text,
                     "usage": {"input": response.usage.input_tokens, "output": response.usage.output_tokens}}
            with open(path, "w") as f:
                json.dump(saved, f)
        if saved["stop_reason"] == "refusal":
            self.refusals += 1
            return None
        if saved["stop_reason"] == "max_tokens":
            raise RuntimeError("response hit max_tokens; raise max_tokens and rerun")
        return json.loads(saved["text"]) if schema else saved["text"]


def run_doc(model, doc):
    """Extracted fields for each pipeline; None where the model refused."""
    company = doc["record"]["company"]

    def extract(text):
        return model.ask(EXTRACT.format(company=company, text=text), SCHEMA)

    out = {"extract": extract(doc["text"])}
    for name, prompt in (("summarize", SUMMARIZE), ("rewrite", REWRITE)):
        intermediate = model.ask(prompt.format(text=doc["text"]))
        out[name] = None if intermediate is None else extract(intermediate)
        out[name + "_text"] = intermediate
    return out


# ── statistics ───────────────────────────────────────────────────────────

PIPELINES = ["extract", "summarize", "rewrite"]


def outcomes(docs, results):
    """One row per (document, field): correct or not in each pipeline. Refused documents are left out."""
    rows = []
    for d in docs:
        res = results[d["id"]]
        if any(res[p] is None for p in PIPELINES):
            continue
        for f in FIELDS:
            rows.append({"doc": d["id"], "field": f, "conditions": d["conditions"],
                         **{p: correct(f, d["record"][f], res[p].get(f, "")) for p in PIPELINES}})
    return rows


def noise(rows):
    """{'extract': n_x, 'summarize': n_s, 'rewrite': n_r} in fid's multiplicative form."""
    if not rows:
        return None
    acc = {p: sum(r[p] for r in rows) / len(rows) for p in PIPELINES}
    out = {"extract": 1 - acc["extract"]}
    for p in ("summarize", "rewrite"):
        out[p] = 1 - acc[p] / acc["extract"] if acc["extract"] > 0 else None
    out["accuracy"] = acc
    out["n"] = len(rows)
    return out


def ranges(rows, rounds=BOOTSTRAP, seed=SEED):
    """95% ranges of each noise from resampling documents with replacement."""
    by_doc = {}
    for r in rows:
        by_doc.setdefault(r["doc"], []).append(r)
    docs = sorted(by_doc)
    rng = random.Random(seed)
    draws = {p: [] for p in PIPELINES}
    for _ in range(rounds):
        sample = [r for _ in docs for r in by_doc[docs[rng.randrange(len(docs))]]]
        n = noise(sample)
        for p in PIPELINES:
            if n[p] is not None:
                draws[p].append(n[p])
    out = {}
    for p, v in draws.items():
        v.sort()
        out[p] = (v[int(0.025 * len(v))], v[min(len(v) - 1, int(0.975 * len(v)))]) if v else (None, None)
    return out


def breakdown(rows):
    """Noise overall, per field, and per condition (present / absent)."""
    out = {"all": {**noise(rows), "range": ranges(rows)}, "field": {}, "condition": {}}
    for f in FIELDS:
        sub = [r for r in rows if r["field"] == f]
        out["field"][f] = {**noise(sub), "range": ranges(sub)}
    affects = {"superseded": ["ceo"], "old_figure": ["employees"], "unit": ["revenue_musd"],
               "competitor": FIELDS, **HARD_CONDITIONS}
    for c in [c for c in affects if rows and c in rows[0]["conditions"]]:
        affected = affects[c]
        for present in (True, False):
            sub = [r for r in rows if r["field"] in affected and r["conditions"][c] == present]
            if sub:
                out["condition"][f"{c}={'yes' if present else 'no'}"] = {**noise(sub), "range": ranges(sub)}
    return out


def held_out(rows):
    """Fit on half the documents, check on the other: is the test estimate inside the train range?"""
    train = [r for r in rows if fidcal.split_train(r["doc"])]
    test = [r for r in rows if not fidcal.split_train(r["doc"])]
    tr, te, rg = noise(train), noise(test), ranges(train)
    return {p: {"train": tr[p], "test": te[p], "train_range": rg[p],
                "test_inside": rg[p][0] is not None and te[p] is not None and rg[p][0] <= te[p] <= rg[p][1]}
            for p in PIPELINES}


def noise_yaml(b):
    def entry(name, key):
        lo, hi = b["all"]["range"][key]
        line = f"{name}:\n  noise: {b['all'][key]:.3f}    # 95% range {lo:.3f} to {hi:.3f}\n"
        conds = [(c, v) for c, v in b["condition"].items() if c.endswith("=yes") and v[key] is not None]
        if conds:
            line += "  conditions:\n" + "".join(f"    {c[:-4]}: {v[key]:.3f}\n" for c, v in conds)
        return line
    return "".join(entry(f"llm_{p}", p) for p in PIPELINES)


def markdown(result):
    b, h = result["breakdown"], result["held_out"]
    pct = lambda v: "—" if v is None else f"{100 * v:.1f}%"
    rng = lambda r: "—" if r[0] is None else f"{100 * r[0]:.1f}–{100 * r[1]:.1f}%"
    corpus_name = " — hard corpus" if result.get("hard") else ""
    lines = [f"### AI transform noise: {result['model']} (effort {result['effort'] or 'default'}){corpus_name}", "",
             f"- {result['documents']} documents, {b['all']['n']} field checks; {result['refused']} documents left out for refusals",
             "- Noise as fid multiplies it: extract = 1 − accuracy; summarize and rewrite = 1 − accuracy(step → extract) ÷ accuracy(extract)",
             "", "| Step | Noise | 95% range | Held-out: train → test |", "|---|---|---|---|"]
    for p in PIPELINES:
        lines.append(f"| {p} | {pct(b['all'][p])} | {rng(b['all']['range'][p])} | "
                     f"{pct(h[p]['train'])} → {pct(h[p]['test'])} {'(inside)' if h[p]['test_inside'] else '(OUTSIDE)'} |")
    lines += ["", "| Field / condition | Extract | Summarize | Rewrite | Checks |", "|---|---|---|---|---|"]
    for name, v in list(b["field"].items()) + list(b["condition"].items()):
        lines.append(f"| {name} | {pct(v['extract'])} | {pct(v['summarize'])} | {pct(v['rewrite'])} | {v['n']} |")
    lines += ["", "```yaml", result["noise_model"].rstrip(), "```", ""]
    return "\n".join(lines)


# ── main ─────────────────────────────────────────────────────────────────

def result_name(model, effort, hard):
    """ai_noise for the default run; other models, efforts and the hard corpus get their own files."""
    parts = ["ai_noise"] + (["hard"] if hard else []) + ([model] if model != DEFAULT_MODEL else [])
    return "_".join(parts + ([effort] if effort else []))


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--docs", type=int, default=200)
    ap.add_argument("--model", default=DEFAULT_MODEL)
    ap.add_argument("--effort", choices=["low", "medium", "high", "xhigh", "max"], default=None,
                    help="output_config.effort; omitted uses the model's default")
    ap.add_argument("--hard", action="store_true",
                    help="harder corpus: buried facts, a correction, a forecast, a subsidiary, a move, a rename")
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--dry-run", action="store_true", help="print the corpus and request count; no API calls")
    args = ap.parse_args(argv)

    docs = corpus(args.docs, hard=args.hard)
    if args.dry_run:
        for d in docs[:4]:
            print(f"--- {d['id']} ({d['style']}, {d['conditions']})\n{d['text']}\n")
        chars = sum(len(d["text"]) for d in docs)
        print(f"{len(docs)} documents, {5 * len(docs)} requests (3 extractions, 1 summary, 1 rewrite each), "
              f"{chars:,} characters of documents")
        return 0

    if not os.environ.get("ANTHROPIC_API_KEY"):
        fidcal.log("ANTHROPIC_API_KEY is not set (or use --dry-run)")
        return 2
    model = Model(args.model, args.effort, fidcal.default_cache_dir())
    with ThreadPoolExecutor(args.workers) as pool:
        results = dict(zip([d["id"] for d in docs], pool.map(lambda d: run_doc(model, d), docs)))
    rows = outcomes(docs, results)
    refused = len(docs) - len({r["doc"] for r in rows})
    b = breakdown(rows)
    result = {"model": args.model, "effort": args.effort, "hard": args.hard, "documents": len(docs),
              "refused": refused, "breakdown": b, "held_out": held_out(rows), "noise_model": noise_yaml(b),
              "requests": {"api": model.calls, "cached": model.cached}}
    here = os.path.dirname(os.path.abspath(__file__))
    out = os.path.join(here, "results", result_name(args.model, args.effort, args.hard))
    with open(out + ".json", "w") as f:
        json.dump(result, f, indent=2)
    with open(out + ".md", "w") as f:
        f.write(markdown(result))
    print(markdown(result))
    fidcal.log(f"{model.calls} API requests, {model.cached} from cache, {model.refusals} refusals")
    return 0


if __name__ == "__main__":
    sys.exit(main())
