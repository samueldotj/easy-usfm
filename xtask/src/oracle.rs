//! The three-way differential oracle — ARCHITECTURE §12.1.
//!
//! ```text
//! corpus file ──┬──► usfm-core   ──► normalized USJ
//!               ├──► usfm3 (direct)   ──► normalized USJ    ← in-process
//!               └──► usfm-grammar     ──► normalized USJ
//!                                             │
//!                                             ▼
//!                                    three-way structural diff
//! ```
//!
//! The two legs answer different questions, and both are worth having.
//!
//! **Ours versus `usfm3` isolates our own bugs.** Both start from the same
//! parse, so any disagreement is in the layer we wrote — the tree conversion
//! and the USJ rendering — and cannot be blamed on interpretation. This is the
//! leg that runs everywhere, costs nothing, and catches the most.
//!
//! **Either versus `usfm-grammar` isolates genuine interpretation
//! differences.** A second independent implementation reading the same
//! specification is the only way to find out that we have been confidently
//! wrong about what a construct means. It needs Node and an npm install, so it
//! is opt-in — and its absence is reported rather than passed over, because a
//! two-way run silently labelled as three-way is worse than no oracle.
//!
//! This lives in xtask rather than in the engine's tests because it is the one
//! place `usfm3` may legitimately be named alongside our own API: it is
//! development tooling, not a layer above the facade, so ADR-001's containment
//! is untouched.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::corpus::{repo_root, usfm_files};

pub struct OracleOpts {
    pub corpus: Option<PathBuf>,
    /// Compare against usfm-grammar as well. Requires node and the package.
    pub with_grammar: bool,
    /// Stop after this many files.
    pub limit: Option<usize>,
    /// Show every difference rather than the first few.
    pub verbose: bool,
}

pub fn run(options: &OracleOpts) -> Result<()> {
    let directory = options
        .corpus
        .clone()
        .unwrap_or_else(|| repo_root().join("corpus").join("core"));

    let mut files = usfm_files(&directory);
    if let Some(limit) = options.limit {
        files.truncate(limit);
    }
    anyhow::ensure!(
        !files.is_empty(),
        "no USFM files under {}",
        directory.display()
    );

    let grammar = if options.with_grammar {
        match GrammarOracle::locate() {
            Ok(oracle) => Some(oracle),
            Err(error) => {
                eprintln!("usfm-grammar unavailable, running two-way: {error}");
                None
            }
        }
    } else {
        None
    };

    eprintln!(
        "oracle: {} files, {}",
        files.len(),
        if grammar.is_some() {
            "three-way"
        } else {
            "two-way (ours vs usfm3)"
        }
    );

    let mut differences: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut compared = 0usize;

    for path in &files {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue; // invalid UTF-8 belongs to the fuzz target, not here
        };
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        compared += 1;

        let ours = normalize(usfm_core::to_usj(
            usfm_core::Document::parse(source.clone()).content(),
        ));
        let theirs = normalize(usfm3_usj(&source)?);

        if ours != theirs {
            differences
                .entry("ours vs usfm3".to_string())
                .or_default()
                .push(format!("{name}\n{}", describe(&ours, &theirs)));
        }

        if let Some(grammar) = &grammar {
            match grammar.to_usj(&source) {
                Ok(third) => {
                    let third = normalize(third);
                    if ours != third {
                        differences
                            .entry("ours vs usfm-grammar".to_string())
                            .or_default()
                            .push(format!("{name}\n{}", describe(&ours, &third)));
                    }
                }
                Err(error) => eprintln!("  {name}: usfm-grammar failed: {error}"),
            }
        }
    }

    report(compared, &differences, options.verbose)
}

fn report(
    compared: usize,
    differences: &BTreeMap<String, Vec<String>>,
    verbose: bool,
) -> Result<()> {
    if differences.is_empty() {
        eprintln!("\nOK — {compared} files, no structural differences");
        return Ok(());
    }

    for (pair, cases) in differences {
        eprintln!("\n{}: {} of {compared} files differ", pair, cases.len());
        let show = if verbose {
            cases.len()
        } else {
            3.min(cases.len())
        };
        for case in cases.iter().take(show) {
            eprintln!("  {case}");
        }
        if show < cases.len() {
            eprintln!("  … {} more (pass --verbose)", cases.len() - show);
        }
    }

    anyhow::bail!("the oracle found structural differences")
}

/// USJ as `usfm3` itself renders it.
///
/// The one place in the repository where `usfm3` is called next to our own
/// API. That is the point of the leg.
fn usfm3_usj(source: &str) -> Result<Value> {
    let parsed = usfm3::parse(source, usfm3::ParseOptions { diagnostics: false });
    parsed
        .to_usj(usfm3::usj::UsjOptions {
            include_spans: false,
        })
        .map_err(|error| anyhow::anyhow!("usfm3 could not render USJ: {error}"))
}

/// Puts both sides into the same shape before comparing.
///
/// ARCHITECTURE §12.1 asks for zero *unexplained* differences, and the value
/// of the oracle depends on keeping that word load-bearing. Each
/// reconciliation below names a difference that has been chased down and
/// understood; anything not listed here shows up as a failure.
///
/// - **Source spans and CST anchors.** USJ does not model them at all.
/// - **Empty content arrays.** Some implementations emit `[]`, others omit
///   the key. The USJ schema does not require either.
/// - **`attributes` as a nested array.** `usfm3` renders marker attributes as
///   `attributes: [{key, value}]`. The USJ schema
///   (`usfm-bible/tcdocs:grammar/usj.js`) declares no such property: `sid`,
///   `number`, `code`, `caller`, `align`, and `category` are named directly on
///   the node, and marker attributes like `src` and `lemma` join them as
///   further properties. Our form follows the schema and `usfm3`'s does not,
///   so the nested array is lifted before comparing. **Found by this oracle on
///   its first run**, on five corpus files carrying figures — and worth
///   raising upstream, since anything consuming `usfm3`'s USJ as USJ will
///   mis-read every attribute in it.
fn normalize(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();

            for (key, value) in map {
                if matches!(key.as_str(), "span" | "spans" | "anchor_cst") {
                    continue;
                }
                if value.as_array().is_some_and(|array| array.is_empty()) {
                    continue;
                }
                if key == "attributes" {
                    lift_attributes(&value, &mut out);
                    continue;
                }
                out.insert(key, normalize(value));
            }

            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(normalize).collect()),
        other => other,
    }
}

/// Turns `[{"key": "src", "value": "x"}]` into `"src": "x"`.
fn lift_attributes(value: &Value, into: &mut serde_json::Map<String, Value>) {
    let Some(entries) = value.as_array() else {
        return;
    };
    for entry in entries {
        if let (Some(key), Some(text)) = (
            entry.get("key").and_then(Value::as_str),
            entry.get("value").and_then(Value::as_str),
        ) {
            into.insert(key.to_string(), Value::String(text.to_string()));
        }
    }
}

/// The first place the two disagree, with enough context to act on.
fn describe(left: &Value, right: &Value) -> String {
    fn walk(left: &Value, right: &Value, path: &str, into: &mut Vec<String>) {
        if into.len() >= 3 || left == right {
            return;
        }
        match (left, right) {
            (Value::Object(a), Value::Object(b)) => {
                let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
                keys.sort();
                keys.dedup();
                for key in keys {
                    match (a.get(key), b.get(key)) {
                        (Some(x), Some(y)) => walk(x, y, &format!("{path}.{key}"), into),
                        (Some(x), None) => {
                            into.push(format!("    {path}.{key}: only on the left ({x})"))
                        }
                        (None, Some(y)) => {
                            into.push(format!("    {path}.{key}: only on the right ({y})"))
                        }
                        (None, None) => {}
                    }
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                if a.len() != b.len() {
                    into.push(format!(
                        "    {path}: {} items on the left, {} on the right",
                        a.len(),
                        b.len()
                    ));
                }
                for (index, (x, y)) in a.iter().zip(b).enumerate() {
                    walk(x, y, &format!("{path}[{index}]"), into);
                }
            }
            _ => into.push(format!("    {path}: {left} vs {right}")),
        }
    }

    let mut found = Vec::new();
    walk(left, right, "", &mut found);
    if found.is_empty() {
        "    (structurally equal after normalization)".to_string()
    } else {
        found.join("\n")
    }
}

// -------------------------------------------------------- usfm-grammar ---

/// The third implementation, run through Node.
struct GrammarOracle {
    script: PathBuf,
}

impl GrammarOracle {
    /// Writes the bridge script and checks the package is importable.
    fn locate() -> Result<Self> {
        let probe = Command::new("node")
            .arg("--version")
            .output()
            .context("node is not on PATH")?;
        anyhow::ensure!(probe.status.success(), "node did not run");

        let script = repo_root().join("xtask").join(".spec").join("grammar.mjs");
        std::fs::create_dir_all(script.parent().unwrap())?;
        std::fs::write(&script, GRAMMAR_BRIDGE)?;

        let check = Command::new("node")
            .arg(&script)
            .arg("--check")
            .output()
            .context("running the usfm-grammar bridge")?;
        anyhow::ensure!(
            check.status.success(),
            "usfm-grammar is not installed — `npm install usfm-grammar` in the \
             repository root, then re-run"
        );

        Ok(Self { script })
    }

    fn to_usj(&self, source: &str) -> Result<Value> {
        let mut child = Command::new("node")
            .arg(&self.script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        use std::io::Write;
        child
            .stdin
            .take()
            .context("stdin")?
            .write_all(source.as_bytes())?;

        let output = child.wait_with_output()?;
        anyhow::ensure!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );

        serde_json::from_slice(&output.stdout).context("usfm-grammar returned invalid JSON")
    }
}

const GRAMMAR_BRIDGE: &str = r#"
// Bridge to usfm-grammar, the third implementation in the oracle.
// Written by `cargo xtask oracle --with-grammar`; not checked in.
import { USFMParser } from 'usfm-grammar';

if (process.argv[2] === '--check') {
    process.exit(0);
}

const chunks = [];
for await (const chunk of process.stdin) chunks.push(chunk);
const source = Buffer.concat(chunks).toString('utf8');

const parser = new USFMParser(source);
process.stdout.write(JSON.stringify(parser.toJSON()));
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_drops_what_usj_does_not_model() {
        let value = serde_json::json!({
            "type": "para",
            "span": {"start": 0, "end": 4},
            "content": []
        });
        let normalized = normalize(value);

        assert!(normalized.get("span").is_none());
        assert!(normalized.get("content").is_none());
        assert_eq!(normalized["type"], "para");
    }

    #[test]
    fn a_nested_attributes_array_is_lifted_to_properties() {
        // usfm3's shape on the left, the USJ schema's on the right.
        let theirs = serde_json::json!({
            "type": "figure",
            "marker": "fig",
            "attributes": [
                {"key": "src", "value": "image.png"},
                {"key": "size", "value": "span"}
            ]
        });
        let ours = serde_json::json!({
            "type": "figure",
            "marker": "fig",
            "src": "image.png",
            "size": "span"
        });

        assert_eq!(normalize(theirs), normalize(ours));
    }

    #[test]
    fn our_usj_agrees_with_usfm3_on_a_small_document() {
        let source = "\\id GEN Genesis\n\\c 1\n\\p\n\\v 1 In the beginning.\n";
        let ours = normalize(usfm_core::to_usj(
            usfm_core::Document::parse(source.to_string()).content(),
        ));
        let theirs = normalize(usfm3_usj(source).expect("usfm3 renders USJ"));

        assert_eq!(ours, theirs, "{}", describe(&ours, &theirs));
    }

    #[test]
    fn a_difference_is_described_rather_than_dumped() {
        let left = serde_json::json!({"type": "para", "marker": "p"});
        let right = serde_json::json!({"type": "para", "marker": "q1"});
        let description = describe(&left, &right);

        assert!(description.contains("marker"), "{description}");
    }
}
