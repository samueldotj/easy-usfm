//! Pinned benchmarks — P4.5.
//!
//! "> 20 % perf regression fails the build."
//!
//! Deliberately not Criterion. Criterion measures well and reports beautifully,
//! and neither is the problem here: the problem is deciding, in CI, whether a
//! number is worse than a number recorded on different hardware. Criterion's own
//! baseline comparison is per-machine, which a hosted runner is not.
//!
//! So the pinning is *relative*. Every measurement is expressed against a
//! calibration loop measured in the same process moments earlier, which cancels
//! the machine out: a runner half the speed makes both numbers twice as large
//! and their ratio the same. The committed baselines are ratios, and the gate
//! compares ratios.
//!
//! Run with `cargo bench -p easy-usfm-core`, and `--` `--json` for the CI gate.

use std::hint::black_box;
use std::time::{Duration, Instant};

use easy_usfm_core::{ByteSpan, Session};

/// What one measurement produced.
struct Measured {
    name: &'static str,
    /// Nanoseconds, and the ratio against the calibration loop.
    nanos: u128,
    ratio: f64,
}

/// A fixed amount of arithmetic, to measure the machine rather than the code.
///
/// Integer work in a black box: it cannot be optimised away and it does not
/// touch memory, so it tracks core speed rather than cache or allocator, which
/// is the part that varies least between a laptop and a hosted runner.
fn calibrate() -> u128 {
    let mut best = u128::MAX;

    // Best of several, for the same reason the measurements are: a calibration
    // that happened to be interrupted would scale every ratio in the run.
    for _ in 0..5 {
        let started = Instant::now();
        let mut total: u64 = 0;
        for step in 0..2_000_000u64 {
            total = black_box(total.wrapping_mul(6364136223846793005).wrapping_add(step));
        }
        black_box(total);
        best = best.min(started.elapsed().as_nanos());
    }
    best.max(1)
}

/// How many rounds each measurement runs, keeping the best.
///
/// The *minimum*, not the mean. A benchmark's noise is entirely one-sided --
/// nothing makes the code faster than it is, and everything else on the machine
/// makes it slower -- so the fastest round is the closest reading of what the
/// code costs and the mean is a reading of what else the runner was doing.
///
/// Measured before this changed: the same code varied by more than thirteen
/// percent between consecutive runs, against a gate set at twenty. A gate that
/// fails for reasons no commit caused is one people learn to re-run.
const ROUNDS: usize = 5;

fn measure(name: &'static str, unit: u128, mut body: impl FnMut()) -> Measured {
    // A warm-up pass, so the first measurement is not paying for lazy
    // initialisation that every later one gets for free.
    body();

    let mut best = u128::MAX;
    for _ in 0..ROUNDS {
        let started = Instant::now();
        let mut runs = 0;
        while started.elapsed() < Duration::from_millis(120) {
            body();
            runs += 1;
        }
        best = best.min(started.elapsed().as_nanos() / runs.max(1));
    }

    Measured {
        name,
        nanos: best,
        ratio: best as f64 / unit as f64,
    }
}

/// A document of `chapters` chapters, each of `verses` verses.
fn document(chapters: usize, verses: usize) -> String {
    let mut text = String::from("\\id GEN Benchmark\n\\h Genesis\n");
    for chapter in 1..=chapters {
        text.push_str(&format!("\\c {chapter}\n\\p\n"));
        for verse in 1..=verses {
            text.push_str(&format!(
                "\\v {verse} In the beginning God created the heaven and the earth, \
                 and the earth was without form and void.\n"
            ));
        }
    }
    text
}

fn main() {
    let json = std::env::args().any(|argument| argument == "--json");
    let unit = calibrate();

    let small = document(5, 10);
    let large = document(50, 30);
    let parsed = Session::new(&large);
    let keystroke_at = large.len() / 2;

    let results = vec![
        measure("open_small", unit, || {
            black_box(Session::new(black_box(&small)));
        }),
        measure("open_large", unit, || {
            black_box(Session::new(black_box(&large)));
        }),
        measure("diagnostics_large", unit, || {
            black_box(parsed.diagnostics());
        }),
        {
            // One character typed into a document already open, which is the
            // operation ARCHITECTURE 8 budgets and a translator performs
            // thousands of times an hour. The session is built once and typed
            // into repeatedly -- rebuilding it inside the loop would measure
            // the initial parse, which is what the first two already do.
            let mut typing = Session::new(&large);
            let mut at = keystroke_at;
            measure("keystroke", unit, move || {
                let _ = typing.edit(ByteSpan::new(at, at), "x");
                at += 1;
                black_box(typing.diagnostics());
            })
        },
    ];

    if json {
        println!("{{");
        println!("  \"calibration_nanos\": {unit},");
        println!("  \"measurements\": {{");
        for (index, result) in results.iter().enumerate() {
            let comma = if index + 1 == results.len() { "" } else { "," };
            println!(
                "    \"{}\": {{ \"nanos\": {}, \"ratio\": {:.6} }}{comma}",
                result.name, result.nanos, result.ratio
            );
        }
        println!("  }}");
        println!("}}");
    } else {
        println!("calibration unit: {unit} ns");
        for result in &results {
            println!(
                "{:<20} {:>12} ns   ratio {:.4}",
                result.name, result.nanos, result.ratio
            );
        }
    }
}
