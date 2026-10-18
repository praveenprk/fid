//! Fidelity computation.
//!
//! Three axes, each independent:
//!   f_source    — initial fidelity from source class
//!   f_temporal  — decay as a function of age and half-life
//!   f_transform — cumulative noise from transformation chain
//!
//! Total: fidelity = f_source × f_temporal × f_transform
//!
//! With a baseline b — the chance a stale fact is right anyway — temporal
//! decay goes toward b instead of toward zero:
//!
//!   fidelity = b + (f_source × f_transform − b) × f_temporal
//!
//! Measured against public data, most facts decay toward a baseline
//! (see calibration/README.md). b = 0 recovers the original model.
//!
//! Without an echo, it never spontaneously increases: new measurement is
//! the only restoration mechanism. A periodic fact (temperature, traffic,
//! load) comes back once per period instead, and its echo adds
//!
//!   r × Σₙ≥₁ 0.5^(nP / h_r) × 0.5^(|age − nP| / w)
//!
//! to the temporal factor: strength r, period P, width w, fading with
//! half-life h_r over periods. The temporal factor stays at most 1.
//! Measured on airport temperature, see docs/FINDINGS.md, Finding 5.

use serde::{Deserialize, Serialize};

/// Parse a half-life string (e.g. "30s", "6h", "7d", "3mo", "3y") into seconds.
///
/// Returns Ok(None) for "infinite" (zero temporal decay).
/// Unparseable or non-positive values are an error — never silently
/// treated as infinite, which would hide decay.
pub fn parse_half_life(s: &str) -> Result<Option<f64>, String> {
    let t = s.trim().to_lowercase();
    if t == "infinite" || t == "inf" {
        return Ok(None);
    }
    let seconds = parse_duration(&t).map_err(|_| {
        format!("invalid half-life '{}' (expected e.g. 30s, 5m, 6h, 7d, 3mo, 3y, infinite)", t)
    })?;
    if seconds <= 0.0 {
        return Err(format!("invalid half-life '{}' (must be a positive duration)", t));
    }
    Ok(Some(seconds))
}

/// Parse a duration ("30s", "5m", "6h", "7d", "3mo", "3y", or bare seconds)
/// into seconds. Zero is allowed; negative, non-finite or unparseable is an error.
pub fn parse_duration(s: &str) -> Result<f64, String> {
    let s = s.trim().to_lowercase();
    let (num_str, multiplier) = if let Some(n) = s.strip_suffix("mo") {
        (n, 30.44 * 86400.0)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1.0)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60.0)
    } else if let Some(n) = s.strip_suffix('h') {
        (n, 3600.0)
    } else if let Some(n) = s.strip_suffix('d') {
        (n, 86400.0)
    } else if let Some(n) = s.strip_suffix('y') {
        (n, 365.25 * 86400.0)
    } else {
        // Assume seconds
        (s.as_str(), 1.0)
    };
    let n: f64 = num_str
        .parse()
        .map_err(|_| format!("invalid duration '{}' (expected e.g. 30s, 5m, 6h, 7d, 3mo, 3y)", s))?;
    if !n.is_finite() || n < 0.0 {
        return Err(format!("invalid duration '{}' (must be zero or more)", s));
    }
    Ok(n * multiplier)
}

/// Compute temporal fidelity decay.
///
/// f_temporal(t) = (1/2)^(t / t_half)
///
/// Where t is the age in seconds and t_half is the half-life in seconds.
/// If half-life is None (infinite), temporal fidelity is 1.0.
pub fn temporal_decay(age_seconds: f64, half_life_seconds: Option<f64>) -> f64 {
    match half_life_seconds {
        None => 1.0, // infinite half-life, no decay
        Some(hl) if hl <= 0.0 => 0.0, // degenerate case
        Some(hl) => {
            // Clock skew can make age negative; fidelity never increases.
            let exponent = age_seconds.max(0.0) / hl;
            0.5_f64.powf(exponent)
        }
    }
}

/// A periodic fact's echo, as written in registries and envelopes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Echo {
    /// Time between returns, e.g. "24h"
    pub period: String,
    /// Height of the first return, before fading (r)
    pub strength: f64,
    /// Half-width of each return, e.g. "3.75h"; less than half the period
    pub width: String,
    /// Half-life of the returns over periods, e.g. "345h", or "infinite"
    pub fade: String,
}

/// An echo in seconds, validated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EchoParams {
    pub period: f64,
    pub strength: f64,
    pub width: f64,
    /// None: the returns never fade
    pub fade: Option<f64>,
}

impl Echo {
    pub fn params(&self) -> Result<EchoParams, String> {
        let duration = |name: &str, text: &str| {
            parse_half_life(text)?.ok_or_else(|| format!("echo {} cannot be infinite", name))
        };
        let period = duration("period", &self.period)?;
        let width = duration("width", &self.width)?;
        let fade = parse_half_life(&self.fade)?;
        if !self.strength.is_finite() || self.strength < 0.0 {
            return Err(format!("invalid echo strength {} (must be zero or more)", self.strength));
        }
        if width >= period / 2.0 {
            return Err(format!("echo width {} must be less than half the period {}", self.width, self.period));
        }
        Ok(EchoParams { period, strength: self.strength, width, fade })
    }
}

/// Parse an envelope's or registry's decay: half-life in seconds, and the
/// echo if any. An echo needs a finite half-life.
pub fn decay_curve(half_life: Option<&str>, echo: Option<&Echo>) -> Result<(Option<f64>, Option<EchoParams>), String> {
    let h = match half_life {
        Some(hl) => parse_half_life(hl)?,
        None => None,
    };
    let e = echo.map(Echo::params).transpose()?;
    if e.is_some() && h.is_none() {
        return Err("an echo needs a finite half-life".to_string());
    }
    Ok((h, e))
}

/// Σₙ≥₁ 0.5^(nP / h_r) × 0.5^(|age − nP| / w), over returns within 30
/// widths of `age`; a farther one adds under 0.5^30 of its peak.
pub fn echo_sum(age_seconds: f64, e: &EchoParams) -> f64 {
    let t = age_seconds.max(0.0);
    let reach = 30.0 * e.width;
    let first = ((t - reach) / e.period).ceil().max(1.0) as u64;
    let last = ((t + reach) / e.period).floor().max(0.0) as u64;
    (first..=last)
        .map(|n| {
            let at = n as f64 * e.period;
            let fade = e.fade.map_or(1.0, |hr| 0.5_f64.powf(at / hr));
            fade * 0.5_f64.powf((t - at).abs() / e.width)
        })
        .sum()
}

/// The temporal factor with an optional echo, at most 1.
pub fn temporal_factor(age_seconds: f64, half_life_seconds: Option<f64>, echo: Option<&EchoParams>) -> f64 {
    let base = temporal_decay(age_seconds, half_life_seconds);
    match echo {
        None => base,
        Some(e) => (base + e.strength * echo_sum(age_seconds, e)).min(1.0),
    }
}

/// Compute transformation noise factor.
///
/// f_transform = ∏(1 - noise_i) for each transform step.
///
/// Each noise_i is clamped to [0.0, 1.0].
pub fn transform_factor(noise_values: &[f64]) -> f64 {
    noise_values
        .iter()
        .map(|n| 1.0 - n.clamp(0.0, 1.0))
        .product()
}

/// Compute total fidelity.
///
/// fidelity = b + (f_source × f_transform − b) × f_temporal
///
/// `baseline` is clamped to [0, f_source × f_transform]: decay can only
/// lower fidelity, never raise it toward a baseline above it.
///
/// Returns None if source fidelity is indeterminate (Unknown source class).
pub fn total_fidelity(
    source_fidelity: Option<f64>,
    age_seconds: f64,
    half_life_seconds: Option<f64>,
    noise_values: &[f64],
    baseline: f64,
) -> Option<f64> {
    let f_source = source_fidelity?;
    let f_temporal = temporal_decay(age_seconds, half_life_seconds);
    let f_transform = transform_factor(noise_values);
    Some(apply_decay(f_source * f_transform, f_temporal, baseline))
}

/// When fidelity falls below a floor, measured from the moment of measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeToFloor {
    /// Seconds after measurement (0 if it starts at or below the floor)
    After(f64),
    /// Never: no decay, or the baseline is at or above the floor
    Never,
}

/// Time for fidelity to fall from `fidelity` (at measurement) to `floor`.
///
/// Solves b + (f − b) × 0.5^(t / h) = floor:
///   t = h × log2((f − b) / (floor − b))
///
/// This is the TTL a store can enforce as a hard cutoff, derived from
/// measured decay instead of declared.
pub fn time_to_floor(fidelity: f64, half_life_seconds: Option<f64>, baseline: f64, floor: f64) -> TimeToFloor {
    if fidelity < floor {
        return TimeToFloor::After(0.0);
    }
    let b = baseline.clamp(0.0, fidelity.max(0.0));
    match half_life_seconds {
        None => TimeToFloor::Never,
        Some(_) if floor <= b => TimeToFloor::Never,
        Some(h) if h <= 0.0 => TimeToFloor::After(0.0),
        Some(h) => TimeToFloor::After(h * ((fidelity - b) / (floor - b)).log2()),
    }
}

/// First time fidelity falls below `floor`, with an echo.
///
/// The curve without the echo is never above the curve with it, so the
/// closed-form crossing without the echo is a lower bound; from there the
/// curve is scanned in steps of a fiftieth of the shorter of half-life
/// and echo width, and the crossing refined by bisection. Never, when it
/// stays at or above the floor for 10,000 periods.
pub fn time_to_floor_echo(fidelity: f64, half_life_seconds: f64, baseline: f64, floor: f64, echo: &EchoParams) -> TimeToFloor {
    let start = match time_to_floor(fidelity, Some(half_life_seconds), baseline, floor) {
        TimeToFloor::Never => return TimeToFloor::Never,
        TimeToFloor::After(t) if t == 0.0 && fidelity < floor => return TimeToFloor::After(0.0),
        TimeToFloor::After(t) => t,
    };
    let at = |t: f64| apply_decay(fidelity, temporal_factor(t, Some(half_life_seconds), Some(echo)), baseline);
    let step = half_life_seconds.min(echo.width) / 50.0;
    let end = start + 10_000.0 * echo.period;
    let (mut lo, mut t) = (start, start);
    while t <= end {
        if at(t) < floor {
            let mut hi = t;
            for _ in 0..100 {
                let mid = 0.5 * (lo + hi);
                if at(mid) < floor {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return TimeToFloor::After(hi);
        }
        lo = t;
        t += step;
    }
    TimeToFloor::Never
}

/// Decay `fidelity` by the temporal factor, toward `baseline` rather than 0.
pub fn apply_decay(fidelity: f64, temporal: f64, baseline: f64) -> f64 {
    let b = baseline.clamp(0.0, fidelity.max(0.0));
    b + (fidelity - b) * temporal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_half_life_seconds() {
        assert_eq!(parse_half_life("30s"), Ok(Some(30.0)));
    }

    #[test]
    fn test_parse_half_life_minutes() {
        assert_eq!(parse_half_life("5m"), Ok(Some(300.0)));
    }

    #[test]
    fn test_parse_half_life_months() {
        let result = parse_half_life("3mo").unwrap().unwrap();
        assert!((result - 3.0 * 30.44 * 86400.0).abs() < 1e-6);
    }

    #[test]
    fn test_parse_half_life_years() {
        assert_eq!(parse_half_life("1y"), Ok(Some(365.25 * 86400.0)));
    }

    #[test]
    fn test_parse_half_life_bare_number_is_seconds() {
        assert_eq!(parse_half_life("90"), Ok(Some(90.0)));
    }

    #[test]
    fn test_parse_half_life_invalid_is_error() {
        assert!(parse_half_life("30ms").is_err());
        assert!(parse_half_life("7days").is_err());
        assert!(parse_half_life("").is_err());
        assert!(parse_half_life("abc").is_err());
    }

    #[test]
    fn test_parse_half_life_non_positive_is_error() {
        assert!(parse_half_life("0d").is_err());
        assert!(parse_half_life("-5h").is_err());
        assert!(parse_half_life("nan").is_err());
        assert!(parse_half_life("infd").is_err());
    }

    #[test]
    fn test_parse_half_life_hours() {
        assert_eq!(parse_half_life("6h"), Ok(Some(21600.0)));
    }

    #[test]
    fn test_parse_half_life_days() {
        assert_eq!(parse_half_life("7d"), Ok(Some(604800.0)));
    }

    #[test]
    fn test_parse_half_life_infinite() {
        assert_eq!(parse_half_life("infinite"), Ok(None));
    }

    #[test]
    fn test_parse_duration_allows_zero() {
        assert_eq!(parse_duration("0d"), Ok(0.0));
        assert_eq!(parse_duration("2h"), Ok(7200.0));
        assert_eq!(parse_duration("90"), Ok(90.0));
        assert!(parse_duration("-1d").is_err());
        assert!(parse_duration("inf").is_err());
        assert!(parse_duration("7days").is_err());
    }

    #[test]
    fn test_temporal_decay_zero_age() {
        let result = temporal_decay(0.0, Some(86400.0));
        assert!((result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_temporal_decay_one_half_life() {
        let result = temporal_decay(86400.0, Some(86400.0));
        assert!((result - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_temporal_decay_two_half_lives() {
        let result = temporal_decay(172800.0, Some(86400.0));
        assert!((result - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_temporal_decay_infinite_half_life() {
        let result = temporal_decay(999999.0, None);
        assert!((result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_temporal_decay_negative_age_does_not_increase() {
        let result = temporal_decay(-86400.0, Some(86400.0));
        assert!((result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_factor_no_transforms() {
        let result = transform_factor(&[]);
        assert!((result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_factor_single() {
        let result = transform_factor(&[0.10]);
        assert!((result - 0.90).abs() < 1e-10);
    }

    #[test]
    fn test_transform_factor_chain() {
        let result = transform_factor(&[0.05, 0.08, 0.10]);
        let expected = 0.95 * 0.92 * 0.90;
        assert!((result - expected).abs() < 1e-10);
    }

    #[test]
    fn test_total_fidelity_basic() {
        // source: 0.90, age: 1 half-life, one transform at 0.10 noise
        let result = total_fidelity(Some(0.90), 86400.0, Some(86400.0), &[0.10], 0.0);
        let expected = 0.90 * 0.5 * 0.90;
        assert!((result.unwrap() - expected).abs() < 1e-10);
    }

    #[test]
    fn test_total_fidelity_unknown_source() {
        let result = total_fidelity(None, 0.0, Some(86400.0), &[], 0.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_total_fidelity_with_baseline() {
        // source 0.90, one half-life, baseline 0.70: 0.70 + 0.20 × 0.5
        let result = total_fidelity(Some(0.90), 86400.0, Some(86400.0), &[], 0.70);
        assert!((result.unwrap() - 0.80).abs() < 1e-10);
    }

    #[test]
    fn test_baseline_is_the_long_run_limit() {
        let result = total_fidelity(Some(0.97), 1e12, Some(60.0), &[], 0.73);
        assert!((result.unwrap() - 0.73).abs() < 1e-10);
    }

    #[test]
    fn test_baseline_above_fidelity_never_raises_it() {
        // fidelity already 0.25 after transforms; baseline 0.70 must not lift it
        let result = total_fidelity(Some(0.50), 86400.0, Some(86400.0), &[0.50], 0.70);
        assert!((result.unwrap() - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_time_to_floor_round_trips() {
        let h = 7.0 * 86400.0;
        match time_to_floor(0.97, Some(h), 0.2, 0.7) {
            TimeToFloor::After(t) => {
                let at = apply_decay(0.97, temporal_decay(t, Some(h)), 0.2);
                assert!((at - 0.7).abs() < 1e-9);
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn test_time_to_floor_one_half_life() {
        // 0.8 → 0.4 with no baseline takes exactly one half-life
        assert_eq!(time_to_floor(0.8, Some(100.0), 0.0, 0.4), TimeToFloor::After(100.0));
    }

    #[test]
    fn test_time_to_floor_never() {
        assert_eq!(time_to_floor(0.97, None, 0.0, 0.7), TimeToFloor::Never);
        // baseline 0.73 never falls below a 0.70 floor
        assert_eq!(time_to_floor(0.97, Some(3600.0), 0.73, 0.70), TimeToFloor::Never);
    }

    #[test]
    fn test_time_to_floor_already_below() {
        assert_eq!(time_to_floor(0.5, Some(3600.0), 0.0, 0.7), TimeToFloor::After(0.0));
    }

    const HOUR: f64 = 3600.0;

    /// Airport temperature, fitted on all of 2024 (calibration/results/weather_periodic.json)
    fn temperature() -> EchoParams {
        Echo { period: "24h".into(), strength: 0.31126325754867784, width: "3.748493227942606h".into(), fade: "345.03278429702254h".into() }
            .params()
            .unwrap()
    }

    #[test]
    fn test_echo_matches_the_study() {
        // calibration/weather_periodic.py echo(t, 1.0, b, h, r, h_r, w) at these lags, in hours.
        // Up to 200h: past that the study leaves out returns beyond its 14 days of data
        let (b, h) = (0.12814426110884522, 2.415993491191711 * HOUR);
        let expected = [(0.0, 1.0), (1.0, 0.7862675984996086), (12.0, 0.1844581420002805), (24.0, 0.3905845824018509), (25.5, 0.3285742065170436), (48.0, 0.38043965521372036), (72.0, 0.3685992505822545), (100.0, 0.2416312385915916), (200.0, 0.17993489411394706)];
        for (t, want) in expected {
            let got = apply_decay(1.0, temporal_factor(t * HOUR, Some(h), Some(&temperature())), b);
            assert!((got - want).abs() < 1e-12, "{}h: {} vs {}", t, got, want);
        }
    }

    #[test]
    fn test_echo_comes_back() {
        let h = 2.4 * HOUR;
        let at = |t: f64| temporal_factor(t * HOUR, Some(h), Some(&temperature()));
        assert!(at(24.0) > at(12.0) * 2.0);
        assert!(at(24.0) > at(20.0) && at(24.0) > at(28.0));
        assert!(at(0.0) <= 1.0);
        assert!(at(48.0) < at(24.0)); // fades
    }

    #[test]
    fn test_echo_validation() {
        let e = |period: &str, strength: f64, width: &str, fade: &str| {
            Echo { period: period.into(), strength, width: width.into(), fade: fade.into() }.params()
        };
        assert!(e("24h", 0.3, "3h", "infinite").unwrap().fade.is_none());
        assert!(e("24h", 0.3, "12h", "10d").unwrap_err().contains("half the period"));
        assert!(e("24h", -0.1, "3h", "10d").is_err());
        assert!(e("infinite", 0.3, "3h", "10d").is_err());
        assert!(e("24h", 0.3, "0h", "10d").is_err());
        let echo = Echo { period: "24h".into(), strength: 0.3, width: "3h".into(), fade: "10d".into() };
        assert!(decay_curve(None, Some(&echo)).unwrap_err().contains("finite half-life"));
        assert!(decay_curve(Some("2h"), Some(&echo)).is_ok());
    }

    #[test]
    fn test_time_to_floor_echo() {
        let h = 2.415993491191711 * HOUR;
        let b = 0.12814426110884522;
        let e = temperature();
        let TimeToFloor::After(t) = time_to_floor_echo(1.0, h, b, 0.5, &e) else { panic!() };
        // Without the echo the crossing is closed-form and earlier
        let TimeToFloor::After(plain) = time_to_floor(1.0, Some(h), b, 0.5) else { panic!() };
        assert!(t > plain, "{} vs {}", t, plain);
        let at = |t: f64| apply_decay(1.0, temporal_factor(t, Some(h), Some(&e)), b);
        assert!((at(t) - 0.5).abs() < 1e-9);
        // and it is the first crossing
        assert!((0..1000).all(|i| at(t * i as f64 / 1000.0) >= 0.5));
        assert_eq!(time_to_floor_echo(1.0, h, b, 0.1, &e), TimeToFloor::Never);
        assert_eq!(time_to_floor_echo(0.4, h, b, 0.5, &e), TimeToFloor::After(0.0));
    }

    #[test]
    fn test_zero_baseline_matches_original_model() {
        let with = total_fidelity(Some(0.9), 3600.0, Some(7200.0), &[0.1, 0.2], 0.0).unwrap();
        let original = 0.9 * temporal_decay(3600.0, Some(7200.0)) * transform_factor(&[0.1, 0.2]);
        assert!((with - original).abs() < 1e-12);
    }
}
