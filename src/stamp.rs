//! `fid stamp` — wrap content in a fidelity envelope.
//!
//! A root stamp records the source. A stamp over already-stamped input
//! (or with an upstream artifact via --derived-from) inherits the
//! source and appends one step to the transform chain. This is how
//! pipeline composition works.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::compute::parse_half_life;
use crate::envelope::{integrity_hash, FidEnvelope, SourceClass, Stamped, TransformStep, FID_VERSION};

/// Options for a single stamp.
#[derive(Debug, Clone)]
pub struct StampOptions {
    /// Source identifier, or the transform step name when re-stamping
    pub source: String,
    /// Source class; only meaningful on a root stamp
    pub source_class: Option<SourceClass>,
    /// Noise introduced by this step (0.0 to 1.0)
    pub noise: f64,
    /// Half-life; overrides any inherited half-life
    pub half_life: Option<String>,
    /// Path to the upstream fid-stamped artifact, recorded as given
    pub derived_from: Option<String>,
}

/// Result of a stamp: the artifact plus any warnings for stderr.
#[derive(Debug)]
pub struct StampOutput {
    pub stamped: Stamped,
    pub warnings: Vec<String>,
}

/// Parse stdin content. Already-stamped input yields its envelope.
///
/// JSON input is embedded as JSON. Anything else is embedded as a string.
/// A JSON object with a `fid` key must be a valid stamped artifact —
/// a malformed envelope is an error, never silently re-rooted.
pub fn parse_input(input: &str) -> Result<(Value, Option<FidEnvelope>), String> {
    match serde_json::from_str::<Value>(input) {
        Ok(Value::Object(map)) if map.contains_key("fid") => {
            let stamped: Stamped = serde_json::from_value(Value::Object(map))
                .map_err(|e| format!("input has a 'fid' key but is not a valid fid-stamped artifact: {}", e))?;
            Ok((stamped.content, Some(stamped.fid)))
        }
        Ok(value) => Ok((value, None)),
        Err(_) => Ok((Value::String(input.to_string()), None)),
    }
}

/// Stamp content.
///
/// `input` is the raw stdin text. `upstream_file` is the envelope read from
/// `--derived-from`, if given; an envelope already on stdin takes precedence.
pub fn stamp(
    input: &str,
    opts: &StampOptions,
    upstream_file: Option<FidEnvelope>,
    now: DateTime<Utc>,
) -> Result<StampOutput, String> {
    if !opts.noise.is_finite() || !(0.0..=1.0).contains(&opts.noise) {
        return Err(format!("invalid noise {} (must be between 0.0 and 1.0)", opts.noise));
    }
    if let Some(hl) = &opts.half_life {
        parse_half_life(hl)?;
    }

    let mut warnings = Vec::new();
    let (content, upstream_stdin) = parse_input(input)?;

    if upstream_stdin.is_some() && upstream_file.is_some() {
        warnings.push("input is already stamped; using its envelope, not --derived-from".to_string());
    }

    let step = TransformStep {
        step: opts.source.clone(),
        noise: opts.noise,
        at: now,
    };

    let mut envelope = match upstream_stdin.or(upstream_file) {
        Some(mut upstream) => {
            if opts.source_class.is_some() {
                warnings.push(format!(
                    "--source-class ignored; source is inherited from upstream ({})",
                    upstream.source_id
                ));
            }
            if opts.half_life.is_some() {
                upstream.half_life = opts.half_life.clone();
            }
            upstream.transforms.push(step);
            upstream.derived_from = opts.derived_from.clone();
            upstream
        }
        None => FidEnvelope {
            fid_version: FID_VERSION.to_string(),
            source_class: opts.source_class.clone().unwrap_or(SourceClass::Unknown),
            source_id: opts.source.clone(),
            measured_at: now,
            half_life: opts.half_life.clone(),
            transforms: if opts.noise > 0.0 { vec![step] } else { Vec::new() },
            fid_at_stamp: None,
            derived_from: opts.derived_from.clone(),
            integrity: String::new(),
        },
    };

    envelope.fid_version = FID_VERSION.to_string();
    envelope.integrity = integrity_hash(&content);
    envelope.fid_at_stamp = envelope.fidelity_at(now)?;

    Ok(StampOutput {
        stamped: Stamped { fid: envelope, content },
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn t0() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    fn opts(source: &str) -> StampOptions {
        StampOptions {
            source: source.to_string(),
            source_class: None,
            noise: 0.0,
            half_life: None,
            derived_from: None,
        }
    }

    fn to_json(out: &StampOutput) -> String {
        serde_json::to_string(&out.stamped).unwrap()
    }

    #[test]
    fn test_root_stamp_json_content() {
        let o = StampOptions {
            source_class: Some(SourceClass::Instrument),
            half_life: Some("30d".to_string()),
            ..opts("sensor-12")
        };
        let out = stamp(r#"{"temp": 21.5}"#, &o, None, t0()).unwrap();
        let env = &out.stamped.fid;
        assert_eq!(out.stamped.content["temp"], 21.5);
        assert_eq!(env.source_id, "sensor-12");
        assert_eq!(env.source_class, SourceClass::Instrument);
        assert_eq!(env.measured_at, t0());
        assert!(env.transforms.is_empty());
        assert_eq!(env.fid_at_stamp, Some(0.97));
        assert_eq!(env.integrity, integrity_hash(&out.stamped.content));
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn test_root_stamp_text_content_is_string() {
        let out = stamp("hello\n", &opts("x"), None, t0()).unwrap();
        assert_eq!(out.stamped.content, Value::String("hello\n".to_string()));
    }

    #[test]
    fn test_root_stamp_with_noise_records_step() {
        let o = StampOptions {
            source_class: Some(SourceClass::Database),
            noise: 0.10,
            ..opts("transform.py")
        };
        let out = stamp("{}", &o, None, t0()).unwrap();
        assert_eq!(out.stamped.fid.transforms.len(), 1);
        assert_eq!(out.stamped.fid.transforms[0].step, "transform.py");
        let f = out.stamped.fid.fid_at_stamp.unwrap();
        assert!((f - 0.90 * 0.90).abs() < 1e-10);
    }

    #[test]
    fn test_root_stamp_unknown_source_is_indeterminate() {
        let out = stamp("{}", &opts("mystery"), None, t0()).unwrap();
        assert_eq!(out.stamped.fid.source_class, SourceClass::Unknown);
        assert_eq!(out.stamped.fid.fid_at_stamp, None);
    }

    #[test]
    fn test_restamp_appends_to_chain() {
        let root = StampOptions {
            source_class: Some(SourceClass::Instrument),
            half_life: Some("1d".to_string()),
            ..opts("sensor-12")
        };
        let first = stamp(r#"{"v": 1}"#, &root, None, t0()).unwrap();

        let later = t0() + Duration::days(1);
        let step = StampOptions { noise: 0.10, ..opts("aggregate.py") };
        let second = stamp(&to_json(&first), &step, None, later).unwrap();
        let env = &second.stamped.fid;

        assert_eq!(env.source_id, "sensor-12");
        assert_eq!(env.source_class, SourceClass::Instrument);
        assert_eq!(env.measured_at, t0());
        assert_eq!(env.transforms.len(), 1);
        assert_eq!(env.transforms[0].step, "aggregate.py");
        assert_eq!(env.transforms[0].at, later);
        assert_eq!(second.stamped.content, serde_json::json!({"v": 1}));
        // one half-life elapsed, one 10% transform
        let f = env.fid_at_stamp.unwrap();
        assert!((f - 0.97 * 0.5 * 0.90).abs() < 1e-10);
    }

    #[test]
    fn test_restamp_ignores_source_class_with_warning() {
        let first = stamp("{}", &StampOptions { source_class: Some(SourceClass::Instrument), ..opts("s") }, None, t0()).unwrap();
        let o = StampOptions { source_class: Some(SourceClass::Hearsay), ..opts("step") };
        let second = stamp(&to_json(&first), &o, None, t0()).unwrap();
        assert_eq!(second.stamped.fid.source_class, SourceClass::Instrument);
        assert_eq!(second.warnings.len(), 1);
    }

    #[test]
    fn test_restamp_half_life_override() {
        let first = stamp("{}", &StampOptions { half_life: Some("1d".into()), ..opts("s") }, None, t0()).unwrap();
        let o = StampOptions { half_life: Some("1y".into()), ..opts("step") };
        let second = stamp(&to_json(&first), &o, None, t0()).unwrap();
        assert_eq!(second.stamped.fid.half_life.as_deref(), Some("1y"));
    }

    #[test]
    fn test_derived_from_file_is_upstream() {
        let upstream = stamp("{}", &StampOptions { source_class: Some(SourceClass::Database), ..opts("db") }, None, t0()).unwrap();
        let o = StampOptions {
            noise: 0.05,
            derived_from: Some("upstream.json".into()),
            ..opts("report.py")
        };
        let out = stamp("plain text", &o, Some(upstream.stamped.fid), t0()).unwrap();
        let env = &out.stamped.fid;
        assert_eq!(env.source_id, "db");
        assert_eq!(env.derived_from.as_deref(), Some("upstream.json"));
        assert_eq!(env.transforms[0].step, "report.py");
        assert_eq!(out.stamped.content, Value::String("plain text".into()));
    }

    #[test]
    fn test_stdin_envelope_beats_derived_from() {
        let a = stamp("{}", &StampOptions { source_class: Some(SourceClass::Database), ..opts("a") }, None, t0()).unwrap();
        let b = stamp("{}", &StampOptions { source_class: Some(SourceClass::Hearsay), ..opts("b") }, None, t0()).unwrap();
        let out = stamp(&to_json(&a), &opts("step"), Some(b.stamped.fid), t0()).unwrap();
        assert_eq!(out.stamped.fid.source_id, "a");
        assert_eq!(out.warnings.len(), 1);
    }

    #[test]
    fn test_malformed_envelope_is_error() {
        let err = stamp(r#"{"fid": {"oops": 1}, "content": 1}"#, &opts("x"), None, t0()).unwrap_err();
        assert!(err.contains("not a valid fid-stamped artifact"));
    }

    #[test]
    fn test_invalid_noise_is_error() {
        for noise in [-0.1, 1.5, f64::NAN] {
            assert!(stamp("{}", &StampOptions { noise, ..opts("x") }, None, t0()).is_err());
        }
    }

    #[test]
    fn test_invalid_half_life_is_error() {
        let o = StampOptions { half_life: Some("7days".into()), ..opts("x") };
        assert!(stamp("{}", &o, None, t0()).is_err());
    }

    #[test]
    fn test_integrity_tracks_new_content() {
        let first = stamp(r#"{"v": 1}"#, &opts("s"), None, t0()).unwrap();
        let mut changed = first.stamped.clone();
        changed.content = serde_json::json!({"v": 2});
        let second = stamp(&serde_json::to_string(&changed).unwrap(), &opts("step"), None, t0()).unwrap();
        assert_eq!(second.stamped.fid.integrity, integrity_hash(&serde_json::json!({"v": 2})));
        assert_ne!(second.stamped.fid.integrity, first.stamped.fid.integrity);
    }
}
