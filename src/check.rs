//! `fid check` — assess a stamped artifact against a fidelity floor.
//!
//! The exit code carries the epistemic state:
//!   0 SIGNAL  — fidelity at or above the floor
//!   1 NOISE   — fidelity below the floor
//!   2 UNKNOWN — fidelity indeterminate (unknown source, or the content
//!               no longer matches the envelope's integrity hash)

use chrono::{DateTime, Utc};

use crate::compute::{apply_decay, transform_factor};
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
    let curve = env.curve().map_err(|e| format!("envelope has {}", e))?;
    let age_seconds = (now - env.measured_at).num_milliseconds() as f64 / 1000.0;
    let noise: Vec<f64> = env.transforms.iter().map(|t| t.noise).collect();

    let source = env.source_fidelity();
    let temporal = curve.temporal(age_seconds);
    let transforms = transform_factor(&noise);
    let baseline = env.baseline.unwrap_or(0.0);
    let fidelity = source.map(|s| apply_decay(s * transforms, temporal, baseline));

    let integrity_ok = integrity_hash(&stamped.content) == env.integrity;
    let status = match fidelity {
        _ if !integrity_ok => FidStatus::Unknown,
        None => FidStatus::Unknown,
        Some(f) if f >= floor => FidStatus::Signal,
        Some(_) => FidStatus::Noise,
    };

    Ok(CheckResult {
        assessment: FidAssessment { source, temporal, baseline, transforms, fidelity, floor, status },
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

/// "half-life 2.3m (vessel_position/underway), baseline 0.73"
pub fn decay_description(env: &crate::envelope::FidEnvelope, baseline: f64) -> String {
    let mut out = format!("half-life {}", env.half_life.as_deref().unwrap_or("none"));
    match (&env.info_type, &env.state) {
        (Some(t), Some(s)) => out.push_str(&format!(" ({}/{})", t, s)),
        (Some(t), None) => out.push_str(&format!(" ({})", t)),
        (None, Some(s)) => out.push_str(&format!(" ({})", s)),
        (None, None) => {}
    }
    if baseline > 0.0 {
        out.push_str(&format!(", baseline {:.2}", baseline));
    }
    if let Some(f) = &env.fast {
        out.push_str(&format!(", {:.0}% on a {} clock", 100.0 * f.share, f.half_life));
    }
    if let Some(k) = env.shape {
        out.push_str(&format!(", shape {}", k));
    }
    if let Some(e) = &env.echo {
        out.push_str(&format!(", echo every {} (strength {:.2})", e.period, e.strength));
    }
    out
}

/// ", measured" when the source fidelity was measured, not the class constant.
pub fn measured_note(env: &crate::envelope::FidEnvelope) -> &'static str {
    if env.source_fidelity.is_some() && env.source_fidelity().is_some() {
        ", measured"
    } else {
        ""
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
        Some(s) => format!("  source      {:.4}  {}{} ({})", s, class, measured_note(env), env.source_id),
        None => format!("  source      ?       {} ({})", class, env.source_id),
    });
    lines.push(format!(
        "  temporal    {:.4}  age {}, {}",
        a.temporal,
        format_age(result.age_seconds),
        decay_description(env, a.baseline)
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
                baseline: None,
                source_fidelity: None,
                echo: None,
                fast: None,
                shape: None,
                info_type: None,
                state: None,
                transforms: noise
                    .iter()
                    .map(|&n| TransformStep { step: "t".to_string(), noise: n, at: t0(), condition: None })
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
    fn test_baseline_keeps_stale_fact_above_floor() {
        // flight category: stale VFR is still right 73% of the time
        let mut s = stamped(SourceClass::Instrument, Some("2.9h"), &[]);
        s.fid.baseline = Some(0.73);
        let r = assess(&s, 0.70, t0() + Duration::days(3)).unwrap();
        assert_eq!(r.assessment.status, FidStatus::Signal);
        assert!((r.assessment.fidelity.unwrap() - 0.73).abs() < 1e-6);

        s.fid.baseline = None;
        let r = assess(&s, 0.70, t0() + Duration::days(3)).unwrap();
        assert_eq!(r.assessment.status, FidStatus::Noise);
    }

    #[test]
    fn test_report_shows_type_state_baseline() {
        let mut s = stamped(SourceClass::Instrument, Some("2.3m"), &[]);
        s.fid.info_type = Some("vessel_position".into());
        s.fid.state = Some("underway".into());
        s.fid.baseline = Some(0.03);
        let r = assess(&s, 0.5, t0()).unwrap();
        assert!(report(&s, &r).contains("half-life 2.3m (vessel_position/underway), baseline 0.03"));
    }

    #[test]
    fn test_measured_source_fidelity_is_used_and_reported() {
        let mut s = stamped(SourceClass::Database, None, &[]);
        s.fid.source_fidelity = Some(0.62);
        let r = assess(&s, 0.70, t0()).unwrap();
        assert_eq!(r.assessment.source, Some(0.62));
        assert_eq!(r.assessment.status, FidStatus::Noise);
        assert!(report(&s, &r).contains("source      0.6200  database, measured (src)"));
    }

    #[test]
    fn test_echo_brings_a_periodic_fact_back() {
        let mut s = stamped(SourceClass::Instrument, Some("2.42h"), &[]);
        s.fid.baseline = Some(0.13);
        s.fid.echo = Some(crate::compute::Echo {
            period: "24h".into(),
            strength: 0.31,
            width: "3.75h".into(),
            fade: "345h".into(),
        });
        let at = |hours: i64| assess(&s, 0.30, t0() + Duration::hours(hours)).unwrap();
        assert_eq!(at(12).assessment.status, FidStatus::Noise);
        assert_eq!(at(24).assessment.status, FidStatus::Signal);
        assert!(at(24).assessment.fidelity.unwrap() > 2.0 * at(12).assessment.fidelity.unwrap());
        assert!(report(&s, &at(24)).contains("echo every 24h (strength 0.31)"));
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
