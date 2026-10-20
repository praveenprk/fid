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
//! fidelity. When the checks at age zero measure it tightly (standard
//! error under `MAX_PIN_SE`, counting one extra right and one extra wrong
//! check, so 20 of 20 is not tight but 500 of 500 is) it is measured there
//! directly, and the decay is fitted with s0 fixed: a fact that changes
//! fast in its first hours is then a fast curve, not a source that is
//! wrong when fresh. Otherwise s0 is the fitted curve's value at age zero:
//! a noisy fresh rate fixed in place would bend the rest of the curve. It is written to the registry only when enough checks fall in
//! fresh bins (the model has decayed less than a tenth of the way to the
//! baseline at the bin's age); otherwise it is an extrapolation and the
//! source class constant stays in force.
//!
//! Uncertainty comes from resampling entities with replacement and
//! refitting (a bootstrap, 200 rounds, fixed seed): the middle 95% of the
//! refitted values is the range. When the half-life's range spans more
//! than a factor of `max_spread`, the data cannot pin it down and
//! calibrate refuses, as it does for too little data.
//!
//! Every state is also fitted on two clocks (see `crate::fast`); the
//! fast share is written only when it beats one clock on the held-out
//! half, so a quick first drop is not read as an unreliable source.
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
/// Largest standard error of the age-zero rate for s0 to be fixed there.
pub const MAX_PIN_SE: f64 = 0.005;
/// A check is fresh while the decay factor 0.5^(age / h) is at least this.
pub const FRESH_DECAY: f64 = 0.9;
/// Bootstrap rounds, and the seed that makes them reproducible.
pub const BOOTSTRAP_ROUNDS: usize = 200;
const BOOTSTRAP_SEED: u64 = 0x5eed_f1d0;
/// Default limit on the half-life range: high end ÷ low end.
pub const MAX_SPREAD: f64 = 2.5;
/// A half-life beyond this many times the oldest check cannot be told
/// apart from no decay within the data.
const BEYOND_DATA: f64 = 10.0;
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

/// The middle 95% of bootstrap refits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub lo: f64,
    pub hi: f64,
}

impl Range {
    fn of(mut values: Vec<f64>) -> Range {
        values.sort_by(f64::total_cmp);
        let n = values.len();
        let lo = values[(0.025 * n as f64).floor() as usize];
        let hi = values[((0.975 * n as f64).ceil() as usize).clamp(1, n) - 1];
        Range { lo, hi }
    }

    pub fn contains(&self, v: f64) -> bool {
        self.lo <= v && v <= self.hi
    }
}

/// Uncertainty of the final model's values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Uncertainty {
    pub half_life: Range,
    pub baseline: Range,
    pub s0: Range,
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
    /// Checks in bins young enough to measure fidelity when fresh
    pub fresh_checks: usize,
    /// s0, when `fresh_checks` is enough to call it measured
    pub source_fidelity: Option<f64>,
    /// 95% ranges from the entity bootstrap, of the curve the registry gets
    /// (two clocks when the fast share is kept)
    pub uncertainty: Uncertainty,
    /// Half-life range, high end ÷ low end; infinite when one end is
    /// within the data and the other beyond it
    pub spread: f64,
    pub brier_baseline: f64,
    pub brier_zero: f64,
    pub brier_constant: f64,
    /// The echo test, when a period was given
    pub echo: Option<crate::echo::EchoFit>,
    /// The two-clock test
    pub fast: crate::fast::FastFit,
}

impl StateFit {
    /// The echo, if one was tested and kept.
    pub fn kept_echo(&self) -> Option<&crate::echo::EchoFit> {
        self.echo.as_ref().filter(|e| e.kept)
    }

    /// The fast share, if it was kept and no echo was (an echo takes precedence).
    pub fn kept_fast(&self) -> Option<&crate::fast::FastFit> {
        if self.kept_echo().is_some() {
            return None;
        }
        Some(&self.fast).filter(|f| f.kept)
    }
}

/// Deterministic 50/50 split by entity — the same as calibration/fidcal.py.
pub fn is_train(entity: &str) -> bool {
    Sha256::digest(entity.as_bytes())[0] % 2 == 0
}

/// The fraction correct among checks at age zero, when they measure it
/// tightly: at least `MIN_FRESH` of them, and a standard error under
/// `MAX_PIN_SE` with one right and one wrong check added (so a perfect
/// record needs about 400 checks).
pub fn zero_age_s0(bins: &[Bin]) -> Option<f64> {
    let (k, n) = bins.iter().filter(|b| b.age == 0.0).fold((0.0, 0), |(k, n), b| (k + b.correct * b.n as f64, n + b.n));
    let p = (k + 1.0) / (n as f64 + 2.0);
    let se = (p * (1.0 - p) / n as f64).sqrt();
    (n >= MIN_FRESH && se < MAX_PIN_SE).then(|| k / n as f64)
}

/// Group observations into bins: exact ages when there are few distinct
/// ages, otherwise equal-count bins at their mean age. Checks at age zero
/// always keep a bin of their own, so they can measure s0.
pub fn bins(obs: &[&Observation]) -> Vec<Bin> {
    let zero: Vec<&Observation> = obs.iter().copied().filter(|o| o.age == 0.0).collect();
    let distinct = obs.iter().map(|o| o.age.to_bits()).collect::<std::collections::BTreeSet<_>>().len();
    if !zero.is_empty() && distinct > MAX_EXACT_AGES {
        let rest: Vec<&Observation> = obs.iter().copied().filter(|o| o.age != 0.0).collect();
        let k = zero.iter().filter(|o| o.correct).count();
        let mut out = vec![Bin { age: 0.0, correct: k as f64 / zero.len() as f64, n: zero.len() }];
        out.extend(binned(&rest));
        return out;
    }
    binned(obs)
}

fn binned(obs: &[&Observation]) -> Vec<Bin> {
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
/// With `pinned`, s0 is that and only b is fitted.
fn solve(bins: &[Bin], h: f64, with_baseline: bool, pinned: Option<f64>) -> (Model, f64) {
    let d: Vec<f64> = bins.iter().map(|b| 0.5_f64.powf(b.age / h)).collect();
    let w: Vec<f64> = bins.iter().map(|b| b.n as f64).collect();
    let y: Vec<f64> = bins.iter().map(|b| b.correct).collect();

    if let Some(s0) = pinned {
        // y − s0 d ≈ b (1 − d)
        let (mut num, mut den) = (0.0, 0.0);
        for i in 0..bins.len() {
            let e = 1.0 - d[i];
            num += w[i] * e * (y[i] - s0 * d[i]);
            den += w[i] * e * e;
        }
        let b = if with_baseline && den > 0.0 { (num / den).clamp(0.0, s0) } else { 0.0 };
        let model = Model { s0, baseline: b, half_life: h };
        let loss = bins.iter().map(|b| b.n as f64 * (b.correct - model.predict(b.age)).powi(2)).sum();
        return (model, loss);
    }

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
///
/// s0 is measured at age zero when there are enough checks there (`zero_age_s0`).
pub fn fit(bins: &[Bin], with_baseline: bool) -> Model {
    let pinned = zero_age_s0(bins);
    let total: f64 = bins.iter().map(|b| b.n as f64).sum();
    let mean = bins.iter().map(|b| b.correct * b.n as f64).sum::<f64>() / total;
    let level = pinned.unwrap_or(mean);
    let no_decay = Model { s0: level, baseline: 0.0, half_life: f64::INFINITY };
    let no_decay_loss: f64 = bins.iter().map(|b| b.n as f64 * (b.correct - level).powi(2)).sum();

    let positive: Vec<f64> = bins.iter().map(|b| b.age).filter(|&a| a > 0.0).collect();
    if positive.is_empty() {
        return no_decay;
    }
    let lo_age = positive.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi_age = positive.iter().cloned().fold(0.0, f64::max);
    let (mut lo, mut hi) = ((lo_age / 100.0).ln(), (hi_age * 1000.0).ln());
    let mut best = solve(bins, lo.exp(), with_baseline, pinned);
    for _ in 0..4 {
        for i in 0..=200 {
            let h = (lo + (hi - lo) * i as f64 / 200.0).exp();
            let candidate = solve(bins, h, with_baseline, pinned);
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

/// splitmix64: a small, fixed pseudo-random sequence, so ranges are reproducible.
pub(crate) struct Rng(pub(crate) u64);

impl Rng {
    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// Uniform in [0, 1).
    #[cfg(test)]
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Random re-checks of `entities` entities at uniform ages up to `max_age`,
/// each correct with the model's probability. For tests.
#[cfg(test)]
pub(crate) fn random_ledger(truth: Model, entities: usize, max_age: f64, rng: &mut Rng) -> Vec<Observation> {
    (0..entities)
        .map(|i| {
            let age = rng.unit() * max_age;
            let correct = rng.unit() < truth.predict(age);
            Observation { entity: format!("r{}", i), age, correct, state: None }
        })
        .collect()
}

/// Bin index of each observation, as `bins` would group the whole group.
fn bin_index(group: &[&Observation]) -> (Vec<usize>, usize) {
    let mut order: Vec<usize> = (0..group.len()).collect();
    order.sort_by(|&a, &b| group[a].age.total_cmp(&group[b].age));
    let mut index = vec![0; group.len()];
    let distinct: Vec<u64> = {
        let mut d: Vec<u64> = order.iter().map(|&i| group[i].age.to_bits()).collect();
        d.dedup();
        d
    };
    if distinct.len() <= MAX_EXACT_AGES {
        for (i, o) in group.iter().enumerate() {
            index[i] = distinct.binary_search(&o.age.to_bits()).expect("age is in its own group");
        }
        return (index, distinct.len());
    }
    // As `bins`: checks at age zero in a bin of their own, the rest in equal-count bins
    let zeros = group.iter().filter(|o| o.age == 0.0).count();
    let offset = (zeros > 0) as usize;
    let rest = group.len() - zeros;
    let per_bin = rest.div_ceil(BINS).max(1);
    for (rank, &i) in order.iter().skip(zeros).enumerate() {
        index[i] = offset + rank / per_bin;
    }
    for &i in order.iter().take(zeros) {
        index[i] = 0;
    }
    (index, offset + rest.div_ceil(per_bin))
}

/// Refit on entities drawn with replacement, `BOOTSTRAP_ROUNDS` times.
///
/// Bin membership is fixed from the whole group; each round sums the
/// drawn entities' counts per bin, so a round costs one pass over the
/// entities, not a sort of the observations.
pub fn bootstrap(group: &[&Observation], with_baseline: bool) -> Uncertainty {
    let (mut h, mut b, mut s0) = (Vec::new(), Vec::new(), Vec::new());
    for sample in bootstrap_samples(group) {
        let m = fit(&sample, with_baseline);
        h.push(m.half_life);
        b.push(m.baseline);
        s0.push(m.s0);
    }
    Uncertainty { half_life: Range::of(h), baseline: Range::of(b), s0: Range::of(s0) }
}

/// As `bootstrap`, for a richer form: the ranges of the main half-life, baseline and s0.
pub fn bootstrap_fast(group: &[&Observation], with_baseline: bool, form: crate::fast::Form) -> Uncertainty {
    let (mut h, mut b, mut s0) = (Vec::new(), Vec::new(), Vec::new());
    for sample in bootstrap_samples(group) {
        let m = crate::fast::fit_form(&sample, &fit(&sample, with_baseline), form);
        h.push(m.half_life);
        b.push(m.baseline);
        s0.push(m.s0);
    }
    Uncertainty { half_life: Range::of(h), baseline: Range::of(b), s0: Range::of(s0) }
}

/// `BOOTSTRAP_ROUNDS` sets of bins from entities drawn with replacement.
pub fn bootstrap_samples(group: &[&Observation]) -> Vec<Vec<Bin>> {
    let (index, nbins) = bin_index(group);
    // Per entity: (bin, sum of ages, correct, n)
    let mut by_entity: BTreeMap<&str, BTreeMap<usize, (f64, usize, usize)>> = BTreeMap::new();
    for (o, &bin) in group.iter().zip(&index) {
        let e = by_entity.entry(o.entity.as_str()).or_default().entry(bin).or_insert((0.0, 0, 0));
        e.0 += o.age;
        e.1 += o.correct as usize;
        e.2 += 1;
    }
    let entities: Vec<Vec<(usize, f64, usize, usize)>> = by_entity
        .into_values()
        .map(|bins| bins.into_iter().map(|(bin, (age, k, n))| (bin, age, k, n)).collect())
        .collect();

    let mut rng = Rng(BOOTSTRAP_SEED);
    let mut out = Vec::with_capacity(BOOTSTRAP_ROUNDS);
    for _ in 0..BOOTSTRAP_ROUNDS {
        let mut sums = vec![(0.0, 0, 0); nbins];
        for _ in 0..entities.len() {
            for &(bin, age, k, n) in &entities[rng.below(entities.len())] {
                sums[bin].0 += age;
                sums[bin].1 += k;
                sums[bin].2 += n;
            }
        }
        out.push(
            sums.into_iter()
                .filter(|&(_, _, n)| n > 0)
                .map(|(age, k, n)| Bin { age: age / n as f64, correct: k as f64 / n as f64, n })
                .collect(),
        );
    }
    out
}

/// Is a half-life too long to tell from no decay, given checks up to `max_age`?
pub fn beyond_data(half_life: f64, max_age: f64) -> bool {
    half_life > BEYOND_DATA * max_age
}

/// High end ÷ low end of a half-life range. Half-lives beyond
/// `BEYOND_DATA` × the oldest age count as no decay: both ends there is
/// a spread of 1, one end there is infinite.
pub fn spread(range: &Range, max_age: f64) -> f64 {
    let beyond = |h: f64| beyond_data(h, max_age);
    match (beyond(range.lo), beyond(range.hi)) {
        (true, true) => 1.0,
        (false, true) => f64::INFINITY,
        _ => range.hi / range.lo,
    }
}

/// Fit every state with the default limit on the half-life range.
pub fn calibrate(obs: &[Observation]) -> Result<Vec<StateFit>, String> {
    calibrate_with(obs, MAX_SPREAD)
}

/// Fit every state. Either every observation has a state or none does.
/// A state whose half-life range spans more than `max_spread` is an error.
pub fn calibrate_with(obs: &[Observation], max_spread: f64) -> Result<Vec<StateFit>, String> {
    calibrate_periodic(obs, max_spread, None)
}

/// As `calibrate_with`, and with a period, test an echo at that period
/// on the held-out half (see `crate::echo`).
pub fn calibrate_periodic(obs: &[Observation], max_spread: f64, period: Option<f64>) -> Result<Vec<StateFit>, String> {
    calibrate_inner(obs, max_spread, period, true)
}

/// As `calibrate_with`, one clock only: no richer form is tried. For callers
/// that compare plain decay curves, such as `fid drift`.
pub fn calibrate_one_clock(obs: &[Observation], max_spread: f64) -> Result<Vec<StateFit>, String> {
    calibrate_inner(obs, max_spread, None, false)
}

fn calibrate_inner(obs: &[Observation], max_spread: f64, period: Option<f64>, richer: bool) -> Result<Vec<StateFit>, String> {
    if let Some(p) = period {
        if !(p.is_finite() && p > 0.0) {
            return Err(format!("invalid period {} (must be a positive duration)", p));
        }
    }
    if max_spread.is_nan() || max_spread < 1.0 {
        return Err(format!("invalid max spread {} (must be at least 1)", max_spread));
    }
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

    groups.into_iter().map(|(state, group)| fit_state(state, &group, max_spread, period, richer)).collect()
}

fn fit_state(state: Option<String>, group: &[&Observation], max_spread: f64, period: Option<f64>, richer: bool) -> Result<StateFit, String> {
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
    // Fresh by the bins the fit sees: a young check grouped into an old bin
    // does not inform s0
    let fresh_checks = all_bins.iter().filter(|b| b.age <= fresh_age(&model)).map(|b| b.n).sum();
    let echo = period.map(|p| crate::echo::test_echo(group, &train, &test, p));
    let mut fast = if richer {
        crate::fast::test_fast(group, &train, &test, uses_baseline)
    } else {
        crate::fast::FastFit::not_tried(&model)
    };
    let max_age = group.iter().map(|o| o.age).fold(0.0, f64::max);
    // The ranges, and the refusal, are for the curve the registry gets: two clocks when the
    // fast share is kept (and no echo is), else one. Two clocks too uncertain to write fall
    // back to one
    let mut written = None;
    if fast.kept && !echo.as_ref().is_some_and(|e| e.kept) {
        // The chosen form, else the next that passed, whose curve is pinned down
        let fallbacks = std::mem::take(&mut fast.fallbacks);
        let mut refused = Vec::new();
        for candidate in std::iter::once(fast.clone()).chain(fallbacks) {
            let u = bootstrap_fast(group, uses_baseline, candidate.form);
            let s = spread(&u.half_life, max_age);
            if s <= max_spread {
                fast = candidate;
                written = Some((u, s));
                break;
            }
            refused.push(format!(
                "{}: main half-life ranges {} to {} (×{:.1})",
                candidate.form.name(),
                format_half_life(u.half_life.lo),
                format_half_life(u.half_life.hi),
                s
            ));
        }
        if written.is_none() {
            fast.kept = false;
            fast.too_uncertain = Some(refused.join("; "));
        }
    }
    let (uncertainty, spread) = match written {
        Some(w) => w,
        None => {
            let u = bootstrap(group, uses_baseline);
            let s = spread(&u.half_life, max_age);
            (u, s)
        }
    };
    if spread > max_spread {
        return Err(format!(
            "{}: half-life too uncertain: 95% range {} to {} (×{:.1}; limit ×{}). Re-check more entities, \
             over a wider range of ages, or raise --max-spread",
            name,
            format_half_life(uncertainty.half_life.lo),
            format_half_life(uncertainty.half_life.hi),
            spread,
            trim(max_spread)
        ));
    }
    Ok(StateFit {
        state,
        observations: group.len(),
        entities,
        shape_error: shape_error(&all_bins, &model),
        empirical_half_life: empirical_half_life(&all_bins),
        bins: all_bins,
        fresh_checks,
        source_fidelity: (fresh_checks >= MIN_FRESH).then_some(model.s0),
        uncertainty,
        spread,
        model,
        uses_baseline,
        brier_baseline,
        brier_zero,
        brier_constant,
        echo,
        fast,
    })
}

// ── baselines per entity ───────────────────────────────────────────────

/// A check is stale while the decay factor is at most this.
pub const STALE_DECAY: f64 = 0.1;

/// One entity's own baseline: the chance its stale facts are right anyway.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityBaseline {
    pub entity: String,
    pub state: Option<String>,
    /// Shrunk toward the state's baseline by how much entities differ
    pub baseline: f64,
    /// The entity's own estimate, before shrinking
    pub raw: f64,
    /// Its stale checks
    pub stale_checks: usize,
}

/// How much baselines differ between entities, for one state.
#[derive(Debug, Clone, PartialEq)]
pub struct BaselineSpread {
    pub state: Option<String>,
    /// The state's baseline, from its fit
    pub baseline: f64,
    /// Standard deviation of true baselines across entities; 0 when the data show none
    pub between: f64,
    /// Entities with stale checks
    pub entities: usize,
}

/// The decay factor the fit uses at `age`: the kept fast share or echo's curve, else one clock.
fn decay_factor(f: &StateFit, age: f64) -> f64 {
    if let Some(e) = f.kept_echo() {
        let m = &e.model;
        return crate::compute::temporal_factor(age, Some(m.half_life), Some(&m.echo));
    }
    if let Some(fast) = f.kept_fast() {
        return fast.model.decay(age);
    }
    if f.model.half_life.is_infinite() {
        1.0
    } else {
        0.5_f64.powf(age.max(0.0) / f.model.half_life)
    }
}

/// Each entity's baseline from its own stale checks, shrunk toward the
/// state's (empirical Bayes, beta-binomial).
///
/// A stale check (decay factor d ≤ 0.1) is right with chance
/// b_e + (s0 − b_e) × d, so b_e is estimated as (k − s0 Σd) / Σ(1 − d).
/// The spread of true baselines, τ², is the variance of those estimates
/// less their sampling variance; shrinking weights an entity's estimate by
/// n / (n + κ) with κ = b(1 − b) / τ² − 1. With no spread in the data
/// (τ² ≤ 0) every entity gets the state's baseline.
pub fn entity_baselines(obs: &[Observation], fits: &[StateFit]) -> (Vec<EntityBaseline>, Vec<BaselineSpread>) {
    let mut out = Vec::new();
    let mut spreads = Vec::new();
    for f in fits {
        let (s0, b) = match (f.kept_echo(), f.kept_fast()) {
            (Some(e), _) => (e.model.s0, e.model.baseline),
            (None, Some(fast)) => (fast.model.s0, fast.model.baseline),
            _ => (f.model.s0, f.model.baseline),
        };
        // (k, Σd, Σ(1 − d), n) per entity
        let mut per: BTreeMap<&str, (f64, f64, f64, usize)> = BTreeMap::new();
        for o in obs.iter().filter(|o| o.state == f.state) {
            let d = decay_factor(f, o.age);
            if d > STALE_DECAY {
                continue;
            }
            let e = per.entry(o.entity.as_str()).or_insert((0.0, 0.0, 0.0, 0));
            e.0 += o.correct as u8 as f64;
            e.1 += d;
            e.2 += 1.0 - d;
            e.3 += 1;
        }
        let raw: Vec<(&str, f64, usize)> = per
            .iter()
            .map(|(e, &(k, sd, s1d, n))| (*e, ((k - s0 * sd) / s1d).clamp(0.0, 1.0), n))
            .collect();
        let m = raw.len() as f64;
        let between = if raw.len() < 2 {
            0.0
        } else {
            let mean = raw.iter().map(|r| r.1).sum::<f64>() / m;
            let var = raw.iter().map(|r| (r.1 - mean).powi(2)).sum::<f64>() / (m - 1.0);
            let sampling = raw.iter().map(|r| b * (1.0 - b) / r.2 as f64).sum::<f64>() / m;
            (var - sampling).max(0.0)
        };
        let kappa = if between > 0.0 { (b * (1.0 - b) / between - 1.0).max(0.0) } else { f64::INFINITY };
        for (entity, r, n) in raw {
            let w = if kappa.is_infinite() { 0.0 } else { n as f64 / (n as f64 + kappa) };
            out.push(EntityBaseline { entity: entity.to_string(), state: f.state.clone(), baseline: w * r + (1.0 - w) * b, raw: r, stale_checks: n });
        }
        spreads.push(BaselineSpread { state: f.state.clone(), baseline: b, between: between.sqrt(), entities: m as usize });
    }
    (out, spreads)
}

/// The per-entity baselines as CSV: entity, state, baseline, raw, stale_checks.
pub fn entity_baselines_csv(rows: &[EntityBaseline]) -> String {
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record(["entity", "state", "baseline", "raw", "stale_checks"]).expect("writing to memory");
    for r in rows {
        w.write_record([
            r.entity.as_str(),
            r.state.as_deref().unwrap_or(""),
            &format!("{:.4}", r.baseline),
            &format!("{:.4}", r.raw),
            &r.stale_checks.to_string(),
        ])
        .expect("writing to memory");
    }
    String::from_utf8(w.into_inner().expect("writing to memory")).expect("CSV is UTF-8")
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
        if let Some(e) = f.kept_echo() {
            let m = &e.model;
            let mut s = format!(
                "{}# echo kept: held-out Brier {:.4} vs decay {:.4}\n{}half_life: {}\n{}baseline: {}\n",
                indent,
                e.brier_echo,
                e.brier_decay,
                indent,
                format_half_life(m.half_life),
                indent,
                trim(m.baseline)
            );
            if e.source_fidelity.is_some() {
                s.push_str(&format!("{}source_fidelity: {}\n", indent, trim3(m.s0)));
            }
            s.push_str(&format!(
                "{}echo: {{ period: {}, strength: {}, width: {}, fade: {} }}\n",
                indent,
                format_half_life(m.echo.period),
                trim(m.echo.strength),
                format_half_life(m.echo.width),
                m.echo.fade.map_or("infinite".to_string(), format_half_life)
            ));
            return s;
        }
        if let Some(fast) = f.kept_fast() {
            let m = &fast.model;
            let u = &f.uncertainty;
            let mut s = format!(
                "{}# {} kept: held-out Brier {:.4} vs one clock {:.4}\n{}half_life: {}  # 95% range {} to {}\n{}baseline: {}  # 95% range {} to {}\n",
                indent,
                fast.form.name(),
                fast.brier_fast,
                fast.brier_plain,
                indent,
                format_half_life(m.half_life),
                format_half_life(u.half_life.lo),
                format_half_life(u.half_life.hi),
                indent,
                trim(m.baseline),
                trim(u.baseline.lo),
                trim(u.baseline.hi)
            );
            if fast.source_fidelity.is_some() {
                s.push_str(&format!("{}source_fidelity: {}  # 95% range {} to {}\n", indent, trim3(m.s0), trim3(u.s0.lo), trim3(u.s0.hi)));
            }
            if fast.form.has_two_clocks() {
                s.push_str(&format!(
                    "{}fast: {{ share: {}, half_life: {} }}\n",
                    indent,
                    trim3(m.share),
                    format_half_life(m.fast_half_life)
                ));
            }
            if fast.form.has_shape() {
                s.push_str(&format!("{}shape: {}\n", indent, trim3(m.shape)));
            }
            return s;
        }
        let u = &f.uncertainty;
        let mut s = format!(
            "{}half_life: {}  # 95% range {} to {}\n",
            indent,
            format_half_life(f.model.half_life),
            format_half_life(u.half_life.lo),
            format_half_life(u.half_life.hi)
        );
        if f.uses_baseline {
            s.push_str(&format!(
                "{}baseline: {}  # 95% range {} to {}\n",
                indent,
                trim(f.model.baseline),
                trim(u.baseline.lo),
                trim(u.baseline.hi)
            ));
        }
        if let Some(sf) = f.source_fidelity {
            s.push_str(&format!(
                "{}source_fidelity: {}  # 95% range {} to {}\n",
                indent,
                trim3(sf),
                trim3(u.s0.lo),
                trim3(u.s0.hi)
            ));
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
        let u = &f.uncertainty;
        lines.push(format!(
            "  95% ranges ({} entity resamples): half-life {} to {} (×{:.2}), baseline {:.2} to {:.2}, when fresh {:.3} to {:.3}",
            BOOTSTRAP_ROUNDS,
            format_half_life(u.half_life.lo),
            format_half_life(u.half_life.hi),
            f.spread,
            u.baseline.lo,
            u.baseline.hi,
            u.s0.lo,
            u.s0.hi
        ));
        lines.push(match f.source_fidelity {
            Some(_) => format!(
                "  source fidelity {:.3}, measured from {} checks in bins younger than {}",
                f.model.s0,
                f.fresh_checks,
                age_label(fresh_age(&f.model))
            ),
            None => format!(
                "  source fidelity not measured: {} checks in bins younger than {}, need {}; the source class constant stays",
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
        if let Some(e) = &f.echo {
            let m = &e.model;
            lines.push(format!(
                "  echo every {}: strength {:.2}, width {}, fade {}; half-life {}, baseline {:.2}",
                format_half_life(m.echo.period),
                m.echo.strength,
                format_half_life(m.echo.width),
                m.echo.fade.map_or("infinite".to_string(), format_half_life),
                format_half_life(m.half_life),
                m.baseline
            ));
            lines.push(format!(
                "  echo vs decay: held-out Brier {:.4} vs {:.4}, largest gap {:.3} vs {:.3}  {}",
                e.brier_echo,
                e.brier_decay,
                e.gap_echo,
                e.gap_decay,
                if e.kept {
                    "→ echo kept"
                } else {
                    "→ echo not kept (needs a 0.1% better Brier and a smaller gap); registry gets plain decay"
                }
            ));
        }
        let fast = &f.fast;
        let m = &fast.model;
        let described = match fast.form {
            crate::fast::Form::Shape => format!("a shape {:.2} on {}", m.shape, format_half_life(m.half_life)),
            crate::fast::Form::TwoClocks => format!(
                "{:.0}% on a {} clock, the rest {}",
                100.0 * m.share,
                format_half_life(m.fast_half_life),
                format_half_life(m.half_life)
            ),
            crate::fast::Form::Both => format!(
                "{:.0}% on a {} clock, the rest {}, shape {:.2}",
                100.0 * m.share,
                format_half_life(m.fast_half_life),
                format_half_life(m.half_life),
                m.shape
            ),
        };
        lines.push(format!(
            "  richer curve (a shape, two clocks, or both; lowest BIC of those that pass): {}; held-out Brier {:.4} vs one clock {:.4}, largest gap {:.3} vs {:.3}  {}",
            described,
            fast.brier_fast,
            fast.brier_plain,
            fast.gap_fast,
            fast.gap_plain,
            match (fast.kept, f.kept_echo().is_some(), &fast.too_uncertain) {
                (true, true, _) => "→ passed, but the echo is kept".to_string(),
                (true, false, _) => format!("→ {} kept", fast.form.name()),
                (false, _, Some(why)) => format!("→ passed on held-out data but too uncertain to write ({}); one clock instead", why),
                _ => "→ not kept (needs a 0.1% better held-out Brier, a 20% smaller largest gap and a lower BIC)".to_string(),
            }
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

    /// Entities whose stale facts are right with their own chance; each checked at `ages`, `per` times.
    fn varied_baselines(baselines: &[f64], per: usize, seed: u64) -> Vec<Observation> {
        let mut rng = Rng(seed);
        let mut out = Vec::new();
        for (e, &b) in baselines.iter().enumerate() {
            let m = Model { s0: 1.0, baseline: b, half_life: DAY };
            for i in 0..per {
                let age = (i % 20) as f64 * DAY; // 0 to 19 days: most checks are fully stale
                out.push(Observation { entity: format!("site-{}", e), age, correct: rng.unit() < m.predict(age), state: None });
            }
        }
        out
    }

    #[test]
    fn test_s0_measured_at_age_zero() {
        // A source right 100% of the time when fresh whose facts drop fast in the first day:
        // fitted freely, one clock would put s0 well under 1
        let truth = |age: f64| 0.1 + 0.9 * (0.6 * 0.5_f64.powf(age / (40.0 * DAY)) + 0.4 * 0.5_f64.powf(age / (0.3 * DAY)));
        let mut obs = Vec::new();
        for (ai, &age) in [0.0, 1.0, 3.0, 7.0, 14.0, 30.0, 60.0, 120.0].iter().enumerate() {
            let k = (500.0 * truth(age * DAY)).round() as usize;
            for i in 0..500 {
                obs.push(Observation { entity: format!("e{}-{}", ai, i), age: age * DAY, correct: i < k, state: None });
            }
        }
        let refs: Vec<&Observation> = obs.iter().collect();
        assert_eq!(zero_age_s0(&bins(&refs)), Some(1.0));
        let f = &calibrate(&obs).unwrap()[0];
        assert_eq!(f.model.s0, 1.0);
        assert_eq!(f.source_fidelity, Some(1.0));
        // fewer than MIN_FRESH checks at age zero: s0 is fitted as before
        let few: Vec<Observation> = obs.iter().filter(|o| o.age > 0.0 || o.entity.ends_with("-1")).cloned().collect();
        let refs: Vec<&Observation> = few.iter().collect();
        assert_eq!(zero_age_s0(&bins(&refs)), None);
    }

    #[test]
    fn test_entity_baselines_follow_the_entity() {
        let truth: Vec<f64> = (0..40).map(|i| if i % 2 == 0 { 0.2 } else { 0.8 }).collect();
        let obs = varied_baselines(&truth, 400, 11);
        let fits = calibrate(&obs).unwrap();
        let (rows, spreads) = entity_baselines(&obs, &fits);
        assert_eq!(rows.len(), 40);
        assert!(spreads[0].between > 0.2, "{:?}", spreads);
        for r in &rows {
            let i: usize = r.entity.trim_start_matches("site-").parse().unwrap();
            assert!((r.baseline - truth[i]).abs() < 0.08, "{:?} vs {}", r, truth[i]);
            // shrinking moves it toward the pooled baseline, never past it
            let pooled = spreads[0].baseline;
            assert!((r.baseline - pooled).abs() <= (r.raw - pooled).abs() + 1e-12);
        }
    }

    #[test]
    fn test_entity_baselines_pool_when_entities_agree() {
        let obs = varied_baselines(&[0.5; 30], 200, 12);
        let fits = calibrate(&obs).unwrap();
        let (rows, spreads) = entity_baselines(&obs, &fits);
        let pooled = spreads[0].baseline;
        // no real spread: estimates sit close to the pooled value
        assert!(spreads[0].between < 0.05, "{:?}", spreads);
        assert!(rows.iter().all(|r| (r.baseline - pooled).abs() < 0.03), "{:?}", rows);
        let csv = entity_baselines_csv(&rows);
        assert!(csv.starts_with("entity,state,baseline,raw,stale_checks\n"));
        assert_eq!(csv.lines().count(), 31);
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
        // the one check at age zero keeps a bin of its own
        assert_eq!(fits[0].bins.len(), BINS + 1);
        assert_eq!(fits[0].bins[0].age, 0.0);
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
    fn test_ranges_cover_the_truth() {
        // 40 independent ledgers from a known model: the 95% half-life
        // range should contain the true value in most of them.
        let truth = Model { s0: 0.95, baseline: 0.0, half_life: 10.0 * DAY };
        let mut rng = Rng(7);
        let (mut covered, mut s0_covered, trials) = (0, 0, 40);
        for _ in 0..trials {
            let obs = random_ledger(truth, 1500, 40.0 * DAY, &mut rng);
            let f = &calibrate_one_clock(&obs, f64::INFINITY).unwrap()[0];
            covered += f.uncertainty.half_life.contains(truth.half_life) as usize;
            s0_covered += f.uncertainty.s0.contains(truth.s0) as usize;
        }
        assert!(covered >= 34, "half-life covered in {} of {}", covered, trials);
        assert!(s0_covered >= 34, "s0 covered in {} of {}", s0_covered, trials);
    }

    #[test]
    fn test_ranges_narrow_with_more_data() {
        let truth = Model { s0: 0.95, baseline: 0.0, half_life: 10.0 * DAY };
        let mut rng = Rng(11);
        let small = &calibrate_one_clock(&random_ledger(truth, 300, 40.0 * DAY, &mut rng), f64::INFINITY).unwrap()[0];
        let large = &calibrate_one_clock(&random_ledger(truth, 5000, 40.0 * DAY, &mut rng), f64::INFINITY).unwrap()[0];
        assert!(large.spread < small.spread, "{} vs {}", large.spread, small.spread);
    }

    #[test]
    fn test_too_uncertain_is_error() {
        // 60 re-checks: the plan's example of a half-life that might be 3 days or 9
        let truth = Model { s0: 0.95, baseline: 0.0, half_life: 6.0 * DAY };
        let obs = random_ledger(truth, 60, 20.0 * DAY, &mut Rng(3));
        let err = calibrate(&obs).unwrap_err();
        assert!(err.contains("too uncertain") && err.contains("--max-spread"), "{}", err);
        assert!(calibrate_with(&obs, f64::INFINITY).is_ok());
        assert!(calibrate_with(&obs, 0.5).unwrap_err().contains("invalid max spread"));
    }

    #[test]
    fn test_ranges_are_reproducible() {
        let truth = Model { s0: 0.9, baseline: 0.2, half_life: 3.0 * DAY };
        let obs = random_ledger(truth, 800, 20.0 * DAY, &mut Rng(5));
        let a = calibrate_one_clock(&obs, f64::INFINITY).unwrap();
        let b = calibrate_one_clock(&obs, f64::INFINITY).unwrap();
        assert_eq!(a[0].uncertainty, b[0].uncertainty);
    }

    #[test]
    fn test_bin_index_matches_bins() {
        let mut rng = Rng(9);
        let truth = Model { s0: 0.9, baseline: 0.0, half_life: DAY };
        for obs in [
            random_ledger(truth, 1000, 5.0 * DAY, &mut rng),
            synthetic(truth, &days(&[0.0, 1.0, 2.0, 4.0]), 50, None),
        ] {
            let group: Vec<&Observation> = obs.iter().collect();
            let (index, nbins) = bin_index(&group);
            let mut sums = vec![(0.0, 0, 0); nbins];
            for (o, &i) in group.iter().zip(&index) {
                sums[i].0 += o.age;
                sums[i].1 += o.correct as usize;
                sums[i].2 += 1;
            }
            let expected = bins(&group);
            assert_eq!(expected.len(), nbins);
            for (bin, (age, k, n)) in expected.iter().zip(sums) {
                assert_eq!(bin.n, n);
                assert!((bin.age - age / n as f64).abs() < 1e-6);
                assert!((bin.correct - k as f64 / n as f64).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn test_spread() {
        let max_age = 10.0;
        assert_eq!(spread(&Range { lo: 2.0, hi: 5.0 }, max_age), 2.5);
        assert_eq!(spread(&Range { lo: 2.0, hi: f64::INFINITY }, max_age), f64::INFINITY);
        // Both ends beyond 10× the oldest check: no decay within the data
        assert_eq!(spread(&Range { lo: 150.0, hi: f64::INFINITY }, max_age), 1.0);
    }

    #[test]
    fn test_registry_yaml_shows_ranges() {
        let obs = synthetic(Model { s0: 0.95, baseline: 0.3, half_life: 30.0 * DAY }, &days(&[0.0, 3.0, 7.0, 14.0, 30.0, 60.0, 120.0, 365.0]), 200, None);
        let fits = calibrate(&obs).unwrap();
        let yaml = registry_yaml("policy_clause", &fits);
        assert_eq!(yaml.matches("# 95% range").count(), 3, "{}", yaml);
        assert!(report("policy_clause", &fits).contains("95% ranges (200 entity resamples)"));
    }

    #[test]
    fn test_young_checks_in_an_old_bin_do_not_measure_source_fidelity() {
        // True half-life 3h, checks spread over 40 days: the youngest of 20
        // equal-count bins averages about a day, so s0 is extrapolated
        let truth = Model { s0: 1.0, baseline: 0.62, half_life: 3.0 * 3600.0 };
        let obs = random_ledger(truth, 50_000, 40.0 * DAY, &mut Rng(13));
        let f = &calibrate_one_clock(&obs, f64::INFINITY).unwrap()[0];
        assert!(obs.iter().filter(|o| o.age < 3600.0).count() >= MIN_FRESH);
        assert_eq!(f.source_fidelity, None, "{:?}", f.model);
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
