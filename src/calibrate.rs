//! `fid calibrate` — fit half-life and baseline from re-checked facts.
//!
//! Input is a log of facts that were checked again later: what the fact
//! was about, how old it was when checked, and whether it still held.
//! Output is the half-life registry YAML that `fid stamp --half-lives`
//! reads, so measured decay replaces declared decay.
//!
//! Model, per state:
//!
//!   S(age) = b + (s0 − b) × 0.5^(age / h)
//!
//! For a fixed h this is linear in b and s0, so each candidate h is
//! solved in closed form by weighted least squares, and h is searched on
//! a log grid. The model is fitted on half the entities and scored on
//! the other half (Brier score); the baseline is kept only when it
//! scores better there than decay to zero. The final values are then
//! refitted on all the data. Too little data is an error, never a guess.
//!
//! s0, the fraction correct when fresh, is the source's measured
//! fidelity. It is written to the registry only when enough checks are
//! fresh (the model has decayed less than a tenth of the way to the
//! baseline); otherwise it is an extrapolation and the source class
//! constant stays in force.
//!
//! The split and the method match calibration/fidcal.py, so the same
//! data gives the same answer as the published study.

use std::collections::BTreeMap;
use std::io::Read;

use chrono::DateTime;
use sha2::{Digest, Sha256};

/// Fewest observations, in total and in each half, a state needs.
pub const MIN_OBSERVATIONS: usize = 50;
pub const MIN_PER_HALF: usize = 10;
/// Fewest fresh checks for s0 to count as measured, not extrapolated.
pub const MIN_FRESH: usize = 20;
/// A check is fresh while the decay factor 0.5^(age / h) is at least this.
pub const FRESH_DECAY: f64 = 0.9;
/// Ages with at most this many distinct values are used as they are.
const MAX_EXACT_AGES: usize = 40;
/// Otherwise ages are grouped into this many equal-count bins.
const BINS: usize = 20;

/// One fact, checked again at some age.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub entity: String,
    /// Seconds between the fact's measurement and the check
    pub age: f64,
    /// Did the fact still hold when checked?
    pub correct: bool,
    pub state: Option<String>,
}

/// The fitted decay model. `half_life` is in seconds; infinite means no decay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Model {
    pub s0: f64,
    pub baseline: f64,
    pub half_life: f64,
}

impl Model {
    pub fn predict(&self, age: f64) -> f64 {
        if self.half_life.is_infinite() {
            return self.s0;
        }
        self.baseline + (self.s0 - self.baseline) * 0.5_f64.powf(age.max(0.0) / self.half_life)
    }
}

/// Observations at one age (or one group of nearby ages).
#[derive(Debug, Clone, PartialEq)]
pub struct Bin {
    pub age: f64,
    pub correct: f64,
    pub n: usize,
}

/// The result for one state (or for all data when there are no states).
#[derive(Debug, Clone)]
pub struct StateFit {
    pub state: Option<String>,
    pub observations: usize,
    pub entities: usize,
    pub bins: Vec<Bin>,
    /// Final model, fitted on all data
    pub model: Model,
    /// Did the baseline beat decay-to-zero on the held-out half?
    pub uses_baseline: bool,
    pub shape_error: f64,
    pub empirical_half_life: Option<f64>,
    /// Checks young enough to measure fidelity when fresh
    pub fresh_checks: usize,
    /// s0, when `fresh_checks` is enough to call it measured
    pub source_fidelity: Option<f64>,
    pub brier_baseline: f64,
    pub brier_zero: f64,
    pub brier_constant: f64,
}

/// Deterministic 50/50 split by entity — the same as calibration/fidcal.py.
pub fn is_train(entity: &str) -> bool {
    Sha256::digest(entity.as_bytes())[0] % 2 == 0
}

/// Group observations into bins: exact ages when there are few distinct
/// ages, otherwise equal-count bins at their mean age.
pub fn bins(obs: &[&Observation]) -> Vec<Bin> {
    let mut sorted: Vec<&Observation> = obs.to_vec();
    sorted.sort_by(|a, b| a.age.total_cmp(&b.age));

    let mut distinct: BTreeMap<u64, (usize, usize)> = BTreeMap::new();
    for o in &sorted {
        let e = distinct.entry(o.age.to_bits()).or_insert((0, 0));
        e.0 += o.correct as usize;
        e.1 += 1;
    }
    if distinct.len() <= MAX_EXACT_AGES {
        let mut out: Vec<Bin> = distinct
            .into_iter()
            .map(|(bits, (k, n))| Bin { age: f64::from_bits(bits), correct: k as f64 / n as f64, n })
            .collect();
        out.sort_by(|a, b| a.age.total_cmp(&b.age));
        return out;
    }

    let per_bin = sorted.len().div_ceil(BINS);
    sorted
        .chunks(per_bin)
        .map(|chunk| {
            let n = chunk.len();
            Bin {
                age: chunk.iter().map(|o| o.age).sum::<f64>() / n as f64,
                correct: chunk.iter().filter(|o| o.correct).count() as f64 / n as f64,
                n,
            }
        })
        .collect()
}

/// Best (s0, b) for a fixed half-life, and its weighted squared error.
fn solve(bins: &[Bin], h: f64, with_baseline: bool) -> (Model, f64) {
    let d: Vec<f64> = bins.iter().map(|b| 0.5_f64.powf(b.age / h)).collect();
    let w: Vec<f64> = bins.iter().map(|b| b.n as f64).collect();
    let y: Vec<f64> = bins.iter().map(|b| b.correct).collect();

    let zero_baseline = || {
        let num: f64 = (0..bins.len()).map(|i| w[i] * d[i] * y[i]).sum();
        let den: f64 = (0..bins.len()).map(|i| w[i] * d[i] * d[i]).sum();
        let s0 = if den > 0.0 { (num / den).clamp(0.0, 1.0) } else { 0.0 };
        Model { s0, baseline: 0.0, half_life: h }
    };

    let model = if !with_baseline {
        zero_baseline()
    } else {
        // y ≈ b × (1 − d) + s0 × d, weighted least squares
        let e: Vec<f64> = d.iter().map(|d| 1.0 - d).collect();
        let (mut ee, mut ed, mut dd, mut ey, mut dy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for i in 0..bins.len() {
            ee += w[i] * e[i] * e[i];
            ed += w[i] * e[i] * d[i];
            dd += w[i] * d[i] * d[i];
            ey += w[i] * e[i] * y[i];
            dy += w[i] * d[i] * y[i];
        }
        let det = ee * dd - ed * ed;
        if det.abs() < 1e-12 {
            zero_baseline()
        } else {
            let b = (ey * dd - ed * dy) / det;
            let s0 = (ee * dy - ed * ey) / det;
            if b < 0.0 {
                zero_baseline()
            } else if s0 > 1.0 {
                // Pin s0 at 1 and refit b
                let num: f64 = (0..bins.len()).map(|i| w[i] * e[i] * (y[i] - d[i])).sum();
                let b = if ee > 0.0 { (num / ee).clamp(0.0, 1.0) } else { 0.0 };
                Model { s0: 1.0, baseline: b, half_life: h }
            } else if b > s0 {
                // Fidelity never rises: no decay at all fits better
                zero_baseline()
            } else {
                Model { s0, baseline: b, half_life: h }
            }
        }
    };
    let loss = bins.iter().map(|b| b.n as f64 * (b.correct - model.predict(b.age)).powi(2)).sum();
    (model, loss)
}

/// Fit the model to binned data. Infinite half-life when no decay fits best.
pub fn fit(bins: &[Bin], with_baseline: bool) -> Model {
    let total: f64 = bins.iter().map(|b| b.n as f64).sum();
    let mean = bins.iter().map(|b| b.correct * b.n as f64).sum::<f64>() / total;
    let no_decay = Model { s0: mean, baseline: 0.0, half_life: f64::INFINITY };
    let no_decay_loss: f64 = bins.iter().map(|b| b.n as f64 * (b.correct - mean).powi(2)).sum();

    let positive: Vec<f64> = bins.iter().map(|b| b.age).filter(|&a| a > 0.0).collect();
    if positive.is_empty() {
        return no_decay;
    }
    let lo_age = positive.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi_age = positive.iter().cloned().fold(0.0, f64::max);
    let (mut lo, mut hi) = ((lo_age / 100.0).ln(), (hi_age * 1000.0).ln());
    let mut best = solve(bins, lo.exp(), with_baseline);
    for _ in 0..4 {
        for i in 0..=200 {
            let h = (lo + (hi - lo) * i as f64 / 200.0).exp();
            let candidate = solve(bins, h, with_baseline);
            if candidate.1 < best.1 {
                best = candidate;
            }
        }
        let step = (hi - lo) / 200.0;
        let center = best.0.half_life.ln();
        lo = center - 2.0 * step;
        hi = center + 2.0 * step;
    }
    if no_decay_loss <= best.1 {
        return no_decay;
    }
    best.0
}

/// Largest gap between the binned curve and the model.
pub fn shape_error(bins: &[Bin], model: &Model) -> f64 {
    bins.iter().map(|b| (b.correct - model.predict(b.age)).abs()).fold(0.0, f64::max)
}

/// Mean squared error of the model's probability on raw observations.
pub fn brier(obs: &[&Observation], predict: impl Fn(f64) -> f64) -> f64 {
    let total: f64 = obs.iter().map(|o| (predict(o.age) - if o.correct { 1.0 } else { 0.0 }).powi(2)).sum();
    total / obs.len() as f64
}

/// First age where the binned curve falls to 0.5, linearly interpolated.
pub fn empirical_half_life(bins: &[Bin]) -> Option<f64> {
    bins.windows(2).find_map(|w| {
        let (a, b) = (&w[0], &w[1]);
        if a.correct > 0.5 && b.correct <= 0.5 {
            if a.correct == b.correct {
                return Some(b.age);
            }
            return Some(a.age + (a.correct - 0.5) * (b.age - a.age) / (a.correct - b.correct));
        }
        None
    })
}

/// Oldest age at which a check still counts as fresh under `model`.
pub fn fresh_age(model: &Model) -> f64 {
    if model.half_life.is_infinite() {
        return f64::INFINITY;
    }
    model.half_life * (1.0 / FRESH_DECAY).log2()
}

/// Fit every state. Either every observation has a state or none does.
pub fn calibrate(obs: &[Observation]) -> Result<Vec<StateFit>, String> {
    if obs.is_empty() {
        return Err("no observations".to_string());
    }
    let with_state = obs.iter().filter(|o| o.state.is_some()).count();
    if with_state != 0 && with_state != obs.len() {
        return Err(format!(
            "{} of {} rows have a state: give every row a state, or none",
            with_state,
            obs.len()
        ));
    }

    let mut groups: BTreeMap<Option<String>, Vec<&Observation>> = BTreeMap::new();
    for o in obs {
        groups.entry(o.state.clone()).or_default().push(o);
    }

    groups.into_iter().map(|(state, group)| fit_state(state, &group)).collect()
}

fn fit_state(state: Option<String>, group: &[&Observation]) -> Result<StateFit, String> {
    let name = state.as_deref().map(|s| format!("state '{}'", s)).unwrap_or_else(|| "data".to_string());
    let train: Vec<&Observation> = group.iter().copied().filter(|o| is_train(&o.entity)).collect();
    let test: Vec<&Observation> = group.iter().copied().filter(|o| !is_train(&o.entity)).collect();
    if group.len() < MIN_OBSERVATIONS || train.len() < MIN_PER_HALF || test.len() < MIN_PER_HALF {
        return Err(format!(
            "{}: too little data ({} observations, {} and {} in the two halves; need {} total and {} per half)",
            name,
            group.len(),
            train.len(),
            test.len(),
            MIN_OBSERVATIONS,
            MIN_PER_HALF
        ));
    }
    let all_bins = bins(group);
    if all_bins.len() < 3 || !all_bins.iter().any(|b| b.age > 0.0) {
        return Err(format!("{}: need at least 3 distinct ages, some above zero", name));
    }

    let train_bins = bins(&train);
    let zero = fit(&train_bins, false);
    let with_b = fit(&train_bins, true);
    let constant = train.iter().filter(|o| o.correct).count() as f64 / train.len() as f64;
    let brier_zero = brier(&test, |a| zero.predict(a));
    let brier_baseline = brier(&test, |a| with_b.predict(a));
    let brier_constant = brier(&test, |_| constant);
    let uses_baseline = with_b.baseline > 0.0 && brier_baseline < brier_zero - 1e-9;

    let model = fit(&all_bins, uses_baseline);
    let entities = group.iter().map(|o| o.entity.as_str()).collect::<std::collections::BTreeSet<_>>().len();
    let fresh_checks = group.iter().filter(|o| o.age <= fresh_age(&model)).count();
    Ok(StateFit {
        state,
        observations: group.len(),
        entities,
        shape_error: shape_error(&all_bins, &model),
        empirical_half_life: empirical_half_life(&all_bins),
        bins: all_bins,
        fresh_checks,
        source_fidelity: (fresh_checks >= MIN_FRESH).then_some(model.s0),
        model,
        uses_baseline,
        brier_baseline,
        brier_zero,
        brier_constant,
    })
}

// ── input ──────────────────────────────────────────────────────────────

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "true" | "t" | "1" | "yes" | "y" => Some(true),
        "false" | "f" | "0" | "no" | "n" => Some(false),
        _ => None,
    }
}

/// Seconds from a number of seconds or a duration ("30d", "6h", "0d").
fn parse_age(s: &str) -> Result<f64, String> {
    crate::compute::parse_duration(s)
        .map_err(|_| format!("invalid age '{}' (zero or more: seconds, or a duration like 30d or 6h)", s.trim()))
}

/// Read observations from CSV with a header row.
///
/// Columns: `entity`, `correct`, and either `age` or both `observed_at`
/// and `checked_at` (RFC 3339); optional `state`. Column names are
/// case-insensitive; other columns are ignored.
pub fn read_csv(input: impl Read) -> Result<Vec<Observation>, String> {
    let mut reader = csv::ReaderBuilder::new().trim(csv::Trim::All).from_reader(input);
    let headers = reader.headers().map_err(|e| format!("cannot read CSV header: {}", e))?.clone();
    let col = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));

    let entity = col("entity").ok_or("CSV needs an 'entity' column")?;
    let correct = col("correct").ok_or("CSV needs a 'correct' column")?;
    let state = col("state");
    let age = col("age");
    let observed = col("observed_at");
    let checked = col("checked_at");
    let times = match (age, observed, checked) {
        (Some(_), None, None) => None,
        (None, Some(o), Some(c)) => Some((o, c)),
        (Some(_), _, _) => return Err("use either an 'age' column or 'observed_at' and 'checked_at', not both".into()),
        _ => return Err("CSV needs an 'age' column, or 'observed_at' and 'checked_at'".into()),
    };

    let mut out = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| format!("invalid CSV: {}", e))?;
        let line = record.position().map(|p| p.line()).unwrap_or(0);
        let field = |i: usize| record.get(i).unwrap_or("");
        let at = |e: String| format!("line {}: {}", line, e);

        let entity_value = field(entity);
        if entity_value.is_empty() {
            return Err(at("empty entity".into()));
        }
        let correct_value = parse_bool(field(correct))
            .ok_or_else(|| at(format!("invalid correct '{}' (true/false, 1/0, yes/no)", field(correct))))?;
        let age_value = match times {
            None => parse_age(field(age.unwrap())).map_err(at)?,
            Some((o, c)) => {
                let parse = |i: usize, name: &str| {
                    DateTime::parse_from_rfc3339(field(i))
                        .map_err(|_| at(format!("invalid {} '{}' (RFC 3339, e.g. 2026-01-04T10:00:00Z)", name, field(i))))
                };
                let seconds = (parse(c, "checked_at")? - parse(o, "observed_at")?).num_milliseconds() as f64 / 1000.0;
                if seconds < 0.0 {
                    return Err(at("checked_at is before observed_at".into()));
                }
                seconds
            }
        };
        let state_value = state.map(field).filter(|s| !s.is_empty()).map(str::to_string);
        out.push(Observation { entity: entity_value.to_string(), age: age_value, correct: correct_value, state: state_value });
    }
    Ok(out)
}

// ── output ─────────────────────────────────────────────────────────────

const UNITS: [(&str, f64); 5] = [("y", 31_557_600.0), ("d", 86_400.0), ("h", 3_600.0), ("m", 60.0), ("s", 1.0)];

/// A half-life as the registry writes it: "5.79y", "2.9h", "infinite".
pub fn format_half_life(seconds: f64) -> String {
    if seconds.is_infinite() {
        return "infinite".to_string();
    }
    for (unit, size) in UNITS {
        if seconds >= size {
            return format!("{}{}", trim(seconds / size), unit);
        }
    }
    format!("{}s", trim(seconds))
}

fn trim(v: f64) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn trim3(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The registry YAML for one information type.
pub fn registry_yaml(info_type: &str, fits: &[StateFit]) -> String {
    let observations: usize = fits.iter().map(|f| f.observations).sum();
    let mut out = format!(
        "# fid calibrate: {} observations. Fitted on half the entities, scored on the other half.\n{}:\n",
        observations, info_type
    );
    let entry = |f: &StateFit, indent: &str| {
        let mut s = format!("{}half_life: {}\n", indent, format_half_life(f.model.half_life));
        if f.uses_baseline {
            s.push_str(&format!("{}baseline: {}\n", indent, trim(f.model.baseline)));
        }
        if let Some(sf) = f.source_fidelity {
            s.push_str(&format!("{}source_fidelity: {}\n", indent, trim3(sf)));
        }
        s
    };
    if fits.len() == 1 && fits[0].state.is_none() {
        out.push_str(&entry(&fits[0], "  "));
    } else {
        out.push_str("  states:\n");
        for f in fits {
            out.push_str(&format!("    {}:\n", f.state.as_deref().unwrap_or("")));
            out.push_str(&entry(f, "      "));
        }
    }
    out
}

/// A compact age for the report: "3d 4h", "45s".
fn age_label(seconds: f64) -> String {
    let mut rest = seconds.max(0.0).round() as u64;
    let mut parts = Vec::new();
    for (unit, size) in UNITS {
        let size = size as u64;
        if rest >= size && parts.len() < 2 {
            parts.push(format!("{}{}", rest / size, unit));
            rest %= size;
        }
    }
    if parts.is_empty() {
        "0s".to_string()
    } else {
        parts.join(" ")
    }
}

/// The human-readable report for stderr.
pub fn report(info_type: &str, fits: &[StateFit]) -> String {
    let mut lines = Vec::new();
    for f in fits {
        let name = match &f.state {
            Some(s) => format!("{} / {}", info_type, s),
            None => info_type.to_string(),
        };
        lines.push(format!("{} — {} observations, {} entities", name, f.observations, f.entities));
        lines.push(format!(
            "  half-life {}, baseline {:.2}, correct when fresh {:.3}{}",
            format_half_life(f.model.half_life),
            f.model.baseline,
            f.model.s0,
            if f.uses_baseline { "" } else { "  (decay to 0: the baseline did not help on held-out data)" }
        ));
        lines.push(match f.source_fidelity {
            Some(_) => format!(
                "  source fidelity {:.3}, measured from {} checks younger than {}",
                f.model.s0,
                f.fresh_checks,
                age_label(fresh_age(&f.model))
            ),
            None => format!(
                "  source fidelity not measured: {} checks younger than {}, need {}; the source class constant stays",
                f.fresh_checks,
                age_label(fresh_age(&f.model)),
                MIN_FRESH
            ),
        });
        lines.push(format!(
            "  half wrong after {}; largest gap from the model {:.3}",
            f.empirical_half_life.map(age_label).unwrap_or_else(|| "— (never within the data)".into()),
            f.shape_error
        ));
        lines.push(format!(
            "  held-out Brier (lower is better): with baseline {:.4} · decay to 0 {:.4} · constant {:.4}",
            f.brier_baseline, f.brier_zero, f.brier_constant
        ));
        lines.push(format!("  {:<10} {:>8} {:>8} {:>9}", "age", "correct", "model", "n"));
        for b in &f.bins {
            lines.push(format!(
                "  {:<10} {:>8.3} {:>8.3} {:>9}",
                age_label(b.age),
                b.correct,
                f.model.predict(b.age),
                b.n
            ));
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compute::parse_half_life;
    use crate::registry::HalfLives;

    const DAY: f64 = 86_400.0;

    /// Exact synthetic data: at each age, round(n × S(age)) of n entities still correct.
    fn synthetic(model: Model, ages: &[f64], n: usize, state: Option<&str>) -> Vec<Observation> {
        let mut out = Vec::new();
        for (ai, &age) in ages.iter().enumerate() {
            let k = (n as f64 * model.predict(age)).round() as usize;
            for i in 0..n {
                out.push(Observation {
                    entity: format!("{}-{}-{}", state.unwrap_or("e"), ai, i),
                    age,
                    correct: i < k,
                    state: state.map(str::to_string),
                });
            }
        }
        out
    }

    fn days(v: &[f64]) -> Vec<f64> {
        v.iter().map(|d| d * DAY).collect()
    }

    #[test]
    fn test_recovers_half_life_without_baseline() {
        let truth = Model { s0: 1.0, baseline: 0.0, half_life: 10.0 * DAY };
        let obs = synthetic(truth, &days(&[0.0, 2.0, 5.0, 10.0, 20.0, 40.0]), 400, None);
        let fits = calibrate(&obs).unwrap();
        let m = fits[0].model;
        assert!((m.half_life / DAY - 10.0).abs() < 0.3, "{:?}", m);
        assert!(!fits[0].uses_baseline);
        assert!(fits[0].shape_error < 0.01);
    }

    #[test]
    fn test_recovers_baseline() {
        let truth = Model { s0: 0.97, baseline: 0.73, half_life: 3.0 * 3600.0 };
        let ages: Vec<f64> = [0.0, 1.0, 2.0, 3.0, 6.0, 12.0, 24.0, 48.0].iter().map(|h| h * 3600.0).collect();
        let obs = synthetic(truth, &ages, 500, None);
        let fits = calibrate(&obs).unwrap();
        let f = &fits[0];
        assert!(f.uses_baseline);
        assert!((f.model.baseline - 0.73).abs() < 0.02, "{:?}", f.model);
        assert!((f.model.half_life / 3600.0 - 3.0).abs() < 0.3, "{:?}", f.model);
        assert!((f.model.s0 - 0.97).abs() < 0.01);
        assert!(f.brier_baseline < f.brier_zero);
    }

    #[test]
    fn test_no_decay_is_infinite() {
        let truth = Model { s0: 0.9, baseline: 0.0, half_life: f64::INFINITY };
        let obs = synthetic(truth, &days(&[0.0, 10.0, 100.0, 1000.0]), 200, None);
        let fits = calibrate(&obs).unwrap();
        assert!(fits[0].model.half_life.is_infinite());
        assert_eq!(format_half_life(fits[0].model.half_life), "infinite");
    }

    #[test]
    fn test_states_fit_separately() {
        let mut obs = synthetic(
            Model { s0: 1.0, baseline: 0.0, half_life: 180.0 },
            &[0.0, 60.0, 180.0, 600.0, 1800.0],
            300,
            Some("underway"),
        );
        obs.extend(synthetic(
            Model { s0: 1.0, baseline: 0.0, half_life: 1.5 * DAY },
            &days(&[0.0, 0.25, 0.5, 1.0, 3.0]),
            300,
            Some("moored"),
        ));
        let fits = calibrate(&obs).unwrap();
        assert_eq!(fits.len(), 2);
        let get = |s: &str| fits.iter().find(|f| f.state.as_deref() == Some(s)).unwrap();
        assert!((get("underway").model.half_life - 180.0).abs() < 10.0);
        assert!((get("moored").model.half_life / DAY - 1.5).abs() < 0.1);
    }

    #[test]
    fn test_continuous_ages_are_binned() {
        let truth = Model { s0: 1.0, baseline: 0.0, half_life: 30.0 * DAY };
        // 3,000 entities at spread-out ages, correct by a deterministic rule matching S(age)
        let obs: Vec<Observation> = (0..3000)
            .map(|i| {
                let age = (i as f64 / 3000.0) * 120.0 * DAY;
                let p = truth.predict(age);
                // low-discrepancy: correct when the fractional sequence falls under p
                let u = (i as f64 * 0.618_033_988_75).fract();
                Observation { entity: format!("c{}", i), age, correct: u < p, state: None }
            })
            .collect();
        let fits = calibrate(&obs).unwrap();
        assert_eq!(fits[0].bins.len(), BINS);
        assert!((fits[0].model.half_life / DAY - 30.0).abs() < 4.0, "{:?}", fits[0].model);
    }

    #[test]
    fn test_registry_yaml_round_trips() {
        let mut obs = synthetic(Model { s0: 1.0, baseline: 0.0, half_life: 180.0 }, &[0.0, 60.0, 180.0, 600.0], 200, Some("underway"));
        obs.extend(synthetic(Model { s0: 0.97, baseline: 0.6, half_life: 7200.0 }, &[0.0, 1800.0, 3600.0, 7200.0, 36000.0, 86400.0], 300, Some("moored")));
        let fits = calibrate(&obs).unwrap();
        let yaml = registry_yaml("vessel_position", &fits);
        let registry = HalfLives::from_yaml(&yaml).unwrap();
        let underway = registry.lookup("vessel_position", Some("underway")).unwrap();
        let fit = fits.iter().find(|f| f.state.as_deref() == Some("underway")).unwrap();
        let parsed = parse_half_life(&underway.half_life).unwrap().unwrap();
        assert!((parsed - fit.model.half_life).abs() / fit.model.half_life < 0.01);
        let moored = registry.lookup("vessel_position", Some("moored")).unwrap();
        assert!(moored.baseline.is_some());
    }

    #[test]
    fn test_registry_yaml_without_states() {
        let obs = synthetic(Model { s0: 1.0, baseline: 0.0, half_life: 5.8 * 31_557_600.0 }, &[0.0, 1e7, 5e7, 1e8, 3e8], 200, None);
        let fits = calibrate(&obs).unwrap();
        let yaml = registry_yaml("company_ceo", &fits);
        assert!(yaml.contains("company_ceo:\n  half_life: "), "{}", yaml);
        assert!(!yaml.contains("states"), "{}", yaml);
        let spec = HalfLives::from_yaml(&yaml).unwrap().lookup("company_ceo", None).unwrap();
        let years = parse_half_life(&spec.half_life).unwrap().unwrap() / 31_557_600.0;
        assert!((years - 5.8).abs() < 0.12, "{}", yaml);
    }

    #[test]
    fn test_source_fidelity_measured_from_fresh_checks() {
        let truth = Model { s0: 0.95, baseline: 0.30, half_life: 30.0 * DAY };
        let obs = synthetic(truth, &days(&[0.0, 3.0, 7.0, 14.0, 30.0, 60.0, 120.0, 365.0]), 200, None);
        let fits = calibrate(&obs).unwrap();
        let f = &fits[0];
        // ages 0 and 3 days are under 0.152 half-lives
        assert_eq!(f.fresh_checks, 400);
        let sf = f.source_fidelity.unwrap();
        assert!((sf - 0.95).abs() < 0.01, "{}", sf);
        let yaml = registry_yaml("policy_clause", &fits);
        let spec = HalfLives::from_yaml(&yaml).unwrap().lookup("policy_clause", None).unwrap();
        assert!((spec.source_fidelity.unwrap() - sf).abs() < 0.001, "{}", yaml);
    }

    #[test]
    fn test_source_fidelity_not_written_when_extrapolated() {
        // Youngest check is two half-lives old: s0 is an extrapolation
        let truth = Model { s0: 0.95, baseline: 0.0, half_life: 10.0 * DAY };
        let obs = synthetic(truth, &days(&[20.0, 30.0, 40.0, 60.0]), 200, None);
        let fits = calibrate(&obs).unwrap();
        assert_eq!(fits[0].fresh_checks, 0);
        assert_eq!(fits[0].source_fidelity, None);
        assert!(!registry_yaml("x", &fits).contains("source_fidelity"));
        assert!(report("x", &fits).contains("source fidelity not measured"));
    }

    #[test]
    fn test_too_little_data_is_error() {
        let obs = synthetic(Model { s0: 1.0, baseline: 0.0, half_life: 100.0 }, &[0.0, 100.0], 10, None);
        assert!(calibrate(&obs).unwrap_err().contains("too little data"));
    }

    #[test]
    fn test_one_age_is_error() {
        let obs = synthetic(Model { s0: 1.0, baseline: 0.0, half_life: 100.0 }, &[50.0], 200, None);
        assert!(calibrate(&obs).unwrap_err().contains("distinct ages"));
    }

    #[test]
    fn test_mixed_states_is_error() {
        let mut obs = synthetic(Model { s0: 1.0, baseline: 0.0, half_life: 100.0 }, &[0.0, 50.0, 100.0], 50, None);
        obs[0].state = Some("x".into());
        assert!(calibrate(&obs).unwrap_err().contains("have a state"));
    }

    #[test]
    fn test_split_matches_python() {
        // Computed with hashlib.sha256(e.encode()).digest()[0] % 2 == 0, as calibration/fidcal.py does
        let expected = [("Q312", false), ("claim:1", true), ("e-0-0", false), ("vessel 366999", true)];
        for (entity, train) in expected {
            assert_eq!(is_train(entity), train, "{}", entity);
        }
    }

    #[test]
    fn test_read_csv_age_column() {
        let csv = "entity,age,correct,note\nc1,30d,true,x\nc2,3600,false,y\nc3,0d,yes,\n";
        let obs = read_csv(csv.as_bytes()).unwrap();
        assert_eq!(obs.len(), 3);
        assert_eq!(obs[0].age, 30.0 * DAY);
        assert_eq!(obs[1].age, 3600.0);
        assert!(!obs[1].correct);
        assert!(obs[2].correct);
        assert_eq!(obs[0].state, None);
    }

    #[test]
    fn test_read_csv_timestamps_and_state() {
        let csv = "Entity,Observed_At,Checked_At,Correct,State\n\
                   claim:1,2026-01-01T00:00:00Z,2026-01-02T00:00:00Z,1,policy\n\
                   claim:2,2026-01-01T00:00:00Z,2026-01-01T06:00:00+00:00,0,policy\n";
        let obs = read_csv(csv.as_bytes()).unwrap();
        assert_eq!(obs[0].age, DAY);
        assert_eq!(obs[1].age, 6.0 * 3600.0);
        assert_eq!(obs[0].state.as_deref(), Some("policy"));
    }

    #[test]
    fn test_read_csv_errors() {
        let cases = [
            ("entity,correct\nx,true\n", "'age' column"),
            ("entity,age,observed_at,checked_at,correct\nx,1,a,b,true\n", "not both"),
            ("age,correct\n1,true\n", "'entity'"),
            ("entity,age,correct\nx,1,maybe\n", "line 2"),
            ("entity,age,correct\nx,7days,true\n", "invalid age"),
            ("entity,age,correct\nx,-5,true\n", "invalid age"),
            ("entity,observed_at,checked_at,correct\nx,2026-01-02T00:00:00Z,2026-01-01T00:00:00Z,true\n", "before"),
            ("entity,observed_at,checked_at,correct\nx,yesterday,2026-01-01T00:00:00Z,true\n", "RFC 3339"),
        ];
        for (csv, want) in cases {
            let err = read_csv(csv.as_bytes()).unwrap_err();
            assert!(err.contains(want), "{} → {}", csv, err);
        }
    }

    #[test]
    fn test_format_half_life() {
        assert_eq!(format_half_life(5.8 * 31_557_600.0), "5.8y");
        assert_eq!(format_half_life(2.9 * 3600.0), "2.9h");
        assert_eq!(format_half_life(180.0), "3m");
        assert_eq!(format_half_life(1.4 * DAY), "1.4d");
        assert_eq!(format_half_life(42.0), "42s");
        for v in [0.5, 42.0, 180.0, 2.9 * 3600.0, 1.4 * DAY, 5.8 * 31_557_600.0] {
            let parsed = parse_half_life(&format_half_life(v)).unwrap().unwrap();
            assert!((parsed - v).abs() / v < 0.01, "{}", v);
        }
    }
}
