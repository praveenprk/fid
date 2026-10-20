"""How fast does "the latest version of package X is Y" go stale?

This is the stale knowledge AI coding tools act on: a model, a cache or a
lock file says "the latest requests is 2.31", and later it isn't.

Ground truth: PyPI's JSON API, which lists every release of a package
with the upload time of each file. Packages: the most downloaded on PyPI
over the last 30 days (hugovk/top-pypi-packages), since those are the
ones code and coding tools actually name.

"Latest" means what `pip install X` picks: the highest final release
(no alphas, betas, release candidates or dev builds) uploaded so far,
ignoring yanked releases. A backport (1.9.4 released after 2.0) does not
change it. We take monthly snapshots, 1 Jan 2015 to 1 Oct 2024, of each
package's latest version, then ask whether it is still the latest 1 day,
1 week, … 2 years later. Snapshots stop two years before the data ends,
so every lag is in the past for every snapshot.

States, because ships showed that state matters more than type:
  just_released  the latest version came out in the 30 days before the snapshot
  quiet          the latest version is 30 days to 1 year old
  dormant        no new latest version for over a year

Caveat: PyPI shows releases as they are now, so a release deleted since
it was uploaded is invisible (yanked ones are listed and are left out).

Usage: python3 pypi_latest.py [--packages 1000]
"""

import argparse
import json
import os
import re
from datetime import datetime, timedelta, timezone

import fidcal

TOP_URL = "https://raw.githubusercontent.com/hugovk/top-pypi-packages/main/top-pypi-packages.min.json"
PYPI_URL = "https://pypi.org/pypi/{name}/json"

SNAPSHOTS = [datetime(y, m, 1, tzinfo=timezone.utc) for y in range(2015, 2025) for m in range(1, 13)
             if (y, m) <= (2024, 10)]
LAGS = [round(d * fidcal.DAY) for d in (0, 1, 3, 7, 14, 30, 61, 91, 182, 274, 365, 548, 730)]
JUST_RELEASED, DORMANT = 30 * fidcal.DAY, fidcal.YEAR

# Final releases only: 1.2, 2.0.1, 1!2.0, 3.1.post1 (a post release replaces its base)
FINAL = re.compile(r"^(?:(\d+)!)?(\d+(?:\.\d+)*)(?:[.-]?post(\d+))?$", re.I)


def version_key(version):
    """Sort key for a final release, or None for pre-releases, dev builds and non-PEP 440 versions."""
    m = FINAL.match(version.strip().lstrip("vV"))
    if not m:
        return None
    epoch, release, post = m.groups()
    parts = [int(p) for p in release.split(".")]
    while len(parts) > 1 and parts[-1] == 0:
        parts.pop()  # 1.0 == 1.0.0
    return (int(epoch or 0), tuple(parts), int(post) if post is not None else -1)


def parse_time(value):
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def releases(data):
    """[(first upload time, version key, version)] for final, unyanked releases, by upload time."""
    out = []
    for version, files in data.get("releases", {}).items():
        key = version_key(version)
        live = [f for f in files if not f.get("yanked")]
        if key is None or not live:
            continue
        out.append((min(parse_time(f["upload_time_iso_8601"]) for f in live), key, version))
    return sorted(out)


def timeline(rels):
    """When the latest version changed: [(time, version)], each entry a new highest release."""
    out, best = [], None
    for at, key, version in rels:
        if best is None or key > best:
            best = key
            out.append((at, version))
    return out


def latest_at(changes, at):
    """(version, since) that pip would pick at `at`, or None before the first release."""
    current = None
    for changed, version in changes:
        if changed > at:
            break
        current = (version, changed)
    return current


def state(age):
    if age <= JUST_RELEASED:
        return "just_released"
    return "dormant" if age > DORMANT else "quiet"


def observations(packages):
    """(package, lag, still latest) per snapshot and lag, split by the package's state at the snapshot."""
    out = {"all": [], "just_released": [], "quiet": [], "dormant": []}
    for name, changes in packages.items():
        for snapshot in SNAPSHOTS:
            now = latest_at(changes, snapshot)
            if now is None:
                continue
            version, since = now
            s = state((snapshot - since).total_seconds())
            for lag in LAGS:
                later = latest_at(changes, snapshot + timedelta(seconds=lag))
                obs = (name, lag, later[0] == version)
                out["all"].append(obs)
                out[s].append(obs)
    return out


def fetch(cache_dir, n, start=0):
    """The `n` most downloaded packages after the first `start`: {name: timeline}, and how many failed."""
    top_path = os.path.join(cache_dir, "pypi", "top-pypi-packages.json")
    fidcal.download(TOP_URL, top_path, pause=0)
    with open(top_path) as f:
        names = [row["project"] for row in json.load(f)["rows"][start:start + n]]
    packages, skipped = {}, 0
    for i, name in enumerate(names):
        path = os.path.join(cache_dir, "pypi", "json", name + ".json")
        try:
            fidcal.download(PYPI_URL.format(name=name), path, pause=0.05, attempts=3)
        except Exception as e:  # a removed or renamed project
            fidcal.log(f"  {name}: {e}")
            skipped += 1
            continue
        with open(path) as f:
            changes = timeline(releases(json.load(f)))
        if changes:
            packages[name] = changes
        if (i + 1) % 100 == 0:
            fidcal.log(f"  {i + 1}/{len(names)} packages")
    return packages, skipped


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--packages", type=int, default=1000, help="how many of the most downloaded packages")
    args = ap.parse_args(argv)

    packages, skipped = fetch(fidcal.default_cache_dir(), args.packages)
    fidcal.log(f"{len(packages):,} packages with final releases ({skipped} could not be fetched)")
    obs = observations(packages)
    truth = "Y is still the highest final, unyanked release on PyPI"
    labels = {"all": "all snapshots", "just_released": "latest released in the 30 days before",
              "quiet": "latest is 30 days to 1 year old", "dormant": "no new latest version for over a year"}
    results = [fidcal.analyze(f"PyPI latest version — {labels[s]}", "the latest version of package X is Y",
                              obs[s], LAGS, truth=truth)
               for s in ("all", "just_released", "quiet", "dormant")]
    fidcal.write_results(results, "pypi_latest")


if __name__ == "__main__":
    main()
