//! Native layer of the fid Python package (`fid._fid`).
//!
//! Every function calls fid's Rust library; nothing is reimplemented.
//! Arguments and results are plain values and JSON strings; the Python
//! layer in python/fid/__init__.py turns them into dicts, datetimes and
//! dataclasses.

use chrono::{DateTime, Utc};
use fid::calibrate::{self as cal, Observation};
use fid::compute::{self, TimeToFloor};
use fid::envelope::{Expiry, FidEnvelope, FidStatus, SourceClass, Stamped};
use fid::registry::{HalfLives, NoiseModel};
use fid::stamp::{stamp as stamp_content, StampOptions};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde_json::json;

fn err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

fn when(now: Option<&str>) -> PyResult<DateTime<Utc>> {
    match now {
        None => Ok(Utc::now()),
        Some(s) => DateTime::parse_from_rfc3339(s)
            .map(|t| t.with_timezone(&Utc))
            .map_err(|_| err(format!("invalid time '{}' (RFC 3339)", s))),
    }
}

fn read_stamped(stamped_json: &str) -> PyResult<Stamped> {
    serde_json::from_str(stamped_json).map_err(|e| err(format!("not a valid fid-stamped artifact: {}", e)))
}

fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(v)
}

/// Seconds in a half-life string; None for "infinite".
#[pyfunction]
fn parse_half_life(s: &str) -> PyResult<Option<f64>> {
    compute::parse_half_life(s).map_err(err)
}

/// Seconds in a duration ("30d", "0d", bare seconds). Zero allowed.
#[pyfunction]
fn parse_duration(s: &str) -> PyResult<f64> {
    compute::parse_duration(s).map_err(err)
}

/// Default fidelity of a source class; None for "unknown".
#[pyfunction]
fn source_fidelity(source_class: &str) -> PyResult<Option<f64>> {
    let class: SourceClass = source_class.parse().map_err(err)?;
    Ok(class.default_fidelity())
}

/// An echo from its JSON ({"period", "strength", "width", "fade"}), validated.
fn echo_params(echo_json: Option<&str>, half_life_seconds: Option<f64>) -> PyResult<Option<compute::EchoParams>> {
    let Some(text) = echo_json else { return Ok(None) };
    let echo: compute::Echo = serde_json::from_str(text).map_err(|e| err(format!("invalid echo: {}", e)))?;
    let params = echo.params().map_err(err)?;
    if half_life_seconds.is_none() {
        return Err(err("an echo needs a finite half-life"));
    }
    Ok(Some(params))
}

/// The whole curve: half-life in seconds, fast share and echo as JSON, validated.
fn curve_of(half_life_seconds: Option<f64>, fast_json: Option<&str>, echo_json: Option<&str>) -> PyResult<compute::Curve> {
    let echo = echo_params(echo_json, half_life_seconds)?;
    let fast = match fast_json {
        None => None,
        Some(text) => {
            let fast: compute::Fast = serde_json::from_str(text).map_err(|e| err(format!("invalid fast share: {}", e)))?;
            let params = fast.params().map_err(err)?;
            match half_life_seconds {
                None => return Err(err("a fast share needs a finite half-life")),
                Some(h) if params.half_life >= h => return Err(err("fast half-life must be shorter than the half-life")),
                _ => Some(params),
            }
        }
    };
    Ok(compute::Curve { half_life: half_life_seconds, fast, echo })
}

/// baseline + (source × ∏(1 − noise) − baseline) × temporal, temporal with an optional fast share and echo
#[pyfunction]
#[pyo3(signature = (source, age_seconds, half_life_seconds, noise, baseline, echo_json=None, fast_json=None))]
fn fidelity(
    source: f64,
    age_seconds: f64,
    half_life_seconds: Option<f64>,
    noise: Vec<f64>,
    baseline: f64,
    echo_json: Option<&str>,
    fast_json: Option<&str>,
) -> PyResult<f64> {
    let curve = curve_of(half_life_seconds, fast_json, echo_json)?;
    Ok(compute::apply_decay(source * compute::transform_factor(&noise), curve.temporal(age_seconds), baseline))
}

/// Seconds after measurement until fidelity first falls below floor; None for never.
#[pyfunction]
#[pyo3(signature = (fidelity, half_life_seconds, baseline, floor, echo_json=None, fast_json=None))]
fn time_to_floor(
    fidelity: f64,
    half_life_seconds: Option<f64>,
    baseline: f64,
    floor: f64,
    echo_json: Option<&str>,
    fast_json: Option<&str>,
) -> PyResult<Option<f64>> {
    let ttl = curve_of(half_life_seconds, fast_json, echo_json)?.time_to_floor(fidelity, baseline, floor);
    Ok(match ttl {
        TimeToFloor::After(s) => Some(s),
        TimeToFloor::Never => None,
    })
}

/// Seconds between re-checks that minimize staleness + price × re-checks per day (or, with a
/// staleness target, the longest interval that keeps it); None for never.
#[pyfunction]
#[pyo3(signature = (fidelity, half_life_seconds, baseline, price=None, staleness=None, echo_json=None, fast_json=None))]
#[allow(clippy::too_many_arguments)]
fn recheck_interval(
    fidelity: f64,
    half_life_seconds: Option<f64>,
    baseline: f64,
    price: Option<f64>,
    staleness: Option<f64>,
    echo_json: Option<&str>,
    fast_json: Option<&str>,
) -> PyResult<Option<f64>> {
    let curve = curve_of(half_life_seconds, fast_json, echo_json)?;
    let r = match (price, staleness) {
        (Some(p), None) => compute::best_interval(&curve, fidelity, baseline, p),
        (None, Some(s)) => compute::interval_for_staleness(&curve, fidelity, baseline, s),
        _ => return Err(err("give price or staleness")),
    }
    .map_err(err)?;
    Ok(match r {
        compute::Recheck::Every(s) => Some(s),
        compute::Recheck::Never => None,
    })
}

/// Mean fraction of the time a fact re-checked every `interval_seconds` is served wrong.
#[pyfunction]
#[pyo3(signature = (fidelity, half_life_seconds, baseline, interval_seconds, echo_json=None, fast_json=None))]
fn mean_staleness(
    fidelity: f64,
    half_life_seconds: Option<f64>,
    baseline: f64,
    interval_seconds: f64,
    echo_json: Option<&str>,
    fast_json: Option<&str>,
) -> PyResult<f64> {
    Ok(curve_of(half_life_seconds, fast_json, echo_json)?.mean_staleness(fidelity, baseline, interval_seconds))
}

/// Wrap content (raw stdin-style text) in an envelope. Returns (stamped JSON, warnings).
#[pyfunction]
#[pyo3(signature = (input, source, source_class, noise, half_life, baseline, source_fidelity, echo_json, info_type, state, condition, derived_from, upstream_json, now, fast_json=None))]
#[allow(clippy::too_many_arguments)]
fn stamp(
    input: &str,
    source: String,
    source_class: Option<&str>,
    noise: f64,
    half_life: Option<String>,
    baseline: Option<f64>,
    source_fidelity: Option<f64>,
    echo_json: Option<&str>,
    info_type: Option<String>,
    state: Option<String>,
    condition: Option<String>,
    derived_from: Option<String>,
    upstream_json: Option<&str>,
    now: Option<&str>,
    fast_json: Option<&str>,
) -> PyResult<(String, Vec<String>)> {
    let source_class = source_class.map(str::parse::<SourceClass>).transpose().map_err(err)?;
    let upstream: Option<FidEnvelope> = upstream_json.map(read_stamped).transpose()?.map(|s| s.fid);
    let echo = echo_json.map(serde_json::from_str::<compute::Echo>).transpose().map_err(|e| err(format!("invalid echo: {}", e)))?;
    let fast = fast_json.map(serde_json::from_str::<compute::Fast>).transpose().map_err(|e| err(format!("invalid fast share: {}", e)))?;
    let opts = StampOptions { source, source_class, noise, half_life, baseline, source_fidelity, echo, fast, info_type, state, condition, derived_from };
    let out = stamp_content(input, &opts, upstream, when(now)?).map_err(err)?;
    let json = serde_json::to_string(&out.stamped).map_err(err)?;
    Ok((json, out.warnings))
}

/// Assess a stamped artifact against a floor. Returns JSON.
#[pyfunction]
#[pyo3(signature = (stamped_json, floor, now))]
fn check(stamped_json: &str, floor: f64, now: Option<&str>) -> PyResult<String> {
    let stamped = read_stamped(stamped_json)?;
    let r = fid::check::assess(&stamped, floor, when(now)?).map_err(err)?;
    let a = &r.assessment;
    let status = match a.status {
        FidStatus::Signal => "SIGNAL",
        FidStatus::Noise => "NOISE",
        FidStatus::Unknown => "UNKNOWN",
    };
    Ok(json!({
        "status": status,
        "fidelity": a.fidelity,
        "source": a.source,
        "temporal": a.temporal,
        "transforms": a.transforms,
        "baseline": a.baseline,
        "floor": a.floor,
        "integrity_ok": r.integrity_ok,
        "age_seconds": r.age_seconds,
    })
    .to_string())
}

/// When a stamped artifact falls below floor: RFC 3339, "never" or "unknown".
#[pyfunction]
fn expires_at(stamped_json: &str, floor: f64) -> PyResult<String> {
    let stamped = read_stamped(stamped_json)?;
    Ok(match stamped.fid.expires_at(floor).map_err(err)? {
        None => "unknown".to_string(),
        Some(Expiry::Never) => "never".to_string(),
        Some(Expiry::At(t)) => t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    })
}

/// When to re-check a stamped artifact: RFC 3339, "never" or "unknown".
#[pyfunction]
#[pyo3(signature = (stamped_json, price=None, staleness=None, floor=None))]
fn recheck_at(stamped_json: &str, price: Option<f64>, staleness: Option<f64>, floor: Option<f64>) -> PyResult<String> {
    let stamped = read_stamped(stamped_json)?;
    let at = match (price, staleness) {
        (Some(p), None) => stamped.fid.recheck_at(p, floor),
        (None, Some(s)) => stamped.fid.recheck_for_staleness(s, floor),
        _ => return Err(err("give price or staleness")),
    }
    .map_err(err)?;
    Ok(match at {
        None => "unknown".to_string(),
        Some(Expiry::Never) => "never".to_string(),
        Some(Expiry::At(t)) => t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    })
}

/// The `fid chain` report.
#[pyfunction]
#[pyo3(signature = (stamped_json, floor, now))]
fn chain(stamped_json: &str, floor: f64, now: Option<&str>) -> PyResult<String> {
    let stamped = read_stamped(stamped_json)?;
    let r = fid::check::assess(&stamped, floor, when(now)?).map_err(err)?;
    Ok(fid::chain::report(&stamped, &r))
}

/// (half_life, baseline, source_fidelity, echo JSON, fast JSON)
type RegistryEntry = (String, Option<f64>, Option<f64>, Option<String>, Option<String>);

/// Look up a half-life registry file.
#[pyfunction]
#[pyo3(signature = (path, info_type, state))]
fn half_lives_lookup(path: &str, info_type: &str, state: Option<&str>) -> PyResult<RegistryEntry> {
    let spec = HalfLives::load(path).map_err(err)?.lookup(info_type, state).map_err(err)?;
    let echo = spec.echo.map(|e| serde_json::to_string(&e)).transpose().map_err(err)?;
    let fast = spec.fast.map(|f| serde_json::to_string(&f)).transpose().map_err(err)?;
    Ok((spec.half_life, spec.baseline, spec.source_fidelity, echo, fast))
}

/// Look up a noise model file.
#[pyfunction]
#[pyo3(signature = (path, transform, condition))]
fn noise_lookup(path: &str, transform: &str, condition: Option<&str>) -> PyResult<f64> {
    NoiseModel::load(path).map_err(err)?.lookup(transform, condition).map_err(err)
}

fn observations(
    entities: Vec<String>,
    ages: Vec<f64>,
    correct: Vec<bool>,
    states: Vec<Option<String>>,
) -> PyResult<Vec<Observation>> {
    let n = entities.len();
    if ages.len() != n || correct.len() != n || states.len() != n {
        return Err(err("entities, ages, correct and states must have the same length"));
    }
    let mut obs = Vec::with_capacity(n);
    for i in 0..n {
        if !(ages[i].is_finite() && ages[i] >= 0.0) {
            return Err(err(format!("row {}: invalid age {}", i, ages[i])));
        }
        obs.push(Observation { entity: entities[i].clone(), age: ages[i], correct: correct[i], state: states[i].clone() });
    }
    Ok(obs)
}

/// Fit decay. Returns (registry YAML, report, fits JSON).
#[pyfunction]
#[pyo3(signature = (entities, ages, correct, states, info_type, max_spread, period_seconds=None))]
fn calibrate(
    entities: Vec<String>,
    ages: Vec<f64>,
    correct: Vec<bool>,
    states: Vec<Option<String>>,
    info_type: &str,
    max_spread: f64,
    period_seconds: Option<f64>,
) -> PyResult<(String, String, String)> {
    let obs = observations(entities, ages, correct, states)?;
    let fits = cal::calibrate_periodic(&obs, max_spread, period_seconds).map_err(err)?;
    let rows: Vec<_> = fits
        .iter()
        .map(|f| {
            json!({
                "state": f.state,
                "observations": f.observations,
                "entities": f.entities,
                "half_life_seconds": finite(f.model.half_life),
                "half_life": cal::format_half_life(f.model.half_life),
                "baseline": f.model.baseline,
                "uses_baseline": f.uses_baseline,
                "correct_when_fresh": f.model.s0,
                "fresh_checks": f.fresh_checks,
                "source_fidelity": f.source_fidelity,
                "half_life_range_seconds": [finite(f.uncertainty.half_life.lo), finite(f.uncertainty.half_life.hi)],
                "baseline_range": [f.uncertainty.baseline.lo, f.uncertainty.baseline.hi],
                "correct_when_fresh_range": [f.uncertainty.s0.lo, f.uncertainty.s0.hi],
                "half_life_spread": finite(f.spread),
                "echo": f.echo.as_ref().map(|e| json!({
                    "kept": e.kept,
                    "period_seconds": e.model.echo.period,
                    "strength": e.model.echo.strength,
                    "width_seconds": e.model.echo.width,
                    "fade_seconds": e.model.echo.fade,
                    "half_life_seconds": e.model.half_life,
                    "baseline": e.model.baseline,
                    "brier_echo": e.brier_echo,
                    "brier_decay": e.brier_decay,
                })),
                "fast": json!({
                    "kept": f.fast.kept,
                    "share": f.fast.model.share,
                    "fast_half_life_seconds": f.fast.model.fast_half_life,
                    "half_life_seconds": f.fast.model.half_life,
                    "baseline": f.fast.model.baseline,
                    "correct_when_fresh": f.fast.model.s0,
                    "brier_fast": f.fast.brier_fast,
                    "brier_one_clock": f.fast.brier_plain,
                }),
                "shape_error": f.shape_error,
                "empirical_half_life_seconds": f.empirical_half_life,
                "brier_baseline": f.brier_baseline,
                "brier_zero": f.brier_zero,
                "brier_constant": f.brier_constant,
            })
        })
        .collect();
    Ok((
        cal::registry_yaml(info_type, &fits),
        cal::report(info_type, &fits),
        serde_json::Value::from(rows).to_string(),
    ))
}

/// Test a registry against recent re-checks. Returns (verdict, report, states JSON).
#[pyfunction]
#[pyo3(signature = (entities, ages, correct, states, info_type, registry_path, max_spread))]
#[allow(clippy::too_many_arguments)]
fn drift(
    entities: Vec<String>,
    ages: Vec<f64>,
    correct: Vec<bool>,
    states: Vec<Option<String>>,
    info_type: &str,
    registry_path: &str,
    max_spread: f64,
) -> PyResult<(String, String, String)> {
    let obs = observations(entities, ages, correct, states)?;
    let registry = HalfLives::load(registry_path).map_err(err)?;
    let drifts = fid::drift::drift(&obs, &registry, info_type, max_spread).map_err(err)?;
    let rows: Vec<_> = drifts
        .iter()
        .map(|d| {
            json!({
                "state": d.recent.state,
                "verdict": format!("{:?}", d.verdict).to_uppercase(),
                "p_value": d.p_value,
                "chi2": d.chi2,
                "df": d.df,
                "design_effect": d.dispersion,
                "registry_half_life_seconds": finite(d.registered.half_life),
                "recent_half_life_seconds": finite(d.recent.model.half_life),
                "recent_half_life_range_seconds": [finite(d.recent.uncertainty.half_life.lo), finite(d.recent.uncertainty.half_life.hi)],
                "registry_baseline": d.registered.baseline,
                "recent_baseline": d.recent.model.baseline,
                "registry_gap": d.registry_gap,
                "refit_gap": d.refit_gap,
            })
        })
        .collect();
    Ok((
        format!("{:?}", fid::drift::overall(&drifts)).to_uppercase(),
        fid::drift::report(info_type, &drifts, max_spread),
        serde_json::Value::from(rows).to_string(),
    ))
}

#[pymodule]
fn _fid(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_half_life, m)?)?;
    m.add_function(wrap_pyfunction!(parse_duration, m)?)?;
    m.add_function(wrap_pyfunction!(source_fidelity, m)?)?;
    m.add_function(wrap_pyfunction!(fidelity, m)?)?;
    m.add_function(wrap_pyfunction!(time_to_floor, m)?)?;
    m.add_function(wrap_pyfunction!(stamp, m)?)?;
    m.add_function(wrap_pyfunction!(check, m)?)?;
    m.add_function(wrap_pyfunction!(expires_at, m)?)?;
    m.add_function(wrap_pyfunction!(recheck_at, m)?)?;
    m.add_function(wrap_pyfunction!(recheck_interval, m)?)?;
    m.add_function(wrap_pyfunction!(mean_staleness, m)?)?;
    m.add_function(wrap_pyfunction!(chain, m)?)?;
    m.add_function(wrap_pyfunction!(half_lives_lookup, m)?)?;
    m.add_function(wrap_pyfunction!(noise_lookup, m)?)?;
    m.add_function(wrap_pyfunction!(calibrate, m)?)?;
    m.add_function(wrap_pyfunction!(drift, m)?)?;
    m.add("FID_VERSION", fid::envelope::FID_VERSION)?;
    m.add("MAX_SPREAD", fid::calibrate::MAX_SPREAD)?;
    Ok(())
}
