//! Fitting an echo: `fid calibrate --period`.
//!
//! A periodic fact comes back once per period (Finding 5, temperature).
//! Given the period, the echo model
//!
//!   S(t) = b + (s0 − b) × min(1, 0.5^(t/h) + r × Σₙ 0.5^(nP/h_r) × 0.5^(|t − nP|/w))
//!
//! is fitted by Nelder–Mead on fine bins (exact ages, or P/48 wide), as
//! calibration/weather_periodic.py does. It is kept only if, fitted on
//! the train half, it beats decay to a baseline on the held-out half by
//! at least 0.1% (Brier), and its largest gap from the curve is smaller.
//! Otherwise the registry gets plain decay and the report says why.

use std::collections::BTreeMap;

use crate::calibrate::{brier, fit, Bin, Model, Observation, FRESH_DECAY, MIN_FRESH};
use crate::compute::{apply_decay, temporal_factor, EchoParams};

/// Exact ages up to this many distinct values; otherwise P/48-wide bins.
const MAX_EXACT_AGES: usize = 2000;
const BINS_PER_PERIOD: f64 = 48.0;
/// Smallest held-out Brier improvement, relative, for the echo to count.
/// Temperature gained 1.7%; flight category 0.05%, which decay already fit.
pub const MIN_GAIN: f64 = 0.001;

/// Decay to a baseline with a periodic return.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EchoModel {
    pub s0: f64,
    pub baseline: f64,
    pub half_life: f64,
    pub echo: EchoParams,
}

impl EchoModel {
    pub fn predict(&self, age: f64) -> f64 {
        apply_decay(self.s0, temporal_factor(age, Some(self.half_life), Some(&self.echo)), self.baseline)
    }
}

/// The echo test for one state.
#[derive(Debug, Clone)]
pub struct EchoFit {
    /// Fitted on all data
    pub model: EchoModel,
    /// Decay to a baseline on the same bins, all data, for comparison
    pub decay: Model,
    /// Held-out Brier, both fitted on the train half
    pub brier_echo: f64,
    pub brier_decay: f64,
    /// Largest gap from the binned curve, all data
    pub gap_echo: f64,
    pub gap_decay: f64,
    /// Did the echo pass on the held-out half?
    pub kept: bool,
    /// Checks in bins young enough to measure s0, as in calibrate
    pub fresh_checks: usize,
    pub source_fidelity: Option<f64>,
}

/// Fine bins for a periodic curve: exact ages, or P/48 wide at their mean age.
pub fn periodic_bins(obs: &[&Observation], period: f64) -> Vec<Bin> {
    let distinct: std::collections::BTreeSet<u64> = obs.iter().map(|o| o.age.to_bits()).collect();
    let width = period / BINS_PER_PERIOD;
    let key = |age: f64| if distinct.len() <= MAX_EXACT_AGES { age.to_bits() } else { (age / width).floor() as u64 };
    let mut groups: BTreeMap<u64, (f64, usize, usize)> = BTreeMap::new();
    for o in obs {
        let g = groups.entry(key(o.age)).or_insert((0.0, 0, 0));
        g.0 += o.age;
        g.1 += o.correct as usize;
        g.2 += 1;
    }
    groups
        .into_values()
        .map(|(age, k, n)| Bin { age: age / n as f64, correct: k as f64 / n as f64, n })
        .collect()
}

/// Minimize `f` over Rⁿ from `x0`. Small and dependency-free; the same
/// method as the study, good enough for six parameters.
pub fn nelder_mead(f: &dyn Fn(&[f64]) -> f64, x0: &[f64], step: f64, iters: usize) -> (Vec<f64>, f64) {
    let n = x0.len();
    let mut simplex: Vec<Vec<f64>> = vec![x0.to_vec()];
    for i in 0..n {
        let mut p = x0.to_vec();
        p[i] += step;
        simplex.push(p);
    }
    let mut values: Vec<f64> = simplex.iter().map(|p| f(p)).collect();
    for _ in 0..iters {
        let mut order: Vec<usize> = (0..=n).collect();
        order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
        simplex = order.iter().map(|&i| simplex[i].clone()).collect();
        values = order.iter().map(|&i| values[i]).collect();
        if (values[n] - values[0]).abs() < 1e-12 {
            break;
        }
        let centroid: Vec<f64> = (0..n).map(|j| simplex[..n].iter().map(|p| p[j]).sum::<f64>() / n as f64).collect();
        let toward = |scale: f64| -> Vec<f64> { (0..n).map(|j| centroid[j] + scale * (simplex[n][j] - centroid[j])).collect() };
        let reflected = toward(-1.0);
        let fr = f(&reflected);
        if fr < values[0] {
            let expanded = toward(-2.0);
            let fe = f(&expanded);
            (simplex[n], values[n]) = if fe < fr { (expanded, fe) } else { (reflected, fr) };
        } else if fr < values[n - 1] {
            (simplex[n], values[n]) = (reflected, fr);
        } else {
            let contracted = toward(0.5);
            let fc = f(&contracted);
            if fc < values[n] {
                (simplex[n], values[n]) = (contracted, fc);
            } else {
                let best = simplex[0].clone();
                for i in 1..=n {
                    simplex[i] = (0..n).map(|j| best[j] + 0.5 * (simplex[i][j] - best[j])).collect();
                    values[i] = f(&simplex[i]);
                }
            }
        }
    }
    let i = (0..=n).min_by(|&a, &b| values[a].total_cmp(&values[b])).unwrap();
    (simplex[i].clone(), values[i])
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn logit(p: f64) -> f64 {
    let p = p.clamp(1e-6, 1.0 - 1e-6);
    (p / (1.0 - p)).ln()
}

/// Parameters on an unconstrained scale: s0 and b/s0 in (0, 1), h, r and
/// h_r positive, w in (0, P/2).
fn unpack(x: &[f64], period: f64) -> EchoModel {
    let s0 = sigmoid(x[0]);
    EchoModel {
        s0,
        baseline: s0 * sigmoid(x[1]),
        half_life: x[2].exp(),
        echo: EchoParams { period, strength: x[3].exp(), fade: Some(x[4].exp()), width: period / 2.0 * sigmoid(x[5]) },
    }
}

fn sse(bins: &[Bin], m: &EchoModel) -> f64 {
    bins.iter().map(|b| b.n as f64 * (b.correct - m.predict(b.age)).powi(2)).sum()
}

/// Fit the echo model to bins, starting from a decay-to-baseline fit.
pub fn fit_echo(bins: &[Bin], period: f64, start: &Model) -> EchoModel {
    let h0 = if start.half_life.is_finite() { start.half_life } else { period / 4.0 };
    let loss = |x: &[f64]| {
        let m = unpack(x, period);
        if !(m.half_life.is_finite() && m.echo.strength.is_finite() && m.echo.fade.unwrap().is_finite()) {
            return f64::INFINITY;
        }
        sse(bins, &m)
    };
    // (r, h_r ÷ P, w ÷ P): a weak long echo, a strong short one, the study's temperature shape
    let starts = [(0.3, 4.0, 1.0 / 6.0), (0.6, 1.0, 1.0 / 5.0), (0.2, 15.0, 1.0 / 12.0)];
    let mut best: Option<(Vec<f64>, f64)> = None;
    for (r, fade, width) in starts {
        let x0 = [
            logit(start.s0.min(0.999)),
            logit(start.baseline.max(0.01) / start.s0.max(0.02)),
            h0.ln(),
            f64::ln(r),
            (fade * period).ln(),
            logit(width * 2.0),
        ];
        let (x, _) = nelder_mead(&loss, &x0, 0.5, 600);
        // Restart from the optimum: Nelder–Mead stalls otherwise
        let (x, v) = nelder_mead(&loss, &x, 0.1, 600);
        if best.as_ref().map_or(true, |(_, bv)| v < *bv) {
            best = Some((x, v));
        }
    }
    unpack(&best.expect("three starts").0, period)
}

fn gap(bins: &[Bin], predict: impl Fn(f64) -> f64) -> f64 {
    bins.iter().map(|b| (b.correct - predict(b.age)).abs()).fold(0.0, f64::max)
}

/// The echo test: fit on `train`, score on `test`, refit on all of `group`.
pub fn test_echo(group: &[&Observation], train: &[&Observation], test: &[&Observation], period: f64) -> EchoFit {
    let train_bins = periodic_bins(train, period);
    let train_decay = fit(&train_bins, true);
    let train_echo = fit_echo(&train_bins, period, &train_decay);
    let brier_echo = brier(test, |a| train_echo.predict(a));
    let brier_decay = brier(test, |a| train_decay.predict(a));

    let all_bins = periodic_bins(group, period);
    let decay = fit(&all_bins, true);
    let model = fit_echo(&all_bins, period, &decay);
    let gap_echo = gap(&all_bins, |a| model.predict(a));
    let gap_decay = gap(&all_bins, |a| decay.predict(a));

    let fresh = model.half_life * (1.0 / FRESH_DECAY).log2();
    let fresh_checks = all_bins.iter().filter(|b| b.age <= fresh).map(|b| b.n).sum();
    EchoFit {
        model,
        decay,
        brier_echo,
        brier_decay,
        gap_echo,
        gap_decay,
        kept: brier_echo < brier_decay * (1.0 - MIN_GAIN) && gap_echo < gap_decay,
        fresh_checks,
        source_fidelity: (fresh_checks >= MIN_FRESH).then_some(model.s0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibrate::{is_train, Rng};

    const HOUR: f64 = 3600.0;

    /// Airport temperature, fitted on all of 2024
    fn temperature() -> EchoModel {
        EchoModel {
            s0: 1.0,
            baseline: 0.13,
            half_life: 2.42 * HOUR,
            echo: EchoParams { period: 24.0 * HOUR, strength: 0.31, width: 3.75 * HOUR, fade: Some(345.0 * HOUR) },
        }
    }

    /// `per_lag` entities checked once at each hourly lag up to 14 days.
    fn hourly(predict: impl Fn(f64) -> f64, per_lag: usize, seed: u64) -> Vec<Observation> {
        let mut rng = Rng(seed);
        let mut out = Vec::new();
        for lag in 0..=336 {
            let age = lag as f64 * HOUR;
            for i in 0..per_lag {
                out.push(Observation { entity: format!("s{}-{}", lag, i), age, correct: rng.unit() < predict(age), state: None });
            }
        }
        out
    }

    fn run(obs: &[Observation]) -> EchoFit {
        let group: Vec<&Observation> = obs.iter().collect();
        let train: Vec<&Observation> = obs.iter().filter(|o| is_train(&o.entity)).collect();
        let test: Vec<&Observation> = obs.iter().filter(|o| !is_train(&o.entity)).collect();
        test_echo(&group, &train, &test, 24.0 * HOUR)
    }

    #[test]
    fn test_recovers_a_daily_echo() {
        let truth = temperature();
        let f = run(&hourly(|a| truth.predict(a), 300, 1));
        assert!(f.kept, "{:?}", f);
        let m = f.model;
        eprintln!(
            "RECOVERED r {:.3} w {:.2}h h_r {:.0}h b {:.3} h {:.2}h; held-out Brier {:.4} vs {:.4}; gap {:.3} vs {:.3}",
            m.echo.strength, m.echo.width / HOUR, m.echo.fade.unwrap() / HOUR, m.baseline, m.half_life / HOUR,
            f.brier_echo, f.brier_decay, f.gap_echo, f.gap_decay
        );
        assert!((m.echo.strength - 0.31).abs() < 0.05, "{:?}", m);
        assert!((m.echo.width / HOUR - 3.75).abs() < 0.6, "{:?}", m);
        assert!((m.baseline - 0.13).abs() < 0.03, "{:?}", m);
        assert!((m.half_life / HOUR - 2.42).abs() < 0.4, "{:?}", m);
        assert!(f.brier_echo < f.brier_decay && f.gap_echo < f.gap_decay);
    }

    #[test]
    fn test_no_echo_is_not_kept() {
        let truth = Model { s0: 1.0, baseline: 0.3, half_life: 6.0 * HOUR };
        let f = run(&hourly(|a| truth.predict(a), 300, 2));
        eprintln!("NO ECHO held-out Brier {:.5} vs {:.5}; gap {:.3} vs {:.3}", f.brier_echo, f.brier_decay, f.gap_echo, f.gap_decay);
        assert!(!f.kept, "echo kept on data without one: {:?}", f);
    }

    /// The measurements quoted in the docs.
    /// `cargo test --release --lib measure_echo -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn measure_echo_keeps() {
        let mut kept = 0;
        let mut kept_strong = 0;
        for seed in 0..20 {
            for truth in [Model { s0: 1.0, baseline: 0.3, half_life: 6.0 * HOUR }, Model { s0: 0.95, baseline: 0.0, half_life: 30.0 * HOUR }] {
                let f = run(&hourly(|a| truth.predict(a), 100, 100 + seed));
                kept += f.kept as usize;
                kept_strong += (f.kept && f.model.echo.strength > 0.05) as usize;
            }
        }
        eprintln!("FALSE KEEPS {} of 40 ({} with strength > 0.05)", kept, kept_strong);
        let truth = temperature();
        let found = (0..20).filter(|&seed| run(&hourly(|a| truth.predict(a), 100, 200 + seed)).kept).count();
        eprintln!("TRUE ECHO KEPT {} of 20 at 100 checks per lag", found);
        let found = (0..20).filter(|&seed| run(&hourly(|a| truth.predict(a), 30, 300 + seed)).kept).count();
        eprintln!("TRUE ECHO KEPT {} of 20 at 30 checks per lag", found);
    }

    #[test]
    fn test_nelder_mead_finds_a_minimum() {
        let f = |x: &[f64]| (x[0] - 3.0).powi(2) + 10.0 * (x[1] + 1.0).powi(2);
        let (x, v) = nelder_mead(&f, &[0.0, 0.0], 0.5, 600);
        assert!((x[0] - 3.0).abs() < 1e-4 && (x[1] + 1.0).abs() < 1e-4 && v < 1e-8, "{:?}", x);
    }

    #[test]
    fn test_periodic_bins() {
        let obs: Vec<Observation> = (0..3000)
            .map(|i| Observation { entity: format!("e{}", i), age: i as f64 * 7.3, correct: i % 2 == 0, state: None })
            .collect();
        let refs: Vec<&Observation> = obs.iter().collect();
        // 3000 distinct ages: P/48 = 1800 s wide bins
        let b = periodic_bins(&refs, 24.0 * HOUR);
        assert_eq!(b.len(), (3000.0 * 7.3 / 1800.0_f64).ceil() as usize);
        assert_eq!(b.iter().map(|b| b.n).sum::<usize>(), 3000);
    }
}
