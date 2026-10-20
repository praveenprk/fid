//! Registries — measured decay and noise values, looked up by name.
//!
//! A half-life registry maps an information type to its half-life, and
//! optionally a baseline and per-state overrides. A type's half-life can
//! depend on the state of the thing observed: a ship's position goes
//! stale in minutes while it is underway and in days while it is moored.
//!
//! ```yaml
//! employee_role: 1y                     # half-life only
//! airport_flight_category:              # decays toward a baseline
//!   half_life: 2.9h
//!   baseline: 0.73
//! policy_clause:                        # measured fidelity when fresh
//!   half_life: 90d
//!   source_fidelity: 0.94
//! package_latest_version:               # a share goes stale on a faster clock
//!   half_life: 80d
//!   baseline: 0.1
//!   fast: { share: 0.3, half_life: 3d }
//! vessel_underway:                      # holds, then drops: shape above 1
//!   half_life: 3m
//!   shape: 2.5
//! airport_temperature:                  # comes back once a day
//!   half_life: 2.42h
//!   baseline: 0.13
//!   echo: { period: 24h, strength: 0.31, width: 3.75h, fade: 345h }
//! vessel_position:                      # depends on state
//!   states:
//!     underway: 2.3m
//!     moored: { half_life: 1.6d }
//! ```
//!
//! A noise model maps a transform to its noise, and optionally to noise
//! conditioned on what is known upstream:
//!
//! ```yaml
//! summarize: 0.15
//! flight_leg:
//!   noise: 0.034
//!   conditions:
//!     previous_leg_late: 0.54
//! ```
//!
//! Lookups fail closed: an unknown type, state, transform or condition
//! is an error, never a silent default.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::compute::{curve, Echo, Fast};

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum HalfLifeEntry {
    /// `type: 3y`
    Simple(String),
    /// `type: { half_life, baseline, source_fidelity, states }`
    Spec(HalfLifeSpec),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HalfLifeSpec {
    pub half_life: Option<String>,
    pub baseline: Option<f64>,
    /// Measured fraction correct when fresh; replaces the source class constant
    pub source_fidelity: Option<f64>,
    /// Periodic return, for facts that come back (temperature, load)
    pub echo: Option<Echo>,
    /// A share that goes stale on a faster clock
    pub fast: Option<Fast>,
    /// Shape of the clocks: 0.5^((t/h)^shape)
    pub shape: Option<f64>,
    #[serde(default)]
    pub states: BTreeMap<String, HalfLifeEntry>,
}

impl HalfLifeEntry {
    fn spec(&self) -> HalfLifeSpec {
        match self {
            Self::Simple(h) => HalfLifeSpec { half_life: Some(h.clone()), ..Default::default() },
            Self::Spec(s) => s.clone(),
        }
    }
}

/// A resolved decay specification, ready for the envelope.
#[derive(Debug, Clone, PartialEq)]
pub struct DecaySpec {
    pub half_life: String,
    pub baseline: Option<f64>,
    pub source_fidelity: Option<f64>,
    pub echo: Option<Echo>,
    pub fast: Option<Fast>,
    pub shape: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct HalfLives(BTreeMap<String, HalfLifeEntry>);

impl HalfLives {
    pub fn from_yaml(text: &str) -> Result<Self, String> {
        serde_yaml::from_str(text)
            .map(Self)
            .map_err(|e| format!("invalid half-life registry: {}", e))
    }

    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;
        Self::from_yaml(&text).map_err(|e| format!("{}: {}", path, e))
    }

    /// Resolve the half-life and baseline for a type, and state if given.
    /// A state's values override the type's.
    pub fn lookup(&self, info_type: &str, state: Option<&str>) -> Result<DecaySpec, String> {
        let entry = self.0.get(info_type).ok_or_else(|| {
            format!("unknown information type '{}' (known: {})", info_type, names(self.0.keys()))
        })?;
        let mut spec = entry.spec();

        if let Some(state) = state {
            let child = spec.states.get(state).ok_or_else(|| {
                if spec.states.is_empty() {
                    format!("information type '{}' has no states", info_type)
                } else {
                    format!(
                        "unknown state '{}' for '{}' (known: {})",
                        state,
                        info_type,
                        names(spec.states.keys())
                    )
                }
            })?;
            let child = child.spec();
            if !child.states.is_empty() {
                return Err(format!("state '{}' of '{}' cannot have its own states", state, info_type));
            }
            spec.half_life = child.half_life.or(spec.half_life);
            spec.baseline = child.baseline.or(spec.baseline);
            spec.source_fidelity = child.source_fidelity.or(spec.source_fidelity);
            spec.echo = child.echo.or(spec.echo);
            spec.fast = child.fast.or(spec.fast);
            spec.shape = child.shape.or(spec.shape);
        }

        let half_life = spec.half_life.ok_or_else(|| {
            format!("information type '{}' needs a state (one of: {})", info_type, names(spec.states.keys()))
        })?;
        curve(Some(&half_life), spec.fast.as_ref(), spec.echo.as_ref(), spec.shape).map_err(|e| format!("'{}': {}", info_type, e))?;
        if let Some(b) = spec.baseline {
            check_unit(b, "baseline").map_err(|e| format!("'{}': {}", info_type, e))?;
        }
        if let Some(s) = spec.source_fidelity {
            check_unit(s, "source fidelity").map_err(|e| format!("'{}': {}", info_type, e))?;
        }
        Ok(DecaySpec {
            half_life,
            baseline: spec.baseline,
            source_fidelity: spec.source_fidelity,
            echo: spec.echo,
            fast: spec.fast,
            shape: spec.shape,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum NoiseEntry {
    /// `transform: 0.15`
    Simple(f64),
    /// `transform: { noise, conditions }`
    Spec(NoiseSpec),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseSpec {
    pub noise: Option<f64>,
    #[serde(default)]
    pub conditions: BTreeMap<String, f64>,
}

#[derive(Debug, Clone)]
pub struct NoiseModel(BTreeMap<String, NoiseEntry>);

impl NoiseModel {
    pub fn from_yaml(text: &str) -> Result<Self, String> {
        serde_yaml::from_str(text)
            .map(Self)
            .map_err(|e| format!("invalid noise model: {}", e))
    }

    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;
        Self::from_yaml(&text).map_err(|e| format!("{}: {}", path, e))
    }

    /// Resolve the noise for a transform, conditioned if a condition is given.
    pub fn lookup(&self, transform: &str, condition: Option<&str>) -> Result<f64, String> {
        let entry = self.0.get(transform).ok_or_else(|| {
            format!("unknown transform '{}' (known: {})", transform, names(self.0.keys()))
        })?;
        let noise = match (entry, condition) {
            (NoiseEntry::Simple(_), Some(c)) => {
                return Err(format!("transform '{}' has no conditions (asked for '{}')", transform, c))
            }
            (NoiseEntry::Simple(n), None) => *n,
            (NoiseEntry::Spec(spec), Some(c)) => *spec.conditions.get(c).ok_or_else(|| {
                format!(
                    "unknown condition '{}' for '{}' (known: {})",
                    c,
                    transform,
                    names(spec.conditions.keys())
                )
            })?,
            (NoiseEntry::Spec(spec), None) => spec.noise.ok_or_else(|| {
                format!(
                    "transform '{}' needs a condition (one of: {})",
                    transform,
                    names(spec.conditions.keys())
                )
            })?,
        };
        check_unit(noise, "noise").map_err(|e| format!("'{}': {}", transform, e))?;
        Ok(noise)
    }
}

/// Check a value is in [0, 1].
pub fn check_unit(value: f64, what: &str) -> Result<(), String> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("invalid {} {} (must be between 0.0 and 1.0)", what, value))
    }
}

fn names<'a>(keys: impl Iterator<Item = &'a String>) -> String {
    keys.map(String::as_str).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF_LIVES: &str = "
employee_role: 1y
legal_name: 20y
medical_diagnosis: domain_specific
airport_flight_category:
  half_life: 2.9h
  baseline: 0.73
vessel_position:
  states:
    underway: 2.3m
    moored: { half_life: 1.6d, baseline: 0.1 }
with_default:
  half_life: 1d
  baseline: 0.2
  source_fidelity: 0.9
  states:
    fast: 1h
    checked: { half_life: 2h, source_fidelity: 0.75 }
";

    const NOISE: &str = "
summarize: 0.15
flight_leg:
  noise: 0.034
  conditions:
    previous_leg_late: 0.54
conditional_only:
  conditions:
    a: 0.1
";

    fn half_lives() -> HalfLives {
        HalfLives::from_yaml(HALF_LIVES).unwrap()
    }

    fn noise() -> NoiseModel {
        NoiseModel::from_yaml(NOISE).unwrap()
    }

    // spec: half-life-registry
    #[test]
    fn test_simple_half_life() {
        let spec = half_lives().lookup("employee_role", None).unwrap();
        assert_eq!(spec, DecaySpec { half_life: "1y".into(), baseline: None, source_fidelity: None, echo: None, fast: None, shape: None });
    }

    // spec: half-life-registry
    #[test]
    fn test_half_life_with_baseline() {
        let spec = half_lives().lookup("airport_flight_category", None).unwrap();
        assert_eq!(spec.half_life, "2.9h");
        assert_eq!(spec.baseline, Some(0.73));
    }

    // spec: half-life-registry
    #[test]
    fn test_state_half_life() {
        let h = half_lives();
        assert_eq!(h.lookup("vessel_position", Some("underway")).unwrap().half_life, "2.3m");
        let moored = h.lookup("vessel_position", Some("moored")).unwrap();
        assert_eq!(moored.half_life, "1.6d");
        assert_eq!(moored.baseline, Some(0.1));
    }

    // spec: half-life-registry (deviation #11)
    #[test]
    fn test_state_inherits_type_baseline() {
        let spec = half_lives().lookup("with_default", Some("fast")).unwrap();
        assert_eq!(spec, DecaySpec { half_life: "1h".into(), baseline: Some(0.2), source_fidelity: Some(0.9), echo: None, fast: None, shape: None });
        assert_eq!(half_lives().lookup("with_default", None).unwrap().half_life, "1d");
    }

    // spec: half-life-registry
    #[test]
    fn test_state_overrides_source_fidelity() {
        let spec = half_lives().lookup("with_default", Some("checked")).unwrap();
        assert_eq!(spec.source_fidelity, Some(0.75));
        assert_eq!(spec.baseline, Some(0.2));
    }

    // spec: half-life-registry
    #[test]
    fn test_echo_lookup_and_validation() {
        let h = HalfLives::from_yaml(
            "temp: { half_life: 2.4h, baseline: 0.13, echo: { period: 24h, strength: 0.31, width: 3.75h, fade: 345h } }\n\
             wide: { half_life: 2.4h, echo: { period: 24h, strength: 0.31, width: 13h, fade: 345h } }\n\
             flat: { half_life: infinite, echo: { period: 24h, strength: 0.31, width: 3h, fade: 345h } }",
        )
        .unwrap();
        let echo = h.lookup("temp", None).unwrap().echo.unwrap();
        assert_eq!(echo.period, "24h");
        assert_eq!(echo.strength, 0.31);
        assert!(h.lookup("wide", None).unwrap_err().contains("half the period"));
        assert!(h.lookup("flat", None).unwrap_err().contains("finite half-life"));
        assert!(HalfLives::from_yaml("x: { half_life: 1h, echo: { period: 24h, strength: 0.3, width: 3h } }").is_err());
    }

    // spec: half-life-registry
    #[test]
    fn test_fast_lookup_and_validation() {
        let h = HalfLives::from_yaml(
            "pkg: { half_life: 80d, baseline: 0.1, fast: { share: 0.3, half_life: 3d }, states: { quiet: { half_life: 120d }, fresh: { fast: { share: 0.5, half_life: 1d } } } }\n\
             slow_fast: { half_life: 1d, fast: { share: 0.3, half_life: 3d } }\n\
             big: { half_life: 80d, fast: { share: 1.5, half_life: 3d } }",
        )
        .unwrap();
        let f = h.lookup("pkg", None).unwrap().fast.unwrap();
        assert_eq!((f.share, f.half_life.as_str()), (0.3, "3d"));
        // a state inherits the type's fast share, or overrides it
        assert_eq!(h.lookup("pkg", Some("quiet")).unwrap().fast.unwrap().share, 0.3);
        assert_eq!(h.lookup("pkg", Some("fresh")).unwrap().fast.unwrap().half_life, "1d");
        assert!(h.lookup("slow_fast", None).unwrap_err().contains("shorter"));
        assert!(h.lookup("big", None).unwrap_err().contains("share"));
        assert!(HalfLives::from_yaml("x: { half_life: 1h, fast: { share: 0.3 } }").is_err());
    }

    // spec: half-life-registry
    #[test]
    fn test_shape_lookup_and_validation() {
        let h = HalfLives::from_yaml(
            "ship: { half_life: 3m, baseline: 0.03, shape: 2.5, states: { slow: { half_life: 8m }, flat: { shape: 1 } } }\n\
             bad: { half_life: 3m, shape: 0 }",
        )
        .unwrap();
        assert_eq!(h.lookup("ship", None).unwrap().shape, Some(2.5));
        assert_eq!(h.lookup("ship", Some("slow")).unwrap().shape, Some(2.5));
        assert_eq!(h.lookup("ship", Some("flat")).unwrap().shape, Some(1.0));
        assert!(h.lookup("bad", None).unwrap_err().contains("shape"));
    }

    // spec: half-life-registry
    #[test]
    fn test_bad_source_fidelity_is_error() {
        let h = HalfLives::from_yaml("x: { half_life: 1d, source_fidelity: 1.2 }").unwrap();
        assert!(h.lookup("x", None).unwrap_err().contains("source fidelity"));
    }

    // spec: half-life-registry
    #[test]
    fn test_stateful_type_without_state_is_error() {
        let err = half_lives().lookup("vessel_position", None).unwrap_err();
        assert!(err.contains("needs a state") && err.contains("underway"));
    }

    // spec: half-life-registry
    #[test]
    fn test_unknown_type_state_is_error() {
        let h = half_lives();
        assert!(h.lookup("nope", None).unwrap_err().contains("employee_role"));
        assert!(h.lookup("vessel_position", Some("sinking")).unwrap_err().contains("moored"));
        assert!(h.lookup("employee_role", Some("x")).unwrap_err().contains("no states"));
    }

    // spec: half-life-registry
    #[test]
    fn test_unparseable_half_life_fails_on_lookup_only() {
        let h = half_lives();
        assert!(h.lookup("medical_diagnosis", None).is_err());
        assert!(h.lookup("legal_name", None).is_ok());
    }

    // spec: half-life-registry
    #[test]
    fn test_bad_baseline_is_error() {
        let h = HalfLives::from_yaml("x: { half_life: 1d, baseline: 1.5 }").unwrap();
        assert!(h.lookup("x", None).is_err());
    }

    // spec: half-life-registry
    #[test]
    fn test_unknown_field_is_error() {
        assert!(HalfLives::from_yaml("x: { half_life: 1d, baselin: 0.5 }").is_err());
        assert!(NoiseModel::from_yaml("x: { noise: 0.1, condition: {} }").is_err());
    }

    // spec: noise-model
    #[test]
    fn test_noise_lookup() {
        let n = noise();
        assert_eq!(n.lookup("summarize", None).unwrap(), 0.15);
        assert_eq!(n.lookup("flight_leg", None).unwrap(), 0.034);
        assert_eq!(n.lookup("flight_leg", Some("previous_leg_late")).unwrap(), 0.54);
    }

    // spec: noise-model
    #[test]
    fn test_noise_lookup_errors() {
        let n = noise();
        assert!(n.lookup("nope", None).unwrap_err().contains("summarize"));
        assert!(n.lookup("summarize", Some("x")).unwrap_err().contains("no conditions"));
        assert!(n.lookup("flight_leg", Some("x")).unwrap_err().contains("previous_leg_late"));
        assert!(n.lookup("conditional_only", None).unwrap_err().contains("needs a condition"));
        let bad = NoiseModel::from_yaml("x: 1.2").unwrap();
        assert!(bad.lookup("x", None).is_err());
    }

    // spec: half-life-registry
    #[test]
    fn test_example_files_parse() {
        let root = env!("CARGO_MANIFEST_DIR");
        HalfLives::load(&format!("{}/examples/half-lives.yaml", root)).unwrap();
        NoiseModel::load(&format!("{}/examples/noise-model.yaml", root)).unwrap();
    }
}
