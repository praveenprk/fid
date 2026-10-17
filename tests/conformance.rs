//! fid against conformance/vectors.json — expected values computed from the
//! written spec by an independent implementation (conformance/generate.py).

use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde_json::Value;

use fid::calibrate;
use fid::check::assess;
use fid::compute::{parse_duration, parse_half_life, time_to_floor, total_fidelity, TimeToFloor};
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
