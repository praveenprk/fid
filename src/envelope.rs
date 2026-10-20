//! The Fidelity Envelope — metadata wrapper that records source,
//! timestamp, transformation history, and current fidelity score.
//!
//! Like EXIF for epistemic state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::str::FromStr;

/// The envelope version this implementation writes (spec/draft-kumar-fid-00.md, Versioning).
pub const FID_VERSION: &str = "1.0";

/// Versions this implementation reads, as (MAJOR, highest MINOR).
const READS: &[(u32, u32)] = &[(0, 4), (1, 0)];

/// Envelope members, by the version that introduced them (draft, Fields).
const MEMBERS: &[(&str, (u32, u32))] = &[
    ("fid_version", (0, 1)),
    ("source_class", (0, 1)),
    ("source_id", (0, 1)),
    ("measured_at", (0, 1)),
    ("half_life", (0, 1)),
    ("transforms", (0, 1)),
    ("fid_at_stamp", (0, 1)),
    ("derived_from", (0, 1)),
    ("integrity", (0, 1)),
    ("baseline", (0, 2)),
    ("info_type", (0, 2)),
    ("state", (0, 2)),
    ("source_fidelity", (0, 3)),
    ("echo", (0, 3)),
    ("fast", (0, 4)),
    ("shape", (0, 4)),
    ("ext", (1, 0)),
];
const ECHO_MEMBERS: &[&str] = &["period", "strength", "width", "fade"];
const FAST_MEMBERS: &[&str] = &["share", "half_life"];
const STEP_MEMBERS: &[&str] = &["step", "noise", "at", "condition"];

/// "MAJOR.MINOR", each decimal digits only.
pub fn parse_version(v: &str) -> Option<(u32, u32)> {
    let (a, b) = v.split_once('.')?;
    let num = |s: &str| if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) { s.parse().ok() } else { None };
    Some((num(a)?, num(b)?))
}

/// Why a scorer cannot read this envelope, or None (draft, Versioning): a
/// version it does not implement, or a member not defined for the version,
/// outside `ext`. A field it does not know may change fidelity, so it answers
/// UNKNOWN rather than compute around it.
pub fn not_understood(fid: &serde_json::Map<String, Value>) -> Option<String> {
    let text = fid.get("fid_version").and_then(Value::as_str).unwrap_or("");
    let Some(version) = parse_version(text) else {
        return Some(format!("fid_version '{}' is not MAJOR.MINOR", text));
    };
    if !READS.iter().any(|&(major, minor)| version.0 == major && version.1 <= minor) {
        return Some(format!("version {} is not one this fid reads (0.1 to 0.4, 1.0)", text));
    }
    let unknown_in = |v: &Value, known: &[&str]| v.as_object().and_then(|o| o.keys().find(|k| !known.contains(&k.as_str())).cloned());
    for (k, v) in fid {
        match MEMBERS.iter().find(|(m, _)| m == k) {
            Some((_, since)) if *since <= version => {}
            _ => return Some(format!("member '{}' is not defined for version {}", k, text)),
        }
        let nested = match k.as_str() {
            "echo" => ECHO_MEMBERS,
            "fast" => FAST_MEMBERS,
            _ => continue,
        };
        if let Some(m) = unknown_in(v, nested) {
            return Some(format!("member '{}' in '{}' is not defined for version {}", m, k, text));
        }
    }
    if let Some(steps) = fid.get("transforms").and_then(Value::as_array) {
        if let Some(m) = steps.iter().find_map(|t| unknown_in(t, STEP_MEMBERS)) {
            return Some(format!("member '{}' in a transform step is not defined for version {}", m, text));
        }
    }
    None
}

/// A stamped artifact as read: one a scorer understands, or why it does not.
#[derive(Debug)]
pub enum Read {
    Understood(Stamped),
    /// Fidelity cannot be computed: scorers answer UNKNOWN (draft, Check)
    NotUnderstood(String),
}

/// Read a stamped artifact. Malformed input (not JSON, other top-level
/// members, missing or mistyped fields of a version it reads) is an error;
/// a version or member it does not understand is `NotUnderstood`.
pub fn read(text: &str) -> Result<Read, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {}", e))?;
    read_value(value)
}

/// As `read`, from a parsed value.
pub fn read_value(value: Value) -> Result<Read, String> {
    let obj = value.as_object().ok_or("a stamped artifact is a JSON object")?;
    if obj.len() != 2 || !obj.contains_key("content") {
        return Err("a stamped artifact has exactly two members, 'fid' and 'content'".to_string());
    }
    let fid = obj.get("fid").and_then(Value::as_object).ok_or("'fid' must be an object")?;
    // No version at all is malformed; a version string this fid does not read is not understood
    if !fid.get("fid_version").is_some_and(Value::is_string) {
        return Err("'fid_version' is missing or not a string".to_string());
    }
    if let Some(reason) = not_understood(fid) {
        return Ok(Read::NotUnderstood(reason));
    }
    serde_json::from_value(value).map(Read::Understood).map_err(|e| e.to_string())
}

/// The integrity of `content` as an envelope of `version` computes it: SHA-256
/// of the JCS text (RFC 8785) from 1.0, of the legacy canonical form before.
pub fn integrity_for(version: &str, content: &Value) -> String {
    match parse_version(version) {
        Some((major, _)) if major >= 1 => {
            let digest = Sha256::digest(crate::jcs::canonical(content).as_bytes());
            format!("sha256:{}", digest.iter().map(|b| format!("{:02x}", b)).collect::<String>())
        }
        _ => integrity_hash(content),
    }
}

/// Content a writer must refuse: not I-JSON (RFC 7493), here an integer that
/// binary64 cannot hold exactly, which parsers would read as different numbers.
pub fn not_ijson(content: &Value) -> Option<String> {
    const LIMIT: u64 = 1 << 53;
    match content {
        Value::Number(n) => match (n.as_u64(), n.as_i64()) {
            (Some(u), _) if u > LIMIT => Some(format!("integer {} is beyond 2^53", u)),
            (_, Some(i)) if i.unsigned_abs() > LIMIT => Some(format!("integer {} is beyond -2^53", i)),
            _ => None,
        },
        Value::Array(items) => items.iter().find_map(not_ijson),
        Value::Object(map) => map.values().find_map(not_ijson),
        _ => None,
    }
}

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

    /// Upstream condition the noise was conditioned on (e.g. "previous_leg_late")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
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

    /// Chance the fact is right anyway once fully stale (0.0 to 1.0).
    /// Temporal decay goes toward this instead of toward zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<f64>,

    /// Measured fraction correct when fresh, from a calibrated registry.
    /// Replaces the source class constant; an unknown class stays unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_fidelity: Option<f64>,

    /// Periodic return of a fact that comes back (e.g. daily temperature)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub echo: Option<crate::compute::Echo>,

    /// A share that goes stale on a faster clock (fid_version 0.4)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<crate::compute::Fast>,

    /// Shape of the decay clocks, 0.5^((age/h)^shape); absent: 1 (fid_version 0.4)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<f64>,

    /// Extensions that do not change fidelity; scorers ignore them (fid_version 1.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<serde_json::Map<String, Value>>,

    /// Information type, as named in a half-life registry (e.g. "vessel_position")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_type: Option<String>,

    /// State of the observed thing at measurement (e.g. "underway", "moored")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,

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
    /// Fidelity when fresh: the measured value if the envelope carries one,
    /// else the source class constant. None if the source class is Unknown.
    pub fn source_fidelity(&self) -> Option<f64> {
        let class = self.source_class.default_fidelity()?;
        Some(self.source_fidelity.unwrap_or(class))
    }

    /// Is the decay known? A null half-life means it is not (draft, Fields): no
    /// fidelity can be computed; "infinite" is how a fact that does not decay says so.
    pub fn decay_known(&self) -> bool {
        self.half_life.is_some()
    }

    /// Total fidelity at `now`, accounting for age since measurement.
    ///
    /// Ok(None) if the source class is Unknown or the decay is unknown (indeterminate).
    pub fn fidelity_at(&self, now: DateTime<Utc>) -> Result<Option<f64>, String> {
        if !self.decay_known() {
            return Ok(None);
        }
        let curve = self.curve()?;
        let age_seconds = (now - self.measured_at).num_milliseconds() as f64 / 1000.0;
        Ok(self.at_measurement().map(|f| {
            crate::compute::apply_decay(f, curve.temporal(age_seconds), self.baseline.unwrap_or(0.0))
        }))
    }

    /// The temporal curve (half-life, fast share, echo), validated.
    pub fn curve(&self) -> Result<crate::compute::Curve, String> {
        crate::compute::curve(self.half_life.as_deref(), self.fast.as_ref(), self.echo.as_ref(), self.shape)
    }

    /// Fidelity at measurement: source × ∏(1 − noise). None if the source is unknown.
    pub fn at_measurement(&self) -> Option<f64> {
        let noise: Vec<f64> = self.transforms.iter().map(|t| t.noise).collect();
        self.source_fidelity().map(|s| s * crate::compute::transform_factor(&noise))
    }

    /// When fidelity falls below `floor`: the TTL implied by measured decay.
    ///
    /// Ok(None) if the source class is Unknown (indeterminate).
    pub fn expires_at(&self, floor: f64) -> Result<Option<Expiry>, String> {
        let curve = self.curve()?;
        if !self.decay_known() {
            return Ok(None);
        }
        let Some(f) = self.at_measurement() else {
            return Ok(None);
        };
        let ttl = curve.time_to_floor(f, self.baseline.unwrap_or(0.0), floor);
        Ok(Some(match ttl {
            crate::compute::TimeToFloor::Never => Expiry::Never,
            crate::compute::TimeToFloor::After(seconds) => Expiry::At(self.after(seconds)),
        }))
    }

    /// When to re-check this fact: the interval that minimizes staleness
    /// plus `price` per re-check a day (`compute::best_interval`), and never
    /// later than the floor's TTL when a floor is given.
    ///
    /// Ok(None) if the source class is Unknown (indeterminate).
    pub fn recheck_at(&self, price: f64, floor: Option<f64>) -> Result<Option<Expiry>, String> {
        let curve = self.curve()?;
        if !self.decay_known() {
            return Ok(None);
        }
        let Some(f) = self.at_measurement() else {
            return Ok(None);
        };
        let interval = crate::compute::best_interval(&curve, f, self.baseline.unwrap_or(0.0), price)?;
        self.guarded(interval, floor).map(Some)
    }

    /// When to re-check this fact to keep its mean staleness at or below
    /// `target`, and never later than the floor's TTL when a floor is given.
    pub fn recheck_for_staleness(&self, target: f64, floor: Option<f64>) -> Result<Option<Expiry>, String> {
        let curve = self.curve()?;
        if !self.decay_known() {
            return Ok(None);
        }
        let Some(f) = self.at_measurement() else {
            return Ok(None);
        };
        let interval = crate::compute::interval_for_staleness(&curve, f, self.baseline.unwrap_or(0.0), target)?;
        self.guarded(interval, floor).map(Some)
    }

    fn guarded(&self, interval: crate::compute::Recheck, floor: Option<f64>) -> Result<Expiry, String> {
        let mut at = match interval {
            crate::compute::Recheck::Never => Expiry::Never,
            crate::compute::Recheck::Every(seconds) => Expiry::At(self.after(seconds)),
        };
        if let Some(floor) = floor {
            if let Some(Expiry::At(ttl)) = self.expires_at(floor)? {
                at = match at {
                    Expiry::At(t) if t <= ttl => Expiry::At(t),
                    _ => Expiry::At(ttl),
                };
            }
        }
        Ok(at)
    }

    fn after(&self, seconds: f64) -> DateTime<Utc> {
        self.measured_at + chrono::Duration::milliseconds((seconds * 1000.0).round().min(9.0e15) as i64)
    }
}

/// When a fact's fidelity falls below a floor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Expiry {
    At(DateTime<Utc>),
    Never,
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

/// Integrity hash of content in the legacy canonical form (versions 0.1 to
/// 0.4): sha256 over its compact JSON serialization, as `sha256:<hex>`.
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

    /// Baseline that temporal decay goes toward (0.0 if none)
    pub baseline: f64,

    /// Transformation noise factor
    pub transforms: f64,

    /// Total fidelity: baseline + (source × transforms − baseline) × temporal
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
            baseline: None,
            source_fidelity: None,
            ext: None,
            echo: None,
            fast: None,
            shape: None,
            info_type: None,
            state: None,
            transforms: vec![TransformStep {
                step: "normalize.py".to_string(),
                noise: 0.05,
                at,
                condition: None,
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
    fn test_v01_envelope_without_new_fields_still_reads() {
        let mut value = serde_json::to_value(sample_envelope()).unwrap();
        let obj = value.as_object_mut().unwrap();
        obj.remove("baseline");
        obj.remove("source_fidelity");
        obj.remove("info_type");
        obj.remove("state");
        obj.insert("fid_version".into(), Value::String("0.1".into()));
        let env: FidEnvelope = serde_json::from_value(value).unwrap();
        assert_eq!(env.baseline, None);
        assert_eq!(env.transforms[0].condition, None);
    }

    #[test]
    fn test_new_fields_omitted_when_unset() {
        let value = serde_json::to_value(sample_envelope()).unwrap();
        assert!(value.get("baseline").is_none());
        assert!(value.get("source_fidelity").is_none());
        assert!(value.get("state").is_none());
        assert!(value["transforms"][0].get("condition").is_none());
    }

    #[test]
    fn test_fidelity_at_uses_baseline() {
        let mut env = sample_envelope();
        env.source_class = SourceClass::Database; // 0.90
        env.transforms.clear();
        env.half_life = Some("1d".into());
        env.baseline = Some(0.70);
        let f = env.fidelity_at(env.measured_at + chrono::Duration::days(1)).unwrap().unwrap();
        assert!((f - 0.80).abs() < 1e-10);
    }

    #[test]
    fn test_measured_source_fidelity_replaces_class() {
        let mut env = sample_envelope();
        env.source_class = SourceClass::Database; // 0.90
        env.transforms.clear();
        env.half_life = Some("1d".into());
        env.source_fidelity = Some(0.80);
        assert_eq!(env.source_fidelity(), Some(0.80));
        let f = env.fidelity_at(env.measured_at + chrono::Duration::days(1)).unwrap().unwrap();
        assert!((f - 0.40).abs() < 1e-10);
        // 0.80 → 0.40 takes one half-life
        assert_eq!(env.expires_at(0.40).unwrap(), Some(Expiry::At(env.measured_at + chrono::Duration::days(1))));
    }

    #[test]
    fn test_unknown_class_stays_unknown_with_measured_fidelity() {
        let mut env = sample_envelope();
        env.source_class = SourceClass::Unknown;
        env.source_fidelity = Some(0.95);
        assert_eq!(env.source_fidelity(), None);
        assert_eq!(env.fidelity_at(env.measured_at).unwrap(), None);
        assert_eq!(env.expires_at(0.5).unwrap(), None);
    }

    #[test]
    fn test_expires_at() {
        let mut env = sample_envelope();
        env.source_class = SourceClass::Database; // 0.90
        env.transforms.clear();
        env.half_life = Some("1d".into());
        // 0.90 → 0.45 takes one half-life
        assert_eq!(env.expires_at(0.45).unwrap(), Some(Expiry::At(env.measured_at + chrono::Duration::days(1))));
        env.baseline = Some(0.5);
        assert_eq!(env.expires_at(0.45).unwrap(), Some(Expiry::Never));
        env.source_class = SourceClass::Unknown;
        assert_eq!(env.expires_at(0.45).unwrap(), None);
    }

    #[test]
    fn test_fast_share_in_the_envelope() {
        let mut env = sample_envelope();
        env.source_class = SourceClass::Instrument;
        env.source_fidelity = Some(1.0);
        env.transforms.clear();
        env.half_life = Some("80d".into());
        env.fast = Some(crate::compute::Fast { share: 0.3, half_life: "3d".into() });
        let day = chrono::Duration::days(3);
        let f = env.fidelity_at(env.measured_at + day).unwrap().unwrap();
        let want = 0.7 * 0.5_f64.powf(3.0 / 80.0) + 0.3 * 0.5;
        assert!((f - want).abs() < 1e-9, "{}", f);
        // round trip, and absent when unset
        let back: FidEnvelope = serde_json::from_value(serde_json::to_value(&env).unwrap()).unwrap();
        assert_eq!(back.fast, env.fast);
        assert!(serde_json::to_value(sample_envelope()).unwrap().get("fast").is_none());
        // an invalid fast share is an error, never ignored
        env.fast = Some(crate::compute::Fast { share: 0.3, half_life: "90d".into() });
        assert!(env.fidelity_at(env.measured_at).is_err());
        assert!(env.expires_at(0.5).is_err());
    }

    #[test]
    fn test_shape_in_the_envelope() {
        let mut env = sample_envelope();
        env.source_class = SourceClass::Instrument;
        env.source_fidelity = Some(1.0);
        env.transforms.clear();
        env.half_life = Some("3m".into());
        env.shape = Some(3.0);
        let f = env.fidelity_at(env.measured_at + chrono::Duration::minutes(3)).unwrap().unwrap();
        assert!((f - 0.5).abs() < 1e-9);
        let Some(Expiry::At(at)) = env.expires_at(0.5).unwrap() else { panic!() };
        assert_eq!(at, env.measured_at + chrono::Duration::minutes(3));
        let back: FidEnvelope = serde_json::from_value(serde_json::to_value(&env).unwrap()).unwrap();
        assert_eq!(back.shape, Some(3.0));
        assert!(serde_json::to_value(sample_envelope()).unwrap().get("shape").is_none());
        env.shape = Some(-1.0);
        assert!(env.fidelity_at(env.measured_at).is_err());
    }

    #[test]
    fn test_recheck_at() {
        let mut env = sample_envelope();
        env.source_class = SourceClass::Instrument;
        env.source_fidelity = Some(1.0);
        env.transforms.clear();
        env.half_life = Some("1d".into());
        env.baseline = Some(0.1);
        let curve = env.curve().unwrap();
        let crate::compute::Recheck::Every(t) = crate::compute::best_interval(&curve, 1.0, 0.1, 1e-3).unwrap() else { panic!() };
        let Some(Expiry::At(at)) = env.recheck_at(1e-3, None).unwrap() else { panic!() };
        assert_eq!(at, env.measured_at + chrono::Duration::milliseconds((t * 1000.0).round() as i64));
        // a floor reached sooner caps it
        let Some(Expiry::At(ttl)) = env.expires_at(0.99).unwrap() else { panic!() };
        assert!(ttl < at);
        assert_eq!(env.recheck_at(1e-3, Some(0.99)).unwrap(), Some(Expiry::At(ttl)));
        // a floor reached later does not
        assert_eq!(env.recheck_at(1e-3, Some(0.2)).unwrap(), Some(Expiry::At(at)));
        // too dear to re-check: never, unless the floor says otherwise
        assert_eq!(env.recheck_at(1e6, None).unwrap(), Some(Expiry::Never));
        assert_eq!(env.recheck_at(1e6, Some(0.5)).unwrap(), env.expires_at(0.5).unwrap());
        // staleness target
        let Some(Expiry::At(s)) = env.recheck_for_staleness(0.05, None).unwrap() else { panic!() };
        let secs = (s - env.measured_at).num_milliseconds() as f64 / 1000.0;
        assert!((curve.mean_staleness(1.0, 0.1, secs) - 0.05).abs() < 1e-6);
        // unknown source: no schedule
        env.source_class = SourceClass::Unknown;
        assert_eq!(env.recheck_at(1e-3, None).unwrap(), None);
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
