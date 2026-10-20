//! Fidelity computation.
//!
//! Three axes, each independent:
//!   f_source    — initial fidelity from source class
//!   f_temporal  — decay as a function of age and half-life
//!   f_transform — cumulative noise from transformation chain
//!
//! Total: fidelity = f_source × f_temporal × f_transform
//!
//! With a baseline b — the chance a stale fact is right anyway — temporal
//! decay goes toward b instead of toward zero:
//!
//!   fidelity = b + (f_source × f_transform − b) × f_temporal
//!
//! Measured against public data, most facts decay toward a baseline
//! (see calibration/README.md). b = 0 recovers the original model.
//!
//! Without an echo, it never spontaneously increases: new measurement is
//! the only restoration mechanism. A periodic fact (temperature, traffic,
//! load) comes back once per period instead, and its echo adds
//!
//!   r × Σₙ≥₁ 0.5^(nP / h_r) × 0.5^(|age − nP| / w)
//!
//! to the temporal factor: strength r, period P, width w, fading with
//! half-life h_r over periods. The temporal factor stays at most 1.
//! Measured on airport temperature, see docs/FINDINGS.md, Finding 5.
//!
//! Some facts go stale on two clocks: most PyPI releases stay the latest
//! for months, but a share is replaced by a patch within days. A fast
//! share m with its own half-life h_f makes the temporal factor
//!
//!   (1 − m) × 0.5^(age / h) + m × 0.5^(age / h_f)
//!
//! so a quick first drop is not read as a source that is wrong when fresh.
//!
//! A fact that goes wrong by crossing a threshold holds first, then drops:
//! a moving ship's position is still within 1 km after a minute, gone
//! after five. A shape k on the clocks (a Weibull curve) makes each
//!
//!   0.5^((age / h)^k)
//!
//! k = 1 is the exponential; k > 1 holds and then falls; k < 1 falls fast
//! and then lingers.
//!
//! A store that re-checks a fact every T seconds serves it wrong a
//! fraction 1 − (1/T) ∫₀ᵀ fidelity of the time. `best_interval` picks
//! the T that minimizes that staleness plus a price per re-check, so one
//! price spends a re-check budget where it buys the most; a floor stays
//! a separate, per-decision guard (calibration/decisions.py).

use serde::{Deserialize, Serialize};

/// Parse a half-life string (e.g. "30s", "6h", "7d", "3mo", "3y") into seconds.
///
/// Returns Ok(None) for "infinite" (zero temporal decay).
/// Unparseable or non-positive values are an error — never silently
/// treated as infinite, which would hide decay.
pub fn parse_half_life(s: &str) -> Result<Option<f64>, String> {
    let t = s.trim().to_lowercase();
    if t == "infinite" || t == "inf" {
        return Ok(None);
    }
    let seconds = parse_duration(&t).map_err(|_| {
        format!("invalid half-life '{}' (expected e.g. 30s, 5m, 6h, 7d, 3mo, 3y, infinite)", t)
    })?;
    if seconds <= 0.0 {
        return Err(format!("invalid half-life '{}' (must be a positive duration)", t));
    }
    Ok(Some(seconds))
}

/// Parse a duration ("30s", "5m", "6h", "7d", "3mo", "3y", or bare seconds)
/// into seconds. Zero is allowed; negative, non-finite or unparseable is an error.
pub fn parse_duration(s: &str) -> Result<f64, String> {
    let s = s.trim().to_lowercase();
    let (num_str, multiplier) = if let Some(n) = s.strip_suffix("mo") {
        (n, 30.44 * 86400.0)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1.0)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60.0)
    } else if let Some(n) = s.strip_suffix('h') {
        (n, 3600.0)
    } else if let Some(n) = s.strip_suffix('d') {
        (n, 86400.0)
    } else if let Some(n) = s.strip_suffix('y') {
        (n, 365.25 * 86400.0)
    } else {
        // Assume seconds
        (s.as_str(), 1.0)
    };
    let n: f64 = num_str
        .parse()
        .map_err(|_| format!("invalid duration '{}' (expected e.g. 30s, 5m, 6h, 7d, 3mo, 3y)", s))?;
    if !n.is_finite() || n < 0.0 {
        return Err(format!("invalid duration '{}' (must be zero or more)", s));
    }
    Ok(n * multiplier)
}

/// Compute temporal fidelity decay.
///
/// f_temporal(t) = (1/2)^(t / t_half)
///
/// Where t is the age in seconds and t_half is the half-life in seconds.
/// If half-life is None (infinite), temporal fidelity is 1.0.
pub fn temporal_decay(age_seconds: f64, half_life_seconds: Option<f64>) -> f64 {
    match half_life_seconds {
        None => 1.0, // infinite half-life, no decay
        Some(hl) if hl <= 0.0 => 0.0, // degenerate case
        Some(hl) => {
            // Clock skew can make age negative; fidelity never increases.
            let exponent = age_seconds.max(0.0) / hl;
            0.5_f64.powf(exponent)
        }
    }
}

/// A periodic fact's echo, as written in registries and envelopes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Echo {
    /// Time between returns, e.g. "24h"
    pub period: String,
    /// Height of the first return, before fading (r)
    pub strength: f64,
    /// Half-width of each return, e.g. "3.75h"; less than half the period
    pub width: String,
    /// Half-life of the returns over periods, e.g. "345h", or "infinite"
    pub fade: String,
}

/// An echo in seconds, validated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EchoParams {
    pub period: f64,
    pub strength: f64,
    pub width: f64,
    /// None: the returns never fade
    pub fade: Option<f64>,
}

impl Echo {
    pub fn params(&self) -> Result<EchoParams, String> {
        let duration = |name: &str, text: &str| {
            parse_half_life(text)?.ok_or_else(|| format!("echo {} cannot be infinite", name))
        };
        let period = duration("period", &self.period)?;
        let width = duration("width", &self.width)?;
        let fade = parse_half_life(&self.fade)?;
        if !self.strength.is_finite() || self.strength < 0.0 {
            return Err(format!("invalid echo strength {} (must be zero or more)", self.strength));
        }
        if width >= period / 2.0 {
            return Err(format!("echo width {} must be less than half the period {}", self.width, self.period));
        }
        Ok(EchoParams { period, strength: self.strength, width, fade })
    }
}

/// Parse an envelope's or registry's decay: half-life in seconds, and the
/// echo if any. An echo needs a finite half-life.
pub fn decay_curve(half_life: Option<&str>, echo: Option<&Echo>) -> Result<(Option<f64>, Option<EchoParams>), String> {
    let h = match half_life {
        Some(hl) => parse_half_life(hl)?,
        None => None,
    };
    let e = echo.map(Echo::params).transpose()?;
    if e.is_some() && h.is_none() {
        return Err("an echo needs a finite half-life".to_string());
    }
    Ok((h, e))
}

/// Σₙ≥₁ 0.5^(nP / h_r) × 0.5^(|age − nP| / w), over returns within 30
/// widths of `age`; a farther one adds under 0.5^30 of its peak.
pub fn echo_sum(age_seconds: f64, e: &EchoParams) -> f64 {
    let t = age_seconds.max(0.0);
    let reach = 30.0 * e.width;
    let first = ((t - reach) / e.period).ceil().max(1.0) as u64;
    let last = ((t + reach) / e.period).floor().max(0.0) as u64;
    (first..=last)
        .map(|n| {
            let at = n as f64 * e.period;
            let fade = e.fade.map_or(1.0, |hr| 0.5_f64.powf(at / hr));
            fade * 0.5_f64.powf((t - at).abs() / e.width)
        })
        .sum()
}

/// The temporal factor with an optional echo, at most 1.
pub fn temporal_factor(age_seconds: f64, half_life_seconds: Option<f64>, echo: Option<&EchoParams>) -> f64 {
    Curve { half_life: half_life_seconds, fast: None, echo: echo.copied(), shape: None }.temporal(age_seconds)
}

/// Compute transformation noise factor.
///
/// f_transform = ∏(1 - noise_i) for each transform step.
///
/// Each noise_i is clamped to [0.0, 1.0].
pub fn transform_factor(noise_values: &[f64]) -> f64 {
    noise_values
        .iter()
        .map(|n| 1.0 - n.clamp(0.0, 1.0))
        .product()
}

/// Compute total fidelity.
///
/// fidelity = b + (f_source × f_transform − b) × f_temporal
///
/// `baseline` is clamped to [0, f_source × f_transform]: decay can only
/// lower fidelity, never raise it toward a baseline above it.
///
/// Returns None if source fidelity is indeterminate (Unknown source class).
pub fn total_fidelity(
    source_fidelity: Option<f64>,
    age_seconds: f64,
    half_life_seconds: Option<f64>,
    noise_values: &[f64],
    baseline: f64,
) -> Option<f64> {
    let f_source = source_fidelity?;
    let f_temporal = temporal_decay(age_seconds, half_life_seconds);
    let f_transform = transform_factor(noise_values);
    Some(apply_decay(f_source * f_transform, f_temporal, baseline))
}

/// When fidelity falls below a floor, measured from the moment of measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeToFloor {
    /// Seconds after measurement (0 if it starts at or below the floor)
    After(f64),
    /// Never: no decay, or the baseline is at or above the floor
    Never,
}

/// Time for fidelity to fall from `fidelity` (at measurement) to `floor`.
///
/// Solves b + (f − b) × 0.5^(t / h) = floor:
///   t = h × log2((f − b) / (floor − b))
///
/// This is the TTL a store can enforce as a hard cutoff, derived from
/// measured decay instead of declared.
pub fn time_to_floor(fidelity: f64, half_life_seconds: Option<f64>, baseline: f64, floor: f64) -> TimeToFloor {
    if fidelity < floor {
        return TimeToFloor::After(0.0);
    }
    let b = baseline.clamp(0.0, fidelity.max(0.0));
    match half_life_seconds {
        None => TimeToFloor::Never,
        Some(_) if floor <= b => TimeToFloor::Never,
        Some(h) if h <= 0.0 => TimeToFloor::After(0.0),
        Some(h) => TimeToFloor::After(h * ((fidelity - b) / (floor - b)).log2()),
    }
}

/// First time fidelity falls below `floor`, with an echo.
///
/// The curve without the echo is never above the curve with it, so the
/// closed-form crossing without the echo is a lower bound; from there the
/// curve is scanned in steps of a fiftieth of the shorter of half-life
/// and echo width, and the crossing refined by bisection. Never, when it
/// stays at or above the floor for 10,000 periods.
pub fn time_to_floor_echo(fidelity: f64, half_life_seconds: f64, baseline: f64, floor: f64, echo: &EchoParams) -> TimeToFloor {
    let start = match time_to_floor(fidelity, Some(half_life_seconds), baseline, floor) {
        TimeToFloor::Never => return TimeToFloor::Never,
        TimeToFloor::After(t) if t == 0.0 && fidelity < floor => return TimeToFloor::After(0.0),
        TimeToFloor::After(t) => t,
    };
    let at = |t: f64| apply_decay(fidelity, temporal_factor(t, Some(half_life_seconds), Some(echo)), baseline);
    let step = half_life_seconds.min(echo.width) / 50.0;
    first_below(&at, floor, start, step, start + 10_000.0 * echo.period)
}

/// Decay `fidelity` by the temporal factor, toward `baseline` rather than 0.
pub fn apply_decay(fidelity: f64, temporal: f64, baseline: f64) -> f64 {
    let b = baseline.clamp(0.0, fidelity.max(0.0));
    b + (fidelity - b) * temporal
}

// ── two clocks ─────────────────────────────────────────────────────────

/// A share of facts that go stale on a faster clock, as written in
/// registries and envelopes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fast {
    /// Fraction of facts on the fast clock, 0 to 1
    pub share: f64,
    /// Their half-life, e.g. "2d"; shorter than the main half-life
    pub half_life: String,
}

/// A fast share in seconds, validated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FastParams {
    pub share: f64,
    pub half_life: f64,
}

impl Fast {
    pub fn params(&self) -> Result<FastParams, String> {
        if !self.share.is_finite() || !(0.0..=1.0).contains(&self.share) {
            return Err(format!("invalid fast share {} (must be between 0.0 and 1.0)", self.share));
        }
        let half_life = parse_half_life(&self.half_life)?.ok_or_else(|| "fast half-life cannot be infinite".to_string())?;
        Ok(FastParams { share: self.share, half_life })
    }
}

/// The temporal curve: main half-life (None: no decay), fast share, echo,
/// and the clocks' shape (None: 1, the exponential).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Curve {
    pub half_life: Option<f64>,
    pub fast: Option<FastParams>,
    pub echo: Option<EchoParams>,
    pub shape: Option<f64>,
}

/// Smallest and largest shape accepted.
pub const SHAPE_RANGE: (f64, f64) = (0.1, 10.0);

/// Check a shape: finite, from 0.1 to 10.
pub fn check_shape(k: f64) -> Result<(), String> {
    if k.is_finite() && (SHAPE_RANGE.0..=SHAPE_RANGE.1).contains(&k) {
        Ok(())
    } else {
        Err(format!("invalid shape {} (must be between {} and {})", k, SHAPE_RANGE.0, SHAPE_RANGE.1))
    }
}

/// Parse and validate a whole curve. A fast share, an echo and a shape need
/// a finite half-life; the fast half-life must be shorter than it.
pub fn curve(half_life: Option<&str>, fast: Option<&Fast>, echo: Option<&Echo>, shape: Option<f64>) -> Result<Curve, String> {
    let (h, e) = decay_curve(half_life, echo)?;
    if let Some(k) = shape {
        check_shape(k)?;
        if h.is_none() {
            return Err("a shape needs a finite half-life".to_string());
        }
    }
    let f = fast.map(Fast::params).transpose()?;
    if let Some(f) = f {
        match h {
            None => return Err("a fast share needs a finite half-life".to_string()),
            Some(h) if f.half_life >= h => {
                return Err(format!(
                    "fast half-life {}s must be shorter than the half-life {}s",
                    f.half_life, h
                ))
            }
            _ => {}
        }
    }
    Ok(Curve { half_life: h, fast: f, echo: e, shape })
}

/// One clock: 0.5^((t / h)^k); 1 when there is no decay.
fn clock(t: f64, h: Option<f64>, k: f64) -> f64 {
    match h {
        None => 1.0,
        Some(h) if h <= 0.0 => 0.0,
        Some(h) if k == 1.0 => 0.5_f64.powf(t.max(0.0) / h),
        Some(h) => 0.5_f64.powf((t.max(0.0) / h).powf(k)),
    }
}

/// ∫₀ᵗ 0.5^((u / h)^k) du; t when there is no decay. For k ≠ 1 it is
/// h × ln2^(−1/k) / k × γ(1/k, ln2 × (t/h)^k), γ the lower incomplete gamma.
fn decay_integral(t: f64, h: Option<f64>, k: f64) -> f64 {
    match h {
        None => t,
        // −expm1 keeps precision when t ≪ h, where 1 − 0.5^(t/h) cancels
        Some(h) if k == 1.0 => h / std::f64::consts::LN_2 * -(-t * std::f64::consts::LN_2 / h).exp_m1(),
        Some(h) => {
            let a = 1.0 / k;
            let x = std::f64::consts::LN_2 * (t / h).powf(k);
            h * std::f64::consts::LN_2.powf(-a) / k * gamma_p(a, x) * ln_gamma(a).exp()
        }
    }
}

/// ln Γ(x) for x > 0 (Lanczos, g = 7, n = 9; relative error under 1e-14).
pub fn ln_gamma(x: f64) -> f64 {
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        // reflection
        return (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + 7.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The regularized lower incomplete gamma P(a, x): a series below a + 1,
/// a continued fraction above (Numerical Recipes 6.2).
pub fn gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x.is_infinite() {
        return 1.0;
    }
    let front = (-x + a * x.ln() - ln_gamma(a)).exp();
    if x < a + 1.0 {
        let (mut sum, mut term, mut n) = (1.0 / a, 1.0 / a, a);
        for _ in 0..1000 {
            n += 1.0;
            term *= x / n;
            sum += term;
            if term.abs() < sum.abs() * 1e-16 {
                break;
            }
        }
        (sum * front).min(1.0)
    } else {
        let tiny = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..1000 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < tiny {
                d = tiny;
            }
            c = b + an / c;
            if c.abs() < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let delta = d * c;
            h *= delta;
            if (delta - 1.0).abs() < 1e-16 {
                break;
            }
        }
        (1.0 - front * h).max(0.0)
    }
}

impl Curve {
    /// The shape of the clocks; 1 when none is given.
    pub fn k(&self) -> f64 {
        self.shape.unwrap_or(1.0)
    }

    /// The temporal factor at `age`: two clocks, plus the echo, at most 1.
    pub fn temporal(&self, age: f64) -> f64 {
        let k = self.k();
        let slow = clock(age, self.half_life, k);
        let base = match self.fast {
            None => slow,
            Some(f) => (1.0 - f.share) * slow + f.share * clock(age, Some(f.half_life), k),
        };
        match &self.echo {
            None => base,
            Some(e) => (base + e.strength * echo_sum(age, e)).min(1.0),
        }
    }

    /// ∫₀ᵗ temporal, in closed form. With an echo, each return integrates
    /// exactly; only where the cap at 1 binds (young ages, where the main
    /// clock is still near 1) is the excess integrated numerically.
    pub fn integral(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        let k = self.k();
        let base = |t: f64| match self.fast {
            None => decay_integral(t, self.half_life, k),
            Some(f) => (1.0 - f.share) * decay_integral(t, self.half_life, k) + f.share * decay_integral(t, Some(f.half_life), k),
        };
        let Some(e) = self.echo else {
            return base(t);
        };
        // Σₙ fadeⁿ ∫₀ᵗ 0.5^(|u − nP| / w) du
        let k = std::f64::consts::LN_2 / e.width;
        let one = |c: f64| {
            if t <= c {
                (-k * (c - t)).exp() * -(-k * t).exp_m1() / k
            } else {
                (-(-k * c).exp_m1() - (-k * (t - c)).exp_m1()) / k
            }
        };
        let last = ((t + 30.0 * e.width) / e.period).ceil() as u64;
        let mut returns = 0.0;
        for n in 1..=last.max(1) {
            let at = n as f64 * e.period;
            returns += e.fade.map_or(1.0, |hr| 0.5_f64.powf(at / hr)) * one(at);
        }
        let uncapped = base(t) + e.strength * returns;
        // The cap binds only while plain decay + strength × (largest possible echo sum) > 1
        let plain = Curve { echo: None, ..*self };
        let largest = 2.0 / (1.0 - 0.5_f64.powf(e.period / e.width));
        let reach = if e.strength * largest >= 1.0 {
            t
        } else {
            let level = 1.0 - e.strength * largest;
            if plain.temporal(t) > level {
                t
            } else {
                let (mut lo, mut hi) = (0.0, t);
                for _ in 0..100 {
                    let mid = 0.5 * (lo + hi);
                    if plain.temporal(mid) > level {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                hi
            }
        };
        let excess = |u: f64| (plain.temporal(u) + e.strength * echo_sum(u, &e) - 1.0).max(0.0);
        let n = 1000;
        let step = reach / n as f64;
        let mut sum = excess(0.0) + excess(reach);
        for i in 1..n {
            sum += excess(i as f64 * step) * if i % 2 == 1 { 4.0 } else { 2.0 };
        }
        uncapped - sum * step / 3.0
    }

    /// When fidelity, `fidelity` at measurement, first falls below `floor`.
    pub fn time_to_floor(&self, fidelity: f64, baseline: f64, floor: f64) -> TimeToFloor {
        if self.fast.is_none() && self.shape.is_none() {
            // The exponential: closed form, and the echo search the spec describes
            return match (self.echo, self.half_life) {
                (None, _) => time_to_floor(fidelity, self.half_life, baseline, floor),
                (Some(e), Some(h)) => time_to_floor_echo(fidelity, h, baseline, floor, &e),
                (Some(_), None) => TimeToFloor::Never,
            };
        }
        // The main clock alone: closed form, h × log2((f − b) / (floor − b))^(1/k)
        let k = self.k();
        let main = match time_to_floor(fidelity, self.half_life, baseline, floor) {
            TimeToFloor::After(t) if t > 0.0 => match self.half_life {
                Some(h) => h * (t / h).powf(1.0 / k),
                None => return TimeToFloor::Never,
            },
            other => return other,
        };
        // A faster share only brings the crossing earlier: the main clock's crossing brackets it
        let plain = Curve { echo: None, ..*self };
        let below = |t: f64| apply_decay(fidelity, plain.temporal(t), baseline) < floor;
        let start = if self.fast.is_none() || !below(main) {
            main
        } else {
            let (mut lo, mut hi) = (0.0, main);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if below(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            hi
        };
        match self.echo {
            None => TimeToFloor::After(start),
            Some(e) => {
                let at = |t: f64| apply_decay(fidelity, self.temporal(t), baseline);
                let mut shortest = e.width.min(self.half_life.unwrap_or(e.width));
                if let Some(f) = self.fast {
                    shortest = shortest.min(f.half_life);
                }
                first_below(&at, floor, start, shortest / 50.0, start + 10_000.0 * e.period)
            }
        }
    }

    /// Mean fraction of the time a fact re-checked every `interval`
    /// seconds is served wrong: 1 − (1/T) ∫₀ᵀ fidelity.
    pub fn mean_staleness(&self, fidelity: f64, baseline: f64, interval: f64) -> f64 {
        let b = baseline.clamp(0.0, fidelity.max(0.0));
        if interval <= 0.0 {
            return 1.0 - fidelity;
        }
        1.0 - (b + (fidelity - b) * self.integral(interval) / interval)
    }

    /// Staleness when never re-checked: the long-run limit.
    fn staleness_never(&self, fidelity: f64, baseline: f64) -> Option<f64> {
        let b = baseline.clamp(0.0, fidelity.max(0.0));
        self.half_life?;
        match self.echo {
            Some(e) if e.fade.is_none() => None, // returns forever: no limit
            _ => Some(1.0 - b),
        }
    }

    /// A span that holds every timescale of the curve, for searches.
    fn scale(&self) -> f64 {
        let mut s = self.half_life.unwrap_or(DAY);
        if let Some(e) = self.echo {
            s = s.max(e.period).max(e.fade.unwrap_or(e.period));
        }
        s
    }
}

const DAY: f64 = 86_400.0;

/// First time `at` falls below `floor`, scanning from `start` in `step`
/// until `end`, refined by bisection. Never if it does not.
fn first_below(at: &dyn Fn(f64) -> f64, floor: f64, start: f64, step: f64, end: f64) -> TimeToFloor {
    let (mut lo, mut t) = (start, start);
    while t <= end {
        if at(t) < floor {
            let mut hi = t;
            for _ in 0..100 {
                let mid = 0.5 * (lo + hi);
                if at(mid) < floor {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return TimeToFloor::After(hi);
        }
        lo = t;
        t += step;
    }
    TimeToFloor::Never
}

/// How often to re-check a fact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Recheck {
    /// Every this many seconds
    Every(f64),
    /// Never: a re-check costs more than the staleness it removes
    Never,
}

/// Golden-section search for a minimum of `f` on [lo, hi].
fn golden(f: &dyn Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    let r = (5.0_f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = (hi - r * (hi - lo), lo + r * (hi - lo));
    let (mut fa, mut fb) = (f(a), f(b));
    for _ in 0..200 {
        if fa < fb {
            hi = b;
            b = a;
            fb = fa;
            a = hi - r * (hi - lo);
            fa = f(a);
        } else {
            lo = a;
            a = b;
            fa = fb;
            b = lo + r * (hi - lo);
            fb = f(b);
        }
    }
    0.5 * (lo + hi)
}

/// The re-check interval that minimizes mean staleness + `price` × re-checks per day.
///
/// `price` is how much staleness one more re-check a day is worth (e.g.
/// 0.001: a daily re-check must cut staleness by 0.1 percentage points).
/// One price across every kind of fact and state is the efficient way to
/// spend a re-check budget: each re-check goes where it removes the most
/// staleness. Searched on a log grid from one second to 10,000 times the
/// curve's longest timescale, then refined; Never when not re-checking
/// costs no more than the best interval.
pub fn best_interval(curve: &Curve, fidelity: f64, baseline: f64, price: f64) -> Result<Recheck, String> {
    if !(price.is_finite() && price > 0.0) {
        return Err(format!("invalid price {} (must be a positive number)", price));
    }
    if curve.half_life.is_none() && curve.echo.is_none() {
        return Ok(Recheck::Never); // no decay: a re-check never changes anything
    }
    let cost = |t: f64| curve.mean_staleness(fidelity, baseline, t) + price * DAY / t;
    let (lo, hi) = (1.0_f64.ln(), (10_000.0 * curve.scale()).ln());
    let grid: Vec<f64> = (0..=600).map(|i| (lo + (hi - lo) * i as f64 / 600.0).exp()).collect();
    let costs: Vec<f64> = grid.iter().map(|&t| cost(t)).collect();
    let i = (0..grid.len()).min_by(|&a, &b| costs[a].total_cmp(&costs[b])).expect("grid is not empty");
    let t = golden(&cost, grid[i.saturating_sub(1)], grid[(i + 1).min(grid.len() - 1)]);
    let best = cost(t).min(costs[i]);
    let t = if cost(t) <= costs[i] { t } else { grid[i] };
    match curve.staleness_never(fidelity, baseline) {
        Some(never) if never <= best => Ok(Recheck::Never),
        None if i == grid.len() - 1 => Ok(Recheck::Never),
        _ => Ok(Recheck::Every(t)),
    }
}

/// The longest interval that keeps mean staleness at or below `target`
/// (the first crossing, for a curve with an echo). An error when the fact
/// is staler than that even fresh.
pub fn interval_for_staleness(curve: &Curve, fidelity: f64, baseline: f64, target: f64) -> Result<Recheck, String> {
    if !(target.is_finite() && (0.0..=1.0).contains(&target)) {
        return Err(format!("invalid staleness target {} (must be between 0.0 and 1.0)", target));
    }
    if 1.0 - fidelity > target {
        return Err(format!(
            "staleness target {} is below the staleness of a fresh fact ({:.4})",
            target,
            1.0 - fidelity
        ));
    }
    let stale = |t: f64| curve.mean_staleness(fidelity, baseline, t);
    if curve.half_life.is_none() && curve.echo.is_none() {
        return Ok(Recheck::Never);
    }
    let (lo, hi) = (1e-3_f64.ln(), (10_000.0 * curve.scale()).ln());
    let mut prev = 1e-3;
    for i in 0..=2000 {
        let t = (lo + (hi - lo) * i as f64 / 2000.0).exp();
        if stale(t) > target {
            let (mut a, mut b) = (prev, t);
            for _ in 0..100 {
                let mid = 0.5 * (a + b);
                if stale(mid) > target {
                    b = mid;
                } else {
                    a = mid;
                }
            }
            return Ok(Recheck::Every(a));
        }
        prev = t;
    }
    Ok(Recheck::Never)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_half_life_seconds() {
        assert_eq!(parse_half_life("30s"), Ok(Some(30.0)));
    }

    #[test]
    fn test_parse_half_life_minutes() {
        assert_eq!(parse_half_life("5m"), Ok(Some(300.0)));
    }

    #[test]
    fn test_parse_half_life_months() {
        let result = parse_half_life("3mo").unwrap().unwrap();
        assert!((result - 3.0 * 30.44 * 86400.0).abs() < 1e-6);
    }

    #[test]
    fn test_parse_half_life_years() {
        assert_eq!(parse_half_life("1y"), Ok(Some(365.25 * 86400.0)));
    }

    #[test]
    fn test_parse_half_life_bare_number_is_seconds() {
        assert_eq!(parse_half_life("90"), Ok(Some(90.0)));
    }

    #[test]
    fn test_parse_half_life_invalid_is_error() {
        assert!(parse_half_life("30ms").is_err());
        assert!(parse_half_life("7days").is_err());
        assert!(parse_half_life("").is_err());
        assert!(parse_half_life("abc").is_err());
    }

    #[test]
    fn test_parse_half_life_non_positive_is_error() {
        assert!(parse_half_life("0d").is_err());
        assert!(parse_half_life("-5h").is_err());
        assert!(parse_half_life("nan").is_err());
        assert!(parse_half_life("infd").is_err());
    }

    #[test]
    fn test_parse_half_life_hours() {
        assert_eq!(parse_half_life("6h"), Ok(Some(21600.0)));
    }

    #[test]
    fn test_parse_half_life_days() {
        assert_eq!(parse_half_life("7d"), Ok(Some(604800.0)));
    }

    #[test]
    fn test_parse_half_life_infinite() {
        assert_eq!(parse_half_life("infinite"), Ok(None));
    }

    #[test]
    fn test_parse_duration_allows_zero() {
        assert_eq!(parse_duration("0d"), Ok(0.0));
        assert_eq!(parse_duration("2h"), Ok(7200.0));
        assert_eq!(parse_duration("90"), Ok(90.0));
        assert!(parse_duration("-1d").is_err());
        assert!(parse_duration("inf").is_err());
        assert!(parse_duration("7days").is_err());
    }

    #[test]
    fn test_temporal_decay_zero_age() {
        let result = temporal_decay(0.0, Some(86400.0));
        assert!((result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_temporal_decay_one_half_life() {
        let result = temporal_decay(86400.0, Some(86400.0));
        assert!((result - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_temporal_decay_two_half_lives() {
        let result = temporal_decay(172800.0, Some(86400.0));
        assert!((result - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_temporal_decay_infinite_half_life() {
        let result = temporal_decay(999999.0, None);
        assert!((result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_temporal_decay_negative_age_does_not_increase() {
        let result = temporal_decay(-86400.0, Some(86400.0));
        assert!((result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_factor_no_transforms() {
        let result = transform_factor(&[]);
        assert!((result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_factor_single() {
        let result = transform_factor(&[0.10]);
        assert!((result - 0.90).abs() < 1e-10);
    }

    #[test]
    fn test_transform_factor_chain() {
        let result = transform_factor(&[0.05, 0.08, 0.10]);
        let expected = 0.95 * 0.92 * 0.90;
        assert!((result - expected).abs() < 1e-10);
    }

    #[test]
    fn test_total_fidelity_basic() {
        // source: 0.90, age: 1 half-life, one transform at 0.10 noise
        let result = total_fidelity(Some(0.90), 86400.0, Some(86400.0), &[0.10], 0.0);
        let expected = 0.90 * 0.5 * 0.90;
        assert!((result.unwrap() - expected).abs() < 1e-10);
    }

    #[test]
    fn test_total_fidelity_unknown_source() {
        let result = total_fidelity(None, 0.0, Some(86400.0), &[], 0.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_total_fidelity_with_baseline() {
        // source 0.90, one half-life, baseline 0.70: 0.70 + 0.20 × 0.5
        let result = total_fidelity(Some(0.90), 86400.0, Some(86400.0), &[], 0.70);
        assert!((result.unwrap() - 0.80).abs() < 1e-10);
    }

    #[test]
    fn test_baseline_is_the_long_run_limit() {
        let result = total_fidelity(Some(0.97), 1e12, Some(60.0), &[], 0.73);
        assert!((result.unwrap() - 0.73).abs() < 1e-10);
    }

    #[test]
    fn test_baseline_above_fidelity_never_raises_it() {
        // fidelity already 0.25 after transforms; baseline 0.70 must not lift it
        let result = total_fidelity(Some(0.50), 86400.0, Some(86400.0), &[0.50], 0.70);
        assert!((result.unwrap() - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_time_to_floor_round_trips() {
        let h = 7.0 * 86400.0;
        match time_to_floor(0.97, Some(h), 0.2, 0.7) {
            TimeToFloor::After(t) => {
                let at = apply_decay(0.97, temporal_decay(t, Some(h)), 0.2);
                assert!((at - 0.7).abs() < 1e-9);
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn test_time_to_floor_one_half_life() {
        // 0.8 → 0.4 with no baseline takes exactly one half-life
        assert_eq!(time_to_floor(0.8, Some(100.0), 0.0, 0.4), TimeToFloor::After(100.0));
    }

    #[test]
    fn test_time_to_floor_never() {
        assert_eq!(time_to_floor(0.97, None, 0.0, 0.7), TimeToFloor::Never);
        // baseline 0.73 never falls below a 0.70 floor
        assert_eq!(time_to_floor(0.97, Some(3600.0), 0.73, 0.70), TimeToFloor::Never);
    }

    #[test]
    fn test_time_to_floor_already_below() {
        assert_eq!(time_to_floor(0.5, Some(3600.0), 0.0, 0.7), TimeToFloor::After(0.0));
    }

    const HOUR: f64 = 3600.0;

    /// Airport temperature, fitted on all of 2024 (calibration/results/weather_periodic.json)
    fn temperature() -> EchoParams {
        Echo { period: "24h".into(), strength: 0.31126325754867784, width: "3.748493227942606h".into(), fade: "345.03278429702254h".into() }
            .params()
            .unwrap()
    }

    #[test]
    fn test_echo_matches_the_study() {
        // calibration/weather_periodic.py echo(t, 1.0, b, h, r, h_r, w) at these lags, in hours.
        // Up to 200h: past that the study leaves out returns beyond its 14 days of data
        let (b, h) = (0.12814426110884522, 2.415993491191711 * HOUR);
        let expected = [(0.0, 1.0), (1.0, 0.7862675984996086), (12.0, 0.1844581420002805), (24.0, 0.3905845824018509), (25.5, 0.3285742065170436), (48.0, 0.38043965521372036), (72.0, 0.3685992505822545), (100.0, 0.2416312385915916), (200.0, 0.17993489411394706)];
        for (t, want) in expected {
            let got = apply_decay(1.0, temporal_factor(t * HOUR, Some(h), Some(&temperature())), b);
            assert!((got - want).abs() < 1e-12, "{}h: {} vs {}", t, got, want);
        }
    }

    #[test]
    fn test_echo_comes_back() {
        let h = 2.4 * HOUR;
        let at = |t: f64| temporal_factor(t * HOUR, Some(h), Some(&temperature()));
        assert!(at(24.0) > at(12.0) * 2.0);
        assert!(at(24.0) > at(20.0) && at(24.0) > at(28.0));
        assert!(at(0.0) <= 1.0);
        assert!(at(48.0) < at(24.0)); // fades
    }

    #[test]
    fn test_echo_validation() {
        let e = |period: &str, strength: f64, width: &str, fade: &str| {
            Echo { period: period.into(), strength, width: width.into(), fade: fade.into() }.params()
        };
        assert!(e("24h", 0.3, "3h", "infinite").unwrap().fade.is_none());
        assert!(e("24h", 0.3, "12h", "10d").unwrap_err().contains("half the period"));
        assert!(e("24h", -0.1, "3h", "10d").is_err());
        assert!(e("infinite", 0.3, "3h", "10d").is_err());
        assert!(e("24h", 0.3, "0h", "10d").is_err());
        let echo = Echo { period: "24h".into(), strength: 0.3, width: "3h".into(), fade: "10d".into() };
        assert!(decay_curve(None, Some(&echo)).unwrap_err().contains("finite half-life"));
        assert!(decay_curve(Some("2h"), Some(&echo)).is_ok());
    }

    #[test]
    fn test_time_to_floor_echo() {
        let h = 2.415993491191711 * HOUR;
        let b = 0.12814426110884522;
        let e = temperature();
        let TimeToFloor::After(t) = time_to_floor_echo(1.0, h, b, 0.5, &e) else { panic!() };
        // Without the echo the crossing is closed-form and earlier
        let TimeToFloor::After(plain) = time_to_floor(1.0, Some(h), b, 0.5) else { panic!() };
        assert!(t > plain, "{} vs {}", t, plain);
        let at = |t: f64| apply_decay(1.0, temporal_factor(t, Some(h), Some(&e)), b);
        assert!((at(t) - 0.5).abs() < 1e-9);
        // and it is the first crossing
        assert!((0..1000).all(|i| at(t * i as f64 / 1000.0) >= 0.5));
        assert_eq!(time_to_floor_echo(1.0, h, b, 0.1, &e), TimeToFloor::Never);
        assert_eq!(time_to_floor_echo(0.4, h, b, 0.5, &e), TimeToFloor::After(0.0));
    }

    fn fast(share: f64, half_life: &str) -> Fast {
        Fast { share, half_life: half_life.into() }
    }

    fn two_clocks() -> Curve {
        curve(Some("80d"), Some(&fast(0.3, "3d")), None, None).unwrap()
    }

    #[test]
    fn test_fast_share_temporal() {
        let c = two_clocks();
        let day = 86_400.0;
        assert!((c.temporal(0.0) - 1.0).abs() < 1e-12);
        let want = 0.7 * 0.5_f64.powf(10.0 / 80.0) + 0.3 * 0.5_f64.powf(10.0 / 3.0);
        assert!((c.temporal(10.0 * day) - want).abs() < 1e-12);
        // long after, only the slow clock is left
        assert!((c.temporal(200.0 * day) / (0.7 * 0.5_f64.powf(200.0 / 80.0)) - 1.0).abs() < 1e-6);
        // a zero share is the plain curve
        let plain = curve(Some("80d"), Some(&fast(0.0, "3d")), None, None).unwrap();
        assert_eq!(plain.temporal(5.0 * day), temporal_decay(5.0 * day, Some(80.0 * day)));
    }

    #[test]
    fn test_fast_share_validation() {
        assert!(curve(Some("80d"), Some(&fast(1.2, "3d")), None, None).unwrap_err().contains("share"));
        assert!(curve(Some("80d"), Some(&fast(f64::NAN, "3d")), None, None).is_err());
        assert!(curve(Some("80d"), Some(&fast(0.3, "infinite")), None, None).unwrap_err().contains("infinite"));
        assert!(curve(Some("80d"), Some(&fast(0.3, "90d")), None, None).unwrap_err().contains("shorter"));
        assert!(curve(None, Some(&fast(0.3, "3d")), None, None).unwrap_err().contains("finite half-life"));
        assert!(curve(Some("infinite"), Some(&fast(0.3, "3d")), None, None).is_err());
    }

    #[test]
    fn test_time_to_floor_with_a_fast_share() {
        let c = two_clocks();
        let (f, b) = (0.99, 0.1);
        for floor in [0.95, 0.9, 0.7, 0.5, 0.2] {
            let TimeToFloor::After(t) = c.time_to_floor(f, b, floor) else { panic!("{}", floor) };
            let at = |t: f64| apply_decay(f, c.temporal(t), b);
            assert!((at(t) - floor).abs() < 1e-9, "{}: {}", floor, at(t));
            // earlier than the slow clock alone
            let TimeToFloor::After(slow) = time_to_floor(f, c.half_life, b, floor) else { panic!() };
            assert!(t < slow);
        }
        assert_eq!(c.time_to_floor(f, b, 0.1), TimeToFloor::Never);
        assert_eq!(c.time_to_floor(0.5, b, 0.7), TimeToFloor::After(0.0));
        // zero share: the closed form
        let plain = curve(Some("80d"), Some(&fast(0.0, "3d")), None, None).unwrap();
        let (TimeToFloor::After(a), TimeToFloor::After(c2)) = (plain.time_to_floor(f, b, 0.6), time_to_floor(f, plain.half_life, b, 0.6)) else { panic!() };
        assert!((a - c2).abs() < 1e-6);
    }

    #[test]
    fn test_time_to_floor_fast_share_and_echo() {
        let e = Echo { period: "24h".into(), strength: 0.31, width: "3.75h".into(), fade: "345h".into() };
        let c = curve(Some("2.4h"), Some(&fast(0.2, "0.5h")), Some(&e), None).unwrap();
        let TimeToFloor::After(t) = c.time_to_floor(1.0, 0.13, 0.5) else { panic!() };
        let at = |t: f64| apply_decay(1.0, c.temporal(t), 0.13);
        assert!((at(t) - 0.5).abs() < 1e-9);
        assert!((0..2000).all(|i| at(t * i as f64 / 2000.0) >= 0.5));
    }

    fn riemann(c: &Curve, t: f64) -> f64 {
        let n = 100_000;
        (0..n).map(|i| c.temporal((i as f64 + 0.5) * t / n as f64)).sum::<f64>() * t / n as f64
    }

    #[test]
    fn test_integral() {
        let day = 86_400.0;
        let c = two_clocks();
        for t in [0.5 * day, 3.0 * day, 40.0 * day, 400.0 * day] {
            assert!((c.integral(t) - riemann(&c, t)).abs() < 1e-6 * t, "{}", t);
        }
        let e = temperature();
        let echo = Curve { half_life: Some(2.4 * HOUR), fast: None, echo: Some(e), shape: None };
        for t in [6.0 * HOUR, 30.0 * HOUR, 100.0 * HOUR] {
            assert!((echo.integral(t) - riemann(&echo, t)).abs() < 1e-5 * t, "{}", t);
        }
        assert_eq!(Curve::default().integral(10.0), 10.0);
    }

    #[test]
    fn test_mean_staleness_limits() {
        let c = two_clocks();
        assert!((c.mean_staleness(0.98, 0.1, 1e-6) - 0.02).abs() < 1e-6);
        assert!((c.mean_staleness(0.98, 0.1, 1e12) - 0.9).abs() < 1e-3);
        // it only grows with the interval
        let mut prev = 0.0;
        for k in 0..60 {
            let s = c.mean_staleness(0.98, 0.1, 60.0 * 1.4_f64.powi(k));
            assert!(s >= prev - 1e-12);
            prev = s;
        }
    }

    fn brute_best(c: &Curve, f: f64, b: f64, price: f64) -> (f64, f64) {
        (0..5_000)
            .map(|i| {
                let t = (1.0_f64.ln() + (1e9_f64.ln() - 1.0_f64.ln()) * i as f64 / 5_000.0).exp();
                (t, c.mean_staleness(f, b, t) + price * 86_400.0 / t)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
    }

    #[test]
    fn test_best_interval_matches_brute_force() {
        let e = temperature();
        let curves = [
            curve(Some("1d"), None, None, None).unwrap(),
            two_clocks(),
            Curve { half_life: Some(2.4 * HOUR), fast: None, echo: Some(e), shape: None },
        ];
        for c in curves {
            for price in [1e-5, 1e-4, 1e-3] {
                let (f, b) = (1.0, 0.1);
                let Recheck::Every(t) = best_interval(&c, f, b, price).unwrap() else { panic!("{:?} {}", c, price) };
                let (bt, bc) = brute_best(&c, f, b, price);
                let got = c.mean_staleness(f, b, t) + price * 86_400.0 / t;
                assert!(got <= bc + 1e-9, "{:?} price {}: {} at {} vs {} at {}", c, price, got, t, bc, bt);
            }
        }
    }

    #[test]
    fn test_best_interval_behaviour() {
        let c = two_clocks();
        // a cheaper re-check means re-checking sooner
        let every = |p: f64| match best_interval(&c, 1.0, 0.1, p).unwrap() {
            Recheck::Every(t) => t,
            Recheck::Never => f64::INFINITY,
        };
        assert!(every(1e-5) < every(1e-4) && every(1e-4) < every(1e-3));
        // too dear: never
        assert_eq!(best_interval(&c, 1.0, 0.1, 1e6).unwrap(), Recheck::Never);
        // no decay: never
        assert_eq!(best_interval(&Curve::default(), 1.0, 0.0, 1e-3).unwrap(), Recheck::Never);
        assert!(best_interval(&c, 1.0, 0.1, 0.0).is_err());
        assert!(best_interval(&c, 1.0, 0.1, f64::NAN).is_err());
    }

    #[test]
    fn test_interval_for_staleness() {
        let c = two_clocks();
        for target in [0.02, 0.05, 0.1, 0.3] {
            let Recheck::Every(t) = interval_for_staleness(&c, 0.99, 0.1, target).unwrap() else { panic!() };
            assert!((c.mean_staleness(0.99, 0.1, t) - target).abs() < 1e-6, "{}", target);
        }
        assert_eq!(interval_for_staleness(&c, 0.99, 0.1, 0.95).unwrap(), Recheck::Never);
        assert!(interval_for_staleness(&c, 0.9, 0.1, 0.05).unwrap_err().contains("fresh"));
        assert!(interval_for_staleness(&c, 0.99, 0.1, 1.5).is_err());
    }

    #[test]
    fn test_special_functions() {
        // Γ(1/2) = √π, Γ(5) = 24, Γ(1/3) = 2.678938534707747
        assert!((ln_gamma(0.5) - std::f64::consts::PI.sqrt().ln()).abs() < 1e-13);
        assert!((ln_gamma(5.0) - 24.0_f64.ln()).abs() < 1e-13);
        assert!((ln_gamma(1.0 / 3.0) - 2.678_938_534_707_747_6_f64.ln()).abs() < 1e-13);
        // P(1, x) = 1 − e^(−x); P(1/2, x) = erf(√x): erf(1) = 0.8427007929497149
        for x in [0.01, 0.5, 2.0, 10.0] {
            assert!((gamma_p(1.0, x) - (1.0 - (-x as f64).exp())).abs() < 1e-14, "{}", x);
        }
        assert!((gamma_p(0.5, 1.0) - 0.842_700_792_949_714_9).abs() < 1e-14);
        assert_eq!(gamma_p(2.0, 0.0), 0.0);
        assert!((gamma_p(0.3, 1e3) - 1.0).abs() < 1e-15);
    }

    fn shaped(k: f64) -> Curve {
        curve(Some("3m"), None, None, Some(k)).unwrap()
    }

    #[test]
    fn test_shape_holds_then_drops() {
        let m = 60.0;
        let c = shaped(3.0);
        assert!((c.temporal(3.0 * m) - 0.5).abs() < 1e-12);
        assert!(c.temporal(1.0 * m) > 0.97 && c.temporal(5.0 * m) < 0.05);
        // shape 1 is the exponential
        assert_eq!(shaped(1.0).temporal(2.0 * m), temporal_decay(2.0 * m, Some(3.0 * m)));
        assert!(curve(Some("3m"), None, None, Some(0.0)).is_err());
        assert!(curve(Some("3m"), None, None, Some(f64::NAN)).is_err());
        assert!(curve(Some("3m"), None, None, Some(11.0)).is_err());
        assert!(curve(None, None, None, Some(2.0)).unwrap_err().contains("finite half-life"));
    }

    #[test]
    fn test_shaped_integrals() {
        let e = temperature();
        let curves = [
            shaped(0.5),
            shaped(2.0),
            shaped(3.3),
            curve(Some("8m"), Some(&fast(0.6, "3m")), None, Some(2.5)).unwrap(),
            Curve { half_life: Some(2.4 * HOUR), fast: None, echo: Some(e), shape: Some(1.7) },
        ];
        for c in curves {
            let scale = c.half_life.unwrap();
            for t in [0.3 * scale, scale, 4.0 * scale, 40.0 * scale] {
                assert!((c.integral(t) - riemann(&c, t)).abs() < 2e-6 * t, "{:?} at {}: {} vs {}", c, t, c.integral(t), riemann(&c, t));
            }
        }
    }

    #[test]
    fn test_shaped_time_to_floor() {
        let e = temperature();
        let curves = [
            shaped(3.0),
            shaped(0.6),
            curve(Some("8m"), Some(&fast(0.6, "3m")), None, Some(2.5)).unwrap(),
            Curve { half_life: Some(2.4 * HOUR), fast: None, echo: Some(e), shape: Some(1.7) },
        ];
        for c in curves {
            for floor in [0.9, 0.6, 0.3] {
                let TimeToFloor::After(t) = c.time_to_floor(1.0, 0.05, floor) else { panic!("{:?} {}", c, floor) };
                let at = |t: f64| apply_decay(1.0, c.temporal(t), 0.05);
                assert!((at(t) - floor).abs() < 1e-8, "{:?} {}: {}", c, floor, at(t));
                assert!((0..2000).all(|i| at(t * i as f64 / 2000.0) >= floor - 1e-12), "{:?} {}", c, floor);
            }
            assert_eq!(c.time_to_floor(1.0, 0.05, 0.04), TimeToFloor::Never);
        }
    }

    #[test]
    fn test_shaped_best_interval_matches_brute_force() {
        for c in [shaped(3.0), curve(Some("8m"), Some(&fast(0.6, "3m")), None, Some(2.5)).unwrap()] {
            for price in [1e-6, 1e-4] {
                let Recheck::Every(t) = best_interval(&c, 1.0, 0.03, price).unwrap() else { panic!() };
                let (_, bc) = brute_best(&c, 1.0, 0.03, price);
                let got = c.mean_staleness(1.0, 0.03, t) + price * 86_400.0 / t;
                assert!(got <= bc + 1e-9, "{:?} price {}: {} vs {}", c, price, got, bc);
            }
        }
    }

    #[test]
    fn test_zero_baseline_matches_original_model() {
        let with = total_fidelity(Some(0.9), 3600.0, Some(7200.0), &[0.1, 0.2], 0.0).unwrap();
        let original = 0.9 * temporal_decay(3600.0, Some(7200.0)) * transform_factor(&[0.1, 0.2]);
        assert!((with - original).abs() < 1e-12);
    }
}
