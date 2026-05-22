use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

mod report;

// ===========================================================================
// CLI
// ===========================================================================

#[derive(Parser)]
#[command(name = "xtask", about = "Trek Parser fuzzing orchestration")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a fuzzing campaign and generate a report
    Fuzz {
        /// Fuzz target name (lua_roundtrip or json_roundtrip)
        #[arg(default_value = "lua_roundtrip")]
        target: String,

        /// Maximum fuzzing duration in seconds
        #[arg(short, long, default_value = "300")]
        max_time: u64,

        /// Number of fuzzer jobs (parallel processes)
        #[arg(short, long, default_value = "1")]
        jobs: u16,

        /// Output report path
        #[arg(short, long, default_value = "fuzz_report.html")]
        output: PathBuf,
    },

    /// Generate a report from existing fuzz artifacts
    Report {
        /// Fuzz target name
        #[arg(default_value = "lua_roundtrip")]
        target: String,

        /// Output report path
        #[arg(short, long, default_value = "fuzz_report.html")]
        output: PathBuf,

        /// Path to a fuzzer log file (optional, for richer stats)
        #[arg(short, long)]
        log: Option<PathBuf>,
    },
}

// ===========================================================================
// Data structures
// ===========================================================================

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct FuzzStats {
    /// Timestamp data points for the chart
    timeline: Vec<DataPoint>,
    /// Crash artifacts found
    crashes: Vec<CrashInfo>,
    /// Corpus size in bytes
    corpus_bytes: u64,
    /// Corpus entry count
    corpus_entries: usize,
    /// Total execs per data point
    execs_per_dp: Vec<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DataPoint {
    time_secs: f64,
    exec_per_sec: f64,
    total_execs: u64,
    corpus_size: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CrashInfo {
    path: String,
    size: u64,
    modified: String,
}

// ===========================================================================
// Main
// ===========================================================================

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Fuzz { target, max_time, jobs, output } => {
            run_fuzz_campaign(&target, max_time, jobs, &output)?
        }
        Commands::Report { target, output, log } => {
            generate_report_from_artifacts(&target, &output, log.as_deref())?
        }
    }
    Ok(())
}

// ===========================================================================
// Fuzz campaign runner
// ===========================================================================

fn run_fuzz_campaign(target: &str, max_time: u64, jobs: u16, output: &Path) -> Result<()> {
    let fuzz_root = project_root().join("crates").join("parser").join("fuzz");
    let artifact_dir = fuzz_root.join("artifacts").join(target);
    let corpus_dir = fuzz_root.join("corpus").join(target);

    // Ensure directories exist
    fs::create_dir_all(&artifact_dir)
        .context("Failed to create artifacts directory")?;
    fs::create_dir_all(&corpus_dir)
        .context("Failed to create corpus directory")?;

    println!("🚀 Starting fuzzing campaign: target={target}, max_time={max_time}s, jobs={jobs}");

    let start = Instant::now();
    let mut stats = FuzzStats::default();

    // Launch fuzzer (run from crates/parser/ so cargo fuzz finds fuzz/Cargo.toml)
    // Override workspace release profile (lto, panic) that break ASAN instrumentation.
    let mut child = Command::new("cargo")
        .args([
            "+nightly", "fuzz", "run", target,
            "--",
        ])
        .arg(format!("-max_total_time={}", max_time))
        .arg(format!("-jobs={}", jobs))
        .arg(format!("-workers={}", jobs))
        .arg("-print_final_stats=1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(project_root().join("crates").join("parser"))
        .env("CARGO_PROFILE_RELEASE_LTO", "false")
        .env("CARGO_PROFILE_RELEASE_PANIC", "unwind")
        .spawn()
        .context("Failed to spawn cargo-fuzz (is it installed via `cargo install cargo-fuzz`?)")?;

    // Collect stderr in a background thread (libfuzzer stats are on stderr)
    let stderr = child.stderr.take().unwrap();
    let stats_handle = {
        let mut stats_clone = stats.clone();
        let target_clone = target.to_string();
        std::thread::spawn(move || {
            let _ = parse_fuzzer_output(stderr, &mut stats_clone, &target_clone);
            stats_clone
        })
    };

    // Discard stdout (nothing meaningful)
    let stdout = child.stdout.take().unwrap();
    let _out_handle = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        std::io::BufReader::new(stdout).read_to_string(&mut buf).ok();
    });

    let status = child.wait().context("Fuzzer process failed")?;

    // Merge timeline data from the background thread
    let thread_stats = stats_handle.join().unwrap();
    stats.timeline = thread_stats.timeline;
    stats.execs_per_dp = thread_stats.execs_per_dp;
    stats.corpus_bytes = thread_stats.corpus_bytes;
    stats.corpus_entries = thread_stats.corpus_entries;

    let elapsed = start.elapsed();
    let _ = _out_handle.join();

    // Scan for crashes
    stats.crashes = scan_artifacts(&artifact_dir);

    println!(
        "Fuzzing completed in {:.1}s — {} crash(es) found, status: {:?}",
        elapsed.as_secs_f64(),
        stats.crashes.len(),
        status.code(),
    );

    // Generate report
    let html = report::generate_report(&stats, target, elapsed);
    fs::write(output, html.into_string())
        .context("Failed to write report HTML")?;

    println!("📄 Report written to {}", output.display());
    Ok(())
}

// ===========================================================================
// Fuzzer output parser
// ===========================================================================

use std::io::{BufRead, BufReader};

fn parse_fuzzer_output<R: std::io::Read>(
    reader: R,
    stats: &mut FuzzStats,
    _target: &str,
) -> Result<()> {
    let reader = BufReader::new(reader);
    let re_exec = regex_lite::Regex::new(r"exec/s:\s*(\d+)").unwrap();
    let re_total = regex_lite::Regex::new(r"#(\d+)\s").unwrap();
    let re_corp = regex_lite::Regex::new(r"corp:\s*(\d+)/\d+[KMG]?b").unwrap();

    for line in reader.lines() {
        let line = line?;
        if !line.starts_with('#') {
            continue;
        }
        let exec_per_sec = re_exec
            .captures(&line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .unwrap_or(0.0);
        let total_execs = re_total
            .captures(&line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0);
        let corpus_entries = re_corp
            .captures(&line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<usize>().ok())
            .unwrap_or(stats.corpus_entries);

        stats.timeline.push(DataPoint {
            time_secs: stats.timeline.len() as f64,
            exec_per_sec,
            total_execs,
            corpus_size: corpus_entries,
        });
        stats.execs_per_dp.push(total_execs);
        stats.corpus_entries = corpus_entries;
    }
    Ok(())
}

// ===========================================================================
// Artifact scanning
// ===========================================================================

fn scan_artifacts(dir: &Path) -> Vec<CrashInfo> {
    let mut crashes = Vec::new();
    if !dir.exists() {
        return crashes;
    }
    for entry in WalkDir::new(dir).max_depth(2) {
        let entry = match entry {
            Ok(e) => e,
            _ => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            _ => continue,
        };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| {
                let dt: DateTime<Local> = t.into();
                Some(dt.format("%Y-%m-%d %H:%M:%S").to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());

        crashes.push(CrashInfo {
            path: entry.path().strip_prefix(dir).unwrap_or(entry.path()).to_string_lossy().to_string(),
            size: meta.len(),
            modified,
        });
    }
    crashes.sort_by(|a, b| b.modified.cmp(&a.modified));
    crashes
}

// ===========================================================================
// Report generation from existing artifacts (no live fuzzing)
// ===========================================================================

fn generate_report_from_artifacts(target: &str, output: &Path, log_path: Option<&Path>) -> Result<()> {
    let fuzz_root = project_root().join("crates").join("parser").join("fuzz");
    let artifact_dir = fuzz_root.join("artifacts").join(target);
    let corpus_dir = fuzz_root.join("corpus").join(target);

    let mut stats = FuzzStats::default();

    // Scan crashes
    stats.crashes = scan_artifacts(&artifact_dir);

    // Corpus info
    if corpus_dir.exists() {
        stats.corpus_entries = fs::read_dir(&corpus_dir)
            .map(|d| d.filter_map(|e| e.ok()).filter(|e| e.file_type().ok().map_or(false, |t| t.is_file())).count())
            .unwrap_or(0);
        stats.corpus_bytes = fs::read_dir(&corpus_dir)
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .filter_map(|e| e.metadata().ok())
                    .map(|m| m.len())
                    .sum()
            })
            .unwrap_or(0);
    }

    // Parse log file if provided
    if let Some(log) = log_path {
        let file = fs::File::open(log)
            .context("Failed to open log file")?;
        parse_fuzzer_output(file, &mut stats, target)?;
    }

    let html = report::generate_report(&stats, target, Duration::ZERO);
    fs::write(output, html.into_string())
        .context("Failed to write report HTML")?;

    println!("📄 Report written to {}", output.display());
    Ok(())
}

// ===========================================================================
// Helpers
// ===========================================================================

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
