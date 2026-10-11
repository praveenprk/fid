//! Fidelity computation.
//!
//! Three axes, each independent:
//!   f_source    — initial fidelity from source class
//!   f_temporal  — decay as a function of age and half-life
//!   f_transform — cumulative noise from transformation chain
//!
//! Total: fidelity = f_source × f_temporal × f_transform
//!
//! It never spontaneously increases.
//! New measurement is the only restoration mechanism.

/// Parse a half-life string (e.g. "30s", "6h", "7d", "3mo", "3y") into seconds.
///
/// Returns Ok(None) for "infinite" (zero temporal decay).
/// Unparseable or non-positive values are an error — never silently
/// treated as infinite, which would hide decay.
pub fn parse_half_life(s: &str) -> Result<Option<f64>, String> {
    let s = s.trim().to_lowercase();
    if s == "infinite" || s == "inf" {
        return Ok(None);
    }

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
        .map_err(|_| format!("invalid half-life '{}' (expected e.g. 30s, 5m, 6h, 7d, 3mo, 3y, infinite)", s))?;
    if !n.is_finite() || n <= 0.0 {
        return Err(format!("invalid half-life '{}' (must be a positive duration)", s));
    }
    Ok(Some(n * multiplier))
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
/// fidelity = f_source × f_temporal × f_transform
///
/// Returns None if source fidelity is indeterminate (Unknown source class).
pub fn total_fidelity(
    source_fidelity: Option<f64>,
    age_seconds: f64,
    half_life_seconds: Option<f64>,
    noise_values: &[f64],
) -> Option<f64> {
    let f_source = source_fidelity?;
    let f_temporal = temporal_decay(age_seconds, half_life_seconds);
    let f_transform = transform_factor(noise_values);
    Some(f_source * f_temporal * f_transform)
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
        let result = total_fidelity(Some(0.90), 86400.0, Some(86400.0), &[0.10]);
        let expected = 0.90 * 0.5 * 0.90;
        assert!((result.unwrap() - expected).abs() < 1e-10);
    }

    #[test]
    fn test_total_fidelity_unknown_source() {
        let result = total_fidelity(None, 0.0, Some(86400.0), &[]);
        assert!(result.is_none());
    }
}
