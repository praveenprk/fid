"""Can openFDA measure how long a drug label's warnings stay unchanged?

The fact would be "the warnings section of drug X's label says Z". To
measure its half-life we need, for each label, its past versions with
their dates: then "still correct" is whether the warnings text at a
later date matches the text at an earlier one.

This script does not fit a half-life. It checks, against the live API,
whether the history needed for one is there:

  1. drug/label: how many labels, how many have a warnings section, how
     many have been revised (version > 1).
  2. Does any set_id (one label across its versions) appear more than
     once? If every set_id appears once, the API serves only the current
     version, and past warnings text cannot be compared.
  3. A sample of revised labels, looked up by set_id one by one: how many
     versions come back for each.
  4. drug/drugsfda: dated labeling supplements per application, the only
     dated history of label changes on api.fda.gov. They do not say which
     section changed.

It writes results/openfda_labels.{json,md}.

Usage: python3 openfda_labels.py [--sample 50]
"""

import argparse
import json
import os
import urllib.parse

import fidcal

LABEL = "https://api.fda.gov/drug/label.json"
DRUGSFDA = "https://api.fda.gov/drug/drugsfda.json"


def url(base, **params):
    return base + "?" + urllib.parse.urlencode(params)


def get(cache, name, link):
    """One API response, cached. A 404 (openFDA's "no matches") reads as no results."""
    path = os.path.join(cache, "openfda", name + ".json")
    try:
        fidcal.download(link, path, pause=0.3, attempts=3)
    except fidcal.urllib.error.HTTPError as e:
        if e.code == 404:
            return {"meta": {"results": {"total": 0}}, "results": []}
        raise
    with open(path) as f:
        return json.load(f)


def total(response):
    return response["meta"]["results"]["total"]


def verdict(max_per_set_id, sample_versions):
    """Is there a version history to measure? Both checks must find more than one version per label."""
    has_history = max_per_set_id > 1 and any(n > 1 for n in sample_versions)
    return {
        "max_records_per_set_id": max_per_set_id,
        "sample_labels": len(sample_versions),
        "sample_labels_with_past_versions": sum(1 for n in sample_versions if n > 1),
        "history_available": has_history,
    }


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--sample", type=int, default=50, help="revised labels to look up by set_id")
    args = ap.parse_args(argv)
    cache = fidcal.default_cache_dir()

    labels = total(get(cache, "labels", url(LABEL, limit=1)))
    with_warnings = total(get(cache, "with_warnings", url(LABEL, search="_exists_:warnings", limit=1)))
    first_version = total(get(cache, "version_1", url(LABEL, search="version:1", limit=1)))
    # count sorts by count, highest first: the top term is the most records any set_id has
    top = get(cache, "count_set_id", url(LABEL, count="set_id"))["results"]
    max_per_set_id = top[0]["count"] if top else 0

    revised = get(cache, "revised_sample",
                  url(LABEL, search="_exists_:warnings AND NOT version:1", limit=args.sample))["results"]
    sample = []
    for r in revised:
        found = get(cache, "set_id_" + r["set_id"], url(LABEL, search=f'set_id:"{r["set_id"]}"', limit=100))
        sample.append({"set_id": r["set_id"], "version": r.get("version"), "effective_time": r.get("effective_time"),
                       "versions_returned": len(found["results"])})

    applications = total(get(cache, "drugsfda", url(DRUGSFDA, limit=1)))
    labeling = total(get(cache, "drugsfda_labeling",
                         url(DRUGSFDA, search='submissions.submission_class_code:"LABELING"', limit=1)))

    v = verdict(max_per_set_id, [s["versions_returned"] for s in sample])
    result = {
        "labels": labels, "labels_with_warnings": with_warnings, "labels_at_version_1": first_version,
        "labels_revised": labels - first_version, **v, "sample": sample,
        "drugsfda_applications": applications, "drugsfda_applications_with_labeling_supplements": labeling,
    }
    here = os.path.dirname(os.path.abspath(__file__))
    with open(os.path.join(here, "results", "openfda_labels.json"), "w") as f:
        json.dump(result, f, indent=1)
        f.write("\n")
    md = markdown(result)
    with open(os.path.join(here, "results", "openfda_labels.md"), "w") as f:
        f.write(md)
    print(md)


def markdown(r):
    yes = "yes" if r["history_available"] else "no"
    return "\n".join([
        "# openFDA drug labels: is there a version history?", "",
        f"- Labels in drug/label: {r['labels']:,}; with a warnings section: {r['labels_with_warnings']:,}",
        f"- Revised at least once (version > 1): {r['labels_revised']:,}; at version 1: {r['labels_at_version_1']:,}",
        f"- Most records for any one set_id: **{r['max_records_per_set_id']}**",
        f"- Revised labels with warnings, looked up by set_id: {r['sample_labels']}; "
        f"with any past version returned: **{r['sample_labels_with_past_versions']}**",
        f"- drug/drugsfda applications: {r['drugsfda_applications']:,}; with a dated labeling supplement: "
        f"{r['drugsfda_applications_with_labeling_supplements']:,} (the section changed is not given)",
        f"- **Version history of label text available: {yes}**", "",
    ])


if __name__ == "__main__":
    main()
