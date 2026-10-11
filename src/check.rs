//! `fid check` — assess a stamped artifact against a fidelity floor.
//!
//! The exit code carries the epistemic state:
//!   0 SIGNAL  — fidelity at or above the floor
//!   1 NOISE   — fidelity below the floor
//!   2 UNKNOWN — fidelity indeterminate (unknown source, or the content
//!               no longer matches the envelope's integrity hash)

use chrono::{DateTime, Utc};

use crate::compute::{parse_half_life, temporal_decay, transform_factor};
use crate::envelope::{integrity_hash, FidAssessment, FidStatus, Stamped};

/// Result of checking one artifact.
#[derive(Debug)]
pub struct CheckResult {
    pub assessment: FidAssessment,
    /// Seconds since the original measurement
    pub age_seconds: f64,
    /// Does the content still match the envelope's integrity hash?
    pub integrity_ok: bool,
}

/// Assess a stamped artifact at `now` against `floor`.
///
/// An integrity mismatch makes the result UNKNOWN: the envelope no
/// longer describes this content, so its fidelity cannot be trusted.
pub fn assess(stamped: &Stamped, floor: f64, now: DateTime<Utc>) -> Result<CheckResult, String> {
    if !floor.is_finite() || !(0.0..=1.0).contains(&floor) {
        return Err(format!("invalid floor {} (must be between 0.0 and 1.0)", floor));
    }

    let env = &stamped.fid;
    let half_life = match &env.half_life {
        Some(hl) => parse_half_life(hl).map_err(|e| format!("envelope has {}", e))?,
        None => None,
    };
    let age_seconds = (now - env.measured_at).num_milliseconds() as f64 / 1000.0;
    let noise: Vec<f64> = env.transforms.iter().map(|t| t.noise).collect();

    let source = env.source_class.default_fidelity();
    let temporal = temporal_decay(age_seconds, half_life);
    let transforms = transform_factor(&noise);
    let fidelity = source.map(|s| s * temporal * transforms);

    let integrity_ok = integrity_hash(&stamped.content) == env.integrity;
    let status = match fidelity {
        _ if !integrity_ok => FidStatus::Unknown,
        None => FidStatus::Unknown,
        Some(f) if f >= floor => FidStatus::Signal,
        Some(_) => FidStatus::Noise,
    };

    Ok(CheckResult {
        assessment: FidAssessment { source, temporal, transforms, fidelity, floor, status },
        age_seconds,
        integrity_ok,
    })
}

/// Human-readable duration, coarsest two units (e.g. "3d 4h", "45s").
pub fn format_age(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    let units = [("y", 31_557_600), ("d", 86_400), ("h", 3_600), ("m", 60), ("s", 1)];
    let mut parts = Vec::new();
    let mut rest = s;
    for (name, size) in units {
        if rest >= size {
            parts.push(format!("{}{}", rest / size, name));
            rest %= size;
        }
        if parts.len() == 2 {
            break;
        }
    }
    if parts.is_empty() {
        "0s".to_string()
    } else {
        parts.join(" ")
    }
}

/// The stderr report for `fid check`.
pub fn report(stamped: &Stamped, result: &CheckResult) -> String {
    let env = &stamped.fid;
    let a = &result.assessment;
    let status = match a.status {
        FidStatus::Signal => "SIGNAL",
        FidStatus::Noise => "NOISE",
        FidStatus::Unknown => "UNKNOWN",
    };
    let class = serde_json::to_value(&env.source_class)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();

    let mut lines = Vec::new();
    lines.push(match (a.fidelity, &a.status) {
        (Some(f), FidStatus::Signal) => format!("{:<8} fidelity {:.4} >= floor {:.2}", status, f, a.floor),
        (Some(f), FidStatus::Noise) => format!("{:<8} fidelity {:.4} <  floor {:.2}", status, f, a.floor),
        _ => format!("{:<8} fidelity indeterminate (floor {:.2})", status, a.floor),
    });
    if !result.integrity_ok {
        lines.push("  integrity mismatch: content changed since last stamp".to_string());
    }
    lines.push(match a.source {
        Some(s) => format!("  source      {:.4}  {} ({})", s, class, env.source_id),
        None => format!("  source      ?       {} ({})", class, env.source_id),
    });
    let half_life = env.half_life.as_deref().unwrap_or("none");
    lines.push(format!(
        "  temporal    {:.4}  age {}, half-life {}",
        a.temporal,
        format_age(result.age_seconds),
        half_life
    ));
    lines.push(format!(
        "  transforms  {:.4}  {} step{}",
        a.transforms,
        env.transforms.len(),
        if env.transforms.len() == 1 { "" } else { "s" }
    ));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{FidEnvelope, SourceClass, TransformStep, FID_VERSION};
    use chrono::{Duration, TimeZone};
    use serde_json::json;

    fn t0() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    fn stamped(class: SourceClass, half_life: Option<&str>, noise: &[f64]) -> Stamped {
        let content = json!({"v": 1});
        Stamped {
            fid: FidEnvelope {
                fid_version: FID_VERSION.to_string(),
                source_class: class,
                source_id: "src".to_string(),
                measured_at: t0(),
                half_life: half_life.map(str::to_string),
                transforms: noise
                    .iter()
                    .map(|&n| TransformStep { step: "t".to_string(), noise: n, at: t0() })
                    .collect(),
                fid_at_stamp: None,
                derived_from: None,
                integrity: integrity_hash(&content),
            },
            content,
        }
    }

    #[test]
    fn test_fresh_instrument_is_signal() {
        let s = stamped(SourceClass::Instrument, Some("30d"), &[]);
        let r = assess(&s, 0.70, t0()).unwrap();
        assert_eq!(r.assessment.status, FidStatus::Signal);
        assert_eq!(r.assessment.fidelity, Some(0.97));
    }

    #[test]
    fn test_decays_to_noise_over_time() {
        // 0.97 → 0.485 after one half-life
        let s = stamped(SourceClass::Instrument, Some("1d"), &[]);
        let r = assess(&s, 0.70, t0() + Duration::days(1)).unwrap();
        assert_eq!(r.assessment.status, FidStatus::Noise);
        assert!((r.assessment.fidelity.unwrap() - 0.485).abs() < 1e-10);
        assert!((r.assessment.temporal - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_transforms_push_below_floor() {
        // 0.90 × 0.70 × 0.70 = 0.441, no temporal decay
        let s = stamped(SourceClass::Database, None, &[0.30, 0.30]);
        let r = assess(&s, 0.50, t0() + Duration::days(9999)).unwrap();
        assert_eq!(r.assessment.status, FidStatus::Noise);
        assert!((r.assessment.transforms - 0.49).abs() < 1e-10);
    }

    #[test]
    fn test_exactly_at_floor_is_signal() {
        let s = stamped(SourceClass::Database, None, &[]);
        let r = assess(&s, 0.90, t0()).unwrap();
        assert_eq!(r.assessment.status, FidStatus::Signal);
    }

    #[test]
    fn test_unknown_source_is_unknown() {
        let s = stamped(SourceClass::Unknown, None, &[]);
        let r = assess(&s, 0.0, t0()).unwrap();
        assert_eq!(r.assessment.status, FidStatus::Unknown);
        assert_eq!(r.assessment.fidelity, None);
    }

    #[test]
    fn test_integrity_mismatch_is_unknown() {
        let mut s = stamped(SourceClass::Instrument, None, &[]);
        s.content = json!({"v": 2});
        let r = assess(&s, 0.10, t0()).unwrap();
        assert!(!r.integrity_ok);
        assert_eq!(r.assessment.status, FidStatus::Unknown);
    }

    #[test]
    fn test_invalid_floor_is_error() {
        let s = stamped(SourceClass::Instrument, None, &[]);
        for floor in [-0.1, 1.1, f64::NAN] {
            assert!(assess(&s, floor, t0()).is_err());
        }
    }

    #[test]
    fn test_invalid_envelope_half_life_is_error() {
        let s = stamped(SourceClass::Instrument, Some("7days"), &[]);
        assert!(assess(&s, 0.5, t0()).is_err());
    }

    #[test]
    fn test_format_age() {
        assert_eq!(format_age(0.0), "0s");
        assert_eq!(format_age(45.0), "45s");
        assert_eq!(format_age(3.0 * 86400.0 + 4.0 * 3600.0 + 5.0), "3d 4h");
        assert_eq!(format_age(-10.0), "0s");
    }

    #[test]
    fn test_report_mentions_status_and_axes() {
        let s = stamped(SourceClass::Instrument, Some("1d"), &[0.1]);
        let r = assess(&s, 0.70, t0() + Duration::days(1)).unwrap();
        let text = report(&s, &r);
        assert!(text.starts_with("NOISE"));
        assert!(text.contains("instrument (src)"));
        assert!(text.contains("age 1d"));
        assert!(text.contains("1 step"));
    }
}
