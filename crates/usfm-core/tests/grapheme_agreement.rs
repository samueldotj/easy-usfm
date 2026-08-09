//! UNICODE §9.1 property 4 — grapheme boundaries agree between Rust's
//! `unicode-segmentation` and the browser's `Intl.Segmenter`.
//!
//! This one cannot be modelled. The other three properties reason about UTF-16
//! indexing, which is fully specified and can be reproduced in Rust with
//! confidence. Grapheme segmentation is a table-driven algorithm that both
//! sides implement independently against a Unicode version each chose for
//! itself — so writing a Rust model of `Intl.Segmenter` and then testing
//! against the model would assert only that we can copy our own assumptions.
//!
//! So it shells out to a real ICU implementation. Node is where one is
//! available without a browser; V8's `Intl.Segmenter` is the same engine
//! Chromium ships, which is what the editor will actually run against.
//!
//! **Skips when Node is absent**, rather than failing. A contributor without
//! Node should still be able to run the suite, and CI installs it so the
//! property is genuinely checked on every push. The skip is announced loudly
//! enough to notice.

use std::process::Command;

use usfm_core::grapheme;

/// Text where Rust and ICU have historically been most likely to disagree:
/// conjunct scripts, reordered vowel signs, emoji sequences with joiners, and
/// regional indicators.
const CASES: &[(&str, &str)] = &[
    ("ascii", "abc"),
    ("tamil conjunct", "க்ஷேமம்"),
    ("devanagari conjunct", "क्षि"),
    ("devanagari reordered vowel", "कि"),
    ("khmer cluster", "ក្ខេ"),
    ("myanmar cluster", "ဗျာ"),
    ("hebrew with points", "בְּרֵאשִׁית"),
    ("arabic", "مرحبا"),
    ("combining acute", "e\u{301}"),
    ("stacked marks", "q\u{0323}\u{0307}"),
    ("astral", "\u{1D400}\u{1D401}"),
    ("emoji zwj sequence", "👨\u{200d}👩\u{200d}👧"),
    ("flag", "🇮🇳"),
    ("skin tone", "👍🏽"),
    ("joiner in text", "Te\u{200c}xt"),
    ("usfm line", "\\v 1 க்ஷேமம் \\nd LORD\\nd*"),
];

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Grapheme boundaries as byte offsets, per `Intl.Segmenter`.
///
/// The segmenter reports UTF-16 indices, so the script converts them to byte
/// offsets before returning — comparing in Rust's coordinate space rather than
/// JavaScript's keeps the assertion about segmentation rather than about
/// indexing, which the other three properties already cover.
fn intl_segmenter_boundaries(text: &str) -> Vec<usize> {
    let script = r#"
        const text = JSON.parse(process.argv[1]);
        const segmenter = new Intl.Segmenter('en', { granularity: 'grapheme' });

        // UTF-16 index -> byte offset, walking code points once.
        const byteOf = new Map([[0, 0]]);
        let units = 0;
        let bytes = 0;
        for (const codePoint of text) {
            units += codePoint.length;
            bytes += Buffer.byteLength(codePoint, 'utf8');
            byteOf.set(units, bytes);
        }

        const offsets = [0];
        for (const { segment, index } of segmenter.segment(text)) {
            offsets.push(byteOf.get(index + segment.length));
        }
        console.log(JSON.stringify(offsets));
    "#;

    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(serde_json::to_string(text).expect("text encodes as JSON"))
        .output()
        .expect("running node");

    assert!(
        output.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("node returned a JSON array of offsets")
}

#[test]
fn grapheme_boundaries_agree_with_intl_segmenter() {
    if !node_available() {
        eprintln!(
            "SKIPPED: grapheme_boundaries_agree_with_intl_segmenter — node not found.\n\
             UNICODE §9.1 property 4 was NOT checked in this run. CI installs node,\n\
             so it is checked there."
        );
        return;
    }

    let mut disagreements = Vec::new();

    for (name, text) in CASES {
        let rust = grapheme::boundaries(text);
        let icu = intl_segmenter_boundaries(text);

        if rust != icu {
            disagreements.push(format!(
                "  {name} ({text:?})\n    rust: {rust:?}\n    icu:  {icu:?}"
            ));
        }
    }

    assert!(
        disagreements.is_empty(),
        "grapheme segmentation disagrees with Intl.Segmenter.\n\
         The failure this causes is a cursor that appears stuck or jumps two\n\
         characters, reported as \"the editor is broken\" and nothing more.\n{}",
        disagreements.join("\n")
    );
}
