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
import re
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

NUMBER = r"[0-9]+(?:\.[0-9]+)?"
SHORT = re.compile(rf"({NUMBER})(s|m|h|d|mo|y)?", re.I)
ISO_UNITS = {"y": 365.25 * 86400.0, "mo": 30.44 * 86400.0, "w": 7 * 86400.0, "d": 86400.0,
             "h": 3600.0, "mi": 60.0, "s": 1.0}
ISO = re.compile(
    rf"p(?:(?P<w>{NUMBER})w"
    rf"|(?:(?P<y>{NUMBER})y)?(?:(?P<mo>{NUMBER})m)?(?:(?P<d>{NUMBER})d)?"
    rf"(?:t(?:(?P<h>{NUMBER})h)?(?:(?P<mi>{NUMBER})m)?(?:(?P<s>{NUMBER})s)?)?)",
    re.I,
)


def parse_duration(text):
    """The draft's Durations grammar: the short form, or an ISO 8601 duration."""
    t = text.strip(" \t")
    m = SHORT.fullmatch(t)
    if m:
        n = float(m.group(1)) * UNITS[(m.group(2) or "s").lower()]
    else:
        m = ISO.fullmatch(t)
        parts = {k: v for k, v in (m.groupdict() if m else {}).items() if v is not None}
        has_t = m is not None and "t" in t.lower()
        time_parts = {"h", "mi", "s"} & set(parts)
        if not parts or (has_t and not time_parts):
            raise ValueError(text)
        n = sum(float(v) * ISO_UNITS[k] for k, v in parts.items())
    if not math.isfinite(n):
        raise ValueError(text)
    return n


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


def echo_seconds(echo):
    """(period, strength, width, fade) in seconds, fade None for infinite; ValueError if invalid."""
    period, width = parse_half_life(echo["period"]), parse_half_life(echo["width"])
    fade = parse_half_life(echo["fade"])
    strength = echo["strength"]
    if period is None or width is None:
        raise ValueError("period and width must be finite")
    if not math.isfinite(strength) or strength < 0:
        raise ValueError("strength")
    if not width < period / 2:
        raise ValueError("width")
    return period, strength, width, fade


def echo_temporal(age, half_life, echo):
    """min(1, 0.5^(t/h) + r × Σₙ≥₁ 0.5^(nP/h_r) × 0.5^(|t − nP|/w)), returns within 30 widths of t."""
    period, strength, width, fade = echo_seconds(echo)
    t = max(age, 0.0)
    total = 0.0
    n = max(1, math.ceil((t - 30 * width) / period))
    while n * period <= t + 30 * width:
        total += (1.0 if fade is None else 0.5 ** (n * period / fade)) * 0.5 ** (abs(t - n * period) / width)
        n += 1
    return min(1.0, 0.5 ** (t / half_life) + strength * total)


def echo_fidelity(source, age, half_life, noise, baseline, echo):
    f = fidelity(source, 0.0, None, noise, 0.0)
    b = min(max(baseline, 0.0), max(f, 0.0))
    return b + (f - b) * echo_temporal(age, half_life, echo)


def echo_time_to_floor(f, half_life, baseline, floor, echo):
    """First time below floor: seconds, or None for never (not within 10,000 periods)."""
    if f < floor:
        return 0.0
    b = min(max(baseline, 0.0), max(f, 0.0))
    if floor <= b:
        return None
    period, _, width, _ = echo_seconds(echo)
    below = lambda t: echo_fidelity(f, t, half_life, [], baseline, echo) < floor
    step = min(half_life, width) / 100.0
    t = 0.0
    while t <= 10_000 * period:
        if below(t + step):
            lo, hi = t, t + step
            for _ in range(100):
                mid = (lo + hi) / 2
                lo, hi = (lo, mid) if below(mid) else (mid, hi)
            return hi
        t += step
    return None


def fast_seconds(fast, half_life):
    """(share, fast half-life) in seconds; ValueError if invalid. Needs a finite, longer main half-life."""
    share = fast["share"]
    if not isinstance(share, (int, float)) or not math.isfinite(share) or not 0 <= share <= 1:
        raise ValueError("share")
    h = parse_half_life(fast["half_life"])
    if h is None:
        raise ValueError("fast half-life must be finite")
    if half_life is None or not h < half_life:
        raise ValueError("fast half-life must be shorter than a finite half-life")
    return share, h


def shape_value(shape, half_life):
    """The clocks' shape k; ValueError unless finite, from 0.1 to 10, with a finite half-life."""
    if shape is None:
        return 1.0
    if not isinstance(shape, (int, float)) or not math.isfinite(shape) or not 0.1 <= shape <= 10:
        raise ValueError("shape")
    if half_life is None:
        raise ValueError("a shape needs a finite half-life")
    return float(shape)


def curve_temporal(age, half_life, fast=None, echo=None, shape=None):
    """(1 − m) 0.5^((t/h)^k) + m 0.5^((t/h_f)^k), plus the echo's returns, at most 1."""
    t = max(age, 0.0)
    k = shape_value(shape, half_life)
    slow = 1.0 if half_life is None else 0.5 ** ((t / half_life) ** k)
    if fast is None:
        base = slow
    else:
        share, hf = fast_seconds(fast, half_life)
        base = (1 - share) * slow + share * 0.5 ** ((t / hf) ** k)
    if echo is None:
        return base
    period, strength, width, fade = echo_seconds(echo)
    total = 0.0
    n = max(1, math.ceil((t - 30 * width) / period))
    while n * period <= t + 30 * width:
        total += (1.0 if fade is None else 0.5 ** (n * period / fade)) * 0.5 ** (abs(t - n * period) / width)
        n += 1
    return min(1.0, base + strength * total)


def curve_fidelity(f, age, half_life, baseline, fast=None, echo=None, shape=None):
    b = min(max(baseline, 0.0), max(f, 0.0))
    return b + (f - b) * curve_temporal(age, half_life, fast, echo, shape)


def shortest(half_life, fast, echo):
    scales = [x for x in (half_life,) if x is not None]
    if fast is not None:
        scales.append(fast_seconds(fast, half_life)[1])
    if echo is not None:
        scales.append(echo_seconds(echo)[2])
    return min(scales)


def curve_time_to_floor(f, half_life, baseline, floor, fast=None, echo=None, shape=None):
    """First time below floor, by scanning in steps of a hundredth of the shortest timescale; None for never."""
    if f < floor:
        return 0.0
    b = min(max(baseline, 0.0), max(f, 0.0))
    if half_life is None or floor <= b:
        return None
    below = lambda t: curve_fidelity(f, t, half_life, baseline, fast, echo, shape) < floor
    step = shortest(half_life, fast, echo) / 100.0
    end = 10_000 * (echo_seconds(echo)[0] if echo else half_life)
    t = 0.0
    while t <= end:
        if below(t + step):
            lo, hi = t, t + step
            for _ in range(100):
                mid = (lo + hi) / 2
                lo, hi = (lo, mid) if below(mid) else (mid, hi)
            return hi
        t += step
    return None


def exp_integral(t, h):
    """∫₀ᵗ 0.5^(u / h) du."""
    if h is None:
        return t
    return -h / math.log(2) * math.expm1(-t * math.log(2) / h)


def lower_gamma_regularized(a, x):
    """P(a, x) = x^a e^(−x) Σₙ xⁿ / Γ(a + n + 1); 1 once x is far past a, where the rest is under 1e-17."""
    if x <= 0:
        return 0.0
    if x > a + 60 + 10 * math.sqrt(a):
        return 1.0
    total, n = 0.0, 0
    while True:
        term = math.exp(n * math.log(x) - math.lgamma(a + n + 1))
        total += term
        if term < 1e-18 * total and n > x:
            break
        n += 1
    return min(1.0, total * math.exp(a * math.log(x) - x))


def clock_integral(t, h, k):
    """∫₀ᵗ 0.5^((u / h)^k) du = h ln2^(−1/k) / k × Γ(1/k) × P(1/k, ln2 (t/h)^k)."""
    if k == 1.0:
        return exp_integral(t, h)
    if h is None:
        return t
    a = 1 / k
    return h * math.log(2) ** (-a) / k * math.exp(math.lgamma(a)) * lower_gamma_regularized(a, math.log(2) * (t / h) ** k)


def uncapped_echo(t, period, strength, width, fade):
    """strength × the echo's returns within 30 widths of t."""
    total = 0.0
    n = max(1, math.ceil((t - 30 * width) / period))
    while n * period <= t + 30 * width:
        total += (1.0 if fade is None else 0.5 ** (n * period / fade)) * 0.5 ** (abs(t - n * period) / width)
        n += 1
    return strength * total


def temporal_integral(t, half_life, fast=None, echo=None, shape=None):
    """∫₀ᵗ temporal. The clocks and each echo return integrate in closed form;
    the cap at 1 is subtracted by Simpson's rule where it can bind."""
    if t <= 0:
        return 0.0
    k = shape_value(shape, half_life)
    if fast is None:
        total = clock_integral(t, half_life, k)
    else:
        share, hf = fast_seconds(fast, half_life)
        total = (1 - share) * clock_integral(t, half_life, k) + share * clock_integral(t, hf, k)
    if echo is None:
        return total
    period, strength, width, fade = echo_seconds(echo)
    lam = math.log(2) / width
    n = 1
    while n * period <= t + 30 * width:
        c = n * period
        weight = 1.0 if fade is None else 0.5 ** (c / fade)
        if t <= c:  # ∫₀ᵗ exp(−λ|u − c|) du
            part = (math.exp(-lam * (c - t)) - math.exp(-lam * c)) / lam
        else:
            part = (1 - math.exp(-lam * c)) / lam + (1 - math.exp(-lam * (t - c))) / lam
        total += strength * weight * part
        n += 1
    # The cap can bind only while plain decay is above 1 − strength × 2 / (1 − 0.5^(P/w))
    plain = lambda u: curve_temporal(u, half_life, fast, None, shape)
    bound = 1 - strength * 2 / (1 - 0.5 ** (period / width))
    reach = t
    if bound > 0 and plain(t) <= bound:
        lo, hi = 0.0, t
        for _ in range(100):
            mid = (lo + hi) / 2
            lo, hi = (mid, hi) if plain(mid) > bound else (lo, mid)
        reach = hi
    excess = lambda u: max(0.0, plain(u) + uncapped_echo(u, period, strength, width, fade) - 1)
    m = 4000
    h = reach / m
    cap = excess(0.0) + excess(reach) + sum(excess(i * h) * (4 if i % 2 else 2) for i in range(1, m))
    return total - cap * h / 3


def mean_staleness(f, half_life, baseline, interval, fast=None, echo=None, shape=None):
    """1 − (1/T) ∫₀ᵀ fidelity."""
    if interval <= 0:
        return 1 - f
    b = min(max(baseline, 0.0), max(f, 0.0))
    return 1 - (b + (f - b) * temporal_integral(interval, half_life, fast, echo, shape) / interval)


def recheck_cost(f, half_life, baseline, price, interval, fast=None, echo=None, shape=None):
    return mean_staleness(f, half_life, baseline, interval, fast, echo, shape) + price * 86400.0 / interval


def best_interval(f, half_life, baseline, price, fast=None, echo=None, shape=None):
    """(seconds, cost) minimizing staleness + price × 86400 / T on [1 s, 10,000 × longest scale]; None for never."""
    scale = half_life
    if echo is not None:
        period, _, _, fade = echo_seconds(echo)
        scale = max(scale, period, fade or period)
    cost = lambda t: recheck_cost(f, half_life, baseline, price, t, fast, echo, shape)
    lo, hi = 0.0, math.log(10_000 * scale)
    grid = [math.exp(lo + (hi - lo) * i / 400) for i in range(401)]
    costs = [cost(t) for t in grid]
    i = min(range(len(grid)), key=costs.__getitem__)
    a, c = math.log(grid[max(i - 1, 0)]), math.log(grid[min(i + 1, len(grid) - 1)])
    for _ in range(100):  # ternary search in log T
        m1, m2 = a + (c - a) / 3, c - (c - a) / 3
        if cost(math.exp(m1)) < cost(math.exp(m2)):
            c = m2
        else:
            a = m1
    t = math.exp((a + c) / 2)
    best = min(cost(t), costs[i])
    b = min(max(baseline, 0.0), max(f, 0.0))
    if 1 - b <= best:
        return None, 1 - b
    return t, best


def interval_for_staleness(f, half_life, baseline, target, fast=None, echo=None, shape=None):
    """Seconds until mean staleness first exceeds target (bisection); None for never."""
    stale = lambda t: mean_staleness(f, half_life, baseline, t, fast, echo, shape)
    t, prev = 1.0, 0.0
    while t < 10_000 * half_life:
        if stale(t) > target:
            lo, hi = prev, t
            for _ in range(60):
                mid = (lo + hi) / 2
                lo, hi = (mid, hi) if stale(mid) <= target else (lo, mid)
            return lo
        prev, t = t, t * 1.5
    return None


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
    good = ["30s", "5m", "6h", "7d", "3mo", "3y", "1.5d", "90", "0d", " 2H ", "0.25y",
            "PT5M", "P1D", "P2W", "P1Y2M3DT4H5M6.5S", "pt90s", "P0D", "PT0.5H", "P1M", "PT1M"]
    bad = ["7days", "30ms", "", "abc", "-1d", "inf", "nan", "infd", "1e400d",
           "1e3d", "+5d", ".5d", "5.d", "1,5d", "5 d",
           "P", "PT", "P1H", "PT1D", "P1W2D", "P1DT", "P1D1Y", "1P"]
    out = [{"input": s, "seconds": parse_duration(s)} for s in good]
    out += [{"input": s, "error": True} for s in bad]
    return out


def half_life_cases():
    good = ["30s", "6h", "7d", "3mo", "5.8y", "infinite", "INF", "90", "P1D", "PT2.4H"]
    bad = ["0d", "0", "-5h", "7days", "nan", "", "PT0S", "1e3d"]
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
    source = envelope_source(source_class, extra.get("source_fidelity"))
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
        "fid_at_stamp": None if source is None else (
            curve_fidelity(fidelity(source, 0.0, None, noise, 0.0), 0.0, hl, baseline or 0.0, extra.get("fast"), extra.get("echo"))
            if "echo" in extra or "fast" in extra
            else fidelity(source, 0.0, hl, noise, baseline or 0.0)),
        "derived_from": None,
        "integrity": integrity(content),
    }
    if baseline is not None:
        env["baseline"] = baseline
    env.update(extra)
    return {"fid": env, "content": content}


def envelope_source(source_class, source_fidelity):
    """The measured source fidelity when there is one, else the class constant; unknown stays unknown."""
    constant = SOURCE_CLASSES[source_class]
    if constant is None:
        return None
    return constant if source_fidelity is None else source_fidelity


def envelope_case(name, stamped, now, floor, tamper=None):
    env = stamped["fid"]
    if tamper is not None:
        stamped = {"fid": env, "content": tamper}
    hl = None if env["half_life"] is None else parse_half_life(env["half_life"])
    measured = datetime.fromisoformat(env["measured_at"].replace("Z", "+00:00"))
    age = (now - measured).total_seconds()
    noise = [t["noise"] for t in env["transforms"]]
    baseline = env.get("baseline") or 0.0
    source = envelope_source(env["source_class"], env.get("source_fidelity"))
    echo = env.get("echo")
    fast = env.get("fast")
    ok = integrity(stamped["content"]) == env["integrity"]

    if source is None:
        f = None
    elif fast is not None:
        f = curve_fidelity(fidelity(source, 0.0, None, noise, 0.0), age, hl, baseline, fast, echo)
    elif echo is not None:
        f = echo_fidelity(source, age, hl, noise, baseline, echo)
    else:
        f = fidelity(source, age, hl, noise, baseline)
    if not ok or f is None:
        status = "UNKNOWN"
    else:
        status = "SIGNAL" if f >= floor else "NOISE"

    if source is None:
        expires = "unknown"
    else:
        at_measurement = fidelity(source, 0.0, None, noise, 0.0)
        if fast is not None:
            ttl = curve_time_to_floor(at_measurement, hl, baseline, floor, fast, echo)
        elif echo is not None:
            ttl = echo_time_to_floor(at_measurement, hl, baseline, floor, echo)
        else:
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
        envelope_case(
            "measured source fidelity replaces the class constant",
            envelope({"clause": 7}, "database", "30d", baseline=0.30, version="0.3", source_fidelity=0.62),
            T0 + timedelta(days=30), 0.50,
        ),
        envelope_case(
            "echo: a day-old temperature is back above the floor",
            envelope({"station": "KLAX", "temp_f": 64}, "instrument", "2.42h", baseline=0.13, version="0.3", echo=TEMPERATURE),
            T0 + timedelta(hours=24), 0.30,
        ),
        envelope_case(
            "echo: half a day old it is not",
            envelope({"station": "KLAX", "temp_f": 64}, "instrument", "2.42h", baseline=0.13, version="0.3", echo=TEMPERATURE),
            T0 + timedelta(hours=12), 0.30,
        ),
        envelope_case(
            "measured source fidelity, unknown class stays unknown",
            envelope({"clause": 7}, "unknown", "30d", version="0.3", source_fidelity=0.95),
            T0, 0.50,
        ),
        envelope_case(
            "fast share: a week-old release",
            envelope({"package": "requests", "latest": "2.32.3"}, "instrument", "80d", baseline=0.1, version="0.4",
                     source_fidelity=1.0, fast={"share": 0.3, "half_life": "3d"}),
            T0 + timedelta(days=7), 0.80,
        ),
        envelope_case(
            "fast share: three months old",
            envelope({"package": "requests", "latest": "2.32.3"}, "instrument", "80d", baseline=0.1, version="0.4",
                     source_fidelity=1.0, fast={"share": 0.3, "half_life": "3d"}),
            T0 + timedelta(days=90), 0.50,
        ),
    ]


# Airport temperature, rounded from calibration/results/weather_periodic.json
TEMPERATURE = {"period": "24h", "strength": 0.31, "width": "3.75h", "fade": "345h"}

ECHO = [
    # name, source, age, half_life, noise, baseline, echo
    ("fresh", 1.0, 0.0, 2.42 * 3600, [], 0.13, TEMPERATURE),
    ("half a day: trough", 1.0, 12 * 3600.0, 2.42 * 3600, [], 0.13, TEMPERATURE),
    ("one day: first return", 1.0, 24 * 3600.0, 2.42 * 3600, [], 0.13, TEMPERATURE),
    ("between returns", 1.0, 30 * 3600.0, 2.42 * 3600, [], 0.13, TEMPERATURE),
    ("a week: faded returns", 1.0, 168 * 3600.0, 2.42 * 3600, [], 0.13, TEMPERATURE),
    ("with source and noise", 0.97, 24 * 3600.0, 2.42 * 3600, [0.05], 0.13, TEMPERATURE),
    ("never fades", 0.9, 240 * 3600.0, 3600.0, [], 0.0, {"period": "1d", "strength": 0.5, "width": "2h", "fade": "infinite"}),
    ("strong echo is capped at 1", 1.0, 3600.0, 3600.0, [], 0.0, {"period": "2h", "strength": 5.0, "width": "0.5h", "fade": "infinite"}),
    ("negative age counts as zero", 0.9, -3600.0, 3600.0, [], 0.0, TEMPERATURE),
]

ECHO_TTL = [
    # name, fidelity, half_life, baseline, floor, echo
    ("temperature to 0.5", 1.0, 2.42 * 3600, 0.13, 0.5, TEMPERATURE),
    ("temperature to 0.3", 1.0, 2.42 * 3600, 0.13, 0.3, TEMPERATURE),
    ("floor under the baseline", 1.0, 2.42 * 3600, 0.13, 0.1, TEMPERATURE),
    ("already below", 0.4, 2.42 * 3600, 0.13, 0.5, TEMPERATURE),
]

ECHO_ERRORS = [
    ({"period": "24h", "strength": 0.3, "width": "12h", "fade": "10d"}, "width not under half the period"),
    ({"period": "24h", "strength": -0.1, "width": "3h", "fade": "10d"}, "negative strength"),
    ({"period": "infinite", "strength": 0.3, "width": "3h", "fade": "10d"}, "infinite period"),
    ({"period": "24h", "strength": 0.3, "width": "0h", "fade": "10d"}, "zero width"),
    ({"period": "24h", "strength": 0.3, "width": "3h", "fade": "0h"}, "zero fade"),
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


# A half-life registry as JSON text (YAML 1.2 reads JSON unchanged)
REGISTRY = {
    "plain": "1y",
    "typed": {"half_life": "2.9h", "baseline": 0.73},
    "shaped": {"half_life": "1h", "shape": 2.0, "baseline": 0.2, "source_fidelity": 0.9,
               "states": {"bare": "30m", "full": {"half_life": "10m", "baseline": 0.05, "shape": 3.0}}},
    "stateful": {"states": {"a": "1d", "b": {"half_life": "2d", "fast": {"share": 0.3, "half_life": "1d"}}}},
}
FIELDS = ("half_life", "baseline", "source_fidelity", "echo", "fast", "shape")


def registry_lookup(registry, info_type, state):
    """The draft's Half-life registry lookup: a state's entry alone, else the type's own fields."""
    if info_type not in registry:
        raise KeyError("unknown type")
    entry = registry[info_type]
    entry = {"half_life": entry} if isinstance(entry, str) else entry
    if state is not None:
        states = entry.get("states", {})
        if not states:
            raise KeyError("type has no states")
        if state not in states:
            raise KeyError("unknown state")
        entry = states[state]
        entry = {"half_life": entry} if isinstance(entry, str) else entry
    if entry.get("half_life") is None:
        raise KeyError("needs a state")
    return {f: entry.get(f) for f in FIELDS}


def registry_cases():
    text = json.dumps(REGISTRY)
    out = []
    for t, st in [("plain", None), ("typed", None), ("shaped", None), ("shaped", "bare"), ("shaped", "full"),
                  ("stateful", "b"), ("nope", None), ("stateful", None), ("plain", "a"), ("stateful", "zzz")]:
        try:
            out.append({"registry": text, "type": t, "state": st, "spec": registry_lookup(REGISTRY, t, st)})
        except KeyError as e:
            out.append({"registry": text, "type": t, "state": st, "error": e.args[0]})
    for bad, reason in [({"x": {"half_life": "1d", "colour": 1}}, "unknown key"),
                        ({"x": {"states": {"a": {"half_life": "1d", "states": {"b": "1h"}}}}}, "a state with states"),
                        ({"x": {"half_life": "1d", "shape": 2.0, "states": {"a": "0d"}}}, "zero half-life"),
                        ({"x": {"half_life": "1d", "fast": {"share": 0.3, "half_life": "2d"}}}, "fast clock not shorter")]:
        state = "a" if "states" in bad["x"] else None
        out.append({"registry": json.dumps(bad), "type": "x", "state": state, "error": reason})
    return out


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


# A PyPI-like release: 30% replaced within days, the rest over months
PACKAGE = {"share": 0.3, "half_life": "3d"}
DAY = 86400.0

FAST = [
    # name, fidelity, age, half_life, baseline, fast, echo[, shape]
    ("fresh", 1.0, 0.0, 80 * DAY, 0.1, PACKAGE, None),
    ("three days: the fast share half gone", 1.0, 3 * DAY, 80 * DAY, 0.1, PACKAGE, None),
    ("a year: only the slow clock", 0.97, 365 * DAY, 80 * DAY, 0.1, PACKAGE, None),
    ("zero share is plain decay", 1.0, 10 * DAY, 80 * DAY, 0.0, {"share": 0.0, "half_life": "3d"}, None),
    ("whole share is the fast clock", 1.0, 10 * DAY, 80 * DAY, 0.0, {"share": 1.0, "half_life": "3d"}, None),
    ("with an echo", 1.0, 24 * 3600.0, 2.42 * 3600, 0.13, {"share": 0.2, "half_life": "0.5h"}, TEMPERATURE),
    ("shape: a moving ship, one minute", 1.0, 60.0, 180.0, 0.03, None, None, 3.0),
    ("shape: a moving ship, five minutes", 1.0, 300.0, 180.0, 0.03, None, None, 3.0),
    ("shape below 1: falls fast, lingers", 0.98, 10 * DAY, 30 * DAY, 0.0, None, None, 0.5),
    ("shape with two clocks", 1.0, 240.0, 480.0, 0.02, {"share": 0.6, "half_life": "3m"}, None, 2.5),
    ("shape with an echo", 1.0, 24 * 3600.0, 2.42 * 3600, 0.13, None, TEMPERATURE, 1.7),
]

FAST_TTL = [
    # name, fidelity, half_life, baseline, floor, fast, echo
    ("package to 0.9", 1.0, 80 * DAY, 0.1, 0.9, PACKAGE, None),
    ("package to 0.5", 1.0, 80 * DAY, 0.1, 0.5, PACKAGE, None),
    ("floor under the baseline", 1.0, 80 * DAY, 0.1, 0.05, PACKAGE, None),
    ("already below", 0.8, 80 * DAY, 0.1, 0.9, PACKAGE, None),
    ("with an echo", 1.0, 2.42 * 3600, 0.13, 0.5, {"share": 0.2, "half_life": "0.5h"}, TEMPERATURE),
    ("shape: a moving ship to 0.9", 1.0, 180.0, 0.03, 0.9, None, None, 3.0),
    ("shape below 1 to 0.5", 0.98, 30 * DAY, 0.0, 0.5, None, None, 0.5),
    ("shape with two clocks to 0.7", 1.0, 480.0, 0.02, 0.7, {"share": 0.6, "half_life": "3m"}, None, 2.5),
]

SHAPE_ERRORS = [
    (0.0, 3600.0, "zero shape"),
    (-1.0, 3600.0, "negative shape"),
    (11.0, 3600.0, "shape above 10"),
    (2.0, None, "no finite half-life"),
]

FAST_ERRORS = [
    ({"share": 1.5, "half_life": "3d"}, 80 * DAY, "share above 1"),
    ({"share": -0.1, "half_life": "3d"}, 80 * DAY, "negative share"),
    ({"share": 0.3, "half_life": "infinite"}, 80 * DAY, "infinite fast half-life"),
    ({"share": 0.3, "half_life": "90d"}, 80 * DAY, "fast half-life not shorter"),
    ({"share": 0.3, "half_life": "3d"}, None, "no finite half-life"),
]

SCHEDULE = [
    # name, fidelity, half_life, baseline, price, fast, echo
    ("one day half-life, cheap re-checks", 1.0, DAY, 0.1, 1e-4, None, None),
    ("one day half-life, dear re-checks", 1.0, DAY, 0.1, 1e-2, None, None),
    ("too dear: never", 1.0, DAY, 0.1, 100.0, None, None),
    ("two clocks", 1.0, 80 * DAY, 0.1, 1e-3, PACKAGE, None),
    ("source below 1", 0.95, 7 * DAY, 0.0, 1e-3, None, None),
    ("temperature with its echo", 1.0, 2.42 * 3600, 0.13, 1e-3, None, TEMPERATURE),
    ("a moving ship, shape 3", 1.0, 180.0, 0.03, 1e-4, None, None, 3.0),
    ("shape with two clocks", 1.0, 480.0, 0.02, 1e-4, {"share": 0.6, "half_life": "3m"}, None, 2.5),
]

STALENESS = [
    # name, fidelity, half_life, baseline, target, fast, echo
    ("one day half-life, 5%", 1.0, DAY, 0.1, 0.05, None, None),
    ("two clocks, 5%", 1.0, 80 * DAY, 0.1, 0.05, PACKAGE, None),
    ("two clocks, 20%", 0.99, 80 * DAY, 0.1, 0.20, PACKAGE, None),
    ("baseline already under the target: never", 1.0, DAY, 0.9, 0.2, None, None),
    ("a moving ship, shape 3, 5%", 1.0, 180.0, 0.03, 0.05, None, None, 3.0),
]


# The draft sections (spec/draft-kumar-fid-00.md anchors) each group of vectors tests
GROUP_SECTIONS = {
    "parse_duration": ["durations"],
    "parse_half_life": ["half-lives"],
    "source_fidelity": ["source-classes"],
    "fidelity": ["at-measurement", "temporal", "fidelity", "tolerances"],
    "time_to_floor": ["time-to-floor", "tolerances"],
    "echo": ["temporal", "fidelity", "tolerances"],
    "echo_time_to_floor": ["time-to-floor", "tolerances"],
    "echo_errors": ["temporal"],
    "fast": ["temporal", "fidelity", "tolerances"],
    "fast_time_to_floor": ["time-to-floor", "tolerances"],
    "fast_errors": ["temporal"],
    "shape_errors": ["temporal"],
    "schedule": ["staleness", "tolerances"],
    "staleness_target": ["staleness", "tolerances"],
    "integrity": ["integrity", "legacy-canonical"],
    "envelope": ["structure", "envelope-fields", "timestamps", "check", "expiry"],
    "ledger_csv": ["ledger"],
    "registry": ["half-life-registry"],
}

# Envelope vectors that also test another section, by name
ENVELOPE_SECTIONS = {
    "transform chain": ["at-measurement"],
    "conditioned transform": ["at-measurement"],
    "exactly at the floor is SIGNAL": ["time-to-floor"],
    "unknown source": ["source-classes"],
    "content changed since stamp": ["integrity"],
    "clock skew: now before measured_at": ["timestamps"],
    "version 0.1 envelope (no baseline field)": ["versioning"],
    "measured source fidelity replaces the class constant": ["source-classes"],
    "measured source fidelity, unknown class stays unknown": ["source-classes"],
}

# Vectors that encode fid 0.4's behaviour where the draft says otherwise: the issue that changes them
DEVIATIONS = {
    "no half-life never decays": 9,
}


def tag(vectors):
    """Give every vector the draft sections it tests, and mark known deviations."""
    for group, sections in GROUP_SECTIONS.items():
        for v in vectors[group]:
            name = v.get("name", "")
            v["sections"] = sections + [s for s in ENVELOPE_SECTIONS.get(name, []) if s not in sections]
            if name in DEVIATIONS:
                v["deviation_issue"] = DEVIATIONS[name]
    vectors["ledger_example"]["sections"] = ["calibrator"]
    names = {v.get("name") for g in GROUP_SECTIONS for v in vectors[g]}
    missing = (set(ENVELOPE_SECTIONS) | set(DEVIATIONS)) - names
    if missing:
        raise SystemExit(f"tagged names with no vector: {sorted(missing)}")
    return vectors


def main():
    vectors = {
        "fid_conformance": "1",
        "spec": "spec/draft-kumar-fid-00.md",
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
        "echo": [
            {"name": n, "source": s, "age_seconds": a, "half_life_seconds": h, "noise": z, "baseline": b, "echo": e,
             "fidelity": echo_fidelity(s, a, h, z, b, e)}
            for n, s, a, h, z, b, e in ECHO
        ],
        "echo_time_to_floor": [
            {"name": n, "fidelity": f, "half_life_seconds": h, "baseline": b, "floor": fl, "echo": e,
             "seconds": echo_time_to_floor(f, h, b, fl, e)}
            for n, f, h, b, fl, e in ECHO_TTL
        ],
        "echo_errors": [{"echo": e, "error": reason} for e, reason in ECHO_ERRORS],
        "fast": [
            {"name": n, "fidelity_at_measurement": f, "age_seconds": a, "half_life_seconds": h, "baseline": b,
             "fast": fa, "echo": e, "shape": k, "fidelity": curve_fidelity(f, a, h, b, fa, e, k)}
            for n, f, a, h, b, fa, e, k in (c if len(c) == 8 else c + (None,) for c in FAST)
        ],
        "fast_time_to_floor": [
            {"name": n, "fidelity": f, "half_life_seconds": h, "baseline": b, "floor": fl, "fast": fa, "echo": e,
             "shape": k, "seconds": curve_time_to_floor(f, h, b, fl, fa, e, k)}
            for n, f, h, b, fl, fa, e, k in (c if len(c) == 8 else c + (None,) for c in FAST_TTL)
        ],
        "fast_errors": [{"fast": fa, "half_life_seconds": h, "error": reason} for fa, h, reason in FAST_ERRORS],
        "shape_errors": [{"shape": k, "half_life_seconds": h, "error": reason} for k, h, reason in SHAPE_ERRORS],
        "schedule": [
            dict(zip(("name", "fidelity", "half_life_seconds", "baseline", "price", "fast", "echo", "shape"), case),
                 **dict(zip(("seconds", "cost"), best_interval(*case[1:]))))
            for case in (c if len(c) == 8 else c + (None,) for c in SCHEDULE)
        ],
        "staleness_target": [
            {"name": n, "fidelity": f, "half_life_seconds": h, "baseline": b, "target": t, "fast": fa, "echo": e,
             "shape": k, "seconds": interval_for_staleness(f, h, b, t, fa, e, k)}
            for n, f, h, b, t, fa, e, k in (c if len(c) == 8 else c + (None,) for c in STALENESS)
        ],
        "integrity": integrity_cases(),
        "envelope": envelope_cases(),
        "ledger_csv": ledger_cases(),
        "registry": registry_cases(),
        "ledger_example": {
            "file": "conformance/ledger-example.csv",
            "type": "company_ceo",
            "truth": LEDGER_TRUTH,
            "half_life_within": 0.10,
            "baseline_within": 0.05,
        },
    }
    tag(vectors)
    untagged = [g for g, v in vectors.items() if isinstance(v, list) and any(not c.get("sections") for c in v)]
    if untagged:
        raise SystemExit(f"vectors without a draft section: {untagged}; add them to GROUP_SECTIONS")
    with open(os.path.join(HERE, "vectors.json"), "w") as f:
        json.dump(vectors, f, indent=1, ensure_ascii=False)
        f.write("\n")
    write_ledger(os.path.join(HERE, "ledger-example.csv"))


if __name__ == "__main__":
    main()
