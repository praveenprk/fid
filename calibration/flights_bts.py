"""Is a published flight schedule true, and does each leg add noise?

Ground truth: every US domestic flight in 2024 from the Bureau of
Transportation Statistics on-time database (~7 million flights) — the
scheduled arrival, and what actually happened. Months are processed one
at a time, so memory stays flat.

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

YEAR = 2024
MONTHS = range(1, 13)
URL = "https://transtats.bts.gov/PREZIP/On_Time_Reporting_Carrier_On_Time_Performance_1987_present_{year}_{month}.zip"
MONTH_NAMES = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
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


class Rate:
    """On-time count and total."""

    def __init__(self):
        self.on_time = self.n = 0

    def add(self, on_time):
        self.on_time += 1 if on_time else 0
        self.n += 1

    @property
    def p(self):
        return self.on_time / self.n


def rate_table(groups, min_n=2000):
    return [(key, r.p, r.n) for key, r in sorted(groups.items()) if r.n >= min_n]


def main():
    cache = fidcal.default_cache_dir()
    overall = Rate()
    by_airline, by_hour, by_month = defaultdict(Rate), defaultdict(Rate), defaultdict(Rate)
    legs = fidcal.Tally()
    after = {"on_time": Rate(), "late": Rate()}

    for month in MONTHS:
        url = URL.format(year=YEAR, month=month)
        path = fidcal.download(url, os.path.join(cache, "bts", f"ontime_{YEAR}_{month:02d}.zip"))
        flights = load(path)
        fidcal.log(f"{YEAR}-{month:02d}: {len(flights):,} flights")

        for f in flights:
            overall.add(f["on_time"])
            by_airline[f["airline"]].add(f["on_time"])
            by_hour[f"{f['dep'] // 100:02d}:00"].add(f["on_time"])
            by_month[f"{YEAR}-{month:02d}"].add(f["on_time"])

        # Legs of each aircraft's day, in scheduled order. Days never span months.
        days = defaultdict(list)
        for f in flights:
            if f["tail"]:
                days[(f["tail"], f["date"])].append(f)
        for (tail, _date), day in days.items():
            day.sort(key=lambda f: f["dep"])
            for k, f in enumerate(day[:MAX_LEG]):
                legs.add(tail, k, f["on_time"])
                if k > 0:
                    after["on_time" if day[k - 1]["on_time"] else "late"].add(f["on_time"])

    chain = fidcal.analyze(
        "Flight legs", "aircraft's k-th leg of the day arrives on schedule",
        legs, list(range(MAX_LEG)),
        truth="arrived within 15 min, not cancelled or diverted; lag = legs flown before it",
        fmt=fmt_legs,
    )
    h = chain["fitted"]["half_life_s"]
    per_leg_noise = 1 - 0.5 ** (1 / h) if h and not math.isinf(h) else 0.0

    p_after_on = after["on_time"].p
    p_after_late = after["late"].p
    p_any = (after["on_time"].on_time + after["late"].on_time) / (after["on_time"].n + after["late"].n)

    lines = [
        "### Flight schedules: source fidelity",
        "",
        f"- {overall.n:,} flights, {YEAR}",
        f"- The schedule is true (on time, ±15 min) for **{overall.p:.1%}** of flights",
        "",
        "| Month | On time | n |", "|---|---|---|",
        *[f"| {k} | {p:.3f} | {n:,} |" for k, p, n in rate_table(by_month)],
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
        f"- On time after an on-time previous leg: **{p_after_on:.3f}** (n = {after['on_time'].n:,})",
        f"- On time after a late previous leg: **{p_after_late:.3f}** (n = {after['late'].n:,})",
        f"- On time, any leg after the first: **{p_any:.3f}**",
        f"- Conditional noise after a late leg: 1 − {p_after_late:.3f} / {p_any:.3f} = **{1 - p_after_late / p_any:.2f}**",
        "- If legs were independent, as multiplying per-step noise assumes, these two would match.",
        "",
    ]
    extra = "\n".join(lines)
    print(extra)

    chain["source_fidelity"] = {
        "overall": overall.p,
        "flights": overall.n,
        "year": YEAR,
        "by_airline": [{"airline": k, "on_time": p, "n": n} for k, p, n in rate_table(by_airline)],
        "by_hour": [{"hour": k, "on_time": p, "n": n} for k, p, n in rate_table(by_hour)],
        "by_month": [{"month": k, "on_time": p, "n": n} for k, p, n in rate_table(by_month)],
    }
    chain["after_previous_leg"] = {
        "per_leg_noise": per_leg_noise,
        "on_time": {"on_time": p_after_on, "n": after["on_time"].n},
        "late": {"on_time": p_after_late, "n": after["late"].n},
        "any_after_first": p_any,
        "noise_given_late": 1 - p_after_late / p_any,
    }

    here = os.path.dirname(os.path.abspath(__file__))
    fidcal.write_results([chain], "flights_bts")
    with open(os.path.join(here, "results", "flights_bts.md"), "a") as f:
        f.write("\n" + extra)


if __name__ == "__main__":
    main()
