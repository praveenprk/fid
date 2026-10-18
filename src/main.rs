use std::io::Read;

use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use fid::check::{assess, report};
use fid::envelope::{SourceClass, Stamped};
use fid::registry::{HalfLives, NoiseModel};
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

/// Options for `fid stamp`.
///
/// Registries supply measured values by name; explicit flags override them.
#[derive(Args)]
struct StampArgs {
    /// Source identifier (who/what produced this data)
    #[arg(long)]
    source: String,

    /// Source class (root stamps only; defaults to unknown):
    /// instrument | human_observation | human_attestation | database |
    /// llm_grounded | llm_ungrounded | hearsay | unknown
    #[arg(long)]
    source_class: Option<String>,

    /// Transform noise (0.0 to 1.0); overrides --noise-model
    #[arg(long)]
    noise: Option<f64>,

    /// Half-life duration (e.g. "7d", "6h", "30s"); overrides --half-lives
    #[arg(long)]
    half_life: Option<String>,

    /// Baseline the fact decays toward instead of 0 (0.0 to 1.0); overrides --half-lives
    #[arg(long)]
    baseline: Option<f64>,

    /// Measured fidelity when fresh (0.0 to 1.0), replacing the source class
    /// constant; overrides --half-lives. Root stamps only
    #[arg(long)]
    source_fidelity: Option<f64>,

    /// Half-life registry (YAML) to look up --type and --state in
    #[arg(long, requires = "info_type")]
    half_lives: Option<String>,

    /// Information type (e.g. "vessel_position")
    #[arg(long = "type")]
    info_type: Option<String>,

    /// State of the observed thing (e.g. "underway", "moored")
    #[arg(long)]
    state: Option<String>,

    /// Noise model (YAML) to look up --transform and --condition in
    #[arg(long, requires = "transform")]
    noise_model: Option<String>,

    /// Transform name in the noise model (e.g. "flight_leg")
    #[arg(long, requires = "noise_model")]
    transform: Option<String>,

    /// Upstream condition this step's noise depends on (e.g. "previous_leg_late")
    #[arg(long)]
    condition: Option<String>,

    /// Derived-from: path to the upstream fid-stamped artifact
    #[arg(long)]
    derived_from: Option<String>,
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
    Stamp(StampArgs),

    /// Inspect the fidelity chain of a stamped artifact
    Chain {
        /// Path to the fid-stamped file ("-" for stdin)
        path: String,

        /// Fidelity floor shown in the TOTAL line
        #[arg(long, default_value = "0.50")]
        floor: f64,
    },

    /// When a stamped fact falls below a floor (its TTL); exit code as `check`
    ///
    /// Prints expires_at (RFC 3339) or "never" to stdout.
    Ttl {
        /// Path to the fid-stamped file ("-" for stdin)
        path: String,

        /// Fidelity floor
        #[arg(long, default_value = "0.50")]
        floor: f64,
    },

    /// Fit half-life and baseline from re-checked facts; print registry YAML
    ///
    /// CSV columns: entity, correct, and age (seconds or e.g. 30d) or
    /// observed_at + checked_at (RFC 3339); optional state.
    Calibrate {
        /// Path to the CSV ("-" for stdin)
        path: String,

        /// Information type to name in the registry (e.g. "company_ceo")
        #[arg(long = "type")]
        info_type: String,

        /// Refuse when the half-life's 95% range spans more than this factor
        /// (high end ÷ low end)
        #[arg(long, default_value_t = fid::calibrate::MAX_SPREAD)]
        max_spread: f64,
    },

    /// Test a registry against recent re-checks; exit 0 current, 1 drifted, 2 unknown
    ///
    /// Prints "current", "drifted" or "unknown" to stdout, the report to stderr.
    Drift {
        /// Recent re-checks, CSV as `calibrate` reads ("-" for stdin)
        path: String,

        /// Information type in the registry (e.g. "company_ceo")
        #[arg(long = "type")]
        info_type: String,

        /// Half-life registry (YAML) in force
        #[arg(long)]
        half_lives: String,

        /// Say unknown, not current, when the recent half-life's 95% range
        /// spans more than this factor
        #[arg(long, default_value_t = fid::calibrate::MAX_SPREAD)]
        max_spread: f64,
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
        Some(Commands::Stamp(args)) => {
            if let Err(e) = run_stamp(args) {
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
        Some(Commands::Ttl { path, floor }) => match run_ttl(&path, floor) {
            Ok(code) => std::process::exit(code),
            Err(e) => {
                eprintln!("fid ttl: {}", e);
                std::process::exit(EXIT_ERROR);
            }
        },
        Some(Commands::Calibrate { path, info_type, max_spread }) => {
            if let Err(e) = run_calibrate(&path, &info_type, max_spread) {
                eprintln!("fid calibrate: {}", e);
                std::process::exit(EXIT_ERROR);
            }
        }
        Some(Commands::Drift { path, info_type, half_lives, max_spread }) => {
            match run_drift(&path, &info_type, &half_lives, max_spread) {
                Ok(code) => std::process::exit(code),
                Err(e) => {
                    eprintln!("fid drift: {}", e);
                    std::process::exit(EXIT_ERROR);
                }
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
fn run_stamp(args: StampArgs) -> Result<(), String> {
    let source_class = args.source_class.as_deref().map(str::parse::<SourceClass>).transpose()?;

    // Registry values first, explicit flags override.
    let (mut half_life, mut baseline, mut source_fidelity, mut echo) = (None, None, None, None);
    if let Some(path) = &args.half_lives {
        let info_type = args.info_type.as_deref().expect("clap enforces --type");
        let spec = HalfLives::load(path)?.lookup(info_type, args.state.as_deref())?;
        half_life = Some(spec.half_life);
        baseline = spec.baseline;
        source_fidelity = spec.source_fidelity;
        echo = spec.echo;
    }
    half_life = args.half_life.or(half_life);
    baseline = args.baseline.or(baseline);
    source_fidelity = args.source_fidelity.or(source_fidelity);

    let mut noise = 0.0;
    if let Some(path) = &args.noise_model {
        let transform = args.transform.as_deref().expect("clap enforces --transform");
        noise = NoiseModel::load(path)?.lookup(transform, args.condition.as_deref())?;
    }
    let noise = args.noise.unwrap_or(noise);
    let derived_from = args.derived_from;

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

    let opts = StampOptions {
        source: args.source,
        source_class,
        noise,
        half_life,
        baseline,
        source_fidelity,
        echo,
        info_type: args.info_type,
        state: args.state,
        condition: args.condition,
        derived_from,
    };
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

/// Fit decay from a CSV of re-checked facts: report to stderr, registry YAML to stdout.
fn run_calibrate(path: &str, info_type: &str, max_spread: f64) -> Result<(), String> {
    let fits = fid::calibrate::calibrate_with(&read_ledger(path)?, max_spread)?;
    eprint!("{}", fid::calibrate::report(info_type, &fits));
    print!("{}", fid::calibrate::registry_yaml(info_type, &fits));
    Ok(())
}

fn read_ledger(path: &str) -> Result<Vec<fid::calibrate::Observation>, String> {
    if path == "-" {
        fid::calibrate::read_csv(std::io::stdin())
    } else {
        let file = std::fs::File::open(path).map_err(|e| format!("cannot read {}: {}", path, e))?;
        fid::calibrate::read_csv(file)
    }
}

/// Test the registry against recent re-checks; the exit code is the verdict.
fn run_drift(path: &str, info_type: &str, half_lives: &str, max_spread: f64) -> Result<i32, String> {
    let registry = HalfLives::load(half_lives)?;
    let drifts = fid::drift::drift(&read_ledger(path)?, &registry, info_type, max_spread)?;
    eprint!("{}", fid::drift::report(info_type, &drifts, max_spread));
    let verdict = fid::drift::overall(&drifts);
    println!("{}", format!("{:?}", verdict).to_lowercase());
    Ok(verdict.exit_code())
}

/// Print when a stamped fact falls below the floor; exit code as `check`.
fn run_ttl(path: &str, floor: f64) -> Result<i32, String> {
    let stamped = read_stamped(path)?;
    let now = Utc::now();
    let result = assess(&stamped, floor, now)?;
    let expiry = stamped.fid.expires_at(floor)?;
    match expiry {
        None => {
            eprintln!("UNKNOWN  source fidelity indeterminate; no TTL");
            println!("unknown");
        }
        Some(fid::envelope::Expiry::Never) => {
            eprintln!("never    stays at or above floor {:.2} (no decay, or baseline at or above it)", floor);
            println!("never");
        }
        Some(fid::envelope::Expiry::At(at)) => {
            let remaining = (at - now).num_milliseconds() as f64 / 1000.0;
            let when = if remaining > 0.0 {
                format!("in {}", fid::check::format_age(remaining))
            } else {
                format!("{} ago", fid::check::format_age(-remaining))
            };
            eprintln!("expires  {} ({}), floor {:.2}", at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true), when, floor);
            if let Some(e) = &stamped.fid.echo {
                eprintln!("         first time below the floor; the fact comes back every {}, so check again later", e.period);
            }
            println!("{}", at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        }
    }
    if !result.integrity_ok {
        eprintln!("integrity mismatch: content changed since last stamp");
    }
    Ok(result.assessment.status.exit_code())
}
