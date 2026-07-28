//! Development tasks for easy-usfm.
//!
//!     cargo xtask corpus fetch --dry-run
//!     cargo xtask corpus select --target 200
//!     cargo xtask corpus verify
//!     cargo xtask corpus classify corpus/core --coverage
//!
//! The `xtask` pattern keeps development commands in the same toolchain as the
//! project: contributors install Rust and nothing else.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod corpus;
mod features;
mod manifest;

#[derive(Parser)]
#[command(name = "xtask", about = "Development tasks for easy-usfm", version)]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Test corpus: fetch, select, verify, classify
    #[command(subcommand)]
    Corpus(CorpusCmd),
}

#[derive(Subcommand)]
enum CorpusCmd {
    /// Download the extended tier from eBible.org (redistributable only)
    Fetch {
        /// List usable translations and exit
        #[arg(long)]
        list: bool,
        /// Show what would be downloaded
        #[arg(long)]
        dry_run: bool,
        /// Comma-separated translation ids
        #[arg(long)]
        ids: Option<String>,
        /// Maximum translations to fetch
        #[arg(long, default_value_t = 60)]
        limit: usize,
        /// Re-download the catalog
        #[arg(long)]
        refresh_catalog: bool,
    },
    /// Choose the committed core tier from the fetched extended tier
    Select {
        /// How many files to end up with
        #[arg(long, default_value_t = 200)]
        target: usize,
        /// Where the fetched files are
        #[arg(long)]
        source: Option<PathBuf>,
        /// Copy the selection here as well as writing the manifest
        #[arg(long)]
        copy_to: Option<PathBuf>,
        /// Where to write the manifest (default corpus/manifest.toml)
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
    /// Verify hashes, provenance, and coverage. Runs in CI.
    Verify {
        #[arg(long)]
        corpus: Option<PathBuf>,
        /// Check hashes only, while the corpus is being assembled
        #[arg(long)]
        skip_coverage: bool,
    },
    /// Report scripts, features, and encoding traits per file
    Classify {
        paths: Vec<PathBuf>,
        /// Summarise coverage and list anything missing
        #[arg(long)]
        coverage: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = corpus::repo_root();

    match cli.command {
        Task::Corpus(cmd) => match cmd {
            CorpusCmd::Fetch {
                list,
                dry_run,
                ids,
                limit,
                refresh_catalog,
            } => corpus::fetch(&corpus::FetchOpts {
                list,
                dry_run,
                ids,
                limit,
                refresh_catalog,
            }),
            CorpusCmd::Select {
                target,
                source,
                copy_to,
                manifest,
            } => {
                let source = source.unwrap_or_else(|| root.join("corpus").join("extended"));
                let copy_to = copy_to.or_else(|| Some(root.join("corpus").join("core")));
                corpus::select(&source, target, copy_to.as_deref(), manifest.as_deref())
            }
            CorpusCmd::Verify {
                corpus: dir,
                skip_coverage,
            } => {
                let dir = dir.unwrap_or_else(|| root.join("corpus"));
                corpus::verify(&dir, skip_coverage)
            }
            CorpusCmd::Classify { paths, coverage } => {
                let paths = if paths.is_empty() {
                    vec![root.join("corpus").join("core")]
                } else {
                    paths
                };
                corpus::classify(&paths, coverage)
            }
        },
    }
}
