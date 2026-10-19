"""How long does a drug label's warnings text stay unchanged?

Fact: "the warnings of drug label X say Z". Warnings means the label's
Boxed Warning, Warnings, and Warnings and Precautions sections together
(LOINC 34066-1, 34071-1, 43685-7), compared as words: case, punctuation,
bullets and spacing are ignored. Any change of a word counts as a change.

openFDA serves only each label's current version (openfda_labels.py), so
history comes from DailyMed: its SPL history lists every published
version of a label with its date, and every version can be downloaded.

Labels: US human prescription drug labels in openFDA, sampled by set_id
(random UUIDs, so the lowest 1,000 are a random sample; the hash split
then picks from those). For each label we read every published version's
warnings text, then take monthly snapshots from January 2008 to five
years before today and ask whether the text is unchanged 1 month,
3 months, … 5 years later. Snapshots stop five years before the data
ends, so every lag is in the past.

States, by how long the warnings text had been unchanged at the
snapshot (counted from the label's first DailyMed version at most):
  recent   changed in the year before
  settled  unchanged for 1 to 5 years
  old      unchanged for over 5 years

Usage: python3 dailymed_warnings.py [--labels 300]   (~1 MB per version; zips are deleted after reading)
"""

import argparse
import hashlib
import io
import json
import os
import re
import urllib.parse
import zipfile
import xml.etree.ElementTree as ET
from datetime import date, datetime, timedelta

import fidcal

OPENFDA = "https://api.fda.gov/drug/label.json"
HISTORY = "https://dailymed.nlm.nih.gov/dailymed/services/v2/spls/{setid}/history.json"
VERSION = "https://dailymed.nlm.nih.gov/dailymed/getFile.cfm?setid={setid}&type=zip&version={version}"
SECTIONS = {"34066-1": "boxed warning", "34071-1": "warnings", "43685-7": "warnings and precautions"}
HL7 = "{urn:hl7-org:v3}"

END = date(2026, 10, 1)
MAX_LAG_DAYS = 1826
SNAPSHOTS = [date(y, m, 1) for y in range(2008, END.year + 1) for m in range(1, 13)
             if date(y, m, 1) + timedelta(days=MAX_LAG_DAYS) <= END]
LAG_DAYS = [0, 30, 91, 182, 274, 365, 548, 730, 1095, 1461, 1826]
LAGS = [d * fidcal.DAY for d in LAG_DAYS]
RECENT, OLD = 365, 5 * 365


# ── label text ───────────────────────────────────────────────────────────

def normalize(text):
    """Words only: lower case, every run of non-alphanumeric characters as one space."""
    return re.sub(r"[^a-z0-9]+", " ", text.lower()).strip()


def warnings_text(xml_bytes):
    """The normalized text of every warnings section, in document order, or None if there is none."""
    root = ET.fromstring(xml_bytes)
    parts = []
    for section in root.iter(HL7 + "section"):
        code = section.find(HL7 + "code")
        if code is not None and code.get("code") in SECTIONS:
            parts.append(normalize(" ".join(section.itertext())))
    # A section nested in another warnings section is already in its parent's text
    kept = [p for i, p in enumerate(parts) if not any(j != i and p in q for j, q in enumerate(parts) if len(q) > len(p))]
    return " | ".join(kept) if kept else None


def xml_from_zip(data):
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        names = [n for n in z.namelist() if n.lower().endswith(".xml")]
        return z.read(names[0]) if names else None


# ── data ─────────────────────────────────────────────────────────────────

def sample_set_ids(cache, n):
    q = {"search": 'openfda.product_type:"HUMAN PRESCRIPTION DRUG"', "count": "set_id", "limit": 1000}
    path = os.path.join(cache, "dailymed", "rx_set_ids.json")
    fidcal.download(OPENFDA + "?" + urllib.parse.urlencode(q), path, pause=0)
    with open(path) as f:
        ids = [r["term"] for r in json.load(f)["results"]]
    return sorted(ids, key=lambda s: hashlib.sha256(s.encode()).hexdigest())[:n]


def history(cache, setid):
    """[(published date, version)], oldest first."""
    path = os.path.join(cache, "dailymed", setid, "history.json")
    fidcal.download(HISTORY.format(setid=setid), path, pause=0.2, attempts=3)
    with open(path) as f:
        rows = json.load(f)["data"]["history"]
    return sorted((datetime.strptime(r["published_date"], "%b %d, %Y").date(), int(r["spl_version"])) for r in rows)


def version_text(cache, setid, version):
    """Warnings text of one version, cached as JSON; the downloaded zip is deleted."""
    path = os.path.join(cache, "dailymed", setid, f"v{version}.json")
    if os.path.exists(path):
        with open(path) as f:
            text = json.load(f)["warnings"]
        return normalize(text) if text else text
    zpath = path[:-5] + ".zip"
    fidcal.download(VERSION.format(setid=setid, version=version), zpath, pause=0.2, attempts=3)
    with open(zpath, "rb") as f:
        xml = xml_from_zip(f.read())
    text = warnings_text(xml) if xml else None
    with open(path, "w") as f:
        json.dump({"warnings": text}, f)
    os.remove(zpath)
    return text


def timeline(versions):
    """[(date, text)] where the warnings text changed, from [(date, text)] per version, oldest first."""
    out = []
    for at, text in versions:
        if not out or text != out[-1][1]:
            out.append((at, text))
    return out


# ── observations ─────────────────────────────────────────────────────────

def text_at(changes, at):
    """(text, since) in force on `at`, or None before the first version."""
    current = None
    for changed, text in changes:
        if changed > at:
            break
        current = (text, changed)
    return current


def state(age_days):
    if age_days <= RECENT:
        return "recent"
    return "old" if age_days > OLD else "settled"


def observations(labels):
    """(label, lag, warnings unchanged) per snapshot and lag, also split by state at the snapshot."""
    out = {"all": [], "recent": [], "settled": [], "old": []}
    for setid, changes in labels.items():
        for snapshot in SNAPSHOTS:
            now = text_at(changes, snapshot)
            if now is None or now[0] is None:
                continue
            text, since = now
            s = state((snapshot - since).days)
            for lag in LAG_DAYS:
                later = text_at(changes, snapshot + timedelta(days=lag))
                obs = (setid, lag * fidcal.DAY, later[0] == text)
                out["all"].append(obs)
                out[s].append(obs)
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--labels", type=int, default=300, help="how many prescription labels to sample")
    args = ap.parse_args(argv)
    cache = fidcal.default_cache_dir()

    labels, skipped, versions_read = {}, 0, 0
    for i, setid in enumerate(sample_set_ids(cache, args.labels)):
        try:
            hist = history(cache, setid)
            texts = [(at, version_text(cache, setid, v)) for at, v in hist]
        except Exception as e:  # a label withdrawn from DailyMed, or a version that will not download
            fidcal.log(f"  {setid}: {e}")
            skipped += 1
            continue
        versions_read += len(texts)
        if any(t for _, t in texts):
            labels[setid] = timeline(texts)
        if (i + 1) % 25 == 0:
            fidcal.log(f"  {i + 1}/{args.labels} labels, {versions_read:,} versions")
    changes = sum(len(c) - 1 for c in labels.values())
    fidcal.log(f"{len(labels)} labels with warnings, {versions_read:,} versions, {changes:,} warnings changes, "
               f"{skipped} skipped")

    obs = observations(labels)
    names = {"all": "all snapshots", "recent": "warnings changed in the year before",
             "settled": "warnings unchanged for 1 to 5 years", "old": "warnings unchanged for over 5 years"}
    truth = "the label's warnings text (boxed warning, warnings, warnings and precautions) is unchanged"
    results = [fidcal.analyze(f"DailyMed warnings — {names[s]}", "the warnings of drug label X say Z",
                              obs[s], LAGS, truth=truth)
               for s in ("all", "recent", "settled", "old") if obs[s]]
    results[0]["sample"] = {"labels_sampled": args.labels, "labels_with_warnings": len(labels),
                            "labels_skipped": skipped, "versions_read": versions_read,
                            "warnings_changes": changes}
    fidcal.write_results(results, "dailymed_warnings",
                         f"\nSample: {args.labels} prescription labels; {len(labels)} with warnings text, "
                         f"{versions_read:,} versions read, {changes:,} changes to the warnings text, "
                         f"{skipped} could not be read.\n")


if __name__ == "__main__":
    main()
