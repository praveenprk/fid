"""How fast does "the CEO of company X is Y" go stale?

Ground truth: Wikidata's CEO statements (P169) with start (P580) and end
(P582) dates. We take yearly snapshots — on 1 Jan 2004, 2005, … 2016 —
of who was CEO, then ask how many of those snapshot facts were still
true 3 months, 1 year, … 10 years later. Snapshots stop at 2016 so the
full 10-year window is in the past for every one of them.

Tests examples/half-lives.yaml's guess `employee_role: 1y`.

Data-quality rules (Wikidata itself goes stale — fittingly):
  * dates must be at least month precision;
  * a CEO with no end date is taken to have ended when a successor
    started, if one is recorded;
  * only companies with two or more dated CEO statements are used, so
    the CEO history is actually being maintained.

Usage: python3 wikidata_ceo.py
"""

import json
import os
import urllib.parse
from collections import defaultdict
from datetime import datetime, timedelta, timezone

import fidcal

QUERY = """
SELECT ?company ?st ?start ?sp ?end ?ep WHERE {
  ?company p:P169 ?st .
  ?st pqv:P580 ?sv .
  ?sv wikibase:timeValue ?start ; wikibase:timePrecision ?sp .
  OPTIONAL {
    ?st pqv:P582 ?ev .
    ?ev wikibase:timeValue ?end ; wikibase:timePrecision ?ep .
  }
}
"""

MONTH_PRECISION = 10  # Wikidata: 9 = year, 10 = month, 11 = day
SNAPSHOT_YEARS = range(2004, 2017)
LAGS = [round(q * fidcal.YEAR / 4) for q in range(0, 41)]  # 0 … 10 years, quarterly


def parse_time(value):
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None  # BCE dates, malformed values


def fetch(cache_dir):
    path = os.path.join(cache_dir, "wikidata", "ceo_statements.json")
    url = "https://query.wikidata.org/sparql?" + urllib.parse.urlencode(
        {"query": QUERY, "format": "json"}
    )
    fidcal.download(url, path)
    with open(path) as f:
        return json.load(f)["results"]["bindings"]


def tenures(rows):
    """Per company: list of (start, end_or_None), cleaned."""
    by_company = defaultdict(dict)
    for row in rows:
        if int(row["sp"]["value"]) < MONTH_PRECISION:
            continue
        start = parse_time(row["start"]["value"])
        if start is None:
            continue
        end = None
        if "end" in row:
            if int(row["ep"]["value"]) < MONTH_PRECISION:
                continue  # ended, but we don't know when precisely enough
            end = parse_time(row["end"]["value"])
            if end is None or end <= start:
                continue
        # A statement can appear twice if it has two end qualifiers; keep the earliest end
        key = row["st"]["value"]
        company = row["company"]["value"]
        prev = by_company[company].get(key)
        if prev is None or (end is not None and (prev[1] is None or end < prev[1])):
            by_company[company][key] = (start, end)

    out = {}
    for company, statements in by_company.items():
        spans = sorted(statements.values(), key=lambda span: span[0])
        if len(spans) < 2:
            continue
        cleaned = []
        for i, (start, end) in enumerate(spans):
            if end is None:
                successors = [s for s, _ in spans[i + 1:] if s > start]
                end = min(successors) if successors else None
            cleaned.append((start, end))
        out[company] = cleaned
    return out


def observations(companies):
    for company, spans in companies.items():
        for year in SNAPSHOT_YEARS:
            snapshot = datetime(year, 1, 1, tzinfo=timezone.utc)
            for start, end in spans:
                if start <= snapshot and (end is None or end > snapshot):
                    for lag in LAGS:
                        at = snapshot + timedelta(seconds=lag)
                        yield (company, lag, end is None or end > at)


def main():
    rows = fetch(fidcal.default_cache_dir())
    fidcal.log(f"{len(rows):,} CEO statements from Wikidata")
    companies = tenures(rows)
    fidcal.log(f"{len(companies):,} companies with a maintained CEO history")

    result = fidcal.analyze(
        "Wikidata CEOs",
        "the CEO of company X is Y",
        observations(companies),
        LAGS,
        guess=fidcal.YEAR,
        guess_label="employee_role: 1y",
        truth="Y is still CEO (no end date yet, and no successor started)",
    )
    fidcal.write_results([result], "wikidata_ceo")


if __name__ == "__main__":
    main()
