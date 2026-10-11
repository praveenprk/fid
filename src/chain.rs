//! `fid chain` — walk the fidelity chain of a stamped artifact.
//!
//! One line per contribution, in order: the source, each transform,
//! then temporal decay. Each line shows the cumulative fidelity after
//! that contribution and how much it removed.
//!
//! ```text
//! sensor-12        → fid: 0.97 (source: instrument)
//! normalize.py     → fid: 0.92 (−0.05 transform noise)
//! temporal (18d)   → fid: 0.62 (−0.30 temporal decay, half-life: 30d)
//! TOTAL:             0.62  SIGNAL (floor: 0.50)
//! ```

use crate::check::{format_age, CheckResult};
use crate::envelope::{FidStatus, Stamped};

/// One contribution to the chain.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainLine {
    /// Label for the left column (source id, transform step, "temporal (age)")
    pub label: String,
    /// Cumulative fidelity after this contribution (None if indeterminate)
    pub fid: Option<f64>,
    /// Description of the contribution
    pub note: String,
}

/// Build the chain lines. Multiplication commutes, so applying temporal
/// decay last gives the same total as `check`.
pub fn chain_lines(stamped: &Stamped, result: &CheckResult) -> Vec<ChainLine> {
    let env = &stamped.fid;
    let a = &result.assessment;
    let class = serde_json::to_value(&env.source_class)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();

    let mut lines = Vec::new();
    let mut current = a.source;
    lines.push(ChainLine {
        label: env.source_id.clone(),
        fid: current,
        note: format!("source: {}", class),
    });

    for t in &env.transforms {
        let before = current;
        current = current.map(|f| f * (1.0 - t.noise.clamp(0.0, 1.0)));
        lines.push(ChainLine {
            label: t.step.clone(),
            fid: current,
            note: match (before, current) {
                (Some(b), Some(c)) => format!("−{:.2} transform noise", b - c),
                _ => format!("noise {:.2}", t.noise),
            },
        });
    }

    let before = current;
    current = current.map(|f| f * a.temporal);
    let half_life = env.half_life.as_deref().unwrap_or("none");
    lines.push(ChainLine {
        label: format!("temporal ({})", format_age(result.age_seconds)),
        fid: current,
        note: match (before, current) {
            (Some(b), Some(c)) => format!("−{:.2} temporal decay, half-life: {}", b - c, half_life),
            _ => format!("decay ×{:.2}, half-life: {}", a.temporal, half_life),
        },
    });

    lines
}

/// The full `fid chain` report.
pub fn report(stamped: &Stamped, result: &CheckResult) -> String {
    let lines = chain_lines(stamped, result);
    let a = &result.assessment;
    let width = lines
        .iter()
        .map(|l| l.label.chars().count())
        .max()
        .unwrap_or(0)
        .max("TOTAL:".len())
        + 2;

    let fmt_fid = |f: Option<f64>| match f {
        Some(f) => format!("{:.2}", f),
        None => "?".to_string(),
    };

    let mut out = Vec::new();
    for l in &lines {
        out.push(format!(
            "{:<width$} → fid: {} ({})",
            l.label,
            fmt_fid(l.fid),
            l.note,
            width = width
        ));
    }

    let status = match a.status {
        FidStatus::Signal => "SIGNAL",
        FidStatus::Noise => "NOISE",
        FidStatus::Unknown => "UNKNOWN",
    };
    // Align TOTAL's value with the "fid: " values above.
    out.push(format!(
        "{:<width$}   {}  {} (floor: {:.2})",
        "TOTAL:",
        fmt_fid(a.fidelity),
        status,
        a.floor,
        width = width + "fid:".len() + 1
    ));
    if !result.integrity_ok {
        out.push("integrity mismatch: content changed since last stamp".to_string());
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::assess;
    use crate::envelope::{integrity_hash, FidEnvelope, SourceClass, TransformStep, FID_VERSION};
    use chrono::{DateTime, Duration, TimeZone, Utc};
    use serde_json::json;

    fn t0() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    fn stamped(class: SourceClass, half_life: Option<&str>, steps: &[(&str, f64)]) -> Stamped {
        let content = json!({"v": 1});
        Stamped {
            fid: FidEnvelope {
                fid_version: FID_VERSION.to_string(),
                source_class: class,
                source_id: "sensor-12".to_string(),
                measured_at: t0(),
                half_life: half_life.map(str::to_string),
                transforms: steps
                    .iter()
                    .map(|&(s, n)| TransformStep { step: s.to_string(), noise: n, at: t0() })
                    .collect(),
                fid_at_stamp: None,
                derived_from: None,
                integrity: integrity_hash(&content),
            },
            content,
        }
    }

    #[test]
    fn test_chain_lines_cumulative() {
        let s = stamped(
            SourceClass::Instrument,
            Some("30d"),
            &[("normalize.py", 0.05), ("aggregate.py", 0.08)],
        );
        let r = assess(&s, 0.50, t0() + Duration::days(30)).unwrap();
        let lines = chain_lines(&s, &r);

        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].label, "sensor-12");
        assert_eq!(lines[1].label, "normalize.py");
        assert_eq!(lines[2].label, "aggregate.py");
        assert_eq!(lines[3].label, "temporal (30d)");

        let expected = [0.97, 0.97 * 0.95, 0.97 * 0.95 * 0.92, 0.97 * 0.95 * 0.92 * 0.5];
        for (line, want) in lines.iter().zip(expected) {
            assert!((line.fid.unwrap() - want).abs() < 1e-10, "{:?}", line);
        }
        // Last line equals check's total
        assert!((lines[3].fid.unwrap() - r.assessment.fidelity.unwrap()).abs() < 1e-12);
        assert_eq!(lines[1].note, "−0.05 transform noise");
        assert!(lines[3].note.contains("half-life: 30d"));
    }

    #[test]
    fn test_chain_unknown_source() {
        let s = stamped(SourceClass::Unknown, None, &[("step", 0.1)]);
        let r = assess(&s, 0.50, t0()).unwrap();
        let lines = chain_lines(&s, &r);
        assert!(lines.iter().all(|l| l.fid.is_none()));
        let text = report(&s, &r);
        assert!(text.contains("TOTAL:"));
        assert!(text.contains("?  UNKNOWN"));
    }

    #[test]
    fn test_report_format() {
        let s = stamped(SourceClass::Instrument, Some("30d"), &[("normalize.py", 0.05)]);
        let r = assess(&s, 0.50, t0()).unwrap();
        let text = report(&s, &r);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].starts_with("sensor-12"));
        assert!(lines[0].ends_with("→ fid: 0.97 (source: instrument)"));
        assert!(lines[3].starts_with("TOTAL:"));
        assert!(lines[3].contains("0.92  SIGNAL (floor: 0.50)"));
        // TOTAL value lines up under the "fid:" values (char columns; → is multibyte)
        let col = |line: &str, pat: &str| line[..line.find(pat).unwrap()].chars().count();
        assert_eq!(col(lines[3], "0.92"), col(lines[0], "0.97"));
    }

    #[test]
    fn test_report_integrity_mismatch() {
        let mut s = stamped(SourceClass::Instrument, None, &[]);
        s.content = json!({"v": 2});
        let r = assess(&s, 0.50, t0()).unwrap();
        let text = report(&s, &r);
        assert!(text.contains("UNKNOWN"));
        assert!(text.contains("integrity mismatch"));
    }
}
