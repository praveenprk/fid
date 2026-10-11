use std::io::Read;

use chrono::Utc;
use clap::{Parser, Subcommand};
use fid::check::{assess, report};
use fid::envelope::{SourceClass, Stamped};
use fid::stamp::{stamp, StampOptions};

/// fid — information fidelity measurement for computing systems.
///
/// Measures the degree to which stored data still corresponds
/// to the reality it represents.
///
/// Exit codes:
///   0  SIGNAL   — fidelity above threshold
///   1  NOISE    — fidelity below floor
///   2  UNKNOWN  — source fidelity indeterminate
///   3  ERROR    — bad arguments, unreadable or malformed input
#[derive(Parser)]
#[command(name = "fid", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to a file to measure fidelity of (shorthand for `fid measure <path>`)
    path: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Measure the fidelity of a single data artifact
    Measure {
        /// Path to the file
        path: String,

        /// Noise floor threshold (overrides domain default)
        #[arg(long)]
        floor: Option<f64>,
    },

    /// Scan a directory and report aggregate fidelity
    Scan {
        /// Directory path
        path: String,

        /// Only show records below the noise floor
        #[arg(long)]
        below_floor: bool,
    },

    /// Check fidelity against a threshold (exit code is the answer)
    Check {
        /// Path to the fid-stamped file ("-" for stdin)
        path: String,

        /// Fidelity floor — exit 0 if above, exit 1 if below
        #[arg(long, default_value = "0.50")]
        floor: f64,
    },

    /// Stamp a fidelity envelope onto stdin content
    Stamp {
        /// Source identifier (who/what produced this data)
        #[arg(long)]
        source: String,

        /// Source class (root stamps only; defaults to unknown):
        /// instrument | human_observation | human_attestation | database |
        /// llm_grounded | llm_ungrounded | hearsay | unknown
        #[arg(long)]
        source_class: Option<String>,

        /// Transform noise (0.0 to 1.0)
        #[arg(long, default_value = "0.0")]
        noise: f64,

        /// Half-life duration (e.g. "7d", "6h", "30s")
        #[arg(long)]
        half_life: Option<String>,

        /// Derived-from: path to the upstream fid-stamped artifact
        #[arg(long)]
        derived_from: Option<String>,
    },

    /// Inspect the fidelity chain of a stamped artifact
    Chain {
        /// Path to the fid-stamped file ("-" for stdin)
        path: String,

        /// Fidelity floor shown in the TOTAL line
        #[arg(long, default_value = "0.50")]
        floor: f64,
    },

    /// Filter stdin (jsonl) to records above the noise floor
    Filter {
        /// Fidelity floor threshold
        #[arg(long, default_value = "0.50")]
        floor: f64,
    },

    /// Watch a directory and alert when fidelity drops
    Watch {
        /// Directory path
        path: String,

        /// Check interval (e.g. "1h", "30m")
        #[arg(long, default_value = "1h")]
        interval: String,

        /// Alert threshold
        #[arg(long, default_value = "0.50")]
        alert_below: f64,
    },
}

/// Exit code for operational errors, kept distinct from the epistemic
/// codes 0/1/2 so a broken input is never mistaken for UNKNOWN.
const EXIT_ERROR: i32 = 3;

fn main() {
    let cli = Cli::try_parse().unwrap_or_else(|e| {
        let code = if e.use_stderr() { EXIT_ERROR } else { 0 };
        let _ = e.print();
        std::process::exit(code);
    });

    match cli.command {
        Some(Commands::Measure { path, .. }) => {
            eprintln!("fid: measure not yet implemented — {}", path);
            std::process::exit(2);
        }
        Some(Commands::Scan { path, .. }) => {
            eprintln!("fid: scan not yet implemented — {}", path);
            std::process::exit(2);
        }
        Some(Commands::Check { path, floor }) => match run_check(&path, floor) {
            Ok(code) => std::process::exit(code),
            Err(e) => {
                eprintln!("fid check: {}", e);
                std::process::exit(EXIT_ERROR);
            }
        },
        Some(Commands::Stamp { source, source_class, noise, half_life, derived_from }) => {
            if let Err(e) = run_stamp(source, source_class, noise, half_life, derived_from) {
                eprintln!("fid stamp: {}", e);
                std::process::exit(EXIT_ERROR);
            }
        }
        Some(Commands::Chain { path, floor }) => {
            if let Err(e) = run_chain(&path, floor) {
                eprintln!("fid chain: {}", e);
                std::process::exit(EXIT_ERROR);
            }
        }
        Some(Commands::Filter { .. }) => {
            eprintln!("fid: filter not yet implemented");
            std::process::exit(2);
        }
        Some(Commands::Watch { path, .. }) => {
            eprintln!("fid: watch not yet implemented — {}", path);
            std::process::exit(2);
        }
        None => {
            // Bare `fid <path>` — shorthand for measure
            if let Some(path) = cli.path {
                eprintln!("fid: measure not yet implemented — {}", path);
                std::process::exit(2);
            } else {
                eprintln!("fid: no command or path given. Try `fid --help`");
                std::process::exit(2);
            }
        }
    }
}

/// Read stdin, wrap it in a fidelity envelope, write JSON to stdout.
fn run_stamp(
    source: String,
    source_class: Option<String>,
    noise: f64,
    half_life: Option<String>,
    derived_from: Option<String>,
) -> Result<(), String> {
    let source_class = source_class
        .map(|s| s.parse::<SourceClass>())
        .transpose()?;

    let upstream_file = match &derived_from {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read --derived-from {}: {}", path, e))?;
            let upstream: Stamped = serde_json::from_str(&text)
                .map_err(|e| format!("--derived-from {} is not a valid fid-stamped artifact: {}", path, e))?;
            Some(upstream.fid)
        }
        None => None,
    };

    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| format!("cannot read stdin: {}", e))?;

    let opts = StampOptions { source, source_class, noise, half_life, derived_from };
    let out = stamp(&input, &opts, upstream_file, Utc::now())?;

    for warning in &out.warnings {
        eprintln!("fid stamp: warning: {}", warning);
    }
    let json = serde_json::to_string(&out.stamped).map_err(|e| e.to_string())?;
    println!("{}", json);
    Ok(())
}

/// Read a stamped artifact from a path, or stdin if the path is "-".
fn read_stamped(path: &str) -> Result<Stamped, String> {
    let text = if path == "-" {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(|e| format!("cannot read stdin: {}", e))?;
        input
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?
    };
    serde_json::from_str(&text).map_err(|e| format!("{} is not a valid fid-stamped artifact: {}", path, e))
}

/// Read a stamped artifact, assess it, report to stderr, return the exit code.
fn run_check(path: &str, floor: f64) -> Result<i32, String> {
    let stamped = read_stamped(path)?;
    let result = assess(&stamped, floor, Utc::now())?;
    eprintln!("{}", report(&stamped, &result));
    Ok(result.assessment.status.exit_code())
}

/// Read a stamped artifact and print its fidelity chain to stdout.
fn run_chain(path: &str, floor: f64) -> Result<(), String> {
    let stamped = read_stamped(path)?;
    let result = assess(&stamped, floor, Utc::now())?;
    println!("{}", fid::chain::report(&stamped, &result));
    Ok(())
}
