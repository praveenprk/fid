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

#[test]
fn calibrate_period_writes_an_echo() {
    // A daily fact: right when fresh, back at every multiple of 24h, wrong in between
    let mut csv = String::from("entity,age,correct\n");
    for lag in 0..=168 {
        let returns = lag % 24 == 0 || lag % 24 == 1 || lag % 24 == 23;
        let p = if lag < 3 { 0.95 } else if returns { 0.6 } else { 0.15 };
        for i in 0..40 {
            csv.push_str(&format!("e{}-{},{}h,{}\n", lag, i, lag, (i as f64) < p * 40.0));
        }
    }
    let out = fid(&["calibrate", "-", "--type", "load", "--period", "24h", "--max-spread", "inf"], &csv);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let yaml = String::from_utf8(out.stdout).unwrap();
    assert!(yaml.contains("echo: { period: 1d"), "{}", yaml);
    assert!(String::from_utf8_lossy(&out.stderr).contains("→ echo kept"));
    let spec = fid::registry::HalfLives::from_yaml(&yaml).unwrap().lookup("load", None).unwrap();
    assert!(spec.echo.unwrap().strength > 0.2);
}

#[test]
fn calibrate_finds_two_clocks_and_stamp_uses_them() {
    // 30% of facts go stale with a 2-day half-life, the rest with 60 days; exact counts per lag
    let mut csv = String::from("entity,age,correct\n");
    for day in [0.0_f64, 1.0, 3.0, 7.0, 14.0, 30.0, 61.0, 91.0, 182.0, 274.0, 365.0] {
        let p = 0.07 + 0.93 * (0.7 * 0.5_f64.powf(day / 60.0) + 0.3 * 0.5_f64.powf(day / 2.0));
        let n = 1000;
        let k = (p * n as f64).round() as usize;
        for i in 0..n {
            csv.push_str(&format!("pkg-{}-{},{}d,{}\n", day, i, day, i < k));
        }
    }
    let out = fid(&["calibrate", "-", "--type", "pkg"], &csv);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let yaml = String::from_utf8(out.stdout).unwrap();
    assert!(yaml.contains("fast: { share: "), "{}", yaml);
    assert!(String::from_utf8_lossy(&out.stderr).contains("fast share kept"));

    let dir = std::env::temp_dir().join(format!("fid-calibrate-fast-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let registry = dir.join("half-lives.yaml");
    std::fs::write(&registry, &yaml).unwrap();
    let stamped = fid(&["stamp", "--source", "pypi", "--source-class", "instrument", "--half-lives", registry.to_str().unwrap(), "--type", "pkg"], "{}");
    assert!(stamped.status.success(), "{}", String::from_utf8_lossy(&stamped.stderr));
    let v: serde_json::Value = serde_json::from_slice(&stamped.stdout).unwrap();
    let share = v["fid"]["fast"]["share"].as_f64().unwrap();
    assert!((share - 0.3).abs() < 0.05, "{}", share);
    // fresh, the fact is near certain: one clock would have said 0.94 or so
    assert!(v["fid"]["fid_at_stamp"].as_f64().unwrap() > 0.97, "{}", v);
    let schedule = fid(&["schedule", "-", "--price", "0.001"], &String::from_utf8(stamped.stdout).unwrap());
    assert_eq!(schedule.status.code(), Some(0), "{}", String::from_utf8_lossy(&schedule.stderr));
}

#[test]
fn calibrate_writes_entity_baselines() {
    let dir = std::env::temp_dir().join(format!("fid-calibrate-entities-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out_path = dir.join("baselines.csv");
    // 30 airports: stale facts right 80% of the time at the dry ones, 20% at the wet ones
    let mut csv = String::from("entity,age,correct\n");
    for a in 0..30 {
        let (site, b) = if a % 2 == 0 { (format!("dry{}", a), 0.8_f64) } else { (format!("wet{}", a), 0.2) };
        for day in 0..20 {
            let p = b + (1.0 - b) * 0.5_f64.powf(day as f64);
            for i in 0..50 {
                csv.push_str(&format!("{},{}d,{}\n", site, day, (i as f64) < p * 50.0));
            }
        }
    }
    let out = fid(&["calibrate", "-", "--type", "flight_category", "--entity-baselines", out_path.to_str().unwrap()], &csv);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("spread between entities"));
    let text = std::fs::read_to_string(&out_path).unwrap();
    let get = |site: &str| -> f64 {
        text.lines().find(|l| l.starts_with(&format!("{},", site))).unwrap().split(',').nth(2).unwrap().parse().unwrap()
    };
    assert!(get("dry0") > 0.7 && get("wet1") < 0.3, "{}", text);
}
