//! End-to-end tests for `fid calibrate`, run against the built binary.

use std::io::Write;
use std::process::{Command, Output, Stdio};

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

/// A ledger where half of facts are wrong after 10 days, with timestamps.
fn ledger() -> String {
    let mut csv = String::from("entity,observed_at,checked_at,correct\n");
    for (day, n_correct) in [(0, 100), (5, 71), (10, 50), (20, 25), (40, 6)] {
        for i in 0..100 {
            csv.push_str(&format!(
                "claim-{}-{},2026-01-01T00:00:00Z,2026-{:02}-{:02}T00:00:00Z,{}\n",
                day,
                i,
                1 + (day / 31),
                1 + (day % 31),
                i < n_correct
            ));
        }
    }
    csv
}

#[test]
fn calibrate_writes_registry_and_report() {
    let out = fid(&["calibrate", "-", "--type", "policy_claim"], &ledger());
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let yaml = String::from_utf8(out.stdout).unwrap();
    assert!(yaml.contains("policy_claim:\n  half_life: "), "{}", yaml);
    let report = String::from_utf8(out.stderr).unwrap();
    assert!(report.contains("policy_claim — 500 observations"), "{}", report);
    assert!(report.contains("held-out Brier"));
}

#[test]
fn calibrated_registry_feeds_stamp() {
    let out = fid(&["calibrate", "-", "--type", "policy_claim"], &ledger());
    let dir = std::env::temp_dir().join(format!("fid-calibrate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let registry = dir.join("half-lives.yaml");
    std::fs::write(&registry, &out.stdout).unwrap();

    let stamped = fid(
        &["stamp", "--source", "policy-db", "--source-class", "database",
          "--half-lives", registry.to_str().unwrap(), "--type", "policy_claim"],
        "{}",
    );
    assert!(stamped.status.success(), "{}", String::from_utf8_lossy(&stamped.stderr));
    let v: serde_json::Value = serde_json::from_slice(&stamped.stdout).unwrap();
    let half_life = v["fid"]["half_life"].as_str().unwrap();
    assert!(half_life.ends_with('d'), "{}", half_life);
    let days: f64 = half_life.trim_end_matches('d').parse().unwrap();
    assert!((days - 10.0).abs() < 1.0, "{}", half_life);
    // 100 checks at age 0, all correct: the measured source fidelity replaces database's 0.90
    assert_eq!(v["fid"]["source_fidelity"].as_f64(), Some(1.0));
    assert_eq!(v["fid"]["fid_at_stamp"].as_f64(), Some(1.0));

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn calibrate_refuses_an_uncertain_half_life() {
    // 60 re-checks, a few at each age: the half-life cannot be pinned down
    let mut csv = String::from("entity,age,correct\n");
    for (i, (day, ok)) in [(0, 5), (2, 4), (4, 4), (6, 3), (8, 2), (10, 2)].iter().enumerate() {
        for j in 0..10 {
            csv.push_str(&format!("c{}-{},{}d,{}\n", i, j, day, j < *ok * 2));
        }
    }
    let out = fid(&["calibrate", "-", "--type", "t"], &csv);
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("too uncertain"), "{}", err);
    let relaxed = fid(&["calibrate", "-", "--type", "t", "--max-spread", "inf"], &csv);
    assert_eq!(relaxed.status.code(), Some(0), "{}", String::from_utf8_lossy(&relaxed.stderr));
    assert!(String::from_utf8_lossy(&relaxed.stdout).contains("# 95% range"));
}

#[test]
fn calibrate_errors_exit_3() {
    let cases: &[(&[&str], &str)] = &[
        (&["calibrate", "-", "--type", "t"], "entity,correct\nx,true\n"),
        (&["calibrate", "-", "--type", "t"], "entity,age,correct\nx,1,true\ny,2,false\n"),
        (&["calibrate", "/nonexistent.csv", "--type", "t"], ""),
        (&["calibrate", "-"], "entity,age,correct\n"),
    ];
    for (args, stdin) in cases {
        let out = fid(args, stdin);
        assert_eq!(out.status.code(), Some(3), "args: {:?}", args);
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn drift_against_a_calibrated_registry() {
    let dir = std::env::temp_dir().join(format!("fid-drift-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let registry = dir.join("half-lives.yaml");
    std::fs::write(&registry, fid(&["calibrate", "-", "--type", "policy_claim"], &ledger()).stdout).unwrap();
    let reg = registry.to_str().unwrap();

    // The same checks again: current
    let same = fid(&["drift", "-", "--type", "policy_claim", "--half-lives", reg], &ledger());
    assert_eq!(same.status.code(), Some(0), "{}", String::from_utf8_lossy(&same.stderr));
    assert_eq!(String::from_utf8_lossy(&same.stdout).trim(), "current");

    // Facts now go stale twice as fast: drifted
    let mut faster = String::from("entity,age,correct\n");
    for (day, n_correct) in [(0, 100), (5, 50), (10, 25), (20, 6), (40, 0)] {
        for i in 0..100 {
            faster.push_str(&format!("new-{}-{},{}d,{}\n", day, i, day, i < n_correct));
        }
    }
    let drifted = fid(&["drift", "-", "--type", "policy_claim", "--half-lives", reg], &faster);
    assert_eq!(drifted.status.code(), Some(1), "{}", String::from_utf8_lossy(&drifted.stderr));
    assert_eq!(String::from_utf8_lossy(&drifted.stdout).trim(), "drifted");
    assert!(String::from_utf8_lossy(&drifted.stderr).contains("recalibrate"));

    let missing = fid(&["drift", "-", "--type", "other", "--half-lives", reg], &ledger());
    assert_eq!(missing.status.code(), Some(3));

    std::fs::remove_dir_all(&dir).unwrap();
}
