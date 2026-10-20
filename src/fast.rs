//! Fitting a fast share: two clocks, tested in every `fid calibrate`.
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
//! It is kept only if, fitted on the train half, it beats one clock on the
//! held-out half by at least 0.1% (Brier), and its largest gap from the
//! curve is smaller. Otherwise the registry gets one clock and the report
//! says why.

use std::collections::BTreeMap;

use crate::calibrate::{brier, fit, zero_age_s0, Bin, Model, Observation, FRESH_DECAY, MIN_FRESH};
use crate::echo::nelder_mead;

/// Exact ages up to this many distinct values; otherwise this many equal-count bins.
const MAX_EXACT_AGES: usize = 2000;
const FINE_BINS: usize = 200;
/// Smallest held-out Brier improvement, relative, for the fast share to count.
pub const MIN_GAIN: f64 = 0.001;
/// The fast clock is at least this much faster than the slow one.
const MAX_RATIO: f64 = 0.5;

/// Decay to a baseline on two clocks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FastModel {
    pub s0: f64,
    pub baseline: f64,
    pub half_life: f64,
    /// Share on the fast clock
    pub share: f64,
    pub fast_half_life: f64,
}

impl FastModel {
    pub fn decay(&self, age: f64) -> f64 {
        let a = age.max(0.0);
        (1.0 - self.share) * 0.5_f64.powf(a / self.half_life) + self.share * 0.5_f64.powf(a / self.fast_half_life)
    }

    pub fn predict(&self, age: f64) -> f64 {
        self.baseline + (self.s0 - self.baseline) * self.decay(age)
    }

    /// Oldest age at which a check still counts as fresh: the decay factor is at least `FRESH_DECAY`.
    pub fn fresh_age(&self) -> f64 {
        let (mut lo, mut hi) = (0.0, self.half_life);
        for _ in 0..100 {
            let mid = 0.5 * (lo + hi);
            if self.decay(mid) >= FRESH_DECAY {
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
    /// Did the fast share pass on the held-out half?
    pub kept: bool,
    /// Checks in bins young enough to measure s0
    pub fresh_checks: usize,
    pub source_fidelity: Option<f64>,
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
fn solve(bins: &[Bin], half_life: f64, share: f64, fast_half_life: f64, pinned: Option<f64>) -> (FastModel, f64) {
    let shape = FastModel { s0: 1.0, baseline: 0.0, half_life, share, fast_half_life };
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

/// x = (ln h, logit of h_f ÷ (h × MAX_RATIO), logit m)
fn unpack(x: &[f64]) -> (f64, f64, f64) {
    let h = x[0].exp();
    (h, h * MAX_RATIO * sigmoid(x[1]), sigmoid(x[2]))
}

/// Fit two clocks to bins, starting from a one-clock fit.
pub fn fit_fast(bins: &[Bin], start: &Model) -> FastModel {
    let oldest = bins.iter().map(|b| b.age).fold(0.0, f64::max).max(1.0);
    let h0 = if start.half_life.is_finite() { start.half_life } else { oldest };
    let pinned = zero_age_s0(bins);
    let loss = |x: &[f64]| {
        let (h, hf, m) = unpack(x);
        if !(h.is_finite() && hf.is_finite() && hf > 0.0) {
            return f64::INFINITY;
        }
        solve(bins, h, m, hf, pinned).1
    };
    let mut best: Option<(Vec<f64>, f64)> = None;
    for (h_scale, ratio, share) in [(1.0, 0.05, 0.2), (2.0, 0.02, 0.3), (1.5, 0.2, 0.1), (3.0, 0.01, 0.5)] {
        let x0 = [(h0 * h_scale).ln(), logit(ratio / MAX_RATIO), logit(share)];
        let (x, _) = nelder_mead(&loss, &x0, 0.5, 400);
        let (x, v) = nelder_mead(&loss, &x, 0.1, 400);
        if best.as_ref().map_or(true, |(_, bv)| v < *bv) {
            best = Some((x, v));
        }
    }
    let (h, hf, m) = unpack(&best.expect("four starts").0);
    solve(bins, h, m, hf, pinned).0
}

fn gap(bins: &[Bin], predict: impl Fn(f64) -> f64) -> f64 {
    bins.iter().map(|b| (b.correct - predict(b.age)).abs()).fold(0.0, f64::max)
}

/// The fast-share test: fit on `train`, score on `test` against one clock
/// (with a baseline when `with_baseline`), refit on all of `group`.
pub fn test_fast(group: &[&Observation], train: &[&Observation], test: &[&Observation], with_baseline: bool) -> FastFit {
    let train_bins = fine_bins(train);
    let train_plain = fit(&train_bins, with_baseline);
    let train_fast = fit_fast(&train_bins, &train_plain);
    let brier_fast = brier(test, |a| train_fast.predict(a));
    let brier_plain = brier(test, |a| train_plain.predict(a));

    let all_bins = fine_bins(group);
    let plain = fit(&all_bins, with_baseline);
    let model = fit_fast(&all_bins, &plain);
    let gap_fast = gap(&all_bins, |a| model.predict(a));
    let gap_plain = gap(&all_bins, |a| plain.predict(a));

    let fresh = model.fresh_age();
    let fresh_checks = all_bins.iter().filter(|b| b.age <= fresh).map(|b| b.n).sum();
    FastFit {
        model,
        brier_fast,
        brier_plain,
        gap_fast,
        gap_plain,
        kept: brier_fast < brier_plain * (1.0 - MIN_GAIN) && gap_fast < gap_plain,
        fresh_checks,
        source_fidelity: (fresh_checks >= MIN_FRESH).then_some(model.s0),
    }
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
        FastModel { s0: 1.0, baseline: 0.07, half_life: 60.0 * DAY, share: 0.3, fast_half_life: 2.0 * DAY }
    }

    #[test]
    fn test_recovers_two_clocks() {
        let truth = release();
        let f = run(&ledger(|a| truth.predict(a), 2000, 1));
        let m = f.model;
        eprintln!("RECOVERED {:?}; Brier {:.5} vs {:.5}; gap {:.3} vs {:.3}", m, f.brier_fast, f.brier_plain, f.gap_fast, f.gap_plain);
        assert!(f.kept, "{:?}", f);
        assert!((m.share - 0.3).abs() < 0.05, "{:?}", m);
        assert!((m.half_life / DAY / 60.0 - 1.0).abs() < 0.15, "{:?}", m);
        assert!((m.fast_half_life / DAY / 2.0 - 1.0).abs() < 0.4, "{:?}", m);
        assert!((m.s0 - 1.0).abs() < 0.01 && (m.baseline - 0.07).abs() < 0.03, "{:?}", m);
        // s0 is measured at age zero: one clock cannot blame the quick drop on the source,
        // so it misfits the curve instead
        let plain = fit(&fine_bins(&ledger(|a| truth.predict(a), 2000, 1).iter().collect::<Vec<_>>()), true);
        assert_eq!(plain.s0, 1.0);
        assert!(f.gap_plain > 0.1, "{:?}", f);
    }

    #[test]
    fn test_one_clock_is_not_kept() {
        let truth = Model { s0: 1.0, baseline: 0.1, half_life: 80.0 * DAY };
        let f = run(&ledger(|a| truth.predict(a), 2000, 2));
        eprintln!("ONE CLOCK Brier {:.5} vs {:.5}; gap {:.3} vs {:.3}", f.brier_fast, f.brier_plain, f.gap_fast, f.gap_plain);
        assert!(!f.kept, "fast share kept on one clock: {:?}", f);
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

    /// How often a fast share is kept on one-clock data, and found on two.
    /// `cargo test --release --lib measure_fast -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn measure_fast_keeps() {
        let mut false_keeps = 0;
        for seed in 0..20 {
            for truth in [Model { s0: 1.0, baseline: 0.1, half_life: 80.0 * DAY }, Model { s0: 0.95, baseline: 0.0, half_life: 20.0 * DAY }] {
                false_keeps += run(&ledger(|a| truth.predict(a), 500, 100 + seed)).kept as usize;
            }
        }
        eprintln!("FALSE KEEPS {} of 40", false_keeps);
        let truth = release();
        let found = (0..20).filter(|&seed| run(&ledger(|a| truth.predict(a), 500, 200 + seed)).kept).count();
        eprintln!("TWO CLOCKS KEPT {} of 20 at 500 checks per lag", found);
    }
}
