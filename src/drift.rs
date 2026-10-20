//! `fid drift` — is a calibrated registry still right?
//!
//! Decay rates change: a company's churn shifts, a season starts. Drift
//! takes recent re-checks (the same CSV `fid calibrate` reads) and the
//! registry in force, and tests whether the registry's curve still
//! describes them.
//!
//! The test is a likelihood ratio on the raw checks: the registry's
//! curve against the curve refitted to the recent checks. Twice the gap
//! in log-likelihood is χ² with one degree of freedom per value the
//! registry fixes (half-life and baseline, plus source fidelity when it
//! has one; without it, s0 is refitted for both). Repeated checks of one
//! entity are not independent, so the statistic is divided by their
//! design effect (residuals summed per entity), when above 1.
//!
//!   DRIFTED  — p < 0.05: recalibrate
//!   UNKNOWN  — not rejected, but the recent half-life's 95% range is
//!              wider than `max_spread`: too few recent checks to tell
//!   CURRENT  — not rejected, and the recent checks pin the half-life

use std::collections::BTreeMap;

use crate::calibrate::{self, bins, fit, format_half_life, Bin, Model, Observation, StateFit};
use crate::compute::parse_half_life;
use crate::registry::HalfLives;

/// Significance level of the drift test.
pub const ALPHA: f64 = 0.05;

/// Drift verdict for one state; maps to exit codes like `check`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    /// Exit 0
    Current,
    /// Exit 1
    Drifted,
    /// Exit 2
    Unknown,
}

impl Verdict {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Current => 0,
            Self::Drifted => 1,
            Self::Unknown => 2,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Current => "CURRENT",
            Self::Drifted => "DRIFTED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// The registry's values for one state; half-life in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Registered {
    pub half_life: f64,
    pub baseline: f64,
    pub source_fidelity: Option<f64>,
}

/// Drift for one state.
#[derive(Debug, Clone)]
pub struct StateDrift {
    /// calibrate's fit of the recent checks, with 95% ranges
    pub recent: StateFit,
    pub registered: Registered,
    /// The registry's curve, s0 refitted when the registry has none
    pub registry_model: Model,
    /// The recent curve the test compares against, baseline allowed
    pub refit: Model,
    /// Likelihood-ratio statistic, after the overdispersion correction
    pub chi2: f64,
    pub df: usize,
    pub dispersion: f64,
    pub p_value: f64,
    /// Largest gap between each curve and the recent binned data
    pub registry_gap: f64,
    pub refit_gap: f64,
    pub verdict: Verdict,
}

/// Compare recent re-checks of `info_type` with its registry entry.
/// Either every observation has a state or none does, as in calibrate.
pub fn drift(obs: &[Observation], registry: &HalfLives, info_type: &str, max_spread: f64) -> Result<Vec<StateDrift>, String> {
    if max_spread.is_nan() || max_spread < 1.0 {
        return Err(format!("invalid max spread {} (must be at least 1)", max_spread));
    }
    let fits = calibrate::calibrate_one_clock(obs, f64::INFINITY)?;

    let mut groups: BTreeMap<Option<String>, Vec<&Observation>> = BTreeMap::new();
    for o in obs {
        groups.entry(o.state.clone()).or_default().push(o);
    }
    groups
        .into_values()
        .zip(fits)
        .map(|(group, recent)| {
            let spec = registry.lookup(info_type, recent.state.as_deref())?;
            if spec.fast.is_some() || spec.shape.is_some() {
                // The test compares plain decay curves; a fast share or shape would be dropped silently
                return Err(format!(
                    "'{}'{} has a fast share or a shape: drift tests plain decay only; refit with fid calibrate instead",
                    info_type,
                    recent.state.as_deref().map(|s| format!(" state '{}'", s)).unwrap_or_default()
                ));
            }
            let registered = Registered {
                half_life: parse_half_life(&spec.half_life)?.unwrap_or(f64::INFINITY),
                baseline: spec.baseline.unwrap_or(0.0),
                source_fidelity: spec.source_fidelity,
            };
            Ok(state_drift(recent, &group, registered, max_spread))
        })
        .collect()
}

/// Least-squares s0 for a fixed half-life and baseline.
fn s0_given(bins: &[Bin], half_life: f64, baseline: f64) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for b in bins {
        let d = if half_life.is_infinite() { 1.0 } else { 0.5_f64.powf(b.age / half_life) };
        num += b.n as f64 * d * (b.correct - baseline);
        den += b.n as f64 * d * d;
    }
    if den > 0.0 {
        (baseline + num / den).clamp(baseline, 1.0)
    } else {
        baseline
    }
}

fn log_likelihood(group: &[&Observation], m: &Model) -> f64 {
    group
        .iter()
        .map(|o| {
            let p = m.predict(o.age).clamp(1e-9, 1.0 - 1e-9);
            if o.correct {
                p.ln()
            } else {
                (1.0 - p).ln()
            }
        })
        .sum()
}

/// Design effect of checking one entity more than once: the refit's
/// residuals summed within each entity, squared, over the plain sum of
/// squared residuals. 1 for one check per entity; at least 1.
fn dispersion(group: &[&Observation], m: &Model) -> f64 {
    let mut by_entity: BTreeMap<&str, f64> = BTreeMap::new();
    let mut plain = 0.0;
    for o in group {
        let r = (o.correct as u8 as f64) - m.predict(o.age);
        *by_entity.entry(o.entity.as_str()).or_insert(0.0) += r;
        plain += r * r;
    }
    if plain <= 0.0 {
        return 1.0;
    }
    (by_entity.values().map(|r| r * r).sum::<f64>() / plain).max(1.0)
}

/// P(χ² with `df` degrees of freedom > x), for df 2 or 3.
pub fn chi2_survival(x: f64, df: usize) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    match df {
        2 => (-x / 2.0).exp(),
        3 => erfc((x / 2.0).sqrt()) + (2.0 * x / std::f64::consts::PI).sqrt() * (-x / 2.0).exp(),
        _ => unreachable!("drift tests 2 or 3 values"),
    }
}

/// Complementary error function (Abramowitz & Stegun 7.1.26, error < 1.5e-7).
fn erfc(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    poly * (-x * x).exp()
}

fn state_drift(recent: StateFit, group: &[&Observation], registered: Registered, max_spread: f64) -> StateDrift {
    let all_bins = bins(group);
    let refit = fit(&all_bins, true);
    let registry_model = Model {
        s0: registered
            .source_fidelity
            .unwrap_or_else(|| s0_given(&all_bins, registered.half_life, registered.baseline)),
        baseline: registered.baseline,
        half_life: registered.half_life,
    };
    let df = if registered.source_fidelity.is_some() { 3 } else { 2 };
    let dispersion = dispersion(group, &refit);
    let lr = 2.0 * (log_likelihood(group, &refit) - log_likelihood(group, &registry_model));
    let chi2 = (lr / dispersion).max(0.0);
    let p_value = chi2_survival(chi2, df);
    let gap = |m: &Model| all_bins.iter().map(|b| (b.correct - m.predict(b.age)).abs()).fold(0.0, f64::max);

    // A rejection holds at any sample size; calling the registry current
    // needs enough recent checks to have seen a change
    let verdict = if p_value < ALPHA {
        Verdict::Drifted
    } else if recent.spread > max_spread {
        Verdict::Unknown
    } else {
        Verdict::Current
    };
    StateDrift {
        registry_gap: gap(&registry_model),
        refit_gap: gap(&refit),
        recent,
        registered,
        registry_model,
        refit,
        chi2,
        df,
        dispersion,
        p_value,
        verdict,
    }
}

/// Exit code for several states: any drift is drift; otherwise any
/// unknown is unknown.
pub fn overall(drifts: &[StateDrift]) -> Verdict {
    if drifts.iter().any(|d| d.verdict == Verdict::Drifted) {
        Verdict::Drifted
    } else if drifts.iter().any(|d| d.verdict == Verdict::Unknown) {
        Verdict::Unknown
    } else {
        Verdict::Current
    }
}

/// The human-readable report for stderr.
pub fn report(info_type: &str, drifts: &[StateDrift], max_spread: f64) -> String {
    let mut lines = Vec::new();
    for d in drifts {
        let r = &d.recent;
        let name = match &r.state {
            Some(s) => format!("{} / {}", info_type, s),
            None => info_type.to_string(),
        };
        lines.push(format!(
            "{:<8} {} — {} recent checks, {} entities",
            d.verdict.label(),
            name,
            r.observations,
            r.entities
        ));
        match d.verdict {
            Verdict::Unknown => lines.push(format!(
                "  not rejected (p = {:.4}), but the recent half-life range ×{:.1} is wider than ×{}: too few recent checks to tell",
                d.p_value, r.spread, max_spread
            )),
            _ => lines.push(format!(
                "  registry curve vs recent refit: χ² {:.2} on {} df (design effect {:.2}), p = {:.4}{}",
                d.chi2,
                d.df,
                d.dispersion,
                d.p_value,
                if d.verdict == Verdict::Drifted { "  → recalibrate" } else { "" }
            )),
        }
        let u = &r.uncertainty;
        lines.push(format!(
            "  half-life        registry {:<10} recent {} (95% {} to {})",
            format_half_life(d.registered.half_life),
            format_half_life(r.model.half_life),
            format_half_life(u.half_life.lo),
            format_half_life(u.half_life.hi)
        ));
        lines.push(format!(
            "  baseline         registry {:<10.2} recent {:.2} (95% {:.2} to {:.2})",
            d.registered.baseline, r.model.baseline, u.baseline.lo, u.baseline.hi
        ));
        lines.push(format!(
            "  source fidelity  registry {:<10} recent {:.3} (95% {:.3} to {:.3})",
            d.registered.source_fidelity.map(|s| format!("{:.3}", s)).unwrap_or_else(|| "—".into()),
            r.model.s0,
            u.s0.lo,
            u.s0.hi
        ));
        lines.push(format!(
            "  largest gap from the recent curve: registry {:.3} · refit {:.3}",
            d.registry_gap, d.refit_gap
        ));
        lines.push(String::new());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibrate::{random_ledger, Rng};

    const DAY: f64 = 86_400.0;

    fn registry(yaml: &str) -> HalfLives {
        HalfLives::from_yaml(yaml).unwrap()
    }

    fn verdict(truth: Model, reg: &HalfLives, entities: usize, rng: &mut Rng) -> Verdict {
        let obs = random_ledger(truth, entities, 60.0 * DAY, rng);
        overall(&drift(&obs, reg, "t", calibrate::MAX_SPREAD).unwrap())
    }

    /// How often each verdict comes up over `trials` independent ledgers.
    fn tally(truth: Model, reg: &HalfLives, trials: usize, seed: u64) -> (usize, usize, usize) {
        let mut rng = Rng(seed);
        let (mut current, mut drifted, mut unknown) = (0, 0, 0);
        for _ in 0..trials {
            match verdict(truth, reg, 1500, &mut rng) {
                Verdict::Current => current += 1,
                Verdict::Drifted => drifted += 1,
                Verdict::Unknown => unknown += 1,
            }
        }
        (current, drifted, unknown)
    }

    #[test]
    fn test_unchanged_decay_is_mostly_current() {
        // Registry equals the truth: drift is a false alarm
        let truth = Model { s0: 0.95, baseline: 0.3, half_life: 10.0 * DAY };
        let reg = registry("t: { half_life: 10d, baseline: 0.3, source_fidelity: 0.95 }");
        let (current, drifted, unknown) = tally(truth, &reg, 30, 21);
        assert!(drifted <= 3, "false alarms: {} of 30", drifted);
        assert!(current >= 25, "current in {} of 30 ({} unknown)", current, unknown);
    }

    #[test]
    fn test_fast_share_is_refused_not_dropped() {
        let mut rng = Rng(5);
        let obs = random_ledger(Model { s0: 1.0, baseline: 0.1, half_life: 10.0 * DAY }, 200, 60.0 * DAY, &mut rng);
        let reg = registry("t: { half_life: 10d, baseline: 0.1, fast: { share: 0.2, half_life: 1d } }");
        assert!(drift(&obs, &reg, "t", calibrate::MAX_SPREAD).unwrap_err().contains("fast share"));
    }

    #[test]
    fn test_halved_half_life_is_drift() {
        let truth = Model { s0: 0.95, baseline: 0.3, half_life: 5.0 * DAY };
        let reg = registry("t: { half_life: 10d, baseline: 0.3 }");
        let (_, drifted, _) = tally(truth, &reg, 30, 22);
        assert!(drifted >= 27, "detected in {} of 30", drifted);
    }

    #[test]
    fn test_moved_baseline_is_drift() {
        let truth = Model { s0: 0.95, baseline: 0.5, half_life: 10.0 * DAY };
        let reg = registry("t: { half_life: 10d, baseline: 0.3 }");
        let (_, drifted, _) = tally(truth, &reg, 30, 23);
        assert!(drifted >= 27, "detected in {} of 30", drifted);
    }

    #[test]
    fn test_worse_source_is_drift() {
        let truth = Model { s0: 0.80, baseline: 0.3, half_life: 10.0 * DAY };
        let reg = registry("t: { half_life: 10d, baseline: 0.3, source_fidelity: 0.95 }");
        let obs = random_ledger(truth, 3000, 60.0 * DAY, &mut Rng(24));
        let drifts = drift(&obs, &reg, "t", calibrate::MAX_SPREAD).unwrap();
        assert_eq!(drifts[0].df, 3);
        assert_eq!(drifts[0].verdict, Verdict::Drifted, "p = {}", drifts[0].p_value);
        assert!(report("t", &drifts, 2.5).contains("recalibrate"));
    }

    #[test]
    fn test_few_recent_checks_is_unknown() {
        let truth = Model { s0: 0.95, baseline: 0.0, half_life: 6.0 * DAY };
        let reg = registry("t: 6d");
        let obs = random_ledger(truth, 60, 20.0 * DAY, &mut Rng(3));
        let d = drift(&obs, &reg, "t", calibrate::MAX_SPREAD).unwrap();
        assert_eq!(overall(&d), Verdict::Unknown);
        assert!(report("t", &d, 2.5).contains("too few recent checks"));
    }

    #[test]
    fn test_no_decay_matches_infinite() {
        let truth = Model { s0: 0.9, baseline: 0.0, half_life: f64::INFINITY };
        let obs = random_ledger(truth, 2000, 30.0 * DAY, &mut Rng(4));
        let d = drift(&obs, &registry("t: infinite"), "t", calibrate::MAX_SPREAD).unwrap();
        assert_eq!(overall(&d), Verdict::Current, "{:?}", d[0].recent.uncertainty);
    }

    #[test]
    fn test_chi2_survival() {
        // Reference values: scipy.stats.chi2.sf
        assert!((chi2_survival(5.991, 2) - 0.05).abs() < 1e-4);
        assert!((chi2_survival(7.815, 3) - 0.05).abs() < 1e-4);
        assert!((chi2_survival(1.0, 3) - 0.801_252).abs() < 1e-5);
        assert_eq!(chi2_survival(0.0, 3), 1.0);
    }

    #[test]
    fn test_states_and_registry_errors() {
        let mut rng = Rng(5);
        let mut obs = random_ledger(Model { s0: 1.0, baseline: 0.0, half_life: DAY }, 1500, 5.0 * DAY, &mut rng);
        for o in &mut obs {
            o.state = Some("moored".into());
        }
        let reg = registry("t: { states: { moored: 1d, underway: 3m } }");
        let d = drift(&obs, &reg, "t", calibrate::MAX_SPREAD).unwrap();
        assert_eq!(d[0].recent.state.as_deref(), Some("moored"));
        assert_eq!(overall(&d), Verdict::Current);
        assert!(drift(&obs, &reg, "nope", 2.5).is_err());
        assert!(drift(&obs, &registry("t: 1d"), "t", 2.5).unwrap_err().contains("no states"));
        assert!(drift(&obs, &reg, "t", 0.5).is_err());
    }
}

/// The measurements quoted in the docs: false alarms and power over 200
/// ledgers each. `cargo test --release --lib measure_ -- --ignored --nocapture`
#[cfg(test)]
mod measure {
    use super::*;
    use crate::calibrate::{random_ledger, Rng};
    const DAY: f64 = 86_400.0;

    /// Each entity's fact changes once at an exponential time; 5 checks per entity.
    fn shared_fate(h: f64, entities: usize, rng: &mut Rng) -> Vec<Observation> {
        let mut out = Vec::new();
        for i in 0..entities {
            let t = -h * (1.0 - rng.unit()).log2();
            for _ in 0..5 {
                let age = rng.unit() * 60.0 * DAY;
                out.push(Observation { entity: format!("e{}", i), age, correct: age < t, state: None });
            }
        }
        out
    }

    #[test]
    #[ignore]
    fn measure_false_alarms_and_power() {
        let reg = HalfLives::from_yaml("t: { half_life: 10d, baseline: 0.3, source_fidelity: 0.95 }").unwrap();
        let truth = Model { s0: 0.95, baseline: 0.3, half_life: 10.0 * DAY };
        let mut rng = Rng(101);
        let mut v = [0; 3];
        let mut ps = Vec::new();
        for _ in 0..200 {
            let d = &drift(&random_ledger(truth, 1500, 60.0 * DAY, &mut rng), &reg, "t", 2.5).unwrap()[0];
            v[d.verdict.exit_code() as usize] += 1;
            ps.push(d.p_value);
        }
        let below = |a: f64| ps.iter().filter(|&&p| p < a).count();
        eprintln!("INDEPENDENT current {} drifted {} unknown {}; p<0.05 {} p<0.2 {} p<0.5 {}", v[0], v[1], v[2], below(0.05), below(0.2), below(0.5));

        let reg = HalfLives::from_yaml("t: 10d").unwrap();
        let mut v = [0; 3];
        let mut ps = Vec::new();
        let mut disp = 0.0;
        for _ in 0..200 {
            let d = &drift(&shared_fate(10.0 * DAY, 600, &mut rng), &reg, "t", 2.5).unwrap()[0];
            v[d.verdict.exit_code() as usize] += 1;
            ps.push(d.p_value);
            disp += d.dispersion;
        }
        let below = |a: f64| ps.iter().filter(|&&p| p < a).count();
        eprintln!("SHARED FATE current {} drifted {} unknown {}; p<0.05 {} p<0.2 {} p<0.5 {}; mean dispersion {:.2}", v[0], v[1], v[2], below(0.05), below(0.2), below(0.5), disp / 200.0);
        let mut drifted = 0;
        for _ in 0..200 {
            let d = &drift(&shared_fate(7.0 * DAY, 600, &mut rng), &reg, "t", 2.5).unwrap()[0];
            drifted += (d.verdict == Verdict::Drifted) as usize;
        }
        eprintln!("SHARED FATE POWER 10d registry, 7d truth: drifted {} of 200", drifted);
        let mut drifted = 0;
        for _ in 0..200 {
            let d = &drift(&shared_fate(5.0 * DAY, 600, &mut rng), &reg, "t", 2.5).unwrap()[0];
            drifted += (d.verdict == Verdict::Drifted) as usize;
        }
        eprintln!("SHARED FATE POWER 10d registry, 5d truth: drifted {} of 200", drifted);
    }
}
