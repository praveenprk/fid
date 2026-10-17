"""Generate the fid conformance vectors from the written spec.

This is a second, independent implementation of the scoring rules in
conformance/README.md, in plain Python (stdlib only). It does not call
fid. The Rust and Python test suites check fid against its output, so
fid and this file have to agree.

    python3 conformance/generate.py      # rewrites vectors.json and ledger-example.csv
"""

import hashlib
import json
import math
import os
from datetime import datetime, timedelta, timezone

HERE = os.path.dirname(os.path.abspath(__file__))

UNITS = {"s": 1.0, "m": 60.0, "h": 3600.0, "d": 86400.0, "mo": 30.44 * 86400.0, "y": 365.25 * 86400.0}

SOURCE_CLASSES = {
    "instrument": 0.97,
    "human_observation": 0.87,
    "human_attestation": 0.80,
    "database": 0.90,
    "llm_grounded": 0.55,
    "llm_ungrounded": 0.25,
    "hearsay": 0.17,
    "unknown": None,
}


# ── the spec ─────────────────────────────────────────────────────────────

def parse_duration(text):
    t = text.strip().lower()
    for suffix in ("mo", "s", "m", "h", "d", "y"):
        if t.endswith(suffix):
            number, unit = t[: -len(suffix)], UNITS[suffix]
            break
    else:
        number, unit = t, 1.0
    # Python's float() also takes surrounding spaces and "1_000"; the spec does not
    if number != number.strip() or "_" in number:
        raise ValueError(text)
    n = float(number)  # ValueError: not a number
    if not math.isfinite(n) or n < 0:
        raise ValueError(text)
    return n * unit


def parse_half_life(text):
    if text.strip().lower() in ("infinite", "inf"):
        return None
    seconds = parse_duration(text)
    if seconds <= 0:
        raise ValueError(text)
    return seconds


def fidelity(source, age, half_life, noise, baseline):
    f = source
    for n in noise:
        f *= 1 - min(max(n, 0.0), 1.0)
    temporal = 1.0 if half_life is None else 0.5 ** (max(age, 0.0) / half_life)
    b = min(max(baseline, 0.0), max(f, 0.0))
    return b + (f - b) * temporal


def time_to_floor(f, half_life, baseline, floor):
    """Seconds, or None for never."""
    if f < floor:
        return 0.0
    b = min(max(baseline, 0.0), max(f, 0.0))
    if half_life is None or floor <= b:
        return None
    return half_life * math.log2((f - b) / (floor - b))


def canonical_number(x):
    """Shortest round-trip digits (as repr), exponent unpadded, decimal down to 1e-5."""
    text = repr(x)
    if "e" not in text:
        return text
    mantissa, exp = text.split("e")
    exp = int(exp)
    if exp == -5:
        sign, digits = ("-", mantissa[1:]) if mantissa.startswith("-") else ("", mantissa)
        return sign + "0.0000" + digits.replace(".", "")
    return f"{mantissa}e{'+' if exp > 0 else '-'}{abs(exp)}"


def canonical(value):
    """The JSON text fid hashes: keys sorted, no whitespace, UTF-8 unescaped."""
    if isinstance(value, dict):
        return "{" + ",".join(json.dumps(k, ensure_ascii=False) + ":" + canonical(value[k]) for k in sorted(value)) + "}"
    if isinstance(value, list):
        return "[" + ",".join(canonical(v) for v in value) + "]"
    if isinstance(value, float):
        return canonical_number(value)
    return json.dumps(value, ensure_ascii=False)


def integrity(content):
    return "sha256:" + hashlib.sha256(canonical(content).encode("utf-8")).hexdigest()


def rfc3339(t):
    return t.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.") + f"{t.microsecond // 1000:03d}Z"


# ── cases ────────────────────────────────────────────────────────────────

DAY = 86400.0
T0 = datetime(2026, 1, 1, tzinfo=timezone.utc)


def duration_cases():
    good = ["30s", "5m", "6h", "7d", "3mo", "3y", "1.5d", "90", "0d", " 2H ", "0.25y"]
    bad = ["7days", "30ms", "", "abc", "-1d", "inf", "nan", "infd", "1e400d"]
    out = [{"input": s, "seconds": parse_duration(s)} for s in good]
    out += [{"input": s, "error": True} for s in bad]
    return out


def half_life_cases():
    good = ["30s", "6h", "7d", "3mo", "5.8y", "infinite", "INF", "90"]
    bad = ["0d", "0", "-5h", "7days", "nan", ""]
    out = [{"input": s, "seconds": parse_half_life(s)} for s in good]
    out += [{"input": s, "error": True} for s in bad]
    return out


def source_cases():
    out = [{"class": c, "fidelity": f} for c, f in SOURCE_CLASSES.items()]
    out += [{"class": "human-observation", "fidelity": 0.87}, {"class": " Database ", "fidelity": 0.90}]
    out += [{"class": "human", "error": True}, {"class": "", "error": True}]
    return out


FIDELITY = [
    # name, source, age, half_life, noise, baseline
    ("fresh", 0.97, 0.0, DAY, [], 0.0),
    ("one half-life", 0.90, DAY, DAY, [], 0.0),
    ("two half-lives", 0.90, 2 * DAY, DAY, [], 0.0),
    ("noise chain", 0.90, 0.0, None, [0.05, 0.08, 0.10], 0.0),
    ("noise and decay", 0.90, DAY, DAY, [0.10], 0.0),
    ("baseline", 0.90, DAY, DAY, [], 0.70),
    ("baseline is the long-run limit", 0.97, 1e12, 60.0, [], 0.73),
    ("baseline above fidelity is clamped", 0.50, DAY, DAY, [0.50], 0.70),
    ("negative baseline is clamped to 0", 0.90, DAY, DAY, [], -0.2),
    ("negative age does not raise fidelity", 0.90, -DAY, DAY, [], 0.0),
    ("infinite half-life", 0.90, 1e9, None, [], 0.5),
    ("noise above 1 is clamped", 0.90, 0.0, None, [1.5], 0.0),
    ("negative noise is clamped", 0.90, 0.0, None, [-0.3], 0.0),
    ("ceo, 5.8y half-life, 2y old", 0.90, 2 * 365.25 * DAY, 5.8 * 365.25 * DAY, [], 0.0),
    ("vessel underway, 2.3m", 0.97, 600.0, 2.3 * 60, [], 0.73),
    ("fractional exponent", 0.80, 12345.0, 7000.0, [0.02, 0.03], 0.15),
]

TTL = [
    # name, fidelity, half_life, baseline, floor
    ("one half-life", 0.80, 100.0, 0.0, 0.40),
    ("with baseline", 0.97, 7 * DAY, 0.20, 0.70),
    ("already below", 0.50, 3600.0, 0.0, 0.70),
    ("exactly at floor", 0.70, 3600.0, 0.0, 0.70),
    ("no decay", 0.97, None, 0.0, 0.70),
    ("baseline above floor", 0.97, 3600.0, 0.73, 0.70),
    ("baseline equal to floor", 0.97, 3600.0, 0.70, 0.70),
    ("ceo 5.8y to 0.5", 0.90, 5.8 * 365.25 * DAY, 0.0, 0.50),
]


def envelope(content, source_class, half_life, baseline=None, transforms=(), measured=T0, version="0.2", **extra):
    hl = None if half_life is None else parse_half_life(half_life)
    source = SOURCE_CLASSES[source_class]
    noise = [t[1] for t in transforms]
    steps = []
    for i, (step, n, *condition) in enumerate(transforms):
        at = (measured + timedelta(seconds=i)).strftime("%Y-%m-%dT%H:%M:%SZ")
        steps.append({"step": step, "noise": n, "at": at, **({"condition": condition[0]} if condition else {})})
    env = {
        "fid_version": version,
        "source_class": source_class,
        "source_id": "conformance",
        "measured_at": measured.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "half_life": half_life,
        "transforms": steps,
        "fid_at_stamp": None if source is None else fidelity(source, 0.0, hl, noise, baseline or 0.0),
        "derived_from": None,
        "integrity": integrity(content),
    }
    if baseline is not None:
        env["baseline"] = baseline
    env.update(extra)
    return {"fid": env, "content": content}


def envelope_case(name, stamped, now, floor, tamper=None):
    env = stamped["fid"]
    if tamper is not None:
        stamped = {"fid": env, "content": tamper}
    hl = None if env["half_life"] is None else parse_half_life(env["half_life"])
    measured = datetime.fromisoformat(env["measured_at"].replace("Z", "+00:00"))
    age = (now - measured).total_seconds()
    noise = [t["noise"] for t in env["transforms"]]
    baseline = env.get("baseline") or 0.0
    source = SOURCE_CLASSES[env["source_class"]]
    ok = integrity(stamped["content"]) == env["integrity"]

    f = None if source is None else fidelity(source, age, hl, noise, baseline)
    if not ok or f is None:
        status = "UNKNOWN"
    else:
        status = "SIGNAL" if f >= floor else "NOISE"

    if source is None:
        expires = "unknown"
    else:
        at_measurement = fidelity(source, 0.0, None, noise, 0.0)
        ttl = time_to_floor(at_measurement, hl, baseline, floor)
        if ttl is None:
            expires = "never"
        else:
            ms = math.floor(ttl * 1000 + 0.5)
            expires = rfc3339(measured + timedelta(milliseconds=ms))

    return {
        "name": name,
        "stamped": stamped,
        "now": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "floor": floor,
        "status": status,
        "fidelity": f,
        "integrity_ok": ok,
        "expires_at": expires,
    }


def envelope_cases():
    reading = {"reading": 42}
    ceo = {"company": "Q12345", "ceo": "Ada"}
    return [
        envelope_case("instrument, fresh", envelope(reading, "instrument", "7d"), T0, 0.70),
        envelope_case("instrument, 7 days", envelope(reading, "instrument", "7d"), T0 + timedelta(days=7), 0.70),
        envelope_case("database ceo, 3 years", envelope(ceo, "database", "5.8y"), T0 + timedelta(days=3 * 365), 0.50),
        envelope_case(
            "baseline keeps it above the floor",
            envelope({"mmsi": 1, "state": "underway"}, "instrument", "2.3m", baseline=0.73,
                     info_type="vessel_position", state="underway"),
            T0 + timedelta(hours=6), 0.70,
        ),
        envelope_case(
            "transform chain",
            envelope(reading, "human_observation", "30d", transforms=[("normalize", 0.05), ("aggregate", 0.07)]),
            T0 + timedelta(days=18), 0.50,
        ),
        envelope_case(
            "conditioned transform",
            envelope({"flight": "AA100"}, "database", "1d", transforms=[("next_leg", 0.30, "previous_leg_late")]),
            T0 + timedelta(hours=2), 0.60,
        ),
        envelope_case("no half-life never decays", envelope(reading, "database", None), T0 + timedelta(days=3650), 0.85),
        envelope_case("exactly at the floor is SIGNAL", envelope(reading, "instrument", "1d"), T0 + timedelta(days=1), 0.485),
        envelope_case("unknown source", envelope(reading, "unknown", "7d"), T0, 0.50),
        envelope_case("content changed since stamp", envelope(reading, "instrument", "7d"), T0, 0.50, tamper={"reading": 43}),
        envelope_case("clock skew: now before measured_at", envelope(reading, "instrument", "1h"), T0 - timedelta(hours=1), 0.90),
        envelope_case("version 0.1 envelope (no baseline field)", envelope(reading, "database", "30d", version="0.1"),
                      T0 + timedelta(days=30), 0.40),
        envelope_case("already below at measurement", envelope(reading, "llm_ungrounded", "7d"), T0, 0.50),
        envelope_case("hearsay, months", envelope({"rumor": "x"}, "hearsay", "3mo"), T0 + timedelta(days=45), 0.10),
    ]


def integrity_cases():
    contents = [
        {"reading": 42},
        {"b": 1, "a": 2},
        {"nested": {"z": [3, 2, 1], "a": None}, "flag": True},
        {"name": "Zoë", "city": "München"},
        "plain text",
        [1, "two", 3.5],
        {"x": 0.1, "y": -2.5e-07},
        [1e-4, 1e-5, 1.5e-5, -3e-5, 9.99e-6, 1e-7, 5e-324],
        [1e15, 1e16, 2e16, 1.2345678901234567e20, 1.7976931348623157e308, 1.0, 100.0],
        {"quote": "a \"b\" \\ c", "tab": "\t", "ctrl": "\u0001"},
    ]
    return [{"content": c, "canonical": canonical(c), "hash": integrity(c)} for c in contents]


def ledger_cases():
    """CSV text an exporter writes → observations fid reads."""
    ok = [
        ("entity,age,correct\nq1,30d,true\nq2,0,false\n",
         [("q1", 30 * DAY, True, None), ("q2", 0.0, False, None)]),
        ("Entity,Correct,Age,State\nq1,yes,6h,underway\nq2,N,90,moored\n",
         [("q1", 6 * 3600.0, True, "underway"), ("q2", 90.0, False, "moored")]),
        ("entity,observed_at,checked_at,correct,note\n"
         "q1,2026-01-01T00:00:00Z,2026-01-31T00:00:00Z,1,ignored\n"
         "q2,2026-01-01T00:00:00+05:30,2026-01-01T00:00:00Z,0,\n",
         [("q1", 30 * DAY, True, None), ("q2", 5.5 * 3600, False, None)]),
        ("entity,age,correct,state\nq1,1d,t,\nq2,2d,f,\n",
         [("q1", DAY, True, None), ("q2", 2 * DAY, False, None)]),
    ]
    bad = [
        ("entity,correct\nq1,true\n", "no age column"),
        ("entity,age\nq1,1d\n", "no correct column"),
        ("age,correct\n1d,true\n", "no entity column"),
        ("entity,age,observed_at,checked_at,correct\nq1,1d,2026-01-01T00:00:00Z,2026-01-02T00:00:00Z,true\n", "both age and times"),
        ("entity,age,correct\nq1,1d,maybe\n", "correct not a boolean"),
        ("entity,age,correct\n,1d,true\n", "empty entity"),
        ("entity,age,correct\nq1,-1d,true\n", "negative age"),
        ("entity,observed_at,checked_at,correct\nq1,2026-01-02T00:00:00Z,2026-01-01T00:00:00Z,true\n", "checked before observed"),
        ("entity,observed_at,checked_at,correct\nq1,2026-01-01,2026-01-02,true\n", "dates without times"),
    ]
    out = [
        {"csv": text, "observations": [{"entity": e, "age_seconds": a, "correct": c, "state": s} for e, a, c, s in obs]}
        for text, obs in ok
    ]
    out += [{"csv": text, "error": reason} for text, reason in bad]
    return out


# ── example ledger ───────────────────────────────────────────────────────

LEDGER_TRUTH = {"s0": 0.95, "baseline": 0.30, "half_life_days": 30.0}


def write_ledger(path):
    """A synthetic re-verification export with known decay, in the format a store would write.

    For each age, the fraction of entities still correct follows the model
    exactly (rounded to whole entities), so `fid calibrate` should recover
    LEDGER_TRUTH closely.
    """
    s0, b, h = LEDGER_TRUTH["s0"], LEDGER_TRUTH["baseline"], LEDGER_TRUTH["half_life_days"]
    per_age = 200
    rows = ["claim_id,entity,observed_at,checked_at,correct,outcome"]
    n = 0
    for days in [0, 3, 7, 14, 21, 30, 45, 60, 90, 120, 180, 270, 365]:
        p = b + (s0 - b) * 0.5 ** (days / h)
        right = round(per_age * p)
        for i in range(per_age):
            n += 1
            # spread the correct ones across entity ids so both held-out halves see the curve
            correct = (i * 7919) % per_age < right
            observed = T0 + timedelta(hours=n % 24)
            checked = observed + timedelta(days=days)
            outcome = "confirmed" if correct else ("superseded" if i % 2 else "rejected")
            rows.append(",".join([
                f"c{n}", f"company-{n:05d}", observed.strftime("%Y-%m-%dT%H:%M:%SZ"),
                checked.strftime("%Y-%m-%dT%H:%M:%SZ"), "true" if correct else "false", outcome,
            ]))
    with open(path, "w") as f:
        f.write("\n".join(rows) + "\n")


def main():
    vectors = {
        "fid_conformance": "1",
        "spec": "conformance/README.md",
        "tolerance": 1e-9,
        "parse_duration": duration_cases(),
        "parse_half_life": half_life_cases(),
        "source_fidelity": source_cases(),
        "fidelity": [
            {"name": n, "source": s, "age_seconds": a, "half_life_seconds": h, "noise": z, "baseline": b,
             "fidelity": fidelity(s, a, h, z, b)}
            for n, s, a, h, z, b in FIDELITY
        ],
        "time_to_floor": [
            {"name": n, "fidelity": f, "half_life_seconds": h, "baseline": b, "floor": fl,
             "seconds": time_to_floor(f, h, b, fl)}
            for n, f, h, b, fl in TTL
        ],
        "integrity": integrity_cases(),
        "envelope": envelope_cases(),
        "ledger_csv": ledger_cases(),
        "ledger_example": {
            "file": "conformance/ledger-example.csv",
            "type": "company_ceo",
            "truth": LEDGER_TRUTH,
            "half_life_within": 0.10,
            "baseline_within": 0.05,
        },
    }
    with open(os.path.join(HERE, "vectors.json"), "w") as f:
        json.dump(vectors, f, indent=1, ensure_ascii=False)
        f.write("\n")
    write_ledger(os.path.join(HERE, "ledger-example.csv"))


if __name__ == "__main__":
    main()
