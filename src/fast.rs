//! Fitting a richer curve: two clocks, a shape, or both, tested in every `fid calibrate`.
//!
//! Some facts go stale on two clocks. Most PyPI releases stay the latest
//! for months, but a share is replaced by a patch within days; fitted with
//! one clock, that first drop reads as a source that is wrong when fresh
//! (s0 = 0.945 for just-released packages in calibration/decisions.py).
//! The model
//!
//!   S(t) = b + (s0 − b) × [(1 − m) × 0.5^(t/h) + m × 0.5^(t/h_f)],   h_f < h
//!
//! is linear in b and s0 for fixed (h, h_f, m), so those two are solved in
//! closed form and the other three searched by Nelder–Mead on fine bins.
//! A fact that goes wrong by crossing a threshold holds, then drops (a
//! moving ship's 1 km); a shape k bends the clocks, 0.5^((t/h)^k).
//!
//! Three forms are tried: a shape alone, two clocks, and both. A form is
//! kept only if, fitted on the train half, it beats one clock on the
//! held-out half by at least 0.1% (Brier), and fitted on all data it cuts
//! one clock's largest gap from the binned data by at least 20% and has a
//! lower BIC than one clock (each extra parameter costs ln n). Of those
//! that pass, the one with the lowest BIC on all data is chosen (held-out
//! Brier barely tells close forms apart; the likelihood does), the others
//! kept in order as fallbacks for when its curve is too uncertain to write.
//! When none passes the registry gets one clock and the report says why.

use std::collections::BTreeMap;

use crate::calibrate::{brier, fit, zero_age_s0, Bin, Model, Observation, FRESH_DECAY, MIN_FRESH};
use crate::echo::nelder_mead;

/// Exact ages up to this many distinct values; otherwise this many equal-count bins.
const MAX_EXACT_AGES: usize = 2000;
const FINE_BINS: usize = 200;
/// Smallest held-out Brier improvement, relative, for the fast share to count.
pub const MIN_GAIN: f64 = 0.001;
/// A richer form must also cut one clock's largest gap by this share
pub const MIN_GAP_CUT: f64 = 0.2;
/// The fast clock is at least this much faster than the slow one.
const MAX_RATIO: f64 = 0.5;

/// Decay to a baseline on two clocks with a shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FastModel {
    pub s0: f64,
    pub baseline: f64,
    pub half_life: f64,
    /// Share on the fast clock (0: one clock)
    pub share: f64,
    pub fast_half_life: f64,
    /// Shape of both clocks (1: exponential)
    pub shape: f64,
}

/// Which richer curve a fit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// One clock with a shape
    Shape,
    /// Two exponential clocks
    TwoClocks,
    /// Two clocks with a shape
    Both,
}

impl Form {
    pub const ALL: [Form; 3] = [Form::Shape, Form::TwoClocks, Form::Both];

    pub fn has_shape(self) -> bool {
        self != Form::TwoClocks
    }

    pub fn has_two_clocks(self) -> bool {
        self != Form::Shape
    }

    /// Decay parameters searched besides s0 and b: h and k, h, h_f and m, or all four.
    pub fn parameters(self) -> usize {
        match self {
            Form::Shape => 2,
            Form::TwoClocks => 3,
            Form::Both => 4,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Form::Shape => "a shape",
            Form::TwoClocks => "two clocks",
            Form::Both => "two clocks with a shape",
        }
    }
}

impl FastModel {
    pub fn decay(&self, age: f64) -> f64 {
        let a = age.max(0.0);
        let clock = |h: f64| {
            if self.shape == 1.0 {
                0.5_f64.powf(a / h)
            } else {
                0.5_f64.powf((a / h).powf(self.shape))
            }
        };
        if self.share == 0.0 {
            return clock(self.half_life);
        }
        (1.0 - self.share) * clock(self.half_life) + self.share * clock(self.fast_half_life)
    }

    pub fn predict(&self, age: f64) -> f64 {
        self.baseline + (self.s0 - self.baseline) * self.decay(age)
    }

    /// Oldest age at which a check still counts as fresh: the decay factor is at least `FRESH_DECAY`.
    pub fn fresh_age(&self) -> f64 {
        self.age_at(FRESH_DECAY)
    }

    /// When the decay factor falls to one half: the curve's own half-life, what one clock's half-life is.
    pub fn halfway(&self) -> f64 {
        self.age_at(0.5)
    }

    fn age_at(&self, level: f64) -> f64 {
        let (mut lo, mut hi) = (0.0, self.half_life);
        for _ in 0..100 {
            let mid = 0.5 * (lo + hi);
            if self.decay(mid) >= level {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }
}

/// The fast-share test for one state.
#[derive(Debug, Clone)]
pub struct FastFit {
    /// Fitted on all data
    pub model: FastModel,
    /// Held-out Brier, both fitted on the train half
    pub brier_fast: f64,
    pub brier_plain: f64,
    /// Largest gap from the fine-binned curve, all data
    pub gap_fast: f64,
    pub gap_plain: f64,
    /// The chosen form: lowest BIC of those that passed, else the best on held-out Brier
    pub form: Form,
    /// Did it pass on the held-out half (and, in calibrate, is its curve pinned down)?
    pub kept: bool,
    /// Passed on the held-out half, but its curve was too uncertain to write
    pub too_uncertain: Option<String>,
    /// Checks in bins young enough to measure s0
    pub fresh_checks: usize,
    pub source_fidelity: Option<f64>,
    /// The other forms that passed, best BIC first
    pub fallbacks: Vec<FastFit>,
}

impl FastFit {
    /// No richer form tried: one clock, not kept.
    pub fn not_tried(plain: &Model) -> FastFit {
        let model = FastModel {
            s0: plain.s0,
            baseline: plain.baseline,
            half_life: plain.half_life,
            share: 0.0,
            fast_half_life: plain.half_life,
            shape: 1.0,
        };
        FastFit {
            model,
            brier_fast: f64::NAN,
            brier_plain: f64::NAN,
            gap_fast: f64::NAN,
            gap_plain: f64::NAN,
            form: Form::Shape,
            kept: false,
            too_uncertain: None,
            fresh_checks: 0,
            source_fidelity: None,
            fallbacks: Vec::new(),
        }
    }
}

/// Fine bins: exact ages, or `FINE_BINS` equal-count bins at their mean age.
pub fn fine_bins(obs: &[&Observation]) -> Vec<Bin> {
    let mut exact: BTreeMap<u64, (usize, usize)> = BTreeMap::new();
    for o in obs {
        let e = exact.entry(o.age.to_bits()).or_insert((0, 0));
        e.0 += o.correct as usize;
        e.1 += 1;
    }
    if exact.len() <= MAX_EXACT_AGES {
        let mut out: Vec<Bin> =
            exact.into_iter().map(|(bits, (k, n))| Bin { age: f64::from_bits(bits), correct: k as f64 / n as f64, n }).collect();
        out.sort_by(|a, b| a.age.total_cmp(&b.age));
        return out;
    }
    // Checks at age zero keep a bin of their own, so they can measure s0
    let mut out = Vec::new();
    if let Some(&(k, n)) = exact.get(&0.0_f64.to_bits()) {
        out.push(Bin { age: 0.0, correct: k as f64 / n as f64, n });
    }
    let mut sorted: Vec<&Observation> = obs.iter().copied().filter(|o| o.age != 0.0).collect();
    sorted.sort_by(|a, b| a.age.total_cmp(&b.age));
    let per_bin = sorted.len().div_ceil(FINE_BINS).max(1);
    out.extend(sorted.chunks(per_bin).map(|chunk| Bin {
        age: chunk.iter().map(|o| o.age).sum::<f64>() / chunk.len() as f64,
        correct: chunk.iter().filter(|o| o.correct).count() as f64 / chunk.len() as f64,
        n: chunk.len(),
    }));
    out
}

/// Best (s0, b) for fixed clocks, by weighted least squares on y ≈ b (1 − d) + s0 d,
/// with 0 ≤ b ≤ s0 ≤ 1; with `pinned`, s0 is that (measured at age zero) and only b
/// is fitted. Returns the model and its weighted squared error.
fn solve(bins: &[Bin], half_life: f64, share: f64, fast_half_life: f64, k: f64, pinned: Option<f64>) -> (FastModel, f64) {
    let shape = FastModel {
        s0: 1.0,
        baseline: 0.0,
        half_life,
        share,
        fast_half_life,
        shape: k,
    };
    let d: Vec<f64> = bins.iter().map(|b| shape.decay(b.age)).collect();
    if let Some(s0) = pinned {
        let (mut num, mut den) = (0.0, 0.0);
        for (bin, &d) in bins.iter().zip(&d) {
            num += bin.n as f64 * (1.0 - d) * (bin.correct - s0 * d);
            den += bin.n as f64 * (1.0 - d) * (1.0 - d);
        }
        let b = if den > 0.0 { (num / den).clamp(0.0, s0) } else { 0.0 };
        let m = FastModel { s0, baseline: b, ..shape };
        let loss = bins.iter().map(|bin| bin.n as f64 * (bin.correct - m.predict(bin.age)).powi(2)).sum();
        return (m, loss);
    }
    let (mut ee, mut ed, mut dd, mut ey, mut dy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (b, &d) in bins.iter().zip(&d) {
        let (w, e, y) = (b.n as f64, 1.0 - d, b.correct);
        ee += w * e * e;
        ed += w * e * d;
        dd += w * d * d;
        ey += w * e * y;
        dy += w * d * y;
    }
    let det = ee * dd - ed * ed;
    let (mut b, mut s0) = if det.abs() < 1e-12 { (0.0, if dd > 0.0 { dy / dd } else { 0.0 }) } else { ((ey * dd - ed * dy) / det, (ee * dy - ed * ey) / det) };
    if b < 0.0 {
        b = 0.0;
        s0 = if dd > 0.0 { dy / dd } else { 0.0 };
    }
    if s0 > 1.0 {
        s0 = 1.0;
        let num: f64 = bins.iter().zip(&d).map(|(bin, &d)| bin.n as f64 * (1.0 - d) * (bin.correct - d)).sum();
        b = if ee > 0.0 { num / ee } else { 0.0 };
    }
    let s0 = s0.clamp(0.0, 1.0);
    let b = b.clamp(0.0, s0);
    let m = FastModel { s0, baseline: b, ..shape };
    let loss = bins.iter().map(|bin| bin.n as f64 * (bin.correct - m.predict(bin.age)).powi(2)).sum();
    (m, loss)
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn logit(p: f64) -> f64 {
    let p = p.clamp(1e-6, 1.0 - 1e-6);
    (p / (1.0 - p)).ln()
}

/// Shapes searched: 0.2 to 8, on a logit scale.
const SHAPE_LO: f64 = 0.2;
const SHAPE_HI: f64 = 8.0;

fn shape_of(x: f64) -> f64 {
    (SHAPE_LO.ln() + (SHAPE_HI.ln() - SHAPE_LO.ln()) * sigmoid(x)).exp()
}

fn shape_x(k: f64) -> f64 {
    logit((k.ln() - SHAPE_LO.ln()) / (SHAPE_HI.ln() - SHAPE_LO.ln()))
}

/// (h, h_f, m, k) from the search vector of a form: ln h, then logit of h_f ÷ (h × MAX_RATIO)
/// and logit m for two clocks, then the shape's logit.
fn unpack(x: &[f64], form: Form) -> (f64, f64, f64, f64) {
    let h = x[0].exp();
    match form {
        Form::Shape => (h, h, 0.0, shape_of(x[1])),
        Form::TwoClocks => (h, h * MAX_RATIO * sigmoid(x[1]), sigmoid(x[2]), 1.0),
        Form::Both => (
            h,
            h * MAX_RATIO * sigmoid(x[1]),
            sigmoid(x[2]),
            shape_of(x[3]),
        ),
    }
}

/// Fit two clocks to bins, starting from a one-clock fit.
pub fn fit_fast(bins: &[Bin], start: &Model) -> FastModel {
    fit_form(bins, start, Form::TwoClocks)
}

/// Fit one form to bins, starting from a one-clock fit.
pub fn fit_form(bins: &[Bin], start: &Model, form: Form) -> FastModel {
    let oldest = bins.iter().map(|b| b.age).fold(0.0, f64::max).max(1.0);
    let h0 = if start.half_life.is_finite() { start.half_life } else { oldest };
    let pinned = zero_age_s0(bins);
    let loss = |x: &[f64]| {
        let (h, hf, m, k) = unpack(x, form);
        if !(h.is_finite() && hf.is_finite() && hf > 0.0 && k.is_finite()) {
            return f64::INFINITY;
        }
        solve(bins, h, m, hf, k, pinned).1
    };
    let starts: Vec<Vec<f64>> = match form {
        Form::Shape => [(1.0, 0.7), (1.0, 1.5), (1.5, 3.0), (0.7, 2.0)]
            .iter()
            .map(|&(hs, k)| vec![(h0 * hs).ln(), shape_x(k)])
            .collect(),
        Form::TwoClocks => [
            (1.0, 0.05, 0.2),
            (2.0, 0.02, 0.3),
            (1.5, 0.2, 0.1),
            (3.0, 0.01, 0.5),
        ]
        .iter()
        .map(|&(hs, r, m)| vec![(h0 * hs).ln(), logit(r / MAX_RATIO), logit(m)])
        .collect(),
        Form::Both => [
            (1.0, 0.05, 0.2, 1.0),
            (2.0, 0.2, 0.3, 2.5),
            (1.5, 0.3, 0.6, 3.0),
            (3.0, 0.02, 0.4, 0.7),
        ]
        .iter()
        .map(|&(hs, r, m, k)| vec![(h0 * hs).ln(), logit(r / MAX_RATIO), logit(m), shape_x(k)])
        .collect(),
    };
    let mut best: Option<(Vec<f64>, f64)> = None;
    for x0 in starts {
        let (x, _) = nelder_mead(&loss, &x0, 0.5, 400);
        let (x, v) = nelder_mead(&loss, &x, 0.1, 400);
        if best.as_ref().map_or(true, |(_, bv)| v < *bv) {
            best = Some((x, v));
        }
    }
    let (h, hf, m, k) = unpack(&best.expect("four starts").0, form);
    solve(bins, h, m, hf, k, pinned).0
}

fn gap(bins: &[Bin], predict: impl Fn(f64) -> f64) -> f64 {
    bins.iter().map(|b| (b.correct - predict(b.age)).abs()).fold(0.0, f64::max)
}

/// The test: each form fitted on `train` and scored on `test` against one
/// clock (with a baseline when `with_baseline`); the chosen form refitted on all of `group`.
pub fn test_fast(
    group: &[&Observation],
    train: &[&Observation],
    test: &[&Observation],
    with_baseline: bool,
) -> FastFit {
    let train_bins = fine_bins(train);
    let train_plain = fit(&train_bins, with_baseline);
    let brier_plain = brier(test, |a| train_plain.predict(a));
    let all_bins = fine_bins(group);
    let plain = fit(&all_bins, with_baseline);
    let gap_plain = gap(&all_bins, |a| plain.predict(a));

    // Binomial log-likelihood of the binned data, for BIC
    let n: usize = all_bins.iter().map(|b| b.n).sum();
    let log_lik = |predict: &dyn Fn(f64) -> f64| -> f64 {
        all_bins
            .iter()
            .map(|b| {
                let p = predict(b.age).clamp(1e-12, 1.0 - 1e-12);
                b.n as f64 * (b.correct * p.ln() + (1.0 - b.correct) * (1.0 - p).ln())
            })
            .sum()
    };
    let bic_of =
        |log_lik: f64, parameters: usize| -2.0 * log_lik + parameters as f64 * (n as f64).ln();
    let bic = |form: Form, m: &FastModel| bic_of(log_lik(&|a| m.predict(a)), form.parameters());
    let bic_plain = bic_of(log_lik(&|a| plain.predict(a)), 1);
    let make = |form: Form, brier_fast: f64, model: FastModel, gap_fast: f64, kept: bool| {
        let fresh = model.fresh_age();
        let fresh_checks = all_bins
            .iter()
            .filter(|b| b.age <= fresh)
            .map(|b| b.n)
            .sum();
        FastFit {
            model,
            brier_fast,
            brier_plain,
            gap_fast,
            gap_plain,
            form,
            kept,
            too_uncertain: None,
            fresh_checks,
            source_fidelity: (fresh_checks >= MIN_FRESH).then_some(model.s0),
            fallbacks: Vec::new(),
        }
    };

    let mut passed = Vec::new();
    let mut failed = Vec::new();
    for &form in &Form::ALL {
        let on_train = fit_form(&train_bins, &train_plain, form);
        let brier_fast = brier(test, |a| on_train.predict(a));
        let model = fit_form(&all_bins, &plain, form);
        let gap_fast = gap(&all_bins, |a| model.predict(a));
        let passes = brier_fast < brier_plain * (1.0 - MIN_GAIN)
            && gap_fast < gap_plain * (1.0 - MIN_GAP_CUT)
            && bic(form, &model) < bic_plain;
        let fit = make(form, brier_fast, model, gap_fast, passes);
        if passes {
            passed.push((bic(form, &model), fit));
        } else {
            failed.push(fit);
        }
    }
    if passed.is_empty() {
        // None passed: report the one with the best held-out Brier
        return failed
            .into_iter()
            .min_by(|a, b| a.brier_fast.total_cmp(&b.brier_fast))
            .expect("three forms");
    }
    passed.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut ranked = passed.into_iter().map(|(_, f)| f);
    let mut chosen = ranked.next().expect("one passed");
    chosen.fallbacks = ranked.collect();
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibrate::{is_train, Rng};

    const DAY: f64 = 86_400.0;
    const LAGS: [f64; 13] = [0.0, 1.0, 3.0, 7.0, 14.0, 30.0, 61.0, 91.0, 182.0, 274.0, 365.0, 548.0, 730.0];

    /// `per_lag` entities checked once at each of PyPI's lags.
    fn ledger(predict: impl Fn(f64) -> f64, per_lag: usize, seed: u64) -> Vec<Observation> {
        let mut rng = Rng(seed);
        let mut out = Vec::new();
        for (li, lag) in LAGS.iter().enumerate() {
            let age = lag * DAY;
            for i in 0..per_lag {
                out.push(Observation { entity: format!("p{}-{}", li, i), age, correct: rng.unit() < predict(age), state: None });
            }
        }
        out
    }

    fn run(obs: &[Observation]) -> FastFit {
        let group: Vec<&Observation> = obs.iter().collect();
        let train: Vec<&Observation> = obs.iter().filter(|o| is_train(&o.entity)).collect();
        let test: Vec<&Observation> = obs.iter().filter(|o| !is_train(&o.entity)).collect();
        test_fast(&group, &train, &test, true)
    }

    fn release() -> FastModel {
        FastModel {
            s0: 1.0,
            baseline: 0.07,
            half_life: 60.0 * DAY,
            share: 0.3,
            fast_half_life: 2.0 * DAY,
            shape: 1.0,
        }
    }

    // spec: reference-procedure
    #[test]
    fn test_recovers_two_clocks() {
        let truth = release();
        let f = run(&ledger(|a| truth.predict(a), 2000, 1));
        let m = f.model;
        eprintln!("RECOVERED {:?}; Brier {:.5} vs {:.5}; gap {:.3} vs {:.3}", m, f.brier_fast, f.brier_plain, f.gap_fast, f.gap_plain);
        assert!(f.kept, "{:?}", f);
        // A shape below 1 is close to a mix of exponentials, so another form may be chosen.
        // Either way the curve must be the true one, within 3 standard errors of 2000 checks
        // (a shape alone misses by 3.7 at 7 days)
        for lag in LAGS {
            let p = truth.predict(lag * DAY);
            let se = (p * (1.0 - p) / 2000.0).sqrt().max(1e-3);
            assert!(
                (m.predict(lag * DAY) - p).abs() < 3.0 * se,
                "{}d: {:?}",
                lag,
                m
            );
        }
        if f.form == Form::TwoClocks {
            assert!(
                (m.share - 0.3).abs() < 0.05 && (m.fast_half_life / DAY / 2.0 - 1.0).abs() < 0.4,
                "{:?}",
                m
            );
        }
        // s0 is measured at age zero: one clock cannot blame the quick drop on the source,
        // so it misfits the curve instead
        let plain = fit(&fine_bins(&ledger(|a| truth.predict(a), 2000, 1).iter().collect::<Vec<_>>()), true);
        assert_eq!(plain.s0, 1.0);
        assert!(f.gap_plain > 0.1, "{:?}", f);
    }

    /// A moving ship's 1 km: right while it has gone less than a kilometre, so the curve
    /// holds, then drops; checked every minute for 30 minutes
    fn ship() -> FastModel {
        FastModel {
            s0: 1.0,
            baseline: 0.03,
            half_life: 180.0,
            share: 0.0,
            fast_half_life: 180.0,
            shape: 3.0,
        }
    }

    fn minutes(predict: impl Fn(f64) -> f64, per_lag: usize, seed: u64) -> Vec<Observation> {
        let mut rng = Rng(seed);
        let mut out = Vec::new();
        for lag in 0..=30 {
            for i in 0..per_lag {
                let age = lag as f64 * 60.0;
                out.push(Observation {
                    entity: format!("v{}-{}", lag, i),
                    age,
                    correct: rng.unit() < predict(age),
                    state: None,
                });
            }
        }
        out
    }

    // spec: reference-procedure
    #[test]
    fn test_recovers_a_shape() {
        let truth = ship();
        let f = run(&minutes(|a| truth.predict(a), 400, 3));
        eprintln!(
            "SHAPE {:?} {:?}; Brier {:.5} vs {:.5}",
            f.form, f.model, f.brier_fast, f.brier_plain
        );
        assert!(f.kept && f.form == Form::Shape, "{:?}", f);
        assert!(
            (f.model.shape - 3.0).abs() < 0.3 && (f.model.half_life / 180.0 - 1.0).abs() < 0.05,
            "{:?}",
            f.model
        );
    }

    // spec: reference-procedure
    #[test]
    fn test_one_clock_is_not_kept() {
        let truth = Model { s0: 1.0, baseline: 0.1, half_life: 80.0 * DAY };
        let f = run(&ledger(|a| truth.predict(a), 2000, 2));
        eprintln!("ONE CLOCK Brier {:.5} vs {:.5}; gap {:.3} vs {:.3}", f.brier_fast, f.brier_plain, f.gap_fast, f.gap_plain);
        assert!(!f.kept, "fast share kept on one clock: {:?}", f);
    }

    #[test]
    fn test_halfway_is_the_curve_half_life() {
        let m = release();
        assert!((m.decay(m.halfway()) - 0.5).abs() < 1e-9);
        let one = FastModel { share: 0.0, ..m };
        assert!((one.halfway() / one.half_life - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_fresh_age() {
        let m = release();
        let t = m.fresh_age();
        assert!((m.decay(t) - FRESH_DECAY).abs() < 1e-9);
        assert!(t < 2.0 * DAY);
    }

    #[test]
    fn test_fine_bins() {
        let obs: Vec<Observation> = (0..5000)
            .map(|i| Observation { entity: format!("e{}", i), age: i as f64, correct: i % 3 == 0, state: None })
            .collect();
        let refs: Vec<&Observation> = obs.iter().collect();
        let b = fine_bins(&refs);
        assert_eq!(b.len(), FINE_BINS + 1); // and one for the check at age zero
        assert_eq!(b.iter().map(|b| b.n).sum::<usize>(), 5000);
    }

    /// The draft's acceptance tests for richer curves (spec/draft-kumar-fid-00.md, "Acceptance
    /// tests"), through the full calibrate path, spread fallback included. Seeds are free; the
    /// bounds must hold for any. (written ledgers kept, ledgers whose written curve misses the
    /// truth by more than 3 standard errors at some age)
    fn acceptance(truth: impl Fn(f64) -> f64, ages: &[f64], per_age: usize, seeds: std::ops::Range<u64>) -> (usize, usize) {
        let (mut kept, mut wrong) = (0, 0);
        for seed in seeds {
            let mut rng = Rng(seed);
            let mut obs = Vec::new();
            for (ai, &age) in ages.iter().enumerate() {
                for i in 0..per_age {
                    obs.push(Observation { entity: format!("a{}-{}", ai, i), age, correct: rng.unit() < truth(age), state: None });
                }
            }
            let fits = crate::calibrate::calibrate(&obs).expect("calibrates");
            if let Some(f) = fits[0].kept_fast() {
                kept += 1;
                let miss = ages.iter().any(|&a| {
                    // one right and one wrong check added, so the error is not zero where p is 0 or 1
                    let (p, n) = (truth(a), per_age as f64);
                    let q = (n * p + 1.0) / (n + 2.0);
                    let se = (q * (1.0 - q) / n).sqrt();
                    let bad = (f.model.predict(a) - p).abs() > 3.0 * se;
                    if bad { eprintln!("MISS seed {} age {} truth {:.4} written {:.4} se {:.4} model {:?}", seed, a, p, f.model.predict(a), se, f.model); }
                    bad
                });
                wrong += miss as usize;
            }
        }
        (kept, wrong)
    }

    fn days() -> Vec<f64> {
        LAGS.iter().map(|d| d * DAY).collect()
    }

    fn minutes_ages() -> Vec<f64> {
        (0..=30).map(|m| m as f64 * 60.0).collect()
    }

    // spec: acceptance
    #[test]
    fn acceptance_no_false_richer_curve() {
        let a = Model { s0: 1.0, baseline: 0.1, half_life: 80.0 * DAY };
        let b = Model { s0: 0.95, baseline: 0.0, half_life: 20.0 * DAY };
        let (ka, _) = acceptance(|t| a.predict(t), &days(), 500, 100..120);
        let (kb, _) = acceptance(|t| b.predict(t), &days(), 500, 120..140);
        eprintln!("ACCEPTANCE false keeps: {} of 40", ka + kb);
        assert!(ka + kb <= 3, "a richer curve kept on one-clock truths in {} of 40 ledgers (bound: 3)", ka + kb);
    }

    // spec: acceptance
    #[test]
    fn acceptance_shape_found() {
        let truth = ship();
        let (kept, wrong) = acceptance(|t| truth.predict(t), &minutes_ages(), 100, 300..320);
        eprintln!("ACCEPTANCE shape: kept {} of 20, wrong curve in {}", kept, wrong);
        assert!(kept >= 18, "shape kept in {} of 20 ledgers (bound: 18)", kept);
        assert_eq!(wrong, 0, "written curve off by more than 3 standard errors in {} ledgers", wrong);
    }

    // spec: acceptance
    #[test]
    fn acceptance_two_clocks_found() {
        let truth = release();
        let (kept, wrong) = acceptance(|t| truth.predict(t), &days(), 500, 200..220);
        eprintln!("ACCEPTANCE two clocks: kept {} of 20, wrong curve in {}", kept, wrong);
        assert!(kept >= 18, "two clocks kept in {} of 20 ledgers (bound: 18)", kept);
        assert_eq!(wrong, 0, "written curve off by more than 3 standard errors in {} ledgers", wrong);
    }
}
