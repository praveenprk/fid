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

/// baseline + (source × ∏(1 − noise) − baseline) × 0.5^(age / half_life)
#[pyfunction]
#[pyo3(signature = (source, age_seconds, half_life_seconds, noise, baseline))]
fn fidelity(source: f64, age_seconds: f64, half_life_seconds: Option<f64>, noise: Vec<f64>, baseline: f64) -> f64 {
    compute::total_fidelity(Some(source), age_seconds, half_life_seconds, &noise, baseline).unwrap_or(0.0)
}

/// Seconds after measurement until fidelity falls below floor; None for never.
#[pyfunction]
#[pyo3(signature = (fidelity, half_life_seconds, baseline, floor))]
fn time_to_floor(fidelity: f64, half_life_seconds: Option<f64>, baseline: f64, floor: f64) -> Option<f64> {
    match compute::time_to_floor(fidelity, half_life_seconds, baseline, floor) {
        TimeToFloor::After(s) => Some(s),
        TimeToFloor::Never => None,
    }
}

/// Wrap content (raw stdin-style text) in an envelope. Returns (stamped JSON, warnings).
#[pyfunction]
#[pyo3(signature = (input, source, source_class, noise, half_life, baseline, source_fidelity, info_type, state, condition, derived_from, upstream_json, now))]
#[allow(clippy::too_many_arguments)]
fn stamp(
    input: &str,
    source: String,
    source_class: Option<&str>,
    noise: f64,
    half_life: Option<String>,
    baseline: Option<f64>,
    source_fidelity: Option<f64>,
    info_type: Option<String>,
    state: Option<String>,
    condition: Option<String>,
    derived_from: Option<String>,
    upstream_json: Option<&str>,
    now: Option<&str>,
) -> PyResult<(String, Vec<String>)> {
    let source_class = source_class.map(str::parse::<SourceClass>).transpose().map_err(err)?;
    let upstream: Option<FidEnvelope> = upstream_json.map(read_stamped).transpose()?.map(|s| s.fid);
    let opts = StampOptions { source, source_class, noise, half_life, baseline, source_fidelity, info_type, state, condition, derived_from };
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

/// The `fid chain` report.
#[pyfunction]
#[pyo3(signature = (stamped_json, floor, now))]
fn chain(stamped_json: &str, floor: f64, now: Option<&str>) -> PyResult<String> {
    let stamped = read_stamped(stamped_json)?;
    let r = fid::check::assess(&stamped, floor, when(now)?).map_err(err)?;
    Ok(fid::chain::report(&stamped, &r))
}

/// Look up a half-life registry file: (half_life, baseline, source_fidelity).
#[pyfunction]
#[pyo3(signature = (path, info_type, state))]
fn half_lives_lookup(path: &str, info_type: &str, state: Option<&str>) -> PyResult<(String, Option<f64>, Option<f64>)> {
    let spec = HalfLives::load(path).map_err(err)?.lookup(info_type, state).map_err(err)?;
    Ok((spec.half_life, spec.baseline, spec.source_fidelity))
}

/// Look up a noise model file.
#[pyfunction]
#[pyo3(signature = (path, transform, condition))]
fn noise_lookup(path: &str, transform: &str, condition: Option<&str>) -> PyResult<f64> {
    NoiseModel::load(path).map_err(err)?.lookup(transform, condition).map_err(err)
}

/// Fit decay. Returns (registry YAML, report, fits JSON).
#[pyfunction]
#[pyo3(signature = (entities, ages, correct, states, info_type, max_spread))]
fn calibrate(
    entities: Vec<String>,
    ages: Vec<f64>,
    correct: Vec<bool>,
    states: Vec<Option<String>>,
    info_type: &str,
    max_spread: f64,
) -> PyResult<(String, String, String)> {
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
    let fits = cal::calibrate_with(&obs, max_spread).map_err(err)?;
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
    m.add_function(wrap_pyfunction!(chain, m)?)?;
    m.add_function(wrap_pyfunction!(half_lives_lookup, m)?)?;
    m.add_function(wrap_pyfunction!(noise_lookup, m)?)?;
    m.add_function(wrap_pyfunction!(calibrate, m)?)?;
    m.add("FID_VERSION", fid::envelope::FID_VERSION)?;
    m.add("MAX_SPREAD", fid::calibrate::MAX_SPREAD)?;
    Ok(())
}
