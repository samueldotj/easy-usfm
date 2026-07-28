//! The facade boundary, asserted rather than promised.
//!
//! ADR-001 accepts a five-month-old crate with one maintainer at the centre of
//! the product, and the first of its four risk controls is that nothing above
//! `easy-usfm-core` knows the parser exists. A facade nobody checks erodes one
//! convenient `pub use` at a time — usually under deadline, usually reasonably
//! — and by the time it matters the swap is no longer cheap.
//!
//! So the containment is a test. `usfm3` may be named in `src/backend/` and
//! nowhere else.

use std::path::Path;

/// The one module allowed to know which parser this is.
const BACKEND: &str = "backend";

#[test]
fn the_parser_is_named_only_in_the_backend_module() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();

    visit_rust_files(&source_root, &mut |path| {
        if path
            .strip_prefix(&source_root)
            .is_ok_and(|relative| relative.starts_with(BACKEND))
        {
            return;
        }

        let source = std::fs::read_to_string(path).expect("source file is readable");
        for (number, line) in source.lines().enumerate() {
            // Prose may discuss the dependency; code may not name it. The
            // distinction is what lets the module documentation explain the
            // boundary it is enforcing.
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.contains("usfm3") {
                offenders.push(format!(
                    "{}:{}: {}",
                    path.strip_prefix(&source_root).unwrap_or(path).display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    });

    assert!(
        offenders.is_empty(),
        "the parser is named outside src/{BACKEND}/, which breaks the facade \
         boundary ADR-001 depends on:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_backend_module_exists_and_does_name_the_parser() {
    // Guards the test above against passing because the thing it checks was
    // renamed or removed.
    let backend = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(BACKEND);

    assert!(backend.is_dir(), "src/{BACKEND}/ is missing");

    let mut names_the_parser = false;
    visit_rust_files(&backend, &mut |path| {
        let source = std::fs::read_to_string(path).expect("source file is readable");
        names_the_parser |= source.contains("usfm3");
    });

    assert!(
        names_the_parser,
        "src/{BACKEND}/ names no parser, so the boundary test proves nothing"
    );
}

fn visit_rust_files(directory: &Path, visit: &mut impl FnMut(&Path)) {
    let entries = std::fs::read_dir(directory).expect("source directory is readable");

    for entry in entries {
        let path = entry.expect("directory entry is readable").path();

        if path.is_dir() {
            visit_rust_files(&path, visit);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            visit(&path);
        }
    }
}
