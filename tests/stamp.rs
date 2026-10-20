//! End-to-end tests for `fid stamp`, run against the built binary.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn fid(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fid"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run fid");
    // fid may exit on bad arguments before reading stdin; its exit code is the test
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{}", e);
    }
    child.wait_with_output().unwrap()
}

fn stdout_json(out: &Output) -> Value {
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).expect("stdout is not JSON")
}

#[test]
fn stamp_root_wraps_content() {
    let out = fid(
        &["stamp", "--source", "sensor-12", "--source-class", "instrument", "--half-life", "30d"],
        r#"{"temp": 21.5}"#,
    );
    let v = stdout_json(&out);
    assert_eq!(v["content"]["temp"], 21.5);
    assert_eq!(v["fid"]["source_id"], "sensor-12");
    assert_eq!(v["fid"]["source_class"], "instrument");
    assert_eq!(v["fid"]["half_life"], "30d");
    assert_eq!(v["fid"]["fid_at_stamp"], 0.97);
    assert!(v["fid"]["integrity"].as_str().unwrap().starts_with("sha256:"));
}

#[test]
fn stamp_pipeline_composes() {
    let first = fid(&["stamp", "--source", "db", "--source-class", "database"], "{}");
    let first = String::from_utf8(first.stdout).unwrap();
    let second = fid(&["stamp", "--source", "normalize.py", "--noise", "0.05"], &first);
    let second = String::from_utf8(second.stdout).unwrap();
    let third = fid(&["stamp", "--source", "aggregate.py", "--noise", "0.08"], &second);

    let v = stdout_json(&third);
    let transforms = v["fid"]["transforms"].as_array().unwrap();
    assert_eq!(transforms.len(), 2);
    assert_eq!(transforms[0]["step"], "normalize.py");
    assert_eq!(transforms[1]["step"], "aggregate.py");
    assert_eq!(v["fid"]["source_id"], "db");
    let f = v["fid"]["fid_at_stamp"].as_f64().unwrap();
    assert!((f - 0.90 * 0.95 * 0.92).abs() < 1e-9);
}

#[test]
fn stamp_derived_from_file() {
    let upstream = fid(&["stamp", "--source", "db", "--source-class", "database"], "{}");
    let dir = std::env::temp_dir().join(format!("fid-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("upstream.json");
    std::fs::write(&path, &upstream.stdout).unwrap();
    let path = path.to_str().unwrap();

    let out = fid(&["stamp", "--source", "report.py", "--noise", "0.1", "--derived-from", path], "plain text");
    let v = stdout_json(&out);
    assert_eq!(v["fid"]["source_id"], "db");
    assert_eq!(v["fid"]["derived_from"], path);
    assert_eq!(v["content"], "plain text");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn stamp_unknown_source_is_indeterminate() {
    let v = stdout_json(&fid(&["stamp", "--source", "mystery"], "{}"));
    assert_eq!(v["fid"]["source_class"], "unknown");
    assert!(v["fid"]["fid_at_stamp"].is_null());
}

#[test]
fn stamp_rejects_bad_input() {
    let cases: &[&[&str]] = &[
        &["stamp", "--source", "x", "--half-life", "7days"],
        &["stamp", "--source", "x", "--source-class", "human"],
        &["stamp", "--source", "x", "--noise", "1.5"],
        &["stamp", "--source", "x", "--derived-from", "/nonexistent/upstream.json"],
    ];
    for args in cases {
        let out = fid(args, "{}");
        assert_eq!(out.status.code(), Some(3), "args: {:?}", args);
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn stamp_rejects_malformed_envelope() {
    let out = fid(&["stamp", "--source", "x"], r#"{"fid": {"broken": true}, "content": 1}"#);
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a valid fid-stamped artifact"));
}

fn example(name: &str) -> String {
    format!("{}/examples/{}", env!("CARGO_MANIFEST_DIR"), name)
}

#[test]
fn stamp_looks_up_state_half_life() {
    let registry = example("half-lives.yaml");
    let out = fid(
        &["stamp", "--source", "ais", "--source-class", "instrument",
          "--half-lives", &registry, "--type", "vessel_position", "--state", "underway"],
        "{}",
    );
    let v = stdout_json(&out);
    assert_eq!(v["fid"]["half_life"], "1.9m");
    assert_eq!(v["fid"]["baseline"], 0.02);
    assert_eq!(v["fid"]["info_type"], "vessel_position");
    assert_eq!(v["fid"]["state"], "underway");
}

#[test]
fn stamp_flags_override_registry() {
    let registry = example("half-lives.yaml");
    let out = fid(
        &["stamp", "--source", "m", "--half-lives", &registry, "--type", "airport_flight_category",
          "--half-life", "1h", "--baseline", "0.5"],
        "{}",
    );
    let v = stdout_json(&out);
    assert_eq!(v["fid"]["half_life"], "1h");
    assert_eq!(v["fid"]["baseline"], 0.5);
}

#[test]
fn stamp_looks_up_conditional_noise() {
    let model = example("noise-model.yaml");
    let first = fid(&["stamp", "--source", "schedule", "--source-class", "database"], "{}");
    let out = fid(
        &["stamp", "--source", "leg-3", "--noise-model", &model, "--transform", "flight_leg",
          "--condition", "previous_leg_late"],
        &String::from_utf8(first.stdout).unwrap(),
    );
    let v = stdout_json(&out);
    assert_eq!(v["fid"]["transforms"][0]["noise"], 0.55);
    assert_eq!(v["fid"]["transforms"][0]["condition"], "previous_leg_late");
}

#[test]
fn stamp_registry_errors_exit_3() {
    let registry = example("half-lives.yaml");
    let model = example("noise-model.yaml");
    let cases: &[&[&str]] = &[
        // stateful type without a state
        &["stamp", "--source", "x", "--half-lives", &registry, "--type", "vessel_position"],
        // unknown state
        &["stamp", "--source", "x", "--half-lives", &registry, "--type", "vessel_position", "--state", "sinking"],
        // unknown type
        &["stamp", "--source", "x", "--half-lives", &registry, "--type", "nope"],
        // registry without --type
        &["stamp", "--source", "x", "--half-lives", &registry],
        // unknown condition
        &["stamp", "--source", "x", "--noise-model", &model, "--transform", "flight_leg", "--condition", "x"],
        // bad baseline
        &["stamp", "--source", "x", "--baseline", "1.5"],
        // missing registry file
        &["stamp", "--source", "x", "--half-lives", "/nonexistent.yaml", "--type", "t"],
    ];
    for args in cases {
        let out = fid(args, "{}");
        assert_eq!(out.status.code(), Some(3), "args: {:?}", args);
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn stamp_carries_a_fast_share_from_the_registry() {
    let dir = std::env::temp_dir().join(format!("fid-stamp-fast-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let registry = dir.join("half-lives.yaml");
    std::fs::write(&registry, "pkg:\n  half_life: 80d\n  baseline: 0.1\n  fast: { share: 0.3, half_life: 3d }\n").unwrap();
    let out = fid(
        &["stamp", "--source", "pypi", "--source-class", "instrument", "--half-lives", registry.to_str().unwrap(), "--type", "pkg"],
        "{}",
    );
    let v = stdout_json(&out);
    assert_eq!(v["fid"]["fast"]["share"], 0.3);
    assert_eq!(v["fid"]["fast"]["half_life"], "3d");
    assert_eq!(v["fid"]["fid_version"], "0.4");
}
