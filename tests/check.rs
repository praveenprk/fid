//! End-to-end tests for `fid check`, run against the built binary.

use std::io::Write;
use std::path::PathBuf;
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
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn stamped(args: &[&str]) -> String {
    let mut full = vec!["stamp"];
    full.extend_from_slice(args);
    let out = fid(&full, r#"{"reading": 42}"#);
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap()
}

/// Write `text` to a fresh temp file, unique per test name.
fn temp_file(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fid-check-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("artifact.json");
    std::fs::write(&path, text).unwrap();
    path
}

fn check(path: &PathBuf, floor: &str) -> Output {
    fid(&["check", path.to_str().unwrap(), "--floor", floor], "")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

#[test]
fn check_fresh_artifact_is_signal() {
    let path = temp_file("fresh", &stamped(&["--source", "s", "--source-class", "instrument", "--half-life", "30d"]));
    let out = check(&path, "0.70");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stderr(&out).starts_with("SIGNAL"));
    assert!(out.stdout.is_empty());
}

#[test]
fn check_below_floor_is_noise() {
    let path = temp_file("noise", &stamped(&["--source", "s", "--source-class", "llm_ungrounded"]));
    let out = check(&path, "0.70");
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).starts_with("NOISE"));
}

#[test]
fn check_stale_artifact_decays_to_noise() {
    // Measured 60 days ago with a 30-day half-life: 0.97 × 0.25 ≈ 0.24
    let mut v: Value = serde_json::from_str(&stamped(&[
        "--source", "s", "--source-class", "instrument", "--half-life", "30d",
    ]))
    .unwrap();
    let old = chrono::Utc::now() - chrono::Duration::days(60);
    v["fid"]["measured_at"] = Value::String(old.to_rfc3339());
    let path = temp_file("stale", &v.to_string());

    let out = check(&path, "0.50");
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("age 60d"));
    assert!(check(&path, "0.20").status.success());
}

#[test]
fn check_unknown_source_is_unknown() {
    let path = temp_file("unknown", &stamped(&["--source", "mystery"]));
    let out = check(&path, "0.10");
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).starts_with("UNKNOWN"));
}

#[test]
fn check_tampered_content_is_unknown() {
    let mut v: Value = serde_json::from_str(&stamped(&["--source", "s", "--source-class", "database"])).unwrap();
    v["content"]["reading"] = Value::from(43);
    let path = temp_file("tampered", &v.to_string());
    let out = check(&path, "0.10");
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("integrity mismatch"));
}

#[test]
fn check_reads_stdin() {
    let artifact = stamped(&["--source", "s", "--source-class", "database"]);
    let out = fid(&["check", "-", "--floor", "0.5"], &artifact);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn check_errors_exit_3() {
    let missing = PathBuf::from("/nonexistent/artifact.json");
    assert_eq!(check(&missing, "0.5").status.code(), Some(3));

    let unstamped = temp_file("unstamped", r#"{"reading": 42}"#);
    assert_eq!(check(&unstamped, "0.5").status.code(), Some(3));

    let good = temp_file("badfloor", &stamped(&["--source", "s", "--source-class", "database"]));
    assert_eq!(check(&good, "1.5").status.code(), Some(3));

    // clap usage errors also exit 3, never the epistemic 2
    assert_eq!(fid(&["check"], "").status.code(), Some(3));
}

#[test]
fn check_in_shell_pipeline() {
    let path = temp_file("shell", &stamped(&["--source", "s", "--source-class", "instrument"]));
    let bin = env!("CARGO_BIN_EXE_fid");
    let script = format!(
        "'{bin}' check '{p}' --floor 0.70 && echo proceed || echo escalate",
        bin = bin,
        p = path.display()
    );
    let out = Command::new("sh").arg("-c").arg(&script).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "proceed");
}
