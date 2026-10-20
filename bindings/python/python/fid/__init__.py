"""fid — how much a stored fact can still be trusted.

    fidelity = baseline + (source × ∏(1 − noise) − baseline) × 0.5^(age / half_life)

The scoring, envelopes, registries and calibration all run in fid's Rust
library; this module gives them a Python shape.

    >>> import fid
    >>> artifact = fid.stamp({"ceo": "Ada"}, source="registry", source_class="database",
    ...                      half_life="5.8y")
    >>> fid.check(artifact, floor=0.7).status
    <Status.SIGNAL: 0>
    >>> fid.expires_at(artifact, floor=0.7)      # when it falls below the floor
    datetime.datetime(...)
"""

from __future__ import annotations

import enum
import json
import math
import warnings
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Any, Iterable, Mapping, Optional, Sequence, Union

from . import _fid

__all__ = [
    "FID_VERSION",
    "Assessment",
    "Calibration",
    "Drift",
    "IndeterminateError",
    "Status",
    "calibrate",
    "chain",
    "check",
    "drift",
    "expires_at",
    "fidelity",
    "half_life",
    "noise",
    "mean_staleness",
    "parse_half_life",
    "recheck_at",
    "recheck_interval",
    "registry",
    "source_fidelity",
    "stamp",
    "time_to_floor",
]

FID_VERSION: str = _fid.FID_VERSION

Duration = Union[str, float, int, timedelta]


class IndeterminateError(ValueError):
    """The source class is unknown, so fidelity cannot be determined."""


class Status(enum.IntEnum):
    """Epistemic state. The value is the CLI exit code."""

    SIGNAL = 0
    NOISE = 1
    UNKNOWN = 2


@dataclass(frozen=True)
class Assessment:
    status: Status
    fidelity: Optional[float]
    source: Optional[float]
    temporal: float
    transforms: float
    baseline: float
    floor: float
    integrity_ok: bool
    age: timedelta


@dataclass(frozen=True)
class Calibration:
    yaml: str
    """Half-life registry YAML, as `fid stamp --half-lives` reads it."""
    report: str
    """The human-readable report `fid calibrate` prints."""
    fits: list
    """One dict per state: half_life_seconds, baseline, Brier scores, …"""


@dataclass(frozen=True)
class Drift:
    verdict: str
    """CURRENT, DRIFTED or UNKNOWN."""
    report: str
    """The human-readable report `fid drift` prints."""
    states: list
    """One dict per state: verdict, p_value, registry and recent values, …"""


# ── helpers ──────────────────────────────────────────────────────────────

def _seconds(value: Optional[Duration]) -> Optional[float]:
    """Seconds from a duration string, number of seconds, or timedelta. None = infinite."""
    if value is None:
        return None
    if isinstance(value, timedelta):
        return value.total_seconds()
    if isinstance(value, (int, float)):
        return math.inf if math.isinf(value) else float(value)
    return _fid.parse_half_life(value)


def _age(value: Duration) -> float:
    """Seconds in an age: a duration string ("0d", "30d"), seconds, or timedelta."""
    if isinstance(value, timedelta):
        seconds = value.total_seconds()
    elif isinstance(value, (int, float)):
        seconds = float(value)
    else:
        return _fid.parse_duration(value)
    if not (math.isfinite(seconds) and seconds >= 0):
        raise ValueError(f"invalid age {value!r} (must be zero or more)")
    return seconds


def _half_life_str(value: Optional[Duration]) -> Optional[str]:
    if value is None or isinstance(value, str):
        return value
    seconds = _seconds(value)
    return "infinite" if seconds is None or math.isinf(seconds) else f"{seconds}s"


def _now(now: Optional[datetime]) -> Optional[str]:
    if now is None:
        return None
    if now.tzinfo is None:
        raise ValueError("now must be timezone-aware")
    return now.astimezone(timezone.utc).isoformat()


def _echo(echo: Optional[Mapping[str, Any]]) -> Optional[str]:
    return None if echo is None else json.dumps(dict(echo))


def _fast(fast: Optional[Mapping[str, Any]]) -> Optional[str]:
    return None if fast is None else json.dumps(dict(fast))


def _hl(half_life: Optional[Duration]) -> Optional[float]:
    h = _seconds(half_life)
    return None if h is None or math.isinf(h) else h


def _json(stamped: Union[Mapping[str, Any], str]) -> str:
    return stamped if isinstance(stamped, str) else json.dumps(stamped)


# ── scoring ──────────────────────────────────────────────────────────────

def parse_half_life(value: str) -> float:
    """Seconds in a half-life such as "30d" or "6h"; math.inf for "infinite"."""
    seconds = _fid.parse_half_life(value)
    return math.inf if seconds is None else seconds


def source_fidelity(source_class: str) -> Optional[float]:
    """Default fidelity of a source class; None for "unknown"."""
    return _fid.source_fidelity(source_class)


def fidelity(
    source: float,
    age: Duration,
    half_life: Optional[Duration] = None,
    *,
    noise: Sequence[float] = (),
    baseline: float = 0.0,
    echo: Optional[Mapping[str, Any]] = None,
    fast: Optional[Mapping[str, Any]] = None,
) -> float:
    """Fidelity of a fact `age` after measurement. half_life None = no decay.

    `echo` is a periodic return, {"period", "strength", "width", "fade"},
    as in a registry: e.g. {"period": "24h", "strength": 0.31, "width": "3.75h", "fade": "345h"}.
    `fast` is a share that goes stale on a faster clock: e.g. {"share": 0.3, "half_life": "3d"}.
    """
    return _fid.fidelity(source, _age(age), _hl(half_life), list(noise), baseline, _echo(echo), _fast(fast))


def time_to_floor(
    fidelity: float,
    half_life: Optional[Duration],
    floor: float,
    *,
    baseline: float = 0.0,
    echo: Optional[Mapping[str, Any]] = None,
    fast: Optional[Mapping[str, Any]] = None,
) -> timedelta | None:
    """Time after measurement until fidelity first falls below floor; None if it never does.

    This is the TTL a store can enforce as a hard cutoff, derived from
    measured decay: half_life × log2((fidelity − b) / (floor − b)). With an
    echo the fact comes back later; this is still the first crossing.
    """
    seconds = _fid.time_to_floor(fidelity, _hl(half_life), baseline, floor, _echo(echo), _fast(fast))
    return None if seconds is None else timedelta(seconds=seconds)


def recheck_interval(
    fidelity: float,
    half_life: Optional[Duration],
    *,
    baseline: float = 0.0,
    price: Optional[float] = None,
    staleness: Optional[float] = None,
    echo: Optional[Mapping[str, Any]] = None,
    fast: Optional[Mapping[str, Any]] = None,
) -> timedelta | None:
    """How often to re-check a fact; None for never.

    With `price` (what one more re-check a day is worth, in staleness): the
    interval that minimizes mean staleness + price × re-checks per day. One
    price across every kind of fact spends a re-check budget where it
    removes the most staleness. With `staleness`: the longest interval that
    keeps mean staleness at or below it.
    """
    seconds = _fid.recheck_interval(fidelity, _hl(half_life), baseline, price, staleness, _echo(echo), _fast(fast))
    return None if seconds is None else timedelta(seconds=seconds)


def mean_staleness(
    fidelity: float,
    half_life: Optional[Duration],
    every: Duration,
    *,
    baseline: float = 0.0,
    echo: Optional[Mapping[str, Any]] = None,
    fast: Optional[Mapping[str, Any]] = None,
) -> float:
    """Fraction of the time a fact re-checked every `every` is served wrong."""
    return _fid.mean_staleness(fidelity, _hl(half_life), baseline, _age(every), _echo(echo), _fast(fast))


# ── envelopes ────────────────────────────────────────────────────────────

def stamp(
    content: Any,
    *,
    source: str,
    source_class: Optional[str] = None,
    noise: float = 0.0,
    half_life: Optional[Duration] = None,
    baseline: Optional[float] = None,
    source_fidelity: Optional[float] = None,
    echo: Optional[Mapping[str, Any]] = None,
    fast: Optional[Mapping[str, Any]] = None,
    type: Optional[str] = None,
    state: Optional[str] = None,
    condition: Optional[str] = None,
    derived_from: Optional[str] = None,
    upstream: Optional[Mapping[str, Any]] = None,
    now: Optional[datetime] = None,
) -> dict:
    """Wrap content in a fidelity envelope: {"fid": {...}, "content": ...}.

    Stamping an already-stamped artifact appends a transform step to its
    chain, as in a pipeline. `upstream` is a stamped artifact this one was
    derived from. `source_fidelity` is a measured fraction correct when
    fresh (see `registry`); it replaces the source class constant.
    """
    raw = content if isinstance(content, str) and _looks_stamped(content) else json.dumps(content)
    result, notes = _fid.stamp(
        raw, source, source_class, float(noise), _half_life_str(half_life), baseline, source_fidelity, _echo(echo),
        type, state, condition, derived_from, None if upstream is None else _json(upstream), _now(now), _fast(fast),
    )
    for note in notes:
        warnings.warn(note, stacklevel=2)
    return json.loads(result)


def _looks_stamped(text: str) -> bool:
    try:
        value = json.loads(text)
    except ValueError:
        return False
    return isinstance(value, dict) and "fid" in value


def check(stamped: Union[Mapping[str, Any], str], floor: float = 0.5, *, now: Optional[datetime] = None) -> Assessment:
    """Assess a stamped artifact against a floor."""
    r = json.loads(_fid.check(_json(stamped), floor, _now(now)))
    return Assessment(
        status=Status[r["status"]],
        fidelity=r["fidelity"],
        source=r["source"],
        temporal=r["temporal"],
        transforms=r["transforms"],
        baseline=r["baseline"],
        floor=r["floor"],
        integrity_ok=r["integrity_ok"],
        age=timedelta(seconds=r["age_seconds"]),
    )


def expires_at(stamped: Union[Mapping[str, Any], str], floor: float) -> Optional[datetime]:
    """When the artifact falls below floor; None if it never does.

    Raises IndeterminateError when the source class is unknown.
    """
    value = _fid.expires_at(_json(stamped), floor)
    if value == "never":
        return None
    if value == "unknown":
        raise IndeterminateError("source fidelity indeterminate; no expiry")
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def recheck_at(
    stamped: Union[Mapping[str, Any], str],
    *,
    price: Optional[float] = None,
    staleness: Optional[float] = None,
    floor: Optional[float] = None,
) -> Optional[datetime]:
    """When to re-check a stamped artifact (see `recheck_interval`), never later
    than its time to `floor` when one is given; None for never.

    Raises IndeterminateError when the source class is unknown.
    """
    value = _fid.recheck_at(_json(stamped), price, staleness, floor)
    if value == "never":
        return None
    if value == "unknown":
        raise IndeterminateError("source fidelity indeterminate; no schedule")
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def chain(stamped: Union[Mapping[str, Any], str], floor: float = 0.5, *, now: Optional[datetime] = None) -> str:
    """The `fid chain` report: each step's contribution to fidelity."""
    return _fid.chain(_json(stamped), floor, _now(now))


# ── registries ───────────────────────────────────────────────────────────

def half_life(registry_path: str, type: str, state: Optional[str] = None) -> tuple[str, Optional[float]]:
    """(half_life, baseline) for a type and state from a half-life registry file."""
    return _fid.half_lives_lookup(registry_path, type, state)[:2]


def registry(registry_path: str, type: str, state: Optional[str] = None) -> dict:
    """Everything a half-life registry holds for a type and state, ready to pass to `stamp`:
    {"half_life", "baseline", "source_fidelity", "echo", "fast"}; absent values are None."""
    h, b, s, e, f = _fid.half_lives_lookup(registry_path, type, state)
    return {"half_life": h, "baseline": b, "source_fidelity": s, "echo": None if e is None else json.loads(e),
            "fast": None if f is None else json.loads(f)}


def noise(model_path: str, transform: str, condition: Optional[str] = None) -> float:
    """Noise for a transform, optionally given an upstream condition, from a noise model file."""
    return _fid.noise_lookup(model_path, transform, condition)


# ── calibration ──────────────────────────────────────────────────────────

def _rows(rows: Iterable[Any]) -> tuple:
    entities, ages, correct, states = [], [], [], []
    for row in rows:
        if isinstance(row, Mapping):
            entity, age, ok, state = row["entity"], row["age"], row["correct"], row.get("state")
        else:
            entity, age, ok, *rest = row
            state = rest[0] if rest else None
        entities.append(str(entity))
        ages.append(_age(age))
        correct.append(bool(ok))
        states.append(None if state in (None, "") else str(state))
    return entities, ages, correct, states


def calibrate(
    rows: Iterable[Any], type: str, *, max_spread: float = _fid.MAX_SPREAD, period: Optional[Duration] = None
) -> Calibration:
    """Fit half-life and baseline from facts that were checked again later.

    Each row is a mapping or tuple: (entity, age, correct[, state]).
    `age` is seconds, a duration string ("30d"), or a timedelta.
    Raises ValueError when the half-life's 95% range spans more than
    `max_spread` (high end ÷ low end); pass math.inf to accept any range.
    `period` ("24h") also tests for a fact that comes back every period;
    the echo is written only if it beats plain decay on held-out data.
    """
    yaml, report, fits = _fid.calibrate(*_rows(rows), type, float(max_spread), _seconds(period))
    return Calibration(yaml=yaml, report=report, fits=json.loads(fits))


def drift(rows: Iterable[Any], type: str, registry_path: str, *, max_spread: float = _fid.MAX_SPREAD) -> Drift:
    """Test a half-life registry against recent re-checks (rows as for `calibrate`).

    `verdict` is CURRENT, DRIFTED (recalibrate) or UNKNOWN (too few recent
    checks to tell); the CLI's exit codes 0 / 1 / 2.
    """
    verdict, report, states = _fid.drift(*_rows(rows), type, registry_path, float(max_spread))
    return Drift(verdict=verdict, report=report, states=json.loads(states))
