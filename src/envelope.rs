//! The Fidelity Envelope — metadata wrapper that records source,
//! timestamp, transformation history, and current fidelity score.
//!
//! Like EXIF for epistemic state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::str::FromStr;

/// Current envelope schema version.
pub const FID_VERSION: &str = "0.1";

/// Source class — determines initial fidelity ceiling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceClass {
    /// Calibrated physical instrument (0.95–0.99)
    Instrument,
    /// Human direct observation (0.80–0.95)
    HumanObservation,
    /// Human attestation, signed (0.70–0.90)
    HumanAttestation,
    /// Verified database record (0.85–0.95)
    Database,
    /// LLM inference, grounded in sources (0.40–0.70)
    LlmGrounded,
    /// LLM inference, ungrounded (0.10–0.40)
    LlmUngrounded,
    /// Hearsay or unverified (0.05–0.30)
    Hearsay,
    /// Unknown source — fidelity indeterminate
    Unknown,
}

impl SourceClass {
    /// Default initial fidelity for this source class (midpoint of range).
    pub fn default_fidelity(&self) -> Option<f64> {
        match self {
            Self::Instrument => Some(0.97),
            Self::HumanObservation => Some(0.87),
            Self::HumanAttestation => Some(0.80),
            Self::Database => Some(0.90),
            Self::LlmGrounded => Some(0.55),
            Self::LlmUngrounded => Some(0.25),
            Self::Hearsay => Some(0.17),
            Self::Unknown => None, // indeterminate
        }
    }

    /// Accepted names, as written on the command line and in envelopes.
    pub const NAMES: &'static [&'static str] = &[
        "instrument",
        "human_observation",
        "human_attestation",
        "database",
        "llm_grounded",
        "llm_ungrounded",
        "hearsay",
        "unknown",
    ];
}

impl FromStr for SourceClass {
    type Err = String;

    /// Parse a source class name. Hyphens are accepted in place of underscores.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let name = s.trim().to_lowercase().replace('-', "_");
        serde_json::from_value(Value::String(name)).map_err(|_| {
            format!(
                "invalid source class '{}' (expected one of: {})",
                s,
                Self::NAMES.join(", ")
            )
        })
    }
}

/// A single transformation step in the fidelity chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformStep {
    /// What performed this transformation
    pub step: String,

    /// Noise introduced (0.0 to 1.0)
    pub noise: f64,

    /// When this transformation occurred
    pub at: DateTime<Utc>,
}

/// The fidelity envelope — carried with every fid-stamped artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FidEnvelope {
    /// Schema version
    pub fid_version: String,

    /// Source class
    pub source_class: SourceClass,

    /// Source identifier (sensor ID, model name, human ID, etc.)
    pub source_id: String,

    /// When the original measurement was taken
    pub measured_at: DateTime<Utc>,

    /// Domain-specific half-life (e.g. "7d", "30s", "3y", "infinite")
    pub half_life: Option<String>,

    /// Ordered list of transformation steps since measurement
    pub transforms: Vec<TransformStep>,

    /// Fidelity at time of last stamp
    pub fid_at_stamp: Option<f64>,

    /// Upstream artifact this was derived from
    pub derived_from: Option<String>,

    /// Integrity hash of the content (sha256)
    pub integrity: String,
}

impl FidEnvelope {
    /// Total fidelity at `now`, accounting for age since measurement.
    ///
    /// Ok(None) if the source class is Unknown (indeterminate).
    pub fn fidelity_at(&self, now: DateTime<Utc>) -> Result<Option<f64>, String> {
        let half_life = match &self.half_life {
            Some(hl) => crate::compute::parse_half_life(hl)?,
            None => None,
        };
        let age_seconds = (now - self.measured_at).num_milliseconds() as f64 / 1000.0;
        let noise: Vec<f64> = self.transforms.iter().map(|t| t.noise).collect();
        Ok(crate::compute::total_fidelity(
            self.source_class.default_fidelity(),
            age_seconds,
            half_life,
            &noise,
        ))
    }
}

/// A fid-stamped artifact: the envelope plus the content it wraps.
///
/// Serialized as `{"fid": {...}, "content": ...}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stamped {
    pub fid: FidEnvelope,
    pub content: Value,
}

/// Integrity hash of content: sha256 over its compact JSON serialization,
/// formatted as `sha256:<hex>`.
pub fn integrity_hash(content: &Value) -> String {
    let bytes = serde_json::to_vec(content).expect("serializing a JSON value cannot fail");
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
    format!("sha256:{}", hex)
}

/// Fidelity assessment result.
#[derive(Debug)]
pub struct FidAssessment {
    /// Source fidelity (from source class)
    pub source: Option<f64>,

    /// Temporal decay factor
    pub temporal: f64,

    /// Transformation noise factor
    pub transforms: f64,

    /// Total fidelity (source × temporal × transforms)
    pub fidelity: Option<f64>,

    /// Noise floor threshold applied
    pub floor: f64,

    /// Is this signal or noise?
    pub status: FidStatus,
}

/// Epistemic status — maps to exit codes.
#[derive(Debug, PartialEq)]
pub enum FidStatus {
    /// Exit 0 — fidelity above floor
    Signal,
    /// Exit 1 — fidelity below floor
    Noise,
    /// Exit 2 — source unknown, fidelity indeterminate
    Unknown,
}

impl FidStatus {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Signal => 0,
            Self::Noise => 1,
            Self::Unknown => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_envelope() -> FidEnvelope {
        let at = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        FidEnvelope {
            fid_version: FID_VERSION.to_string(),
            source_class: SourceClass::HumanObservation,
            source_id: "sensor-12".to_string(),
            measured_at: at,
            half_life: Some("30d".to_string()),
            transforms: vec![TransformStep {
                step: "normalize.py".to_string(),
                noise: 0.05,
                at,
            }],
            fid_at_stamp: Some(0.8265),
            derived_from: None,
            integrity: "sha256:00".to_string(),
        }
    }

    #[test]
    fn test_source_class_serializes_snake_case() {
        let json = serde_json::to_string(&SourceClass::LlmUngrounded).unwrap();
        assert_eq!(json, "\"llm_ungrounded\"");
    }

    #[test]
    fn test_source_class_names_all_parse() {
        for name in SourceClass::NAMES {
            let class: SourceClass = name.parse().unwrap();
            assert_eq!(serde_json::to_value(&class).unwrap(), Value::String(name.to_string()));
        }
    }

    #[test]
    fn test_source_class_parse_accepts_hyphens_and_case() {
        assert_eq!("Human-Observation".parse(), Ok(SourceClass::HumanObservation));
    }

    #[test]
    fn test_source_class_parse_rejects_unknown_name() {
        let err = "human".parse::<SourceClass>().unwrap_err();
        assert!(err.contains("human_observation"));
    }

    #[test]
    fn test_envelope_round_trip() {
        let env = sample_envelope();
        let json = serde_json::to_string(&env).unwrap();
        let back: FidEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source_class, env.source_class);
        assert_eq!(back.source_id, env.source_id);
        assert_eq!(back.measured_at, env.measured_at);
        assert_eq!(back.half_life, env.half_life);
        assert_eq!(back.transforms.len(), 1);
        assert_eq!(back.transforms[0].step, "normalize.py");
        assert_eq!(back.transforms[0].noise, 0.05);
        assert_eq!(back.fid_at_stamp, env.fid_at_stamp);
        assert_eq!(back.integrity, env.integrity);
    }

    #[test]
    fn test_envelope_field_names() {
        let value = serde_json::to_value(sample_envelope()).unwrap();
        assert_eq!(value["source_class"], "human_observation");
        assert_eq!(value["measured_at"], "2026-01-02T03:04:05Z");
        assert_eq!(value["transforms"][0]["step"], "normalize.py");
        assert!(value["derived_from"].is_null());
    }

    #[test]
    fn test_stamped_rejects_unknown_fields() {
        let mut value = serde_json::to_value(Stamped {
            fid: sample_envelope(),
            content: Value::Null,
        })
        .unwrap();
        value["extra"] = Value::Bool(true);
        assert!(serde_json::from_value::<Stamped>(value).is_err());
    }

    #[test]
    fn test_integrity_hash_is_stable_and_content_sensitive() {
        let a = serde_json::json!({"b": 1, "a": 2});
        let b = serde_json::json!({"a": 2, "b": 1});
        let c = serde_json::json!({"a": 2, "b": 3});
        assert_eq!(integrity_hash(&a), integrity_hash(&b));
        assert_ne!(integrity_hash(&a), integrity_hash(&c));
        assert!(integrity_hash(&a).starts_with("sha256:"));
        assert_eq!(integrity_hash(&a).len(), "sha256:".len() + 64);
    }
}
