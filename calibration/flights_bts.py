"""Is a published flight schedule true, and does each leg add noise?

Ground truth: every US domestic flight in January 2024 from the Bureau
of Transportation Statistics on-time database (~550,000 flights) — the
scheduled arrival, and what actually happened.

Fact: "flight F arrives at its scheduled time". Correct if it arrived
within 15 minutes of schedule (the DOT definition of on-time), and was
neither cancelled nor diverted.

Three tests:
  1. Source fidelity — how often is the schedule true at all? fid gives
     each source class one number; is one number enough, or does it vary
     by airline and time of day?
  2. Transform noise — an aircraft flies several legs a day, each
     inheriting the last one's delay. fid models each step as
     ×(1 − noise). Treat leg k as k−1 transforms of the first leg's
     schedule and fit that model.
  3. Independence — fid multiplies per-step noise, which assumes steps
     are independent. Compare on-time rates after an on-time leg and
     after a late leg.

Usage: python3 flights_bts.py
"""

import csv
import io
import math
import os
import zipfile
from collections import defaultdict

import fidcal

URL = "https://transtats.bts.gov/PREZIP/On_Time_Reporting_Carrier_On_Time_Performance_1987_present_2024_1.zip"
MAX_LEG = 8


def fmt_legs(k):
    if k is None:
        return "—"
    if math.isinf(k):
        return "infinite"
    return f"{k:.1f} legs" if k != int(k) else f"{int(k)} legs"


def load(path):
    flights = []
    with zipfile.ZipFile(path) as z:
        name = next(n for n in z.namelist() if n.endswith(".csv"))
        with z.open(name) as raw:
            for row in csv.DictReader(io.TextIOWrapper(raw, encoding="latin-1", newline="")):
                cancelled = row["Cancelled"] == "1.00"
                diverted = row["Diverted"] == "1.00"
                late = row["ArrDel15"] == "1.00"
                if not (cancelled or diverted) and row["ArrDel15"] == "":
                    continue  # no outcome recorded
                flights.append({
                    "date": row["FlightDate"],
                    "airline": row["Reporting_Airline"],
                    "tail": row["Tail_Number"],
                    "dep": int(row["CRSDepTime"]),
                    "on_time": not (cancelled or diverted or late),
                })
    return flights


def rate_table(groups, min_n=2000):
    rows = []
    for key, outcomes in sorted(groups.items()):
        if len(outcomes) >= min_n:
            rows.append((key, sum(outcomes) / len(outcomes), len(outcomes)))
    return rows


def main():
    path = fidcal.download(URL, os.path.join(fidcal.default_cache_dir(), "bts", "ontime_2024_01.zip"))
    flights = load(path)
    fidcal.log(f"{len(flights):,} flights")

    overall = sum(f["on_time"] for f in flights) / len(flights)
    by_airline = defaultdict(list)
    by_hour = defaultdict(list)
    for f in flights:
        by_airline[f["airline"]].append(f["on_time"])
        by_hour[f"{f['dep'] // 100:02d}:00"].append(f["on_time"])

    # Legs of each aircraft's day, in scheduled order
    days = defaultdict(list)
    for f in flights:
        if f["tail"]:
            days[(f["tail"], f["date"])].append(f)
    legs = []
    after = {"on_time": [], "late": []}
    for (tail, _date), day in days.items():
        day.sort(key=lambda f: f["dep"])
        for k, f in enumerate(day[:MAX_LEG]):
            legs.append((tail, k, f["on_time"]))
            if k > 0:
                after["on_time" if day[k - 1]["on_time"] else "late"].append(f["on_time"])

    chain = fidcal.analyze(
        "Flight legs", "aircraft's k-th leg of the day arrives on schedule",
        legs, list(range(MAX_LEG)),
        truth="arrived within 15 min, not cancelled or diverted; lag = legs flown before it",
        fmt=fmt_legs,
    )
    h = chain["fitted"]["half_life_s"]
    per_leg_noise = 1 - 0.5 ** (1 / h) if h and not math.isinf(h) else 0.0

    p_after_on = sum(after["on_time"]) / len(after["on_time"])
    p_after_late = sum(after["late"]) / len(after["late"])

    lines = [
        "### Flight schedules: source fidelity",
        "",
        f"- {len(flights):,} flights, January 2024",
        f"- The schedule is true (on time, ±15 min) for **{overall:.1%}** of flights",
        "",
        "| Airline | On time | n |", "|---|---|---|",
        *[f"| {k} | {p:.3f} | {n:,} |" for k, p, n in rate_table(by_airline)],
        "",
        "| Scheduled departure | On time | n |", "|---|---|---|",
        *[f"| {k} | {p:.3f} | {n:,} |" for k, p, n in rate_table(by_hour)],
        "",
        "### Flight legs: independence of transform noise",
        "",
        f"- Fitted per-leg noise (fid model): **{per_leg_noise:.3f}**",
        f"- On time after an on-time previous leg: **{p_after_on:.3f}** (n = {len(after['on_time']):,})",
        f"- On time after a late previous leg: **{p_after_late:.3f}** (n = {len(after['late']):,})",
        "- If legs were independent, as multiplying per-step noise assumes, these two would match.",
        "",
    ]
    extra = "\n".join(lines)
    print(extra)

    chain["source_fidelity"] = {
        "overall": overall,
        "flights": len(flights),
        "by_airline": [{"airline": k, "on_time": p, "n": n} for k, p, n in rate_table(by_airline)],
        "by_hour": [{"hour": k, "on_time": p, "n": n} for k, p, n in rate_table(by_hour)],
    }
    chain["after_previous_leg"] = {
        "per_leg_noise": per_leg_noise,
        "on_time": {"on_time": p_after_on, "n": len(after["on_time"])},
        "late": {"on_time": p_after_late, "n": len(after["late"])},
    }

    here = os.path.dirname(os.path.abspath(__file__))
    fidcal.write_results([chain], "flights_bts")
    with open(os.path.join(here, "results", "flights_bts.md"), "a") as f:
        f.write("\n" + extra)


if __name__ == "__main__":
    main()
