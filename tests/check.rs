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
    // fid may exit on bad arguments before reading stdin; its exit code is the test
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{}", e);
    }
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

// spec: cli, check
#[test]
fn check_fresh_artifact_is_signal() {
    let path = temp_file("fresh", &stamped(&["--source", "s", "--source-class", "instrument", "--half-life", "30d"]));
    let out = check(&path, "0.70");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stderr(&out).starts_with("SIGNAL"));
    assert!(out.stdout.is_empty());
}

// spec: integrity, stamping
#[test]
fn check_stamped_floats_keep_integrity() {
    // Without correctly rounded float parsing, reading back 1.2345678901234567e+20
    // gives a different float, and an unchanged artifact fails its integrity check.
    let out = fid(
        &["stamp", "--source", "s", "--source-class", "instrument"],
        r#"{"v": 123456789012345680000.0, "w": [1e-7, 0.00001, 2.5e300]}"#,
    );
    assert!(out.status.success());
    let path = temp_file("floats", &String::from_utf8(out.stdout).unwrap());
    let out = check(&path, "0.50");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

// spec: cli, check
#[test]
fn check_below_floor_is_noise() {
    let path = temp_file("noise", &stamped(&["--source", "s", "--source-class", "llm_ungrounded"]));
    let out = check(&path, "0.70");
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).starts_with("NOISE"));
}

// spec: check
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

// spec: cli, check, source-classes
#[test]
fn check_unknown_source_is_unknown() {
    let path = temp_file("unknown", &stamped(&["--source", "mystery"]));
    let out = check(&path, "0.10");
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).starts_with("UNKNOWN"));
}

// spec: cli, check, integrity
#[test]
fn check_tampered_content_is_unknown() {
    let mut v: Value = serde_json::from_str(&stamped(&["--source", "s", "--source-class", "database"])).unwrap();
    v["content"]["reading"] = Value::from(43);
    let path = temp_file("tampered", &v.to_string());
    let out = check(&path, "0.10");
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("integrity mismatch"));
}

// spec: cli
#[test]
fn unknown_or_unimplemented_commands_are_errors() {
    // never exit 2: a pipeline must not read a broken call as UNKNOWN
    for args in [&["measure", "x.json"][..], &["scan", "."], &["filter"], &["watch", "."], &["record.json"], &[]] {
        let out = fid(args, "");
        assert_eq!(out.status.code(), Some(3), "fid {:?}", args);
        assert!(out.stdout.is_empty(), "fid {:?}", args);
    }
    let help = fid(&["--help"], "");
    assert_eq!(help.status.code(), Some(0));
    let text = String::from_utf8_lossy(&help.stdout);
    for gone in ["measure", "scan", "filter", "watch"] {
        assert!(!text.contains(&format!("  {} ", gone)), "help still lists {}: {}", gone, text);
    }
}

// spec: cli
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

// spec: cli
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

// spec: cli
#[test]
fn chain_walks_pipeline() {
    let first = stamped(&["--source", "sensor-12", "--source-class", "instrument", "--half-life", "30d"]);
    let second = fid(&["stamp", "--source", "normalize.py", "--noise", "0.05"], &first);
    let out = fid(&["chain", "-", "--floor", "0.5"], &String::from_utf8(second.stdout).unwrap());
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4);
    assert!(lines[0].starts_with("sensor-12"));
    assert!(lines[1].contains("normalize.py") && lines[1].contains("−0.05 transform noise"));
    assert!(lines[2].starts_with("temporal ("));
    assert!(lines[3].contains("0.92  SIGNAL (floor: 0.50)"));
}

#[test]
fn chain_errors_exit_3() {
    assert_eq!(fid(&["chain", "/nonexistent/a.json"], "").status.code(), Some(3));
    assert_eq!(fid(&["chain", "-"], "not json").status.code(), Some(3));
}

// spec: cli, expiry
#[test]
fn ttl_prints_expiry_and_exit_code() {
    // database 0.90, 1d half-life: falls below 0.45 one day after measurement
    let artifact = stamped(&["--source", "s", "--source-class", "database", "--half-life", "1d"]);
    let v: Value = serde_json::from_str(&artifact).unwrap();
    let measured = chrono::DateTime::parse_from_rfc3339(v["fid"]["measured_at"].as_str().unwrap()).unwrap();
    let out = fid(&["ttl", "-", "--floor", "0.45"], &artifact);
    assert_eq!(out.status.code(), Some(0));
    let printed = String::from_utf8(out.stdout).unwrap();
    let at = chrono::DateTime::parse_from_rfc3339(printed.trim()).unwrap();
    assert!(((at - measured).num_seconds() - 86_400).abs() <= 1, "{}", printed);
}

// spec: cli, expiry
#[test]
fn ttl_never_and_unknown() {
    let never = stamped(&["--source", "s", "--source-class", "instrument", "--half-life", "1h", "--baseline", "0.8"]);
    let out = fid(&["ttl", "-", "--floor", "0.7"], &never);
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), "never");
    assert_eq!(out.status.code(), Some(0));

    let unknown = stamped(&["--source", "mystery"]);
    let out = fid(&["ttl", "-", "--floor", "0.7"], &unknown);
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), "unknown");
    assert_eq!(out.status.code(), Some(2));
}

// spec: cli, recheck, staleness
#[test]
fn schedule_prints_recheck_time_and_exit_code() {
    let artifact = stamped(&["--source", "s", "--source-class", "instrument", "--source-fidelity", "1", "--half-life", "1d", "--baseline", "0.1"]);
    let v: Value = serde_json::from_str(&artifact).unwrap();
    let measured = chrono::DateTime::parse_from_rfc3339(v["fid"]["measured_at"].as_str().unwrap()).unwrap();
    let at = |args: &[&str]| {
        let mut full = vec!["schedule", "-"];
        full.extend_from_slice(args);
        let out = fid(&full, &artifact);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let printed = String::from_utf8(out.stdout).unwrap();
        if printed.trim() == "never" {
            return f64::INFINITY;
        }
        (chrono::DateTime::parse_from_rfc3339(printed.trim()).unwrap() - measured).num_milliseconds() as f64 / 1000.0
    };
    let cheap = at(&["--price", "0.0001"]);
    let dear = at(&["--price", "0.01"]);
    assert!(cheap > 0.0 && cheap < dear && dear < f64::INFINITY, "{} {}", cheap, dear);
    assert_eq!(at(&["--price", "1000"]), f64::INFINITY);
    // the floor guard caps the interval at the TTL: 1d × log2(0.9 / 0.8) for floor 0.9
    let guarded = at(&["--price", "0.01", "--floor", "0.9"]);
    assert!((guarded - 86_400.0 * (0.9_f64 / 0.8).log2()).abs() < 1.0, "{}", guarded);
    // staleness target
    let s = at(&["--staleness", "0.05"]);
    assert!(s > 0.0 && s < f64::INFINITY);
}

#[test]
fn schedule_errors_exit_3() {
    let artifact = stamped(&["--source", "s", "--source-class", "instrument", "--half-life", "1d"]);
    for args in [vec!["schedule", "-"], vec!["schedule", "-", "--price", "0"], vec!["schedule", "-", "--price", "1", "--staleness", "0.1"],
                 vec!["schedule", "-", "--price", "0.001", "--floor", "2"], vec!["schedule", "-", "--staleness", "0.01"]] {
        let out = fid(&args, &artifact);
        assert_eq!(out.status.code(), Some(3), "{:?}: {}", args, stderr(&out));
    }
    let unknown = stamped(&["--source", "mystery", "--half-life", "1d"]);
    let out = fid(&["schedule", "-", "--price", "0.001"], &unknown);
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), "unknown");
    assert_eq!(out.status.code(), Some(2));
}
