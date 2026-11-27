"""Run the calibration studies with entity-bootstrap ranges and check their inputs.

Usage: python3 robustness.py [--resamples 200] [--seed 0]
       [--only wikidata_ceo,weather_metar]
"""

import argparse
import hashlib
import json
import math
import os
import subprocess
import sys

import fidcal

HERE = os.path.dirname(os.path.abspath(__file__))
EXPERIMENTS = [
    "wikidata_ceo",
    "weather_metar",
    "ships_ais",
    "flights_bts",
    "pypi_latest",
    "dailymed_warnings",
    "eia_demand",
]


def compare_results(committed, generated):
    """Compare result trees, ignoring intervals and tolerating numeric noise."""
    differing_paths = []

    def record(path):
        if len(differing_paths) < 10:
            differing_paths.append(path or "$")

    def compare(left, right, path=""):
        if len(differing_paths) >= 10:
            return
        if isinstance(left, dict) and isinstance(right, dict):
            keys = list(left) + sorted(
                (key for key in right if key not in left), key=str
            )
            for key in keys:
                if key == "intervals":
                    continue
                child = f"{path}.{key}" if path else str(key)
                if key not in left or key not in right:
                    record(child)
                else:
                    compare(left[key], right[key], child)
                if len(differing_paths) >= 10:
                    break
            return
        if isinstance(left, list) and isinstance(right, list):
            if len(left) != len(right):
                record(path)
                return
            for i, (left_item, right_item) in enumerate(zip(left, right)):
                compare(left_item, right_item, f"{path}[{i}]")
                if len(differing_paths) >= 10:
                    break
            return
        if (
            isinstance(left, (int, float))
            and not isinstance(left, bool)
            and isinstance(right, (int, float))
            and not isinstance(right, bool)
        ):
            if not math.isclose(left, right, rel_tol=1e-12, abs_tol=1e-12):
                record(path)
            return
        if left != right or type(left) is not type(right):
            record(path)

    compare(committed, generated)
    result = {"matches_committed": not differing_paths}
    if differing_paths:
        result["differing_paths"] = differing_paths
    return result


def input_manifest(cache_dir):
    """The path, SHA-256 and byte size of every cached input file."""
    files = []
    for directory, dirnames, filenames in os.walk(cache_dir):
        dirnames.sort()
        for filename in filenames:
            path = os.path.join(directory, filename)
            digest = hashlib.sha256()
            with open(path, "rb") as f:
                for chunk in iter(lambda: f.read(1 << 20), b""):
                    digest.update(chunk)
            relative = os.path.relpath(path, HERE).replace(os.sep, "/")
            files.append(
                {
                    "path": relative,
                    "sha256": digest.hexdigest(),
                    "size": os.path.getsize(path),
                }
            )
    return sorted(files, key=lambda item: item["path"])


def write_readme(rows, resamples, seed):
    lines = [
        "# Bootstrap 95% ranges over entities",
        "",
        "| Experiment | Resamples | Seed | Matches committed | Results |",
        "|---|---:|---:|---|---|",
    ]
    for row in rows:
        if row.get("not_run"):
            matches = f"not run: {row['not_run']}"
            link = "—"
            n = "—"
            row_seed = "—"
        else:
            matches = "yes" if row["matches_committed"] else "no"
            link = f"[{row['slug']}.md]({row['slug']}.md)"
            n = str(resamples)
            row_seed = str(seed)
        lines.append(f"| {row['slug']} | {n} | {row_seed} | {matches} | {link} |")
    differences = [row for row in rows if row.get("differing_paths")]
    if differences:
        lines += ["", "## First differing paths", ""]
        for row in differences:
            paths = ", ".join(f"`{path}`" for path in row["differing_paths"])
            lines.append(f"- `{row['slug']}`: {paths}")
    lines += [
        "",
        "Cite a range only where its experiment matches the committed results; "
        "otherwise the data has changed since the committed run.",
        "",
    ]
    path = os.path.join(HERE, "results", "robustness", "README.md")
    with open(path, "w") as f:
        f.write("\n".join(lines))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--resamples", type=int, default=200)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--only", default=None)
    args = parser.parse_args(argv)
    only = set(args.only.split(",")) if args.only is not None else None
    selected = [slug for slug in EXPERIMENTS if only is None or slug in only]

    out_dir = os.path.join(HERE, "results", "robustness")
    os.makedirs(out_dir, exist_ok=True)
    env = os.environ.copy()
    env["FID_CAL_RESAMPLES"] = str(args.resamples)
    env["FID_CAL_SEED"] = str(args.seed)
    env["FID_CAL_RESULTS_DIR"] = out_dir

    rows = []
    for slug in selected:
        if slug == "eia_demand" and not env.get("FID_EIA_API_KEY"):
            rows.append({"slug": slug, "not_run": "no FID_EIA_API_KEY"})
            continue
        completed = subprocess.run(
            [sys.executable, f"{slug}.py"], cwd=HERE, env=env, check=False
        )
        if completed.returncode:
            return completed.returncode
        output_path = os.path.join(out_dir, f"{slug}.json")
        committed_path = os.path.join(HERE, "results", f"{slug}.json")
        with open(output_path) as f:
            generated = json.load(f)
        with open(committed_path) as f:
            committed = json.load(f)
        comparison = compare_results(committed, generated)
        rows.append({"slug": slug, **comparison})

    manifest = input_manifest(fidcal.default_cache_dir())
    with open(os.path.join(out_dir, "inputs.json"), "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")
    write_readme(rows, args.resamples, args.seed)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
