"""Build the public results site from calibration/results.

Writes site/_site/:
  index.html   — shell.html with page.html inside it
  data/*.json  — the datasets page.html reads

page.html is the same page published on claude.ai; shell.html supplies
what that host provides there (the `dash` data API, theme tokens, d3),
plus a live panel that needs network access.

Usage: python3 site/build.py
"""

import json
import os
import shutil
import sys
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
CAL = os.path.join(os.path.dirname(HERE), "calibration")
OUT = os.path.join(HERE, "_site")

sys.path.insert(0, CAL)
import fidcal  # noqa: E402

# Facts shown on the page: id, experiment name in results, label, data source
FACTS = [
    ("ceo", "Wikidata CEOs", "Company CEO", "Wikidata"),
    ("ifr", "METAR adverse conditions", "Airport is IFR or worse", "METAR, 10 US airports"),
    ("flight_category", "METAR flight category", "Airport flight category", "METAR, 10 US airports"),
    ("temperature", "METAR temperature", "Airport temperature ±2 °F", "METAR, 10 US airports"),
    ("ship_underway", "AIS 1 km, underway", "Ship position, underway", "NOAA AIS"),
    ("ship_moored", "AIS 1 km, moored or at anchor", "Ship position, moored", "NOAA AIS"),
]


def load_results():
    results = {}
    for slug in ("wikidata_ceo", "weather_metar", "flights_bts", "ships_ais"):
        with open(os.path.join(CAL, "results", f"{slug}.json")) as f:
            for r in json.load(f):
                results[r["experiment"]] = r
    return results


def model(lag, fit, baseline=0.0):
    h = fit["half_life_s"]
    if h is None:
        return fit["s0"]
    return baseline + (fit["s0"] - baseline) * 0.5 ** (lag / h)


def datasets(results):
    findings, curves = [], []
    for fid, exp, label, source in FACTS:
        r = results[exp]
        zero, base = r["fitted"], r["fitted_with_baseline"]
        emp = r["empirical_half_life_s"]
        findings.append({
            "id": fid,
            "fact": label,
            "source": source,
            "observations": r["observations"],
            "guess": r["guess"]["label"],
            "guess_s": r["guess"]["half_life_s"],
            "measured_s": round(emp) if emp else None,
            "window_s": max(c["lag_s"] for c in r["curve"]),
            "s0": round(base["s0"], 4),
            "baseline": round(base["baseline"], 2),
            "half_life_baseline_s": round(base["half_life_s"]) if base["half_life_s"] else None,
            "shape_error_zero": round(r["shape_error"], 3),
            "shape_error_baseline": round(r["shape_error_with_baseline"], 3),
            "brier_guess": round(r["brier_test"]["guessed_half_life"], 4),
            "brier_zero": round(r["brier_test"]["fitted_half_life"], 4),
            "brier_baseline": round(r["brier_test"]["fitted_half_life_with_baseline"], 4),
            "brier_constant": round(r["brier_test"]["constant_base_rate"], 4),
        })
        for c in r["curve"]:
            curves.append({
                "series": fid,
                "lag_s": c["lag_s"],
                "lag": c["lag"],
                "still_correct": round(c["still_correct"], 4),
                "n": c["n"],
                "model_zero": round(model(c["lag_s"], zero), 4),
                "model_baseline": round(model(c["lag_s"], base, base["baseline"]), 4),
            })

    flights = results["Flight legs"]
    sf, after = flights["source_fidelity"], flights["after_previous_leg"]
    rate = lambda rows, key: [{key: x[key], "on_time": round(x["on_time"], 4), "n": x["n"]} for x in rows]
    n_on, n_late = after["on_time"]["n"], after["late"]["n"]
    legs = [
        {"condition": "After an on-time leg", "on_time": round(after["on_time"]["on_time"], 4), "n": n_on},
        {"condition": "After a late leg", "on_time": round(after["late"]["on_time"], 4), "n": n_late},
        {"condition": "Any leg after the first", "on_time": round(after["any_after_first"], 4), "n": n_on + n_late},
    ]
    return {
        "findings": findings,
        "curves": curves,
        "flights_hour": rate(sf["by_hour"], "hour"),
        "flights_airline": rate(sf["by_airline"], "airline"),
        "flights_month": rate(sf["by_month"], "month"),
        "flights_legs": legs,
    }


def main():
    data = datasets(load_results())
    if os.path.exists(OUT):
        shutil.rmtree(OUT)
    os.makedirs(os.path.join(OUT, "data"))
    for name, rows in data.items():
        with open(os.path.join(OUT, "data", f"{name}.json"), "w") as f:
            json.dump(rows, f, indent=1)
            f.write("\n")
    meta = {"built_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")}
    with open(os.path.join(OUT, "data", "meta.json"), "w") as f:
        json.dump(meta, f)

    with open(os.path.join(HERE, "shell.html")) as f:
        shell = f.read()
    with open(os.path.join(HERE, "page.html")) as f:
        page = f.read()
    assert "{{PAGE}}" in shell
    with open(os.path.join(OUT, "index.html"), "w") as f:
        f.write(shell.replace("{{PAGE}}", page))
    with open(os.path.join(OUT, ".nojekyll"), "w") as f:
        pass
    fidcal.log(f"built {OUT}: " + ", ".join(f"{k} {len(v)}" for k, v in data.items()))


if __name__ == "__main__":
    main()
