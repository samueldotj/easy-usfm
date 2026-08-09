//! Command-line access to the USFM engine.
//!
//! ARCHITECTURE §3 notes that `usfm-core` builds standalone with no Tauri
//! and no filesystem, and that the Phase 0 CLI "becomes a debugging tool used
//! throughout every later phase". That is what this is for: when the preview
//! shows something unexpected in M3, the question *is it the engine or the
//! interface* should be answerable in one command rather than by adding
//! logging to a worker.
//!
//! ```text
//! easy-usfm parse       corpus/core         tree shape, one line per file
//! easy-usfm diagnostics corpus/core         every diagnostic, with codes
//! easy-usfm usj         a.usfm              the USJ document model
//! easy-usfm bench       corpus/core         throughput against the budget
//! easy-usfm check       corpus/core         the fuzz invariants, deterministically
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};
use usfm_core::{invariants, to_usj, Document, Severity};

#[derive(Parser)]
#[command(name = "easy-usfm", about = "USFM engine tools", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and report the shape of each file
    Parse { paths: Vec<PathBuf> },
    /// Report diagnostics
    Diagnostics {
        paths: Vec<PathBuf>,
        /// Only this severity and above
        #[arg(long, default_value = "information")]
        min: String,
        /// Suppress a code, repeatable
        #[arg(long = "suppress")]
        suppressed: Vec<String>,
    },
    /// Print the USJ document model
    Usj {
        paths: Vec<PathBuf>,
        #[arg(long)]
        pretty: bool,
    },
    /// Time parsing, against the ARCHITECTURE §11 budget
    Bench {
        paths: Vec<PathBuf>,
        /// Passes over each file
        #[arg(long, default_value_t = 3)]
        runs: usize,
        /// Budget in ms per 2 MB. 400 native, 700 wasm.
        #[arg(long, default_value_t = 400)]
        budget_ms: u64,
    },
    /// Assert the fuzz invariants over real files
    Check { paths: Vec<PathBuf> },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Parse { paths } => parse(&collect(&paths)),
        Command::Diagnostics {
            paths,
            min,
            suppressed,
        } => diagnostics(&collect(&paths), &min, &suppressed),
        Command::Usj { paths, pretty } => usj(&collect(&paths), pretty),
        Command::Bench {
            paths,
            runs,
            budget_ms,
        } => bench(&collect(&paths), runs, budget_ms),
        Command::Check { paths } => check(&collect(&paths)),
    };

    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Every USFM file under the given paths, which may be files or directories.
fn collect(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_dir() {
            walk(path, &mut files);
        } else {
            files.push(path.clone());
        }
    }
    files.sort();
    files
}

fn walk(directory: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, into);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("usfm") || e.eq_ignore_ascii_case("sfm"))
        {
            into.push(path);
        }
    }
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into()
}

// ------------------------------------------------------------------ parse ---

fn parse(files: &[PathBuf]) -> Result<bool, String> {
    for path in files {
        let source = read(path)?;
        let document = Document::parse(source.clone());
        let verses = document.verses();

        println!(
            "{:<32} {:>7} KB  {:>4} nodes  {:>4} verses  {:>3} diagnostics  usfm {}",
            name(path),
            source.len() / 1024,
            document.descendants().count(),
            verses.entries().len(),
            document.diagnostics().len(),
            document.config().document_version,
        );
    }
    Ok(true)
}

// ------------------------------------------------------------ diagnostics ---

fn diagnostics(files: &[PathBuf], min: &str, suppressed: &[String]) -> Result<bool, String> {
    let floor = match min {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        _ => Severity::Information,
    };

    let mut errors = 0usize;
    let mut shown = 0usize;

    for path in files {
        let source = read(path)?;
        let document = Document::parse(source.clone());

        for diagnostic in document.diagnostics() {
            if diagnostic.severity < floor
                || suppressed
                    .iter()
                    .any(|code| code == diagnostic.code.as_str())
            {
                continue;
            }
            shown += 1;
            if diagnostic.severity == Severity::Error {
                errors += 1;
            }

            let (line, column) = position(&source, diagnostic.span.start);
            println!(
                "{}:{line}:{column}: {} {}: {}",
                name(path),
                diagnostic.severity,
                diagnostic.code,
                diagnostic.message
            );
        }
    }

    eprintln!("\n{shown} diagnostics, {errors} of them errors");
    // Errors are reported, not fatal. Diagnostics never prevent saving
    // (PRODUCT §9), and a tool that exits non-zero on a document the editor
    // would happily open would be lying about what the engine thinks.
    Ok(true)
}

/// One-based line and column, counting columns in grapheme clusters.
///
/// UNICODE §2 — the column a user is shown counts what they perceive as
/// characters, so a Tamil conjunct advances it by one however many code points
/// it took to write.
fn position(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset.min(source.len())];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let column = usfm_core::grapheme::column(&source[line_start..], offset - line_start) + 1;
    (line, column)
}

// -------------------------------------------------------------------- usj ---

fn usj(files: &[PathBuf], pretty: bool) -> Result<bool, String> {
    for path in files {
        let document = Document::parse(read(path)?);
        let value = to_usj(document.content());

        let rendered = if pretty {
            serde_json::to_string_pretty(&value)
        } else {
            serde_json::to_string(&value)
        }
        .map_err(|error| error.to_string())?;

        println!("{rendered}");
    }
    Ok(true)
}

// ------------------------------------------------------------------ bench ---

fn bench(files: &[PathBuf], runs: usize, budget_ms: u64) -> Result<bool, String> {
    const BUDGET_BYTES: f64 = 2.0 * 1024.0 * 1024.0;

    let mut total_bytes = 0usize;
    let mut total = std::time::Duration::ZERO;
    let mut slowest: Option<(String, f64)> = None;

    for path in files {
        let source = read(path)?;
        let mut best = std::time::Duration::MAX;

        for _ in 0..runs.max(1) {
            let started = Instant::now();
            let document = Document::parse(source.clone());
            // The whole cost, not just the cheap staged part -- the budget is
            // about opening a file, which builds the tree and the diagnostics.
            let _ = document.content();
            let _ = document.diagnostics();
            best = best.min(started.elapsed());
        }

        total_bytes += source.len();
        total += best;

        let scaled = best.as_secs_f64() * 1000.0 * (BUDGET_BYTES / source.len().max(1) as f64);
        if slowest.as_ref().is_none_or(|(_, worst)| scaled > *worst) {
            slowest = Some((name(path), scaled));
        }
    }

    if total_bytes == 0 {
        return Err("no files to benchmark".into());
    }

    let seconds = total.as_secs_f64();
    let mb = total_bytes as f64 / 1024.0 / 1024.0;
    let per_2mb = seconds * 1000.0 * (BUDGET_BYTES / total_bytes as f64);

    println!("files      {}", files.len());
    println!("total      {mb:.1} MB in {:.0} ms", seconds * 1000.0);
    println!("throughput {:.1} MB/s", mb / seconds.max(f64::EPSILON));
    println!("scaled     {per_2mb:.0} ms per 2 MB   (budget {budget_ms} ms)");

    if let Some((file, worst)) = &slowest {
        println!("slowest    {file} at {worst:.0} ms per 2 MB");
    }

    // Reported against the aggregate rather than the worst file: the budget in
    // ARCHITECTURE §11 is about opening one 2 MB document, and a 3 KB file
    // scaled up by 700x measures startup noise, not throughput.
    let within = per_2mb <= budget_ms as f64;
    if !within {
        eprintln!("\nover budget: {per_2mb:.0} ms per 2 MB against {budget_ms} ms");
    }
    Ok(within)
}

// ------------------------------------------------------------------ check ---

fn check(files: &[PathBuf]) -> Result<bool, String> {
    let mut failures = 0usize;

    for path in files {
        let source = read(path)?;
        if let Err(error) = invariants::check(&source) {
            failures += 1;
            println!("{}: {error}", name(path));
        }
    }

    if failures == 0 {
        eprintln!("OK — {} files, every invariant holds", files.len());
    } else {
        eprintln!("\n{failures} of {} files broke an invariant", files.len());
    }
    Ok(failures == 0)
}
