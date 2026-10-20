//! fid against conformance/vectors.json — expected values computed from the
//! written spec by an independent implementation (conformance/generate.py).

use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde_json::Value;

use fid::calibrate;
use fid::check::assess;
use fid::compute::{
    apply_decay, parse_duration, parse_half_life, temporal_factor, time_to_floor, time_to_floor_echo, total_fidelity,
    transform_factor, Curve, Echo, Fast, Recheck, TimeToFloor,
};
use fid::envelope::{integrity_hash, Expiry, FidStatus, SourceClass, Stamped};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn vectors() -> Value {
    let text = std::fs::read_to_string(root().join("conformance/vectors.json")).expect("conformance/vectors.json");
    serde_json::from_str(&text).unwrap()
}

fn cases<'a>(v: &'a Value, section: &str) -> &'a Vec<Value> {
    let list = v[section].as_array().unwrap_or_else(|| panic!("no section {}", section));
    assert!(!list.is_empty(), "empty section {}", section);
    list
}

fn close(got: f64, want: f64, tol: f64, what: &str) {
    assert!((got - want).abs() <= tol, "{}: got {}, want {}", what, got, want);
}

fn time(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

#[test]
fn durations() {
    let v = vectors();
    for c in cases(&v, "parse_duration") {
        let input = c["input"].as_str().unwrap();
        match c.get("error") {
            Some(_) => assert!(parse_duration(input).is_err(), "'{}' should be an error", input),
            None => close(parse_duration(input).unwrap(), c["seconds"].as_f64().unwrap(), 1e-6, input),
        }
    }
    for c in cases(&v, "parse_half_life") {
        let input = c["input"].as_str().unwrap();
        match c.get("error") {
            Some(_) => assert!(parse_half_life(input).is_err(), "'{}' should be an error", input),
            None => match (parse_half_life(input).unwrap(), c["seconds"].as_f64()) {
                (Some(got), Some(want)) => close(got, want, 1e-6, input),
                (None, None) => {}
                (got, want) => panic!("{}: got {:?}, want {:?}", input, got, want),
            },
        }
    }
}

#[test]
fn source_classes() {
    let v = vectors();
    for c in cases(&v, "source_fidelity") {
        let name = c["class"].as_str().unwrap();
        match c.get("error") {
            Some(_) => assert!(SourceClass::from_str(name).is_err(), "'{}' should be an error", name),
            None => assert_eq!(SourceClass::from_str(name).unwrap().default_fidelity(), c["fidelity"].as_f64(), "{}", name),
        }
    }
}

#[test]
fn fidelity() {
    let v = vectors();
    let tol = v["tolerance"].as_f64().unwrap();
    for c in cases(&v, "fidelity") {
        let noise: Vec<f64> = c["noise"].as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect();
        let got = total_fidelity(
            c["source"].as_f64(),
            c["age_seconds"].as_f64().unwrap(),
            c["half_life_seconds"].as_f64(),
            &noise,
            c["baseline"].as_f64().unwrap(),
        )
        .unwrap();
        close(got, c["fidelity"].as_f64().unwrap(), tol, c["name"].as_str().unwrap());
    }
}

#[test]
fn ttl() {
    let v = vectors();
    for c in cases(&v, "time_to_floor") {
        let name = c["name"].as_str().unwrap();
        let got = time_to_floor(
            c["fidelity"].as_f64().unwrap(),
            c["half_life_seconds"].as_f64(),
            c["baseline"].as_f64().unwrap(),
            c["floor"].as_f64().unwrap(),
        );
        match (got, c["seconds"].as_f64()) {
            (TimeToFloor::After(s), Some(want)) => close(s, want, 1e-6, name),
            (TimeToFloor::Never, None) => {}
            (got, want) => panic!("{}: got {:?}, want {:?}", name, got, want),
        }
    }
}

fn echo(c: &Value) -> fid::compute::EchoParams {
    serde_json::from_value::<Echo>(c["echo"].clone()).unwrap().params().unwrap()
}

#[test]
fn echoes() {
    let v = vectors();
    let tol = v["tolerance"].as_f64().unwrap();
    for c in cases(&v, "echo") {
        let noise: Vec<f64> = c["noise"].as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect();
        let temporal = temporal_factor(c["age_seconds"].as_f64().unwrap(), c["half_life_seconds"].as_f64(), Some(&echo(c)));
        let got = apply_decay(c["source"].as_f64().unwrap() * transform_factor(&noise), temporal, c["baseline"].as_f64().unwrap());
        close(got, c["fidelity"].as_f64().unwrap(), tol, c["name"].as_str().unwrap());
    }
    for c in cases(&v, "echo_time_to_floor") {
        let name = c["name"].as_str().unwrap();
        let got = time_to_floor_echo(
            c["fidelity"].as_f64().unwrap(),
            c["half_life_seconds"].as_f64().unwrap(),
            c["baseline"].as_f64().unwrap(),
            c["floor"].as_f64().unwrap(),
            &echo(c),
        );
        match (got, c["seconds"].as_f64()) {
            (TimeToFloor::After(s), Some(want)) => close(s, want, 1e-6, name),
            (TimeToFloor::Never, None) => {}
            (got, want) => panic!("{}: got {:?}, want {:?}", name, got, want),
        }
    }
    for c in cases(&v, "echo_errors") {
        let e: Echo = serde_json::from_value(c["echo"].clone()).unwrap();
        assert!(e.params().is_err(), "{}", c["error"]);
    }
}

/// The case's curve: half-life, fast share and echo (each may be null).
fn case_curve(c: &Value) -> Curve {
    let fast: Option<Fast> = serde_json::from_value(c["fast"].clone()).unwrap();
    let echo: Option<Echo> = serde_json::from_value(c["echo"].clone()).unwrap();
    let h = c["half_life_seconds"].as_f64().map(|h| format!("{}s", h));
    fid::compute::curve(h.as_deref(), fast.as_ref(), echo.as_ref(), c["shape"].as_f64()).unwrap()
}

#[test]
fn fast_shares() {
    let v = vectors();
    let tol = v["tolerance"].as_f64().unwrap();
    for c in cases(&v, "fast") {
        let got = apply_decay(
            c["fidelity_at_measurement"].as_f64().unwrap(),
            case_curve(c).temporal(c["age_seconds"].as_f64().unwrap()),
            c["baseline"].as_f64().unwrap(),
        );
        close(got, c["fidelity"].as_f64().unwrap(), tol, c["name"].as_str().unwrap());
    }
    for c in cases(&v, "fast_time_to_floor") {
        let name = c["name"].as_str().unwrap();
        let got = case_curve(c).time_to_floor(
            c["fidelity"].as_f64().unwrap(),
            c["baseline"].as_f64().unwrap(),
            c["floor"].as_f64().unwrap(),
        );
        match (got, c["seconds"].as_f64()) {
            (TimeToFloor::After(s), Some(want)) => close(s, want, 1e-6, name),
            (TimeToFloor::Never, None) => {}
            (got, want) => panic!("{}: got {:?}, want {:?}", name, got, want),
        }
    }
    for c in cases(&v, "shape_errors") {
        let h = c["half_life_seconds"].as_f64().map(|h| format!("{}s", h));
        assert!(fid::compute::curve(h.as_deref(), None, None, c["shape"].as_f64()).is_err(), "{}", c["error"]);
    }
    for c in cases(&v, "fast_errors") {
        let fast: Fast = serde_json::from_value(c["fast"].clone()).unwrap();
        let h = c["half_life_seconds"].as_f64().map(|h| format!("{}s", h));
        assert!(fid::compute::curve(h.as_deref(), Some(&fast), None, None).is_err(), "{}", c["error"]);
    }
}

#[test]
fn schedules() {
    let v = vectors();
    for c in cases(&v, "schedule") {
        let name = c["name"].as_str().unwrap();
        let curve = case_curve(c);
        let (f, b, price) = (c["fidelity"].as_f64().unwrap(), c["baseline"].as_f64().unwrap(), c["price"].as_f64().unwrap());
        let cost = |t: f64| curve.mean_staleness(f, b, t) + price * 86_400.0 / t;
        match (fid::compute::best_interval(&curve, f, b, price).unwrap(), c["seconds"].as_f64()) {
            (Recheck::Every(t), Some(want)) => {
                // The same staleness at the spec's interval, and fid's interval at least as cheap
                close(cost(want), c["cost"].as_f64().unwrap(), 1e-7, name);
                assert!(cost(t) <= c["cost"].as_f64().unwrap() + 1e-7, "{}: {} at {} vs {} at {}", name, cost(t), t, c["cost"], want);
            }
            (Recheck::Never, None) => {}
            (got, want) => panic!("{}: got {:?}, want {:?}", name, got, want),
        }
    }
    for c in cases(&v, "staleness_target") {
        let name = c["name"].as_str().unwrap();
        let curve = case_curve(c);
        let got = fid::compute::interval_for_staleness(&curve, c["fidelity"].as_f64().unwrap(), c["baseline"].as_f64().unwrap(), c["target"].as_f64().unwrap()).unwrap();
        match (got, c["seconds"].as_f64()) {
            (Recheck::Every(t), Some(want)) => assert!((t / want - 1.0).abs() < 1e-6, "{}: {} vs {}", name, t, want),
            (Recheck::Never, None) => {}
            (got, want) => panic!("{}: got {:?}, want {:?}", name, got, want),
        }
    }
}

#[test]
fn integrity() {
    let v = vectors();
    for c in cases(&v, "integrity") {
        assert_eq!(serde_json::to_string(&c["content"]).unwrap(), c["canonical"].as_str().unwrap());
        assert_eq!(integrity_hash(&c["content"]), c["hash"].as_str().unwrap(), "{}", c["content"]);
    }
}

#[test]
fn envelopes() {
    let v = vectors();
    let tol = v["tolerance"].as_f64().unwrap();
    for c in cases(&v, "envelope") {
        let name = c["name"].as_str().unwrap();
        let stamped: Stamped = serde_json::from_value(c["stamped"].clone()).unwrap_or_else(|e| panic!("{}: {}", name, e));
        let floor = c["floor"].as_f64().unwrap();
        let r = assess(&stamped, floor, time(c["now"].as_str().unwrap())).unwrap();

        let status = match r.assessment.status {
            FidStatus::Signal => "SIGNAL",
            FidStatus::Noise => "NOISE",
            FidStatus::Unknown => "UNKNOWN",
        };
        assert_eq!(status, c["status"].as_str().unwrap(), "{}: status", name);
        assert_eq!(r.integrity_ok, c["integrity_ok"].as_bool().unwrap(), "{}: integrity", name);
        match (r.assessment.fidelity, c["fidelity"].as_f64()) {
            (Some(got), Some(want)) => close(got, want, tol, name),
            (None, None) => {}
            (got, want) => panic!("{}: fidelity {:?}, want {:?}", name, got, want),
        }

        let expires = match stamped.fid.expires_at(floor).unwrap() {
            None => "unknown".to_string(),
            Some(Expiry::Never) => "never".to_string(),
            Some(Expiry::At(t)) => t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        };
        assert_eq!(expires, c["expires_at"].as_str().unwrap(), "{}: expires_at", name);
    }
}

#[test]
fn ledger_csv() {
    let v = vectors();
    for c in cases(&v, "ledger_csv") {
        let text = c["csv"].as_str().unwrap();
        let got = calibrate::read_csv(text.as_bytes());
        if let Some(reason) = c.get("error") {
            assert!(got.is_err(), "{} should be an error: {:?}", reason, got);
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{:?}: {}", text, e));
        let want = c["observations"].as_array().unwrap();
        assert_eq!(got.len(), want.len(), "{:?}", text);
        for (g, w) in got.iter().zip(want) {
            assert_eq!(g.entity, w["entity"].as_str().unwrap());
            close(g.age, w["age_seconds"].as_f64().unwrap(), 1e-6, &g.entity);
            assert_eq!(g.correct, w["correct"].as_bool().unwrap(), "{}", g.entity);
            assert_eq!(g.state.as_deref(), w["state"].as_str(), "{}", g.entity);
        }
    }
}

#[test]
fn ledger_example_recovers_known_decay() {
    let v = vectors();
    let ex = &v["ledger_example"];
    let file = std::fs::File::open(root().join(ex["file"].as_str().unwrap())).unwrap();
    let fits = calibrate::calibrate(&calibrate::read_csv(file).unwrap()).unwrap();
    assert_eq!(fits.len(), 1);
    let m = fits[0].model;

    let truth_h = ex["truth"]["half_life_days"].as_f64().unwrap() * 86400.0;
    let truth_b = ex["truth"]["baseline"].as_f64().unwrap();
    assert!(fits[0].uses_baseline, "the baseline should beat decay-to-zero");
    assert!(
        (m.half_life / truth_h - 1.0).abs() <= ex["half_life_within"].as_f64().unwrap(),
        "half-life {}d, truth {}d",
        m.half_life / 86400.0,
        truth_h / 86400.0
    );
    assert!(
        (m.baseline - truth_b).abs() <= ex["baseline_within"].as_f64().unwrap(),
        "baseline {}, truth {}",
        m.baseline,
        truth_b
    );
}
