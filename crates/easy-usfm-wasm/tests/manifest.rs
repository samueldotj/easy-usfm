//! The two manifest values this crate spells out instead of inheriting.
//!
//! `license` and `repository` are written literally in this crate's
//! `Cargo.toml` because `wasm-pack` parses that file itself and older builds
//! of it do not resolve workspace inheritance — see the comment there. The
//! cost of spelling them out is that they can drift from the workspace's
//! without anything noticing, which is what this stops.

use std::path::Path;

/// The value of a top-level `key = "value"` line in a TOML table.
///
/// A three-line reader rather than a TOML dependency: this runs over two files
/// whose shape is known, and adding a parser to the build to read two strings
/// out of the build's own configuration is not a trade worth making.
fn value(manifest: &str, table: &str, key: &str) -> Option<String> {
    manifest
        .split(&format!("[{table}]"))
        .nth(1)?
        .lines()
        // Stop at the next table, or `[dependencies]`'s `license` would count.
        .take_while(|line| !line.trim_start().starts_with('['))
        .find_map(|line| {
            let (name, rest) = line.split_once('=')?;
            (name.trim() == key).then(|| rest.trim().trim_matches('"').to_string())
        })
}

#[test]
fn the_spelled_out_values_match_the_workspace() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crate_manifest = std::fs::read_to_string(here.join("Cargo.toml")).expect("our manifest");
    let workspace = std::fs::read_to_string(here.join("../../Cargo.toml")).expect("the workspace");

    for key in ["license", "repository"] {
        let ours = value(&crate_manifest, "package", key);
        let theirs = value(&workspace, "workspace.package", key);

        assert!(theirs.is_some(), "the workspace should set {key}");
        assert_eq!(
            ours, theirs,
            "{key} differs from the workspace's. This crate spells it out for \
             wasm-pack's benefit; update both or neither."
        );
    }
}
